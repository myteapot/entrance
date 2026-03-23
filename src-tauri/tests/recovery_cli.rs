use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Result<Self> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time should be after UNIX_EPOCH")?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("entrance-recovery-cli-{name}-{suffix}"));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create temp dir at {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn recovery_import_seed_cli_absorbs_seed_rows_into_existing_runtime_db() -> Result<()> {
    let temp_dir = TempDir::new("absorb-seed-runtime-db")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;
    seed_preexisting_runtime_db(&app_data_dir.join("entrance.db"))?;

    let recovery_seed_path = write_test_recovery_seed(temp_dir.path())?;
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "recovery",
            "import-seed",
            "--file",
            recovery_seed_path
                .to_str()
                .context("recovery seed path should be valid UTF-8")?,
        ])
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .output()
        .context("failed to spawn `entrance recovery import-seed --file ...`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance recovery import-seed --file ...` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")?;
    let report: Value = serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;
    assert_eq!(report["source_system"], "recovery_seed");
    assert_eq!(report["source_workspace"], "repo_root");
    assert_eq!(report["source_project"], "Entrance");
    assert_eq!(report["imported_table_count"], 7);
    assert_eq!(report["imported_row_count"], 7);
    assert_eq!(report["imported_artifact_count"], 9);
    assert_eq!(report["table_row_counts"]["chat_logs"], 0);
    assert_eq!(report["table_row_counts"]["documents"], 1);
    assert_eq!(report["table_row_counts"]["memory_fragments"], 2);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;

    assert_eq!(count_rows(&connection, "core_event_log")?, 1);
    assert_eq!(count_rows(&connection, "plugin_forge_tasks")?, 1);

    assert_eq!(count_rows(&connection, "source_ingest_runs")?, 1);
    assert_eq!(count_rows(&connection, "source_artifacts")?, 9);
    assert_eq!(count_rows(&connection, "promotion_records")?, 9);

    let document_payload = connection.query_row(
        r#"
        SELECT payload_json
        FROM source_artifacts
        WHERE artifact_kind = 'recovery_seed_row'
          AND artifact_key = 'documents:1'
        "#,
        [],
        |row| row.get::<_, String>(0),
    )?;
    let document_payload: Value = serde_json::from_str(&document_payload)
        .context("stored document payload should be JSON")?;
    assert_eq!(document_payload["source_table"], "documents");
    assert_eq!(document_payload["source_row"]["title"], "Recovered doc");

    let manifest_payload = connection.query_row(
        r#"
        SELECT payload_json
        FROM source_artifacts
        WHERE artifact_kind = 'recovery_seed_manifest'
        "#,
        [],
        |row| row.get::<_, String>(0),
    )?;
    let manifest_payload: Value = serde_json::from_str(&manifest_payload)
        .context("stored manifest payload should be JSON")?;
    assert_eq!(manifest_payload["table_row_counts"]["schema_meta"], 1);
    assert_eq!(manifest_payload["table_row_counts"]["todos"], 1);

    Ok(())
}

#[test]
fn recovery_cli_lists_absorbed_seed_runs_and_rows() -> Result<()> {
    let temp_dir = TempDir::new("list-seed-runtime-db")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;
    seed_preexisting_runtime_db(&app_data_dir.join("entrance.db"))?;

    let recovery_seed_path = write_test_recovery_seed(temp_dir.path())?;
    run_recovery_cli(
        &app_data_dir,
        &[
            "recovery",
            "import-seed",
            "--file",
            recovery_seed_path
                .to_str()
                .context("recovery seed path should be valid UTF-8")?,
        ],
    )?;

    let runs_output = run_recovery_cli(&app_data_dir, &["recovery", "runs"])?;
    let runs: Value =
        serde_json::from_str(&runs_output).context("recovery runs output should be valid JSON")?;
    let runs = runs
        .as_array()
        .context("recovery runs output should be an array")?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["source_system"], "recovery_seed");
    assert_eq!(runs[0]["imported_table_count"], 7);
    assert_eq!(runs[0]["imported_row_count"], 7);
    assert_eq!(runs[0]["table_row_counts"]["memory_fragments"], 2);

    let rows_output = run_recovery_cli(
        &app_data_dir,
        &["recovery", "rows", "--table", "documents", "--limit", "5"],
    )?;
    let rows: Value =
        serde_json::from_str(&rows_output).context("recovery rows output should be valid JSON")?;
    assert_eq!(rows["ingest_run"]["source_system"], "recovery_seed");
    assert_eq!(rows["requested_table"], "documents");
    assert_eq!(rows["total_matching_rows"], 1);
    assert_eq!(
        rows["rows"]
            .as_array()
            .context("rows should be an array")?
            .len(),
        1
    );
    assert_eq!(rows["rows"][0]["source_table"], "documents");
    assert_eq!(rows["rows"][0]["source_row"]["title"], "Recovered doc");
    assert_eq!(rows["rows"][0]["promotion_state"], "storage_only");

    Ok(())
}

