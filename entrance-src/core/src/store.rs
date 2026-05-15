use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct MigrationStep {
    pub name: &'static str,
    pub sql: &'static str,
}

const CORE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS drawer_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    title           TEXT NOT NULL,
    kind            TEXT NOT NULL,
    source_path     TEXT,
    storage_path    TEXT,
    tags_json       TEXT NOT NULL,
    encrypted       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS hive_runs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    title           TEXT NOT NULL,
    mode            TEXT NOT NULL,
    status          TEXT NOT NULL,
    project_dir     TEXT,
    summary         TEXT,
    payload_json    TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS launcher_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    command         TEXT NOT NULL UNIQUE,
    arguments       TEXT,
    working_dir     TEXT,
    source          TEXT NOT NULL,
    launch_count    INTEGER NOT NULL DEFAULT 0,
    pinned          INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bus_commands (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    topic           TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
"#;

#[derive(Debug, Clone)]
pub struct Store {
    db_path: PathBuf,
    connection: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub app_root: String,
    pub db_path: String,
    pub drawer_entries: i64,
    pub hive_runs: i64,
    pub launcher_entries: i64,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerEntry {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub source_path: Option<String>,
    pub storage_path: Option<String>,
    pub tags: Vec<String>,
    pub encrypted: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerEntryCreate {
    pub title: String,
    pub kind: String,
    pub source_path: Option<String>,
    pub storage_path: Option<String>,
    pub tags: Vec<String>,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DrawerFilter {
    pub kind: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DrawerMode {
    FileSystem,
    Database,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveRun {
    pub id: i64,
    pub title: String,
    pub mode: String,
    pub status: String,
    pub project_dir: Option<String>,
    pub summary: Option<String>,
    pub payload_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveRunCreate {
    pub title: String,
    pub mode: String,
    pub status: String,
    pub project_dir: Option<String>,
    pub summary: Option<String>,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherEntry {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub command: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub source: String,
    pub launch_count: i64,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedCommand {
    pub id: i64,
    pub topic: String,
    pub payload_json: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl PersistedCommand {
    pub fn ephemeral(topic: impl Into<String>, payload: &serde_json::Value) -> Self {
        let now = timestamp();
        Self {
            id: 0,
            topic: topic.into(),
            payload_json: payload.to_string(),
            status: "ephemeral".to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherEntryCreate {
    pub name: String,
    pub command: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub source: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LauncherQuery {
    pub query: String,
    pub limit: usize,
}

impl Store {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(&db_path)
            .with_context(|| format!("failed to open database at {}", db_path.display()))?;
        connection.execute_batch(CORE_SCHEMA)?;

        Ok(Self {
            db_path,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn apply_migrations(&self, steps: &[MigrationStep]) -> Result<()> {
        let connection = self.connection();
        for step in steps {
            connection
                .execute_batch(step.sql)
                .with_context(|| format!("failed to apply migration {}", step.name))?;
        }
        Ok(())
    }

    pub fn insert_drawer_entry(&self, row: DrawerEntryCreate) -> Result<i64> {
        let now = timestamp();
        let tags_json = serde_json::to_string(&row.tags)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO drawer_entries (title, kind, source_path, storage_path, tags_json, encrypted, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.title,
                row.kind,
                row.source_path,
                row.storage_path,
                tags_json,
                row.encrypted as i64,
                now,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_drawer_entries(&self, filter: &DrawerFilter) -> Result<Vec<DrawerEntry>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, title, kind, source_path, storage_path, tags_json, encrypted, created_at, updated_at
             FROM drawer_entries ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], map_drawer_entry)?;

        let mut entries = Vec::new();
        for row in rows {
            let row = row?;
            if filter
                .kind
                .as_ref()
                .is_some_and(|kind| row.kind.as_str() != kind.as_str())
            {
                continue;
            }

            if filter
                .tag
                .as_ref()
                .is_some_and(|tag| !row.tags.iter().any(|value| value == tag))
            {
                continue;
            }

            entries.push(row);
        }
        Ok(entries)
    }

    pub fn get_drawer_entry(&self, id: i64) -> Result<Option<DrawerEntry>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, title, kind, source_path, storage_path, tags_json, encrypted, created_at, updated_at
             FROM drawer_entries WHERE id = ?1 LIMIT 1",
        )?;
        statement
            .query_row(params![id], map_drawer_entry)
            .optional()
            .map_err(Into::into)
    }

    pub fn update_drawer_entry_paths(
        &self,
        id: i64,
        source_path: Option<&str>,
        storage_path: Option<&str>,
    ) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE drawer_entries
             SET source_path = COALESCE(?2, source_path),
                 storage_path = ?3,
                 updated_at = ?4
             WHERE id = ?1",
            params![id, source_path, storage_path, timestamp()],
        )?;
        Ok(())
    }

    pub fn insert_hive_run(&self, row: HiveRunCreate) -> Result<i64> {
        let now = timestamp();
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_runs (title, mode, status, project_dir, summary, payload_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.title,
                row.mode,
                row.status,
                row.project_dir,
                row.summary,
                row.payload_json,
                now,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_runs(&self) -> Result<Vec<HiveRun>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, title, mode, status, project_dir, summary, payload_json, created_at, updated_at
             FROM hive_runs ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], map_hive_run)?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_hive_run(&self, id: i64) -> Result<Option<HiveRun>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, title, mode, status, project_dir, summary, payload_json, created_at, updated_at
             FROM hive_runs WHERE id = ?1 LIMIT 1",
        )?;
        statement
            .query_row(params![id], map_hive_run)
            .optional()
            .map_err(Into::into)
    }

    pub fn update_hive_run_status(
        &self,
        id: i64,
        status: &str,
        summary: Option<&str>,
    ) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE hive_runs SET status = ?2, summary = COALESCE(?3, summary), updated_at = ?4 WHERE id = ?1",
            params![id, status, summary, timestamp()],
        )?;
        Ok(())
    }

    pub fn enqueue_command(
        &self,
        topic: &str,
        payload: &serde_json::Value,
    ) -> Result<PersistedCommand> {
        let now = timestamp();
        let payload_json = serde_json::to_string(payload)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO bus_commands (topic, payload_json, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![topic, payload_json, "pending", now, now],
        )?;
        let id = connection.last_insert_rowid();

        Ok(PersistedCommand {
            id,
            topic: topic.to_string(),
            payload_json,
            status: "pending".to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn list_pending_commands(&self, topic: Option<&str>) -> Result<Vec<PersistedCommand>> {
        let connection = self.connection();
        let mut entries = Vec::new();

        if let Some(topic) = topic {
            let mut statement = connection.prepare(
                "SELECT id, topic, payload_json, status, created_at, updated_at
                 FROM bus_commands
                 WHERE status = 'pending' AND topic = ?1
                 ORDER BY id ASC",
            )?;
            let rows = statement.query_map(params![topic], map_persisted_command)?;
            for row in rows {
                entries.push(row?);
            }
        } else {
            let mut statement = connection.prepare(
                "SELECT id, topic, payload_json, status, created_at, updated_at
                 FROM bus_commands
                 WHERE status = 'pending'
                 ORDER BY id ASC",
            )?;
            let rows = statement.query_map([], map_persisted_command)?;
            for row in rows {
                entries.push(row?);
            }
        }

        Ok(entries)
    }

    pub fn update_command_status(&self, id: i64, status: &str) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE bus_commands SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status, timestamp()],
        )?;
        Ok(())
    }

    pub fn upsert_launcher_entries(&self, entries: &[LauncherEntryCreate]) -> Result<()> {
        let connection = self.connection();
        for entry in entries {
            let now = timestamp();
            connection.execute(
                "INSERT INTO launcher_entries
                 (name, normalized_name, command, arguments, working_dir, source, launch_count, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?9)
                 ON CONFLICT(command) DO UPDATE SET
                    name = excluded.name,
                    normalized_name = excluded.normalized_name,
                    arguments = excluded.arguments,
                    working_dir = excluded.working_dir,
                    source = excluded.source,
                    pinned = excluded.pinned,
                    updated_at = excluded.updated_at",
                params![
                    entry.name,
                    normalize_text(&entry.name),
                    entry.command,
                    entry.arguments,
                    entry.working_dir,
                    entry.source,
                    entry.pinned as i64,
                    now,
                    now,
                ],
            )?;
        }
        Ok(())
    }

    pub fn search_launcher_entries(&self, query: &LauncherQuery) -> Result<Vec<LauncherEntry>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, name, normalized_name, command, arguments, working_dir, source, launch_count, pinned, created_at, updated_at
             FROM launcher_entries ORDER BY pinned DESC, launch_count DESC, name ASC",
        )?;
        let rows = statement.query_map([], map_launcher_entry)?;

        let normalized_query = normalize_text(&query.query);
        let limit = if query.limit == 0 { 20 } else { query.limit };
        let mut entries = Vec::new();
        for row in rows {
            let row = row?;
            if normalized_query.is_empty()
                || row.normalized_name.contains(&normalized_query)
                || normalize_text(&row.command).contains(&normalized_query)
            {
                entries.push(row);
            }
            if entries.len() >= limit {
                break;
            }
        }
        Ok(entries)
    }

    pub fn list_launcher_entries(&self) -> Result<Vec<LauncherEntry>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, name, normalized_name, command, arguments, working_dir, source, launch_count, pinned, created_at, updated_at
             FROM launcher_entries ORDER BY pinned DESC, launch_count DESC, name ASC",
        )?;
        let rows = statement.query_map([], map_launcher_entry)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn record_launcher_launch(&self, command: &str) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE launcher_entries SET launch_count = launch_count + 1, updated_at = ?2 WHERE command = ?1",
            params![command, timestamp()],
        )?;
        Ok(())
    }

    pub fn set_launcher_pinned(&self, command: &str, pinned: bool) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE launcher_entries SET pinned = ?2, updated_at = ?3 WHERE command = ?1",
            params![command, pinned as i64, timestamp()],
        )?;
        Ok(())
    }

    pub fn app_status(&self, app_root: &Path) -> Result<AppStatus> {
        Ok(AppStatus {
            app_root: app_root.display().to_string(),
            db_path: self.db_path.display().to_string(),
            drawer_entries: self.count("drawer_entries")?,
            hive_runs: self.count("hive_runs")?,
            launcher_entries: self.count("launcher_entries")?,
            generated_at: timestamp(),
        })
    }

    fn count(&self, table: &str) -> Result<i64> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!("SELECT COUNT(*) FROM {table}"))?;
        let count = statement.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection.lock().expect("database mutex poisoned")
    }
}

