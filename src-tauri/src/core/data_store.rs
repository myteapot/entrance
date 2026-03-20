use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::plugins::launcher::scanner::DiscoveredApp;

const CORE_MIGRATION: MigrationStep = MigrationStep {
    name: "0000_create_core_tables",
    sql: include_str!("../../migrations/0000_create_core_tables.sql"),
};

#[derive(Debug, Clone, Copy)]
pub struct MigrationStep {
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct MigrationPlan<'a> {
    pub core: &'a [MigrationStep],
    pub plugins: &'a [MigrationStep],
}

impl<'a> MigrationPlan<'a> {
    pub fn new(plugins: &'a [MigrationStep]) -> Self {
        Self {
            core: core_migrations(),
            plugins,
        }
    }
}

pub fn core_migrations() -> &'static [MigrationStep] {
    &[CORE_MIGRATION]
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredLauncherApp {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub path: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub icon_path: Option<String>,
    pub source: String,
    pub launch_count: i64,
    pub last_used: Option<String>,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredForgeTask {
    pub id: i64,
    pub name: String,
    pub command: String,
    pub args: String, // JSON
    pub status: String,
    pub exit_code: Option<i64>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredForgeTaskLog {
    pub id: i64,
    pub task_id: i64,
    pub stream: String,
    pub line: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVaultToken {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct EncryptedVaultToken {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub encrypted_value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVaultTokenSecret {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVaultMcpConfig {
    pub id: i64,
    pub name: String,
    pub transport: String,
    pub endpoint: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct DataStore {
    connection: Arc<Mutex<Connection>>,
    path: Arc<PathBuf>,
}

impl std::fmt::Debug for DataStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl DataStore {
    pub fn open(path: impl AsRef<Path>, migration_plan: MigrationPlan<'_>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(&path)?;

        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            path: Arc::new(path),
        };
        store.migrate(migration_plan)?;
        Ok(store)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn in_memory(migration_plan: MigrationPlan<'_>) -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            path: Arc::new(PathBuf::from(":memory:")),
        };
        store.migrate(migration_plan)?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn launcher_app_count(&self) -> Result<i64> {
        self.with_connection(|connection| {
            let count =
                connection.query_row("SELECT COUNT(*) FROM plugin_launcher_apps", [], |row| {
                    row.get(0)
                })?;
            Ok(count)
        })
    }

    pub fn append_core_event_log(&self, topic: &str, payload: Option<&str>) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute(
                r#"
                INSERT INTO core_event_log (topic, payload)
                VALUES (?1, ?2)
                "#,
                params![topic, payload],
            )?;
            Ok(())
        })
    }

    pub fn upsert_launcher_apps(&self, apps: &[DiscoveredApp]) -> Result<()> {
        if apps.is_empty() {
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;

        for app in apps {
            transaction.execute(
                r#"
                INSERT INTO plugin_launcher_apps (
                    name,
                    normalized_name,
                    path,
                    arguments,
                    working_dir,
                    icon_path,
                    source,
                    launch_count,
                    last_used,
                    pinned,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, 0, ?8, ?8)
                ON CONFLICT(path) DO UPDATE SET
                    name = excluded.name,
                    normalized_name = excluded.normalized_name,
                    arguments = COALESCE(excluded.arguments, plugin_launcher_apps.arguments),
                    working_dir = COALESCE(excluded.working_dir, plugin_launcher_apps.working_dir),
                    icon_path = COALESCE(excluded.icon_path, plugin_launcher_apps.icon_path),
                    source = excluded.source,
                    updated_at = excluded.updated_at
                "#,
                params![
                    app.name,
                    app.normalized_name,
                    app.path,
                    app.arguments,
                    app.working_dir,
                    app.icon_path,
                    app.source,
                    now,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn list_launcher_apps(&self) -> Result<Vec<StoredLauncherApp>> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                r#"
                SELECT
                    id,
                    name,
                    normalized_name,
                    path,
                    arguments,
                    working_dir,
                    icon_path,
                    source,
                    launch_count,
                    last_used,
                    pinned,
                    created_at,
                    updated_at
                FROM plugin_launcher_apps
                ORDER BY pinned DESC, launch_count DESC, name ASC
                "#,
            )?;

            let rows = statement.query_map([], map_launcher_row)?;
            let apps = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(apps)
        })
    }

    pub fn get_launcher_app_by_path(&self, path: &str) -> Result<Option<StoredLauncherApp>> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    r#"
                    SELECT
                        id,
                        name,
                        normalized_name,
                        path,
                        arguments,
                        working_dir,
                        icon_path,
                        source,
                        launch_count,
                        last_used,
                        pinned,
                        created_at,
                        updated_at
                    FROM plugin_launcher_apps
                    WHERE path = ?1
                    "#,
                    [path],
                    map_launcher_row,
                )
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn record_launcher_launch(&self, path: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        self.with_connection(|connection| {
            let changed = connection.execute(
                r#"
                UPDATE plugin_launcher_apps
                SET launch_count = launch_count + 1,
                    last_used = ?2,
                    updated_at = ?2
                WHERE path = ?1
                "#,
                params![path, now],
            )?;

            if changed == 0 {
                connection.execute(
                    r#"
                    INSERT INTO plugin_launcher_apps (
                        name,
                        normalized_name,
                        path,
                        arguments,
                        working_dir,
                        icon_path,
                        source,
                        launch_count,
                        last_used,
                        pinned,
                        created_at,
                        updated_at
                    ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, 'manual', 1, ?4, 0, ?4, ?4)
                    "#,
                    params![
                        fallback_app_name(path),
                        crate::plugins::launcher::search::normalize_text(path),
                        path,
                        now,
                    ],
                )?;
            }

            Ok(())
        })
    }

    pub fn set_launcher_pinned(&self, path: &str, pinned: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let changed = self.with_connection(|connection| {
            Ok(connection.execute(
                r#"
                UPDATE plugin_launcher_apps
                SET pinned = ?2,
                    updated_at = ?3
                WHERE path = ?1
                "#,
                params![path, if pinned { 1 } else { 0 }, now],
            )?)
        })?;

        if changed == 0 {
            return Err(anyhow!("launcher app `{path}` does not exist in the index"));
        }

        Ok(())
    }

    pub fn insert_forge_task(&self, name: &str, command: &str, args: &str) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_forge_tasks (
                    name, command, args, status, exit_code, created_at, finished_at
                ) VALUES (?1, ?2, ?3, 'Pending', NULL, ?4, NULL)
                "#,
                params![name, command, args, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn update_forge_task_status(
        &self,
        id: i64,
        status: &str,
        exit_code: Option<i32>,
    ) -> Result<()> {
        let now = if matches!(status, "Done" | "Failed" | "Cancelled") {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };
        self.with_connection(|conn| {
            if let Some(finished_at) = now {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3, finished_at = ?4
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code, finished_at],
                )?;
            } else {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code],
                )?;
            }
            Ok(())
        })
    }

    pub fn list_forge_tasks(&self) -> Result<Vec<StoredForgeTask>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, command, args, status, exit_code, created_at, finished_at FROM plugin_forge_tasks ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map([], map_forge_row)?;
            let tasks = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(tasks)
        })
    }

    pub fn get_forge_task(&self, id: i64) -> Result<Option<StoredForgeTask>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT id, name, command, args, status, exit_code, created_at, finished_at FROM plugin_forge_tasks WHERE id = ?1",
                [id],
                map_forge_row,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn append_forge_task_log(&self, task_id: i64, stream: &str, line: &str) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_forge_task_logs (
                    task_id, stream, line, created_at
                ) VALUES (?1, ?2, ?3, ?4)
                "#,
                params![task_id, stream, line, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn list_forge_task_logs(&self, task_id: i64) -> Result<Vec<StoredForgeTaskLog>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, task_id, stream, line, created_at
                FROM plugin_forge_task_logs
                WHERE task_id = ?1
                ORDER BY id ASC
                "#,
            )?;
            let rows = stmt.query_map([task_id], map_forge_log_row)?;
            let logs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(logs)
        })
    }

    pub fn insert_vault_token(
        &self,
        name: &str,
        provider: &str,
        encrypted_value: &str,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_vault_tokens (
                    name, provider, encrypted_value, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?4)
                "#,
                params![name, provider, encrypted_value, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn list_vault_tokens(&self) -> Result<Vec<StoredVaultToken>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, provider, created_at, updated_at
                FROM plugin_vault_tokens
                ORDER BY provider ASC, name ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_vault_token_row)?;
            let tokens = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(tokens)
        })
    }

    pub fn get_vault_token(&self, id: i64) -> Result<Option<EncryptedVaultToken>> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT id, name, provider, encrypted_value, created_at, updated_at
                FROM plugin_vault_tokens
                WHERE id = ?1
                "#,
                [id],
                map_encrypted_vault_token_row,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn delete_vault_token(&self, id: i64) -> Result<()> {
        let changed = self.with_connection(|conn| {
            Ok(conn.execute("DELETE FROM plugin_vault_tokens WHERE id = ?1", [id])?)
        })?;

        if changed == 0 {
            return Err(anyhow!("vault token `{id}` does not exist"));
        }

        Ok(())
    }

    pub fn list_vault_mcp_configs(&self) -> Result<Vec<StoredVaultMcpConfig>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, transport, endpoint, enabled, created_at, updated_at
                FROM plugin_vault_mcp_configs
                ORDER BY enabled DESC, name ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_vault_mcp_row)?;
            let configs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(configs)
        })
    }

    pub fn upsert_vault_mcp_config(
        &self,
        id: Option<i64>,
        name: &str,
        transport: &str,
        endpoint: &str,
        enabled: bool,
    ) -> Result<StoredVaultMcpConfig> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            let id = if let Some(id) = id {
                let changed = conn.execute(
                    r#"
                    UPDATE plugin_vault_mcp_configs
                    SET name = ?2,
                        transport = ?3,
                        endpoint = ?4,
                        enabled = ?5,
                        updated_at = ?6
                    WHERE id = ?1
                    "#,
                    params![
                        id,
                        name,
                        transport,
                        endpoint,
                        if enabled { 1 } else { 0 },
                        now
                    ],
                )?;

                if changed == 0 {
                    return Err(anyhow!("vault MCP config `{id}` does not exist"));
                }

                id
            } else {
                conn.execute(
                    r#"
                    INSERT INTO plugin_vault_mcp_configs (
                        name, transport, endpoint, enabled, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                    "#,
                    params![name, transport, endpoint, if enabled { 1 } else { 0 }, now],
                )?;
                conn.last_insert_rowid()
            };

            fetch_vault_mcp_config(conn, id)?
                .ok_or_else(|| anyhow!("vault MCP config `{id}` could not be reloaded"))
        })
    }

    fn migrate(&self, migration_plan: MigrationPlan<'_>) -> Result<()> {
        self.with_connection(|connection| {
            for migration in migration_plan
                .core
                .iter()
                .chain(migration_plan.plugins.iter())
            {
                let _ = migration.name;
                connection.execute_batch(migration.sql)?;
            }
            Ok(())
        })
    }

    fn with_connection<T, F>(&self, callback: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let connection = self.lock_connection()?;
        callback(&connection)
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| anyhow!("database connection lock poisoned"))
    }
}