#[test]
fn recovery_promote_safe_v0_cli_promotes_stable_memory_families_idempotently() -> Result<()> {
    let temp_dir = TempDir::new("promote-safe-v0-runtime-db")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;
    seed_preexisting_runtime_db(&app_data_dir.join("entrance.db"))?;

    let recovery_seed_path = write_promotable_recovery_seed(temp_dir.path())?;
    run_recovery_cli(
        &app_data_dir,
        &[
            "recovery",
            "import-seed",
            "--file",
            recovery_seed_path
                .to_str()
                .context("recovery seed path should be valid UTF-8")?,
        ],
    )?;

    let promote_output = run_recovery_cli(&app_data_dir, &["recovery", "promote-safe-v0"])?;
    let report: Value = serde_json::from_str(&promote_output)
        .context("recovery promote-safe-v0 output should be valid JSON")?;
    assert_eq!(report["total_candidate_rows"], 4);
    assert_eq!(report["upserted_row_count"], 4);
    assert_eq!(report["new_promotion_record_count"], 4);
    assert_eq!(report["rows_by_table"]["documents"], 1);
    assert_eq!(report["rows_by_table"]["todos"], 1);
    assert_eq!(report["rows_by_table"]["instincts"], 1);
    assert_eq!(report["rows_by_table"]["coffee_chats"], 1);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(count_rows(&connection, "documents")?, 1);
    assert_eq!(count_rows(&connection, "todos")?, 1);
    assert_eq!(count_rows(&connection, "instincts")?, 1);
    assert_eq!(count_rows(&connection, "coffee_chats")?, 1);

    assert!(table_has_column(&connection, "todos", "due_on")?);
    assert!(table_has_column(&connection, "todos", "reminder_status")?);
    assert!(table_has_column(
        &connection,
        "instincts",
        "lifecycle_status"
    )?);
    assert!(table_has_column(&connection, "instincts", "temperature")?);
    assert!(table_has_column(
        &connection,
        "coffee_chats",
        "temperature"
    )?);

    let rerun_output = run_recovery_cli(&app_data_dir, &["recovery", "promote-safe-v0"])?;
    let rerun: Value = serde_json::from_str(&rerun_output)
        .context("recovery promote-safe-v0 rerun output should be valid JSON")?;
    assert_eq!(rerun["upserted_row_count"], 4);
    assert_eq!(rerun["new_promotion_record_count"], 0);

    Ok(())
}

