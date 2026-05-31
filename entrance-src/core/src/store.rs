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

CREATE TABLE IF NOT EXISTS hive_loop_contracts (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    title               TEXT NOT NULL,
    goal                TEXT NOT NULL,
    boundary            TEXT NOT NULL,
    approach_space_json TEXT NOT NULL,
    eval_space_json     TEXT NOT NULL,
    review_surface      TEXT NOT NULL,
    autonomy_level      TEXT NOT NULL,
    runtime             TEXT NOT NULL,
    status              TEXT NOT NULL,
    active_phase        TEXT NOT NULL,
    current_round       INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS hive_loop_stages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER NOT NULL,
    round           INTEGER NOT NULL,
    role            TEXT NOT NULL,
    status          TEXT NOT NULL,
    summary         TEXT,
    input_json      TEXT NOT NULL,
    output_json     TEXT NOT NULL,
    started_at      TEXT,
    completed_at    TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id)
);

CREATE TABLE IF NOT EXISTS hive_loop_policies (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER NOT NULL,
    object_kind     TEXT NOT NULL,
    writer_role     TEXT NOT NULL,
    route_from      TEXT NOT NULL,
    route_to        TEXT NOT NULL,
    gate            TEXT NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id)
);

CREATE TABLE IF NOT EXISTS hive_loop_packets (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER NOT NULL,
    round           INTEGER NOT NULL,
    object_kind     TEXT NOT NULL,
    writer_role     TEXT NOT NULL,
    route_from      TEXT NOT NULL,
    route_to        TEXT NOT NULL,
    state_code      TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id)
);

CREATE TABLE IF NOT EXISTS hive_loop_admissions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER NOT NULL,
    packet_id       INTEGER NOT NULL,
    result          TEXT NOT NULL,
    reason          TEXT NOT NULL,
    policy_json     TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id),
    FOREIGN KEY(packet_id) REFERENCES hive_loop_packets(id)
);

CREATE TABLE IF NOT EXISTS hive_loop_evidence (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER NOT NULL,
    stage_id        INTEGER,
    round           INTEGER NOT NULL,
    kind            TEXT NOT NULL,
    summary         TEXT NOT NULL,
    path            TEXT,
    payload_json    TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id),
    FOREIGN KEY(stage_id) REFERENCES hive_loop_stages(id)
);

CREATE TABLE IF NOT EXISTS hive_loop_verdicts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER NOT NULL,
    round           INTEGER NOT NULL,
    decision        TEXT NOT NULL,
    summary         TEXT NOT NULL,
    score_json      TEXT NOT NULL,
    evidence_json   TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id)
);

CREATE TABLE IF NOT EXISTS hive_issues (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    loop_id         INTEGER,
    title           TEXT NOT NULL,
    status          TEXT NOT NULL,
    summary         TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    FOREIGN KEY(loop_id) REFERENCES hive_loop_contracts(id)
);