fn map_launcher_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredLauncherApp> {
    Ok(StoredLauncherApp {
        id: row.get(0)?,
        name: row.get(1)?,
        normalized_name: row.get(2)?,
        path: row.get(3)?,
        arguments: row.get(4)?,
        working_dir: row.get(5)?,
        icon_path: row.get(6)?,
        source: row.get(7)?,
        launch_count: row.get(8)?,
        last_used: row.get(9)?,
        pinned: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_forge_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredForgeTask> {
    Ok(StoredForgeTask {
        id: row.get(0)?,
        name: row.get(1)?,
        command: row.get(2)?,
        args: row.get(3)?,
        status: row.get(4)?,
        exit_code: row.get(5)?,
        created_at: row.get(6)?,
        finished_at: row.get(7)?,
    })
}

fn map_forge_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredForgeTaskLog> {
    Ok(StoredForgeTaskLog {
        id: row.get(0)?,
        task_id: row.get(1)?,
        stream: row.get(2)?,
        line: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn map_vault_token_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVaultToken> {
    Ok(StoredVaultToken {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_encrypted_vault_token_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncryptedVaultToken> {
    Ok(EncryptedVaultToken {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        encrypted_value: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_vault_mcp_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVaultMcpConfig> {
    Ok(StoredVaultMcpConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        transport: row.get(2)?,
        endpoint: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn fetch_vault_mcp_config(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredVaultMcpConfig>> {
    connection
        .query_row(
            r#"
            SELECT id, name, transport, endpoint, enabled, created_at, updated_at
            FROM plugin_vault_mcp_configs
            WHERE id = ?1
            "#,
            [id],
            map_vault_mcp_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fallback_app_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_task_logs_round_trip() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0004_create_plugin_forge_task_logs",
                sql: include_str!("../../migrations/0004_create_plugin_forge_task_logs.sql"),
            },
        ]))?;

        let task_id = store.insert_forge_task("Echo", "echo", r#"["hello"]"#)?;
        store.append_forge_task_log(task_id, "stdout", "hello")?;
        store.append_forge_task_log(task_id, "stderr", "warn")?;

        let logs = store.list_forge_task_logs(task_id)?;

        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].stream, "stdout");
        assert_eq!(logs[0].line, "hello");
        assert_eq!(logs[1].stream, "stderr");
        assert_eq!(logs[1].line, "warn");

        Ok(())
    }
}
