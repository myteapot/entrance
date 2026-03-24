use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
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
        let path = std::env::temp_dir().join(format!("entrance-landing-cli-{name}-{suffix}"));
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
fn landing_import_cli_absorbs_snapshot_into_existing_runtime_db() -> Result<()> {
    let temp_dir = TempDir::new("absorb-runtime-db")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;
    seed_pre_landing_runtime_db(&app_data_dir.join("entrance.db"))?;

    let snapshot_path = write_test_snapshot(temp_dir.path())?;
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "landing",
            "import",
            "--file",
            snapshot_path
                .to_str()
                .context("snapshot path should be valid UTF-8")?,
        ])
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .output()
        .context("failed to spawn `entrance landing import --file ...`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance landing import --file ...` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")?;
    let report: Value = serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;
    assert_eq!(report["source_system"], "linear");
    assert_eq!(report["source_workspace"], "microt");
    assert_eq!(report["source_project"], "Entrance");
    assert_eq!(report["imported_issue_count"], 2);
    assert_eq!(report["imported_document_count"], 1);
    assert_eq!(report["imported_milestone_count"], 1);
    assert_eq!(report["imported_planning_item_count"], 3);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;

    assert_eq!(count_rows(&connection, "core_event_log")?, 1);
    assert_eq!(count_rows(&connection, "plugin_forge_tasks")?, 1);
    assert_eq!(count_rows(&connection, "plugin_forge_task_logs")?, 1);

    assert_eq!(count_rows(&connection, "source_ingest_runs")?, 1);
    assert_eq!(count_rows(&connection, "source_artifacts")?, 5);
    assert_eq!(count_rows(&connection, "external_issue_mirrors")?, 2);
    assert_eq!(count_rows(&connection, "planning_items")?, 3);
    assert_eq!(count_rows(&connection, "planning_item_links")?, 5);
    assert_eq!(count_rows(&connection, "promotion_records")?, 5);

    let imported_artifact_path = connection.query_row(
        "SELECT artifact_path FROM source_ingest_runs ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get::<_, Option<String>>(0),
    )?;
    assert_eq!(imported_artifact_path.as_deref(), snapshot_path.to_str());

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

fn seed_pre_landing_runtime_db(db_path: &Path) -> Result<()> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute_batch(include_str!("../migrations/0000_create_core_tables.sql"))?;
    connection.execute_batch(include_str!(
        "../migrations/0002_create_plugin_forge_tasks.sql"
    ))?;
    connection.execute_batch(include_str!(
        "../migrations/0004_create_plugin_forge_task_logs.sql"
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
    let task_id = connection.last_insert_rowid();
    connection.execute(
        "INSERT INTO plugin_forge_task_logs (task_id, stream, line, created_at) VALUES (?1, ?2, ?3, ?4)",
        (task_id, "stdout", "runtime log", "2026-03-23T00:00:01Z"),
    )?;

    Ok(())
}

fn count_rows(connection: &Connection, table: &str) -> Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&query, [], |row| row.get(0))?)
}

fn write_test_snapshot(root: &Path) -> Result<PathBuf> {
    let path = root.join("linear-entrance-snapshot-test.json");
    let payload = r##"{
  "generated_at": "2026-03-22T10:37:32.223Z",
  "source": {
    "system": "linear",
    "workspace": "microt",
    "project": "Entrance"
  },
  "project": {
    "id": "project-1",
    "name": "Entrance",
    "url": "https://linear.app/microt/project/entrance",
    "description": "Entrance project",
    "summary": "",
    "state": "Backlog",
    "priority": "High",
    "startDate": null,
    "targetDate": null
  },
  "milestones": [
    {
      "id": "milestone-1",
      "name": "Bootstrap Ownership",
      "description": "First candidate milestone",
      "targetDate": null
    }
  ],
  "documents": [
    {
      "id": "doc-1",
      "title": "Landing Notes",
      "slug": "landing-notes",
      "updatedAt": "2026-03-22T10:00:00.000Z",
      "content": "# Notes"
    }
  ],
  "issues": [
    {
      "id": "MYT-100",
      "title": "Seed landing layer",
      "description": "Import the first snapshot",
      "state": "Todo",
      "priority": "High",
      "url": "https://linear.app/microt/issue/MYT-100",
      "project": "Entrance",
      "team": "Pub",
      "parentId": null,
      "labels": ["Feature"],
      "createdAt": "2026-03-22T10:00:00.000Z",
      "updatedAt": "2026-03-22T10:10:00.000Z",
      "completedAt": null,
      "archivedAt": null,
      "dueDate": null,
      "gitBranchName": "kc2003/myt-100",
      "relations": {
        "blocks": ["MYT-101"],
        "blockedBy": [],
        "relatedTo": [],
        "duplicateOf": null
      }
    },
    {
      "id": "MYT-101",
      "title": "Read landing layer",
      "description": "List imported planning items",
      "state": "Backlog",
      "priority": "Medium",
      "url": "https://linear.app/microt/issue/MYT-101",
      "project": "Entrance",
      "team": "Pub",
      "parentId": "MYT-100",
      "labels": [],
      "createdAt": "2026-03-22T10:05:00.000Z",
      "updatedAt": "2026-03-22T10:15:00.000Z",
      "completedAt": null,
      "archivedAt": null,
      "dueDate": null,
      "gitBranchName": null,
      "relations": {
        "blocks": [],
        "blockedBy": ["MYT-100"],
        "relatedTo": [],
        "duplicateOf": null
      }
    }
  ]
}"##;

    fs::write(&path, payload)
        .with_context(|| format!("failed to write test snapshot to `{}`", path.display()))?;
    Ok(path)
}