CREATE TABLE IF NOT EXISTS hive_comments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id        INTEGER NOT NULL,
    author          TEXT NOT NULL,
    body            TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    FOREIGN KEY(issue_id) REFERENCES hive_issues(id)
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
    pub hive_loops: i64,
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
pub struct HiveLoopContract {
    pub id: i64,
    pub title: String,
    pub goal: String,
    pub boundary: String,
    pub approach_space: Vec<String>,
    pub eval_space: Vec<String>,
    pub review_surface: String,
    pub autonomy_level: String,
    pub runtime: String,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopContractCreate {
    pub title: String,
    pub goal: String,
    pub boundary: String,
    pub approach_space: Vec<String>,
    pub eval_space: Vec<String>,
    pub review_surface: String,
    pub autonomy_level: String,
    pub runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopStage {
    pub id: i64,
    pub loop_id: i64,
    pub round: i64,
    pub role: String,
    pub status: String,
    pub summary: Option<String>,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopStageCreate {
    pub loop_id: i64,
    pub round: i64,
    pub role: String,
    pub status: String,
    pub summary: Option<String>,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopPolicy {
    pub id: i64,
    pub loop_id: i64,
    pub object_kind: String,
    pub writer_role: String,
    pub route_from: String,
    pub route_to: String,
    pub gate: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopPolicyCreate {
    pub loop_id: i64,
    pub object_kind: String,
    pub writer_role: String,
    pub route_from: String,
    pub route_to: String,
    pub gate: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopPacket {
    pub id: i64,
    pub loop_id: i64,
    pub round: i64,
    pub object_kind: String,
    pub writer_role: String,
    pub route_from: String,
    pub route_to: String,
    pub state_code: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopPacketCreate {
    pub loop_id: i64,
    pub round: i64,
    pub object_kind: String,
    pub writer_role: String,
    pub route_from: String,
    pub route_to: String,
    pub state_code: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopAdmission {
    pub id: i64,
    pub loop_id: i64,
    pub packet_id: i64,
    pub result: String,
    pub reason: String,
    pub policy: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopAdmissionCreate {
    pub loop_id: i64,
    pub packet_id: i64,
    pub result: String,
    pub reason: String,
    pub policy: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidence {
    pub id: i64,
    pub loop_id: i64,
    pub stage_id: Option<i64>,
    pub round: i64,
    pub kind: String,
    pub summary: String,
    pub path: Option<String>,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceCreate {
    pub loop_id: i64,
    pub stage_id: Option<i64>,
    pub round: i64,
    pub kind: String,
    pub summary: String,
    pub path: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopVerdict {
    pub id: i64,
    pub loop_id: i64,
    pub round: i64,
    pub decision: String,
    pub summary: String,
    pub score: serde_json::Value,
    pub evidence: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopVerdictCreate {
    pub loop_id: i64,
    pub round: i64,
    pub decision: String,
    pub summary: String,
    pub score: serde_json::Value,
    pub evidence: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveIssue {
    pub id: i64,
    pub loop_id: Option<i64>,
    pub title: String,
    pub status: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveIssueCreate {
    pub loop_id: Option<i64>,
    pub title: String,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveComment {
    pub id: i64,
    pub issue_id: i64,
    pub author: String,
    pub body: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveCommentCreate {
    pub issue_id: i64,
    pub author: String,
    pub body: String,
    pub payload: serde_json::Value,
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

    pub fn insert_hive_loop_contract(&self, row: HiveLoopContractCreate) -> Result<i64> {
        let now = timestamp();
        let approach_space_json = serde_json::to_string(&row.approach_space)?;
        let eval_space_json = serde_json::to_string(&row.eval_space)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_contracts
             (title, goal, boundary, approach_space_json, eval_space_json, review_surface, autonomy_level, runtime, status, active_phase, current_round, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'todo', 'explorer', 1, ?9, ?10)",
            params![
                row.title,
                row.goal,
                row.boundary,
                approach_space_json,
                eval_space_json,
                row.review_surface,
                row.autonomy_level,
                row.runtime,
                now,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_loop_contracts(&self) -> Result<Vec<HiveLoopContract>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, title, goal, boundary, approach_space_json, eval_space_json, review_surface, autonomy_level, runtime, status, active_phase, current_round, created_at, updated_at
             FROM hive_loop_contracts ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], map_hive_loop_contract)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_hive_loop_contract(&self, id: i64) -> Result<Option<HiveLoopContract>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, title, goal, boundary, approach_space_json, eval_space_json, review_surface, autonomy_level, runtime, status, active_phase, current_round, created_at, updated_at
             FROM hive_loop_contracts WHERE id = ?1 LIMIT 1",
        )?;
        statement
            .query_row(params![id], map_hive_loop_contract)
            .optional()
            .map_err(Into::into)
    }

    pub fn update_hive_loop_contract_state(
        &self,
        id: i64,
        status: &str,
        active_phase: &str,
        current_round: i64,
    ) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE hive_loop_contracts
             SET status = ?2, active_phase = ?3, current_round = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id, status, active_phase, current_round, timestamp()],
        )?;
        Ok(())
    }

    pub fn insert_hive_loop_stage(&self, row: HiveLoopStageCreate) -> Result<i64> {
        let now = timestamp();
        let input_json = serde_json::to_string(&row.input)?;
        let output_json = serde_json::to_string(&row.output)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_stages
             (loop_id, round, role, status, summary, input_json, output_json, started_at, completed_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.loop_id,
                row.round,
                row.role,
                row.status,
                row.summary,
                input_json,
                output_json,
                row.started_at,
                row.completed_at,
                now,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_loop_stages(&self, loop_id: i64) -> Result<Vec<HiveLoopStage>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, round, role, status, summary, input_json, output_json, started_at, completed_at, created_at, updated_at
             FROM hive_loop_stages WHERE loop_id = ?1 ORDER BY round ASC, id ASC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_loop_stage)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn insert_hive_loop_policy(&self, row: HiveLoopPolicyCreate) -> Result<i64> {
        let now = timestamp();
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_policies
             (loop_id, object_kind, writer_role, route_from, route_to, gate, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.loop_id,
                row.object_kind,
                row.writer_role,
                row.route_from,
                row.route_to,
                row.gate,
                row.status,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_loop_policies(&self, loop_id: i64) -> Result<Vec<HiveLoopPolicy>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, object_kind, writer_role, route_from, route_to, gate, status, created_at
             FROM hive_loop_policies WHERE loop_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_loop_policy)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn insert_hive_loop_packet(&self, row: HiveLoopPacketCreate) -> Result<i64> {
        let now = timestamp();
        let payload_json = serde_json::to_string(&row.payload)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_packets
             (loop_id, round, object_kind, writer_role, route_from, route_to, state_code, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.loop_id,
                row.round,
                row.object_kind,
                row.writer_role,
                row.route_from,
                row.route_to,
                row.state_code,
                payload_json,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn get_hive_loop_packet(&self, id: i64) -> Result<Option<HiveLoopPacket>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, round, object_kind, writer_role, route_from, route_to, state_code, payload_json, created_at
             FROM hive_loop_packets WHERE id = ?1 LIMIT 1",
        )?;
        statement
            .query_row(params![id], map_hive_loop_packet)
            .optional()
            .map_err(Into::into)
    }

    pub fn list_hive_loop_packets(&self, loop_id: i64) -> Result<Vec<HiveLoopPacket>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, round, object_kind, writer_role, route_from, route_to, state_code, payload_json, created_at
             FROM hive_loop_packets WHERE loop_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_loop_packet)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn insert_hive_loop_admission(&self, row: HiveLoopAdmissionCreate) -> Result<i64> {
        let now = timestamp();
        let policy_json = serde_json::to_string(&row.policy)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_admissions
             (loop_id, packet_id, result, reason, policy_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.loop_id,
                row.packet_id,
                row.result,
                row.reason,
                policy_json,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_loop_admissions(&self, loop_id: i64) -> Result<Vec<HiveLoopAdmission>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, packet_id, result, reason, policy_json, created_at
             FROM hive_loop_admissions WHERE loop_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_loop_admission)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn insert_hive_loop_evidence(&self, row: HiveLoopEvidenceCreate) -> Result<i64> {
        let now = timestamp();
        let payload_json = serde_json::to_string(&row.payload)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_evidence
             (loop_id, stage_id, round, kind, summary, path, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.loop_id,
                row.stage_id,
                row.round,
                row.kind,
                row.summary,
                row.path,
                payload_json,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_loop_evidence(&self, loop_id: i64) -> Result<Vec<HiveLoopEvidence>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, stage_id, round, kind, summary, path, payload_json, created_at
             FROM hive_loop_evidence WHERE loop_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_loop_evidence)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn insert_hive_loop_verdict(&self, row: HiveLoopVerdictCreate) -> Result<i64> {
        let now = timestamp();
        let score_json = serde_json::to_string(&row.score)?;
        let evidence_json = serde_json::to_string(&row.evidence)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_loop_verdicts
             (loop_id, round, decision, summary, score_json, evidence_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.loop_id,
                row.round,
                row.decision,
                row.summary,
                score_json,
                evidence_json,
                now
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_loop_verdicts(&self, loop_id: i64) -> Result<Vec<HiveLoopVerdict>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, round, decision, summary, score_json, evidence_json, created_at
             FROM hive_loop_verdicts WHERE loop_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_loop_verdict)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn insert_hive_issue(&self, row: HiveIssueCreate) -> Result<i64> {
        let now = timestamp();
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_issues (loop_id, title, status, summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![row.loop_id, row.title, row.status, row.summary, now, now],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_issues(&self) -> Result<Vec<HiveIssue>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, title, status, summary, created_at, updated_at
             FROM hive_issues ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], map_hive_issue)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_hive_issues_for_loop(&self, loop_id: i64) -> Result<Vec<HiveIssue>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, loop_id, title, status, summary, created_at, updated_at
             FROM hive_issues WHERE loop_id = ?1 ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![loop_id], map_hive_issue)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn update_hive_issue_status(
        &self,
        id: i64,
        status: &str,
        summary: Option<&str>,
    ) -> Result<()> {
        let connection = self.connection();
        connection.execute(
            "UPDATE hive_issues
             SET status = ?2, summary = COALESCE(?3, summary), updated_at = ?4
             WHERE id = ?1",
            params![id, status, summary, timestamp()],
        )?;
        Ok(())
    }

    pub fn insert_hive_comment(&self, row: HiveCommentCreate) -> Result<i64> {
        let now = timestamp();
        let payload_json = serde_json::to_string(&row.payload)?;
        let connection = self.connection();
        connection.execute(
            "INSERT INTO hive_comments (issue_id, author, body, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![row.issue_id, row.author, row.body, payload_json, now],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_hive_comments(&self, issue_id: i64) -> Result<Vec<HiveComment>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, issue_id, author, body, payload_json, created_at
             FROM hive_comments WHERE issue_id = ?1 ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![issue_id], map_hive_comment)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
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
            hive_loops: self.count("hive_loop_contracts")?,
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

fn map_hive_loop_contract(row: &Row<'_>) -> rusqlite::Result<HiveLoopContract> {
    let approach_space_json: String = row.get(4)?;
    let eval_space_json: String = row.get(5)?;
    Ok(HiveLoopContract {
        id: row.get(0)?,
        title: row.get(1)?,
        goal: row.get(2)?,
        boundary: row.get(3)?,
        approach_space: parse_json_vec(&approach_space_json),
        eval_space: parse_json_vec(&eval_space_json),
        review_surface: row.get(6)?,
        autonomy_level: row.get(7)?,
        runtime: row.get(8)?,
        status: row.get(9)?,
        active_phase: row.get(10)?,
        current_round: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn map_hive_loop_stage(row: &Row<'_>) -> rusqlite::Result<HiveLoopStage> {
    let input_json: String = row.get(6)?;
    let output_json: String = row.get(7)?;
    Ok(HiveLoopStage {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        round: row.get(2)?,
        role: row.get(3)?,
        status: row.get(4)?,
        summary: row.get(5)?,
        input: parse_json_value(&input_json),
        output: parse_json_value(&output_json),
        started_at: row.get(8)?,
        completed_at: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn map_hive_loop_policy(row: &Row<'_>) -> rusqlite::Result<HiveLoopPolicy> {
    Ok(HiveLoopPolicy {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        object_kind: row.get(2)?,
        writer_role: row.get(3)?,
        route_from: row.get(4)?,
        route_to: row.get(5)?,
        gate: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn map_hive_loop_packet(row: &Row<'_>) -> rusqlite::Result<HiveLoopPacket> {
    let payload_json: String = row.get(8)?;
    Ok(HiveLoopPacket {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        round: row.get(2)?,
        object_kind: row.get(3)?,
        writer_role: row.get(4)?,
        route_from: row.get(5)?,
        route_to: row.get(6)?,
        state_code: row.get(7)?,
        payload: parse_json_value(&payload_json),
        created_at: row.get(9)?,
    })
}

fn map_hive_loop_admission(row: &Row<'_>) -> rusqlite::Result<HiveLoopAdmission> {
    let policy_json: String = row.get(5)?;
    Ok(HiveLoopAdmission {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        packet_id: row.get(2)?,
        result: row.get(3)?,
        reason: row.get(4)?,
        policy: parse_json_value(&policy_json),
        created_at: row.get(6)?,
    })
}

fn map_hive_loop_evidence(row: &Row<'_>) -> rusqlite::Result<HiveLoopEvidence> {
    let payload_json: String = row.get(7)?;
    Ok(HiveLoopEvidence {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        stage_id: row.get(2)?,
        round: row.get(3)?,
        kind: row.get(4)?,
        summary: row.get(5)?,
        path: row.get(6)?,
        payload: parse_json_value(&payload_json),
        created_at: row.get(8)?,
    })
}

fn map_hive_loop_verdict(row: &Row<'_>) -> rusqlite::Result<HiveLoopVerdict> {
    let score_json: String = row.get(5)?;
    let evidence_json: String = row.get(6)?;
    Ok(HiveLoopVerdict {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        round: row.get(2)?,
        decision: row.get(3)?,
        summary: row.get(4)?,
        score: parse_json_value(&score_json),
        evidence: parse_json_value(&evidence_json),
        created_at: row.get(7)?,
    })
}

fn map_hive_issue(row: &Row<'_>) -> rusqlite::Result<HiveIssue> {
    Ok(HiveIssue {
        id: row.get(0)?,
        loop_id: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        summary: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn map_hive_comment(row: &Row<'_>) -> rusqlite::Result<HiveComment> {
    let payload_json: String = row.get(4)?;
    Ok(HiveComment {
        id: row.get(0)?,
        issue_id: row.get(1)?,
        author: row.get(2)?,
        body: row.get(3)?,
        payload: parse_json_value(&payload_json),
        created_at: row.get(5)?,
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

fn parse_json_vec(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn parse_json_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({ "raw": raw }))
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
