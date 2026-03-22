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

const CORE_LANDING_MIGRATION: MigrationStep = MigrationStep {
    name: "0005_create_core_landing_tables",
    sql: include_str!("../../migrations/0005_create_core_landing_tables.sql"),
};

const CORE_MIGRATIONS: [MigrationStep; 2] = [CORE_MIGRATION, CORE_LANDING_MIGRATION];

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
    &CORE_MIGRATIONS
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
    pub working_dir: Option<String>,
    pub stdin_text: Option<String>,
    pub required_tokens: String, // JSON
    pub metadata: String,        // JSON
    pub status: String,
    pub status_message: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct StoredSourceIngestRun {
    pub id: i64,
    pub source_system: String,
    pub source_workspace: String,
    pub source_project: String,
    pub artifact_path: Option<String>,
    pub artifact_sha256: Option<String>,
    pub status: String,
    pub imported_issue_count: i64,
    pub imported_document_count: i64,
    pub imported_milestone_count: i64,
    pub imported_planning_item_count: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredSourceArtifact {
    pub id: i64,
    pub ingest_run_id: i64,
    pub artifact_kind: String,
    pub artifact_key: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredExternalIssueMirror {
    pub id: i64,
    pub mirror_key: String,
    pub source_system: String,
    pub source_workspace: String,
    pub source_project: String,
    pub external_issue_id: String,
    pub project_name: Option<String>,
    pub team_name: Option<String>,
    pub parent_external_issue_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub state: Option<String>,
    pub priority: Option<String>,
    pub url: Option<String>,
    pub labels_json: String,
    pub relations_json: String,
    pub payload_json: String,
    pub git_branch_name: Option<String>,
    pub due_date: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
    pub archived_at: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub last_ingest_run_id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredPlanningItem {
    pub id: i64,
    pub canonical_key: Option<String>,
    pub item_type: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub reconciliation_status: String,
    pub source_system: Option<String>,
    pub source_workspace: Option<String>,
    pub source_project: Option<String>,
    pub source_key: Option<String>,
    pub seeded_from_mirror_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredPlanningItemLink {
    pub id: i64,
    pub planning_item_id: i64,
    pub link_type: String,
    pub target_planning_item_id: Option<i64>,
    pub target_external_issue_mirror_id: Option<i64>,
    pub metadata_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredPromotionRecord {
    pub id: i64,
    pub subject_kind: String,
    pub subject_id: i64,
    pub promotion_state: String,
    pub reason: Option<String>,
    pub source_ingest_run_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewSourceIngestRun<'a> {
    pub source_system: &'a str,
    pub source_workspace: &'a str,
    pub source_project: &'a str,
    pub artifact_path: Option<&'a str>,
    pub artifact_sha256: Option<&'a str>,
    pub status: &'a str,
}

#[derive(Debug, Clone)]
pub struct SourceIngestRunCompletion<'a> {
    pub status: &'a str,
    pub imported_issue_count: i64,
    pub imported_document_count: i64,
    pub imported_milestone_count: i64,
    pub imported_planning_item_count: i64,
    pub error_message: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct NewSourceArtifact<'a> {
    pub ingest_run_id: i64,
    pub artifact_kind: &'a str,
    pub artifact_key: &'a str,
    pub title: Option<&'a str>,
    pub url: Option<&'a str>,
    pub payload_json: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertExternalIssueMirror<'a> {
    pub ingest_run_id: i64,
    pub mirror_key: &'a str,
    pub source_system: &'a str,
    pub source_workspace: &'a str,
    pub source_project: &'a str,
    pub external_issue_id: &'a str,
    pub project_name: Option<&'a str>,
    pub team_name: Option<&'a str>,
    pub parent_external_issue_id: Option<&'a str>,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub state: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub url: Option<&'a str>,
    pub labels_json: &'a str,
    pub relations_json: &'a str,
    pub payload_json: &'a str,
    pub git_branch_name: Option<&'a str>,
    pub due_date: Option<&'a str>,
    pub created_at: Option<&'a str>,
    pub updated_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub archived_at: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct UpsertPlanningItem<'a> {
    pub canonical_key: Option<&'a str>,
    pub item_type: &'a str,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub status: &'a str,
    pub reconciliation_status: &'a str,
    pub source_system: Option<&'a str>,
    pub source_workspace: Option<&'a str>,
    pub source_project: Option<&'a str>,
    pub source_key: Option<&'a str>,
    pub seeded_from_mirror_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewPlanningItemLink<'a> {
    pub planning_item_id: i64,
    pub link_type: &'a str,
    pub target_planning_item_id: Option<i64>,
    pub target_external_issue_mirror_id: Option<i64>,
    pub metadata_json: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewPromotionRecord<'a> {
    pub subject_kind: &'a str,
    pub subject_id: i64,
    pub promotion_state: &'a str,
    pub reason: Option<&'a str>,
    pub source_ingest_run_id: Option<i64>,
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

    pub fn insert_forge_task(
        &self,
        name: &str,
        command: &str,
        args: &str,
        working_dir: Option<&str>,
        stdin_text: Option<&str>,
        required_tokens: &str,
        metadata: &str,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_forge_tasks (
                    name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, finished_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'Pending', NULL, NULL, ?8, NULL)
                "#,
                params![
                    name,
                    command,
                    args,
                    working_dir,
                    stdin_text,
                    required_tokens,
                    metadata,
                    now
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn update_forge_task_status(
        &self,
        id: i64,
        status: &str,
        exit_code: Option<i32>,
        status_message: Option<&str>,
    ) -> Result<()> {
        let now = if matches!(status, "Done" | "Failed" | "Cancelled" | "Blocked") {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };
        self.with_connection(|conn| {
            if let Some(finished_at) = now {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3, status_message = ?4, finished_at = ?5
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code, status_message, finished_at],
                )?;
            } else {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3, status_message = ?4
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code, status_message],
                )?;
            }
            Ok(())
        })
    }

    pub fn list_forge_tasks(&self) -> Result<Vec<StoredForgeTask>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, finished_at FROM plugin_forge_tasks ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map([], map_forge_row)?;
            let tasks = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(tasks)
        })
    }

    pub fn get_forge_task(&self, id: i64) -> Result<Option<StoredForgeTask>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT id, name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, finished_at FROM plugin_forge_tasks WHERE id = ?1",
                [id],
                map_forge_row,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn append_forge_task_log(
        &self,
        task_id: i64,
        stream: &str,
        line: &str,
    ) -> Result<StoredForgeTaskLog> {
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
            Ok(StoredForgeTaskLog {
                id: conn.last_insert_rowid(),
                task_id,
                stream: stream.to_string(),
                line: line.to_string(),
                created_at: now,
            })
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

    pub fn update_vault_token(
        &self,
        id: i64,
        name: &str,
        provider: &str,
        encrypted_value: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let changed = self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                UPDATE plugin_vault_tokens
                SET name = ?2,
                    provider = ?3,
                    encrypted_value = ?4,
                    updated_at = ?5
                WHERE id = ?1
                "#,
                params![id, name, provider, encrypted_value, now],
            )?)
        })?;

        if changed == 0 {
            return Err(anyhow!("vault token `{id}` does not exist"));
        }

        Ok(())
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

    pub fn get_vault_token_by_provider(
        &self,
        provider: &str,
    ) -> Result<Option<EncryptedVaultToken>> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT id, name, provider, encrypted_value, created_at, updated_at
                FROM plugin_vault_tokens
                WHERE LOWER(provider) = LOWER(?1)
                ORDER BY updated_at DESC, id DESC
                LIMIT 1
                "#,
                [provider],
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

    pub fn create_source_ingest_run(
        &self,
        new_run: NewSourceIngestRun<'_>,
    ) -> Result<StoredSourceIngestRun> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO source_ingest_runs (
                    source_system,
                    source_workspace,
                    source_project,
                    artifact_path,
                    artifact_sha256,
                    status,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    new_run.source_system,
                    new_run.source_workspace,
                    new_run.source_project,
                    new_run.artifact_path,
                    new_run.artifact_sha256,
                    new_run.status,
                    now,
                ],
            )?;

            fetch_source_ingest_run(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("source ingest run disappeared after creation"))
        })
    }

    pub fn complete_source_ingest_run(
        &self,
        id: i64,
        completion: SourceIngestRunCompletion<'_>,
    ) -> Result<StoredSourceIngestRun> {
        let completed_at = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            let changed = conn.execute(
                r#"
                UPDATE source_ingest_runs
                SET status = ?2,
                    imported_issue_count = ?3,
                    imported_document_count = ?4,
                    imported_milestone_count = ?5,
                    imported_planning_item_count = ?6,
                    error_message = ?7,
                    completed_at = ?8
                WHERE id = ?1
                "#,
                params![
                    id,
                    completion.status,
                    completion.imported_issue_count,
                    completion.imported_document_count,
                    completion.imported_milestone_count,
                    completion.imported_planning_item_count,
                    completion.error_message,
                    completed_at,
                ],
            )?;

            if changed == 0 {
                return Err(anyhow!("source ingest run `{id}` does not exist"));
            }

            fetch_source_ingest_run(conn, id)?
                .ok_or_else(|| anyhow!("source ingest run `{id}` could not be reloaded"))
        })
    }

    pub fn list_source_ingest_runs(&self) -> Result<Vec<StoredSourceIngestRun>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    source_system,
                    source_workspace,
                    source_project,
                    artifact_path,
                    artifact_sha256,
                    status,
                    imported_issue_count,
                    imported_document_count,
                    imported_milestone_count,
                    imported_planning_item_count,
                    error_message,
                    created_at,
                    completed_at
                FROM source_ingest_runs
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_source_ingest_run_row)?;
            let runs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(runs)
        })
    }

    pub fn insert_source_artifact(
        &self,
        artifact: NewSourceArtifact<'_>,
    ) -> Result<StoredSourceArtifact> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO source_artifacts (
                    ingest_run_id,
                    artifact_kind,
                    artifact_key,
                    title,
                    url,
                    payload_json,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(ingest_run_id, artifact_kind, artifact_key) DO UPDATE SET
                    title = excluded.title,
                    url = excluded.url,
                    payload_json = excluded.payload_json,
                    created_at = excluded.created_at
                "#,
                params![
                    artifact.ingest_run_id,
                    artifact.artifact_kind,
                    artifact.artifact_key,
                    artifact.title,
                    artifact.url,
                    artifact.payload_json,
                    now,
                ],
            )?;

            fetch_source_artifact(
                conn,
                artifact.ingest_run_id,
                artifact.artifact_kind,
                artifact.artifact_key,
            )?
            .ok_or_else(|| anyhow!("source artifact disappeared after insert"))
        })
    }

    pub fn list_source_artifacts(&self, ingest_run_id: i64) -> Result<Vec<StoredSourceArtifact>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    ingest_run_id,
                    artifact_kind,
                    artifact_key,
                    title,
                    url,
                    payload_json,
                    created_at
                FROM source_artifacts
                WHERE ingest_run_id = ?1
                ORDER BY artifact_kind ASC, artifact_key ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([ingest_run_id], map_source_artifact_row)?;
            let artifacts = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(artifacts)
        })
    }

    pub fn upsert_external_issue_mirror(
        &self,
        mirror: UpsertExternalIssueMirror<'_>,
    ) -> Result<StoredExternalIssueMirror> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO external_issue_mirrors (
                    mirror_key,
                    source_system,
                    source_workspace,
                    source_project,
                    external_issue_id,
                    project_name,
                    team_name,
                    parent_external_issue_id,
                    title,
                    description,
                    state,
                    priority,
                    url,
                    labels_json,
                    relations_json,
                    payload_json,
                    git_branch_name,
                    due_date,
                    created_at,
                    updated_at,
                    completed_at,
                    archived_at,
                    first_seen_at,
                    last_seen_at,
                    last_ingest_run_id
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?23, ?24
                )
                ON CONFLICT(mirror_key) DO UPDATE SET
                    source_system = excluded.source_system,
                    source_workspace = excluded.source_workspace,
                    source_project = excluded.source_project,
                    external_issue_id = excluded.external_issue_id,
                    project_name = excluded.project_name,
                    team_name = excluded.team_name,
                    parent_external_issue_id = excluded.parent_external_issue_id,
                    title = excluded.title,
                    description = excluded.description,
                    state = excluded.state,
                    priority = excluded.priority,
                    url = excluded.url,
                    labels_json = excluded.labels_json,
                    relations_json = excluded.relations_json,
                    payload_json = excluded.payload_json,
                    git_branch_name = excluded.git_branch_name,
                    due_date = excluded.due_date,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at,
                    completed_at = excluded.completed_at,
                    archived_at = excluded.archived_at,
                    last_seen_at = excluded.last_seen_at,
                    last_ingest_run_id = excluded.last_ingest_run_id
                "#,
                params![
                    mirror.mirror_key,
                    mirror.source_system,
                    mirror.source_workspace,
                    mirror.source_project,
                    mirror.external_issue_id,
                    mirror.project_name,
                    mirror.team_name,
                    mirror.parent_external_issue_id,
                    mirror.title,
                    mirror.description,
                    mirror.state,
                    mirror.priority,
                    mirror.url,
                    mirror.labels_json,
                    mirror.relations_json,
                    mirror.payload_json,
                    mirror.git_branch_name,
                    mirror.due_date,
                    mirror.created_at,
                    mirror.updated_at,
                    mirror.completed_at,
                    mirror.archived_at,
                    now,
                    mirror.ingest_run_id,
                ],
            )?;

            fetch_external_issue_mirror_by_key(conn, mirror.mirror_key)?
                .ok_or_else(|| anyhow!("external issue mirror disappeared after upsert"))
        })
    }

    pub fn list_external_issue_mirrors(&self) -> Result<Vec<StoredExternalIssueMirror>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    mirror_key,
                    source_system,
                    source_workspace,
                    source_project,
                    external_issue_id,
                    project_name,
                    team_name,
                    parent_external_issue_id,
                    title,
                    description,
                    state,
                    priority,
                    url,
                    labels_json,
                    relations_json,
                    payload_json,
                    git_branch_name,
                    due_date,
                    created_at,
                    updated_at,
                    completed_at,
                    archived_at,
                    first_seen_at,
                    last_seen_at,
                    last_ingest_run_id
                FROM external_issue_mirrors
                ORDER BY external_issue_id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_external_issue_mirror_row)?;
            let mirrors = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(mirrors)
        })
    }

    pub fn upsert_planning_item(&self, item: UpsertPlanningItem<'_>) -> Result<StoredPlanningItem> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            if let Some(canonical_key) = item.canonical_key {
                conn.execute(
                    r#"
                    INSERT INTO planning_items (
                        canonical_key,
                        item_type,
                        title,
                        description,
                        status,
                        reconciliation_status,
                        source_system,
                        source_workspace,
                        source_project,
                        source_key,
                        seeded_from_mirror_id,
                        created_at,
                        updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                    ON CONFLICT(canonical_key) DO UPDATE SET
                        item_type = excluded.item_type,
                        title = excluded.title,
                        description = excluded.description,
                        status = excluded.status,
                        reconciliation_status = excluded.reconciliation_status,
                        source_system = excluded.source_system,
                        source_workspace = excluded.source_workspace,
                        source_project = excluded.source_project,
                        source_key = excluded.source_key,
                        seeded_from_mirror_id = excluded.seeded_from_mirror_id,
                        updated_at = excluded.updated_at
                    "#,
                    params![
                        canonical_key,
                        item.item_type,
                        item.title,
                        item.description,
                        item.status,
                        item.reconciliation_status,
                        item.source_system,
                        item.source_workspace,
                        item.source_project,
                        item.source_key,
                        item.seeded_from_mirror_id,
                        now,
                    ],
                )?;

                fetch_planning_item_by_canonical_key(conn, canonical_key)?
                    .ok_or_else(|| anyhow!("planning item disappeared after upsert"))
            } else {
                conn.execute(
                    r#"
                    INSERT INTO planning_items (
                        canonical_key,
                        item_type,
                        title,
                        description,
                        status,
                        reconciliation_status,
                        source_system,
                        source_workspace,
                        source_project,
                        source_key,
                        seeded_from_mirror_id,
                        created_at,
                        updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                    "#,
                    params![
                        Option::<&str>::None,
                        item.item_type,
                        item.title,
                        item.description,
                        item.status,
                        item.reconciliation_status,
                        item.source_system,
                        item.source_workspace,
                        item.source_project,
                        item.source_key,
                        item.seeded_from_mirror_id,
                        now,
                    ],
                )?;

                fetch_planning_item(conn, conn.last_insert_rowid())?
                    .ok_or_else(|| anyhow!("planning item disappeared after insert"))
            }
        })
    }

    pub fn list_planning_items(&self) -> Result<Vec<StoredPlanningItem>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    canonical_key,
                    item_type,
                    title,
                    description,
                    status,
                    reconciliation_status,
                    source_system,
                    source_workspace,
                    source_project,
                    source_key,
                    seeded_from_mirror_id,
                    created_at,
                    updated_at
                FROM planning_items
                ORDER BY item_type ASC, title ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_planning_item_row)?;
            let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(items)
        })
    }

    pub fn list_unreconciled_planning_items(&self) -> Result<Vec<StoredPlanningItem>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    canonical_key,
                    item_type,
                    title,
                    description,
                    status,
                    reconciliation_status,
                    source_system,
                    source_workspace,
                    source_project,
                    source_key,
                    seeded_from_mirror_id,
                    created_at,
                    updated_at
                FROM planning_items
                WHERE reconciliation_status = 'unreconciled'
                ORDER BY item_type ASC, title ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_planning_item_row)?;
            let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(items)
        })
    }

    pub fn ensure_planning_item_link(
        &self,
        link: NewPlanningItemLink<'_>,
    ) -> Result<StoredPlanningItemLink> {
        self.with_connection(|conn| {
            if let Some(existing) = fetch_planning_item_link(
                conn,
                link.planning_item_id,
                link.link_type,
                link.target_planning_item_id,
                link.target_external_issue_mirror_id,
            )? {
                return Ok(existing);
            }

            let now = Utc::now().to_rfc3339();
            conn.execute(
                r#"
                INSERT INTO planning_item_links (
                    planning_item_id,
                    link_type,
                    target_planning_item_id,
                    target_external_issue_mirror_id,
                    metadata_json,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    link.planning_item_id,
                    link.link_type,
                    link.target_planning_item_id,
                    link.target_external_issue_mirror_id,
                    link.metadata_json,
                    now,
                ],
            )?;

            fetch_planning_item_link_by_id(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("planning item link disappeared after insert"))
        })
    }

    pub fn list_planning_item_links(&self) -> Result<Vec<StoredPlanningItemLink>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    planning_item_id,
                    link_type,
                    target_planning_item_id,
                    target_external_issue_mirror_id,
                    metadata_json,
                    created_at
                FROM planning_item_links
                ORDER BY planning_item_id ASC, link_type ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_planning_item_link_row)?;
            let links = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(links)
        })
    }

    pub fn append_promotion_record(
        &self,
        record: NewPromotionRecord<'_>,
    ) -> Result<StoredPromotionRecord> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO promotion_records (
                    subject_kind,
                    subject_id,
                    promotion_state,
                    reason,
                    source_ingest_run_id,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    record.subject_kind,
                    record.subject_id,
                    record.promotion_state,
                    record.reason,
                    record.source_ingest_run_id,
                    now,
                ],
            )?;

            fetch_promotion_record(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("promotion record disappeared after insert"))
        })
    }

    pub fn list_promotion_records(&self) -> Result<Vec<StoredPromotionRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    subject_kind,
                    subject_id,
                    promotion_state,
                    reason,
                    source_ingest_run_id,
                    created_at
                FROM promotion_records
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_promotion_record_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
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
            ensure_forge_task_columns(connection)?;
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
        working_dir: row.get(4)?,
        stdin_text: row.get(5)?,
        required_tokens: row.get(6)?,
        metadata: row.get(7)?,
        status: row.get(8)?,
        status_message: row.get(9)?,
        exit_code: row.get(10)?,
        created_at: row.get(11)?,
        finished_at: row.get(12)?,
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