#[test]
fn recovery_promote_remaining_v0_cli_promotes_remaining_memory_families_idempotently() -> Result<()>
{
    let temp_dir = TempDir::new("promote-remaining-v0-runtime-db")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;
    seed_preexisting_runtime_db(&app_data_dir.join("entrance.db"))?;

    let recovery_seed_path = write_remaining_promotable_recovery_seed(temp_dir.path())?;
    run_recovery_cli(
        &app_data_dir,
        &[
            "recovery",
            "import-seed",
            "--file",
            recovery_seed_path
                .to_str()
                .context("recovery seed path should be valid UTF-8")?,
        ],
    )?;

    let promote_output = run_recovery_cli(&app_data_dir, &["recovery", "promote-remaining-v0"])?;
    let report: Value = serde_json::from_str(&promote_output)
        .context("recovery promote-remaining-v0 output should be valid JSON")?;
    assert_eq!(report["total_candidate_rows"], 4);
    assert_eq!(report["upserted_row_count"], 4);
    assert_eq!(report["new_promotion_record_count"], 4);
    assert_eq!(report["rows_by_table"]["decisions"], 1);
    assert_eq!(report["rows_by_table"]["visions"], 1);
    assert_eq!(report["rows_by_table"]["memory_fragments"], 1);
    assert_eq!(report["rows_by_table"]["memory_links"], 1);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(count_rows(&connection, "decisions")?, 1);
    assert_eq!(count_rows(&connection, "visions")?, 1);
    assert_eq!(count_rows(&connection, "memory_fragments")?, 1);
    assert_eq!(count_rows(&connection, "memory_links")?, 1);

    let rerun_output = run_recovery_cli(&app_data_dir, &["recovery", "promote-remaining-v0"])?;
    let rerun: Value = serde_json::from_str(&rerun_output)
        .context("recovery promote-remaining-v0 rerun output should be valid JSON")?;
    assert_eq!(rerun["upserted_row_count"], 4);
    assert_eq!(rerun["new_promotion_record_count"], 0);

    Ok(())
}

fn seed_app_state(app_data_dir: &Path) -> Result<()> {
    fs::create_dir_all(app_data_dir)?;
    fs::write(
        app_data_dir.join("entrance.toml"),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = false

[plugins.launcher]
enabled = false
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = true
http_port = 9721

[plugins.vault]
enabled = false
"#,
    )?;

    Ok(())
}

fn run_recovery_cli(app_data_dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(args)
        .env("ENTRANCE_APP_DATA_DIR", app_data_dir)
        .output()
        .with_context(|| format!("failed to spawn `entrance {}`", args.join(" ")))?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")
}

fn seed_preexisting_runtime_db(db_path: &Path) -> Result<()> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute_batch(include_str!("../migrations/0000_create_core_tables.sql"))?;
    connection.execute_batch(include_str!(
        "../migrations/0002_create_plugin_forge_tasks.sql"
    ))?;

    connection.execute(
        "INSERT INTO core_event_log (topic, payload, created_at) VALUES (?1, ?2, ?3)",
        ("test:runtime", Some("{}"), "2026-03-23T00:00:00Z"),
    )?;
    connection.execute(
        r#"
        INSERT INTO plugin_forge_tasks (
            name,
            command,
            args,
            working_dir,
            stdin_text,
            required_tokens,
            metadata,
            status,
            status_message,
            exit_code,
            created_at,
            finished_at
        ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?5, ?6, NULL, NULL, ?7, NULL)
        "#,
        (
            "Runtime task",
            "codex",
            "[]",
            "[]",
            "{}",
            "Pending",
            "2026-03-23T00:00:00Z",
        ),
    )?;

    Ok(())
}