fn normalize_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn timestamp() -> String {
    let value: DateTime<Utc> = Utc::now();
    value.to_rfc3339()
}

fn map_drawer_entry(row: &Row<'_>) -> rusqlite::Result<DrawerEntry> {
    let tags_json: String = row.get(5)?;
    let tags = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(DrawerEntry {
        id: row.get(0)?,
        title: row.get(1)?,
        kind: row.get(2)?,
        source_path: row.get(3)?,
        storage_path: row.get(4)?,
        tags,
        encrypted: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_hive_run(row: &Row<'_>) -> rusqlite::Result<HiveRun> {
    Ok(HiveRun {
        id: row.get(0)?,
        title: row.get(1)?,
        mode: row.get(2)?,
        status: row.get(3)?,
        project_dir: row.get(4)?,
        summary: row.get(5)?,
        payload_json: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_launcher_entry(row: &Row<'_>) -> rusqlite::Result<LauncherEntry> {
    Ok(LauncherEntry {
        id: row.get(0)?,
        name: row.get(1)?,
        normalized_name: row.get(2)?,
        command: row.get(3)?,
        arguments: row.get(4)?,
        working_dir: row.get(5)?,
        source: row.get(6)?,
        launch_count: row.get(7)?,
        pinned: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn map_persisted_command(row: &Row<'_>) -> rusqlite::Result<PersistedCommand> {
    Ok(PersistedCommand {
        id: row.get(0)?,
        topic: row.get(1)?,
        payload_json: row.get(2)?,
        status: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}