fn map_source_ingest_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSourceIngestRun> {
    Ok(StoredSourceIngestRun {
        id: row.get(0)?,
        source_system: row.get(1)?,
        source_workspace: row.get(2)?,
        source_project: row.get(3)?,
        artifact_path: row.get(4)?,
        artifact_sha256: row.get(5)?,
        status: row.get(6)?,
        imported_issue_count: row.get(7)?,
        imported_document_count: row.get(8)?,
        imported_milestone_count: row.get(9)?,
        imported_planning_item_count: row.get(10)?,
        error_message: row.get(11)?,
        created_at: row.get(12)?,
        completed_at: row.get(13)?,
    })
}

fn map_source_artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSourceArtifact> {
    Ok(StoredSourceArtifact {
        id: row.get(0)?,
        ingest_run_id: row.get(1)?,
        artifact_kind: row.get(2)?,
        artifact_key: row.get(3)?,
        title: row.get(4)?,
        url: row.get(5)?,
        payload_json: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn map_external_issue_mirror_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredExternalIssueMirror> {
    Ok(StoredExternalIssueMirror {
        id: row.get(0)?,
        mirror_key: row.get(1)?,
        source_system: row.get(2)?,
        source_workspace: row.get(3)?,
        source_project: row.get(4)?,
        external_issue_id: row.get(5)?,
        project_name: row.get(6)?,
        team_name: row.get(7)?,
        parent_external_issue_id: row.get(8)?,
        title: row.get(9)?,
        description: row.get(10)?,
        state: row.get(11)?,
        priority: row.get(12)?,
        url: row.get(13)?,
        labels_json: row.get(14)?,
        relations_json: row.get(15)?,
        payload_json: row.get(16)?,
        git_branch_name: row.get(17)?,
        due_date: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
        completed_at: row.get(21)?,
        archived_at: row.get(22)?,
        first_seen_at: row.get(23)?,
        last_seen_at: row.get(24)?,
        last_ingest_run_id: row.get(25)?,
    })
}

fn map_planning_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPlanningItem> {
    Ok(StoredPlanningItem {
        id: row.get(0)?,
        canonical_key: row.get(1)?,
        item_type: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        status: row.get(5)?,
        reconciliation_status: row.get(6)?,
        source_system: row.get(7)?,
        source_workspace: row.get(8)?,
        source_project: row.get(9)?,
        source_key: row.get(10)?,
        seeded_from_mirror_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn map_planning_item_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPlanningItemLink> {
    Ok(StoredPlanningItemLink {
        id: row.get(0)?,
        planning_item_id: row.get(1)?,
        link_type: row.get(2)?,
        target_planning_item_id: row.get(3)?,
        target_external_issue_mirror_id: row.get(4)?,
        metadata_json: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn map_promotion_record_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPromotionRecord> {
    Ok(StoredPromotionRecord {
        id: row.get(0)?,
        subject_kind: row.get(1)?,
        subject_id: row.get(2)?,
        promotion_state: row.get(3)?,
        reason: row.get(4)?,
        source_ingest_run_id: row.get(5)?,
        created_at: row.get(6)?,
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

fn fetch_source_ingest_run(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredSourceIngestRun>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                source_system,
                source_workspace,
                source_project,
                artifact_path,
                artifact_sha256,
                status,
                imported_issue_count,
                imported_document_count,
                imported_milestone_count,
                imported_planning_item_count,
                error_message,
                created_at,
                completed_at
            FROM source_ingest_runs
            WHERE id = ?1
            "#,
            [id],
            map_source_ingest_run_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_source_artifact(
    connection: &Connection,
    ingest_run_id: i64,
    artifact_kind: &str,
    artifact_key: &str,
) -> Result<Option<StoredSourceArtifact>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                ingest_run_id,
                artifact_kind,
                artifact_key,
                title,
                url,
                payload_json,
                created_at
            FROM source_artifacts
            WHERE ingest_run_id = ?1
              AND artifact_kind = ?2
              AND artifact_key = ?3
            "#,
            params![ingest_run_id, artifact_kind, artifact_key],
            map_source_artifact_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_external_issue_mirror_by_key(
    connection: &Connection,
    mirror_key: &str,
) -> Result<Option<StoredExternalIssueMirror>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                mirror_key,
                source_system,
                source_workspace,
                source_project,
                external_issue_id,
                project_name,
                team_name,
                parent_external_issue_id,
                title,
                description,
                state,
                priority,
                url,
                labels_json,
                relations_json,
                payload_json,
                git_branch_name,
                due_date,
                created_at,
                updated_at,
                completed_at,
                archived_at,
                first_seen_at,
                last_seen_at,
                last_ingest_run_id
            FROM external_issue_mirrors
            WHERE mirror_key = ?1
            "#,
            [mirror_key],
            map_external_issue_mirror_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item(connection: &Connection, id: i64) -> Result<Option<StoredPlanningItem>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                canonical_key,
                item_type,
                title,
                description,
                status,
                reconciliation_status,
                source_system,
                source_workspace,
                source_project,
                source_key,
                seeded_from_mirror_id,
                created_at,
                updated_at
            FROM planning_items
            WHERE id = ?1
            "#,
            [id],
            map_planning_item_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item_by_canonical_key(
    connection: &Connection,
    canonical_key: &str,
) -> Result<Option<StoredPlanningItem>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                canonical_key,
                item_type,
                title,
                description,
                status,
                reconciliation_status,
                source_system,
                source_workspace,
                source_project,
                source_key,
                seeded_from_mirror_id,
                created_at,
                updated_at
            FROM planning_items
            WHERE canonical_key = ?1
            "#,
            [canonical_key],
            map_planning_item_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item_link(
    connection: &Connection,
    planning_item_id: i64,
    link_type: &str,
    target_planning_item_id: Option<i64>,
    target_external_issue_mirror_id: Option<i64>,
) -> Result<Option<StoredPlanningItemLink>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                planning_item_id,
                link_type,
                target_planning_item_id,
                target_external_issue_mirror_id,
                metadata_json,
                created_at
            FROM planning_item_links
            WHERE planning_item_id = ?1
              AND link_type = ?2
              AND ((target_planning_item_id IS NULL AND ?3 IS NULL) OR target_planning_item_id = ?3)
              AND ((target_external_issue_mirror_id IS NULL AND ?4 IS NULL) OR target_external_issue_mirror_id = ?4)
            LIMIT 1
            "#,
            params![
                planning_item_id,
                link_type,
                target_planning_item_id,
                target_external_issue_mirror_id
            ],
            map_planning_item_link_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item_link_by_id(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredPlanningItemLink>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                planning_item_id,
                link_type,
                target_planning_item_id,
                target_external_issue_mirror_id,
                metadata_json,
                created_at
            FROM planning_item_links
            WHERE id = ?1
            "#,
            [id],
            map_planning_item_link_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_promotion_record(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredPromotionRecord>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                subject_kind,
                subject_id,
                promotion_state,
                reason,
                source_ingest_run_id,
                created_at
            FROM promotion_records
            WHERE id = ?1
            "#,
            [id],
            map_promotion_record_row,
        )
        .optional()
        .map_err(Into::into)
}

fn ensure_forge_task_columns(connection: &Connection) -> Result<()> {
    if !table_exists(connection, "plugin_forge_tasks")? {
        return Ok(());
    }

    let mut statement = connection.prepare("PRAGMA table_info(plugin_forge_tasks)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    if !columns.iter().any(|column| column == "required_tokens") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN required_tokens TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "status_message") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN status_message TEXT",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "working_dir") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN working_dir TEXT",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "stdin_text") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN stdin_text TEXT",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "metadata") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN metadata TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }

    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(exists != 0)
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

        let task_id =
            store.insert_forge_task("Echo", "echo", r#"["hello"]"#, None, None, "[]", "{}")?;
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

    #[test]
    fn landing_tables_round_trip() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let run = store.create_source_ingest_run(NewSourceIngestRun {
            source_system: "linear",
            source_workspace: "microt",
            source_project: "Entrance",
            artifact_path: Some("A:/Agent/linear-snapshot.json"),
            artifact_sha256: Some("abc123"),
            status: "running",
        })?;

        let artifact = store.insert_source_artifact(NewSourceArtifact {
            ingest_run_id: run.id,
            artifact_kind: "snapshot",
            artifact_key: "linear:microt:Entrance:snapshot:test",
            title: Some("Entrance snapshot"),
            url: Some("https://linear.app/project/entrance"),
            payload_json: r#"{"issues":[]}"#,
        })?;

        let mirror = store.upsert_external_issue_mirror(UpsertExternalIssueMirror {
            ingest_run_id: run.id,
            mirror_key: "linear:microt:Entrance:issue:MYT-1",
            source_system: "linear",
            source_workspace: "microt",
            source_project: "Entrance",
            external_issue_id: "MYT-1",
            project_name: Some("Entrance"),
            team_name: Some("Pub"),
            parent_external_issue_id: None,
            title: "Bootstrap ownership",
            description: Some("first issue"),
            state: Some("Todo"),
            priority: Some("High"),
            url: Some("https://linear.app/microt/issue/MYT-1"),
            labels_json: r#"["Feature"]"#,
            relations_json: r#"{"blocks":[],"blockedBy":[],"relatedTo":[],"duplicateOf":null}"#,
            payload_json: r#"{"id":"MYT-1"}"#,
            git_branch_name: Some("kc2003/myt-1"),
            due_date: None,
            created_at: Some("2026-03-22T00:00:00.000Z"),
            updated_at: Some("2026-03-22T00:00:00.000Z"),
            completed_at: None,
            archived_at: None,
        })?;

        let planning_item = store.upsert_planning_item(UpsertPlanningItem {
            canonical_key: Some("linear:microt:Entrance:issue:MYT-1"),
            item_type: "issue",
            title: "Bootstrap ownership",
            description: Some("seeded from mirror"),
            status: "seeded",
            reconciliation_status: "unreconciled",
            source_system: Some("linear"),
            source_workspace: Some("microt"),
            source_project: Some("Entrance"),
            source_key: Some("MYT-1"),
            seeded_from_mirror_id: Some(mirror.id),
        })?;

        let link = store.ensure_planning_item_link(NewPlanningItemLink {
            planning_item_id: planning_item.id,
            link_type: "mirrors",
            target_planning_item_id: None,
            target_external_issue_mirror_id: Some(mirror.id),
            metadata_json: r#"{"seed":"external_issue_mirror"}"#,
        })?;

        let promotion = store.append_promotion_record(NewPromotionRecord {
            subject_kind: "planning_item",
            subject_id: planning_item.id,
            promotion_state: "storage_only",
            reason: Some("seeded on import"),
            source_ingest_run_id: Some(run.id),
        })?;

        let run = store.complete_source_ingest_run(
            run.id,
            SourceIngestRunCompletion {
                status: "completed",
                imported_issue_count: 1,
                imported_document_count: 0,
                imported_milestone_count: 0,
                imported_planning_item_count: 1,
                error_message: None,
            },
        )?;

        let runs = store.list_source_ingest_runs()?;
        let artifacts = store.list_source_artifacts(run.id)?;
        let mirrors = store.list_external_issue_mirrors()?;
        let items = store.list_planning_items()?;
        let unreconciled = store.list_unreconciled_planning_items()?;
        let links = store.list_planning_item_links()?;
        let promotions = store.list_promotion_records()?;

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(runs[0].imported_issue_count, 1);
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].id, artifact.id);
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].external_issue_id, "MYT-1");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, planning_item.id);
        assert_eq!(unreconciled.len(), 1);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, link.id);
        assert_eq!(promotions.len(), 1);
        assert_eq!(promotions[0].id, promotion.id);

        Ok(())
    }
}