fn write_test_recovery_seed(root: &Path) -> Result<PathBuf> {
    let db_path = root.join("recovered-entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute_batch(
        r#"
        CREATE TABLE schema_meta (
            version INTEGER,
            applied_at TEXT
        );
        CREATE TABLE chat_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT,
            role TEXT,
            summary TEXT,
            content TEXT,
            created_at TEXT
        );
        CREATE TABLE documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            category TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            statement TEXT NOT NULL,
            rationale TEXT NOT NULL,
            decision_type TEXT NOT NULL,
            actor_scope TEXT NOT NULL,
            enforcement_level TEXT NOT NULL,
            scope_type TEXT NOT NULL,
            scope_ref TEXT NOT NULL,
            decision_status TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.8,
            decided_by TEXT NOT NULL DEFAULT '',
            source_ref TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE memory_fragments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            source_type TEXT NOT NULL DEFAULT '',
            source_ref TEXT NOT NULL DEFAULT '',
            confidence REAL NOT NULL DEFAULT 0.8,
            status TEXT NOT NULL DEFAULT 'active',
            target_table TEXT NOT NULL DEFAULT '',
            target_ref TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_hash TEXT NOT NULL DEFAULT '',
            scope_type TEXT NOT NULL DEFAULT '',
            scope_ref TEXT NOT NULL DEFAULT '',
            triage_status TEXT NOT NULL DEFAULT 'new',
            temperature TEXT NOT NULL DEFAULT 'warm'
        );
        CREATE TABLE memory_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            src_kind TEXT NOT NULL,
            src_id INTEGER NOT NULL,
            dst_kind TEXT NOT NULL,
            dst_id INTEGER NOT NULL,
            relation_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            priority INTEGER NOT NULL DEFAULT 2,
            project TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            done_at TEXT,
            temperature TEXT NOT NULL DEFAULT 'warm',
            due_on TEXT NOT NULL DEFAULT '',
            remind_every_days INTEGER NOT NULL DEFAULT 0,
            remind_next_on TEXT NOT NULL DEFAULT '',
            last_reminded_at TEXT NOT NULL DEFAULT '',
            reminder_status TEXT NOT NULL DEFAULT 'none'
        );
        "#,
    )?;

    connection.execute(
        "INSERT INTO schema_meta (version, applied_at) VALUES (?1, ?2)",
        (8, "2026-03-22T07:08:04Z"),
    )?;
    connection.execute(
        r#"
        INSERT INTO documents (id, slug, title, content, category, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        (
            1,
            "recovered-doc",
            "Recovered doc",
            "# recovered",
            "architecture",
            "2026-03-22T00:00:00Z",
            "2026-03-22T00:10:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO decisions (
            id, title, statement, rationale, decision_type, actor_scope, enforcement_level,
            scope_type, scope_ref, decision_status, confidence, decided_by, source_ref,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        "#,
        (
            1,
            "Keep runtime owner",
            "Runtime DB is canonical owner",
            "Need one owner",
            "architecture",
            "nota",
            "required",
            "project",
            "Entrance",
            "active",
            0.95f64,
            "nota",
            "recovery",
            "2026-03-22T00:00:00Z",
            "2026-03-22T00:05:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_fragments (
            id, kind, title, content, source_type, source_ref, confidence, status,
            target_table, target_ref, tags, notes, created_at, updated_at, source_hash,
            scope_type, scope_ref, triage_status, temperature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
        "#,
        params![
            1,
            "note",
            "Fragment one",
            "Remember import boundary",
            "manual",
            "seed",
            0.8f64,
            "active",
            "documents",
            "1",
            "import,boundary",
            "",
            "2026-03-22T00:00:00Z",
            "2026-03-22T00:00:00Z",
            "hash-1",
            "project",
            "Entrance",
            "new",
            "warm",
        ],
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_fragments (
            id, kind, title, content, source_type, source_ref, confidence, status,
            target_table, target_ref, tags, notes, created_at, updated_at, source_hash,
            scope_type, scope_ref, triage_status, temperature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
        "#,
        params![
            2,
            "note",
            "Fragment two",
            "Preserve provenance",
            "manual",
            "seed",
            0.7f64,
            "active",
            "decisions",
            "1",
            "provenance",
            "",
            "2026-03-22T00:01:00Z",
            "2026-03-22T00:01:00Z",
            "hash-2",
            "project",
            "Entrance",
            "new",
            "cold",
        ],
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_links (
            id, src_kind, src_id, dst_kind, dst_id, relation_type, status, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        (
            1,
            "memory_fragment",
            1,
            "document",
            1,
            "supports",
            "active",
            "2026-03-22T00:02:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO todos (
            id, title, status, priority, project, created_at, done_at, temperature,
            due_on, remind_every_days, remind_next_on, last_reminded_at, reminder_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        (
            1,
            "Absorb recovery seed",
            "pending",
            1,
            "Entrance",
            "2026-03-22T00:03:00Z",
            "warm",
            "2026-03-30",
            0,
            "",
            "",
            "none",
        ),
    )?;

    let manifest_path = root.join("recovered-entrance.db.manifest.json");
    fs::write(
        &manifest_path,
        r#"{
  "kind": "entrance-db-seed",
  "created_at": "2026-03-22",
  "path": "A:\\Agent\\Entrance\\entrance.db",
  "sha256": "test-seed",
  "counts": {
    "schema_meta": 1,
    "chat_logs": 0,
    "documents": 1,
    "decisions": 1,
    "memory_fragments": 2,
    "memory_links": 1,
    "todos": 1
  }
}"#,
    )?;

    Ok(db_path)
}

fn write_promotable_recovery_seed(root: &Path) -> Result<PathBuf> {
    let db_path = root.join("promotable-recovery-seed.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute_batch(
        r#"
        CREATE TABLE schema_meta (
            version INTEGER,
            applied_at TEXT
        );
        CREATE TABLE documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            category TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            priority INTEGER NOT NULL DEFAULT 2,
            project TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            done_at TEXT,
            temperature TEXT NOT NULL DEFAULT 'warm',
            due_on TEXT NOT NULL DEFAULT '',
            remind_every_days INTEGER NOT NULL DEFAULT 0,
            remind_next_on TEXT NOT NULL DEFAULT '',
            last_reminded_at TEXT NOT NULL DEFAULT '',
            reminder_status TEXT NOT NULL DEFAULT 'none'
        );
        CREATE TABLE instincts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pattern TEXT NOT NULL,
            action TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.8,
            source TEXT NOT NULL DEFAULT '',
            ref TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            surfaced_to TEXT NOT NULL DEFAULT '',
            review_status TEXT NOT NULL DEFAULT '',
            origin_type TEXT NOT NULL DEFAULT 'manual',
            lifecycle_status TEXT NOT NULL DEFAULT 'active',
            temperature TEXT NOT NULL DEFAULT 'warm',
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE coffee_chats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project TEXT NOT NULL,
            stage TEXT NOT NULL,
            retro TEXT NOT NULL,
            forward TEXT NOT NULL,
            priorities TEXT NOT NULL,
            created_at TEXT NOT NULL,
            temperature TEXT NOT NULL DEFAULT 'warm'
        );
        "#,
    )?;

    connection.execute(
        "INSERT INTO schema_meta (version, applied_at) VALUES (?1, ?2)",
        (8, "2026-03-23T00:00:00Z"),
    )?;
    connection.execute(
        r#"
        INSERT INTO documents (id, slug, title, content, category, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        (
            1,
            "promoted-doc",
            "Promoted doc",
            "# promoted",
            "architecture",
            "2026-03-23T00:00:00Z",
            "2026-03-23T00:10:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO todos (
            id, title, status, priority, project, created_at, done_at, temperature,
            due_on, remind_every_days, remind_next_on, last_reminded_at, reminder_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        (
            1,
            "Promoted todo",
            "pending",
            1,
            "Entrance",
            "2026-03-23T00:15:00Z",
            "warm",
            "2026-03-30",
            0,
            "",
            "",
            "none",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO instincts (
            id, pattern, action, confidence, source, ref, created_at, status,
            surfaced_to, review_status, origin_type, lifecycle_status, temperature, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
        (
            1,
            "When recovery row is stable",
            "promote into cold memory",
            0.9f64,
            "recovery",
            "seed",
            "2026-03-23T00:20:00Z",
            "active",
            "",
            "approved",
            "manual",
            "active",
            "warm",
            "2026-03-23T00:25:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO coffee_chats (
            id, project, stage, retro, forward, priorities, created_at, temperature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        (
            1,
            "Entrance",
            "bootstrap",
            "absorbed recovery seed",
            "promote stable memory",
            "documents,todos",
            "2026-03-23T00:30:00Z",
            "warm",
        ),
    )?;

    Ok(db_path)
}

fn write_remaining_promotable_recovery_seed(root: &Path) -> Result<PathBuf> {
    let db_path = root.join("promotable-remaining-recovery-seed.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute_batch(
        r#"
        CREATE TABLE schema_meta (
            version INTEGER,
            applied_at TEXT
        );
        CREATE TABLE decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            statement TEXT NOT NULL,
            rationale TEXT NOT NULL DEFAULT '',
            decision_type TEXT NOT NULL DEFAULT '',
            decision_status TEXT NOT NULL DEFAULT 'accepted',
            scope_type TEXT NOT NULL DEFAULT '',
            scope_ref TEXT NOT NULL DEFAULT '',
            source_ref TEXT NOT NULL DEFAULT '',
            decided_by TEXT NOT NULL DEFAULT '',
            enforcement_level TEXT NOT NULL DEFAULT '',
            actor_scope TEXT NOT NULL DEFAULT '',
            confidence REAL NOT NULL DEFAULT 1.0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE visions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            statement TEXT NOT NULL,
            horizon TEXT NOT NULL DEFAULT '',
            vision_status TEXT NOT NULL DEFAULT 'active',
            scope_type TEXT NOT NULL DEFAULT '',
            scope_ref TEXT NOT NULL DEFAULT '',
            source_ref TEXT NOT NULL DEFAULT '',
            confidence REAL NOT NULL DEFAULT 1.0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE memory_fragments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT '',
            source_type TEXT NOT NULL DEFAULT '',
            source_ref TEXT NOT NULL DEFAULT '',
            source_hash TEXT NOT NULL DEFAULT '',
            scope_type TEXT NOT NULL DEFAULT '',
            scope_ref TEXT NOT NULL DEFAULT '',
            target_table TEXT NOT NULL DEFAULT '',
            target_ref TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT '',
            triage_status TEXT NOT NULL DEFAULT '',
            temperature TEXT NOT NULL DEFAULT 'warm',
            tags TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            confidence REAL NOT NULL DEFAULT 0.0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE memory_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            src_kind TEXT NOT NULL,
            src_id INTEGER NOT NULL,
            dst_kind TEXT NOT NULL,
            dst_id INTEGER NOT NULL,
            relation_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL
        );
        "#,
    )?;

    connection.execute(
        "INSERT INTO schema_meta (version, applied_at) VALUES (?1, ?2)",
        (9, "2026-03-23T00:00:00Z"),
    )?;
    connection.execute(
        r#"
        INSERT INTO decisions (
            id, title, statement, rationale, decision_type, decision_status, scope_type,
            scope_ref, source_ref, decided_by, enforcement_level, actor_scope,
            confidence, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        "#,
        (
            1,
            "Single runtime db",
            "Entrance should converge on one runtime db.",
            "Avoid split truth between repo root and app data.",
            "storage",
            "accepted",
            "project",
            "Entrance",
            "memory_fragments:1",
            "Human+NOTA",
            "hard",
            "system",
            0.95f64,
            "2026-03-23T00:00:00Z",
            "2026-03-23T00:05:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO visions (
            id, title, statement, horizon, vision_status, scope_type, scope_ref,
            source_ref, confidence, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        (
            1,
            "NOTA control plane",
            "Human should primarily interact through NOTA.",
            "long",
            "active",
            "system",
            "nota-control-plane",
            "memory_fragments:2",
            0.92f64,
            "2026-03-23T00:10:00Z",
            "2026-03-23T00:15:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_fragments (
            id, title, content, kind, source_type, source_ref, source_hash, scope_type,
            scope_ref, target_table, target_ref, status, triage_status, temperature,
            tags, notes, confidence, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
        "#,
        params![
            1,
            "Delete directory safety",
            "Raw directory deletion is forbidden.",
            "decision",
            "human-chat",
            "chat:2026-03-21/raw-directory-delete-policy",
            "seed-hash",
            "system",
            "filesystem",
            "decisions",
            "1",
            "promoted",
            "promoted",
            "hot",
            "safety",
            "Recovered and clarified.",
            1.0f64,
            "2026-03-23T00:20:00Z",
            "2026-03-23T00:25:00Z",
        ],
    )?;
    connection.execute(
        r#"
        INSERT INTO memory_links (
            id, src_kind, src_id, dst_kind, dst_id, relation_type, status, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        (
            1,
            "decision",
            1,
            "memory_fragments",
            1,
            "derived_from",
            "active",
            "2026-03-23T00:30:00Z",
        ),
    )?;

    Ok(db_path)
}

fn count_rows(connection: &Connection, table: &str) -> Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&query, [], |row| row.get(0))?)
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let query = format!("PRAGMA table_info({table})");
    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|name| name == column))
}
