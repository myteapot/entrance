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

fn fallback_app_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}
