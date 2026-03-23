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
        let path = std::env::temp_dir().join(format!("entrance-nota-cli-{name}-{suffix}"));
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
fn nota_checkpoint_cli_persists_cadence_checkpoint_without_memory_fragment_fallback() -> Result<()>
{
    let temp_dir = TempDir::new("checkpoint")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let first_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "checkpoint",
            "--stable-level",
            "single-ingress, checkpointed, DB-first NOTA host",
            "--landed",
            "cadence object storage cut",
            "--remaining",
            "Do automatic checkpoint/receipt",
            "--remaining",
            "design-governance persistence",
            "--human-continuity-bus",
            "reduced but not eliminated",
            "--selected-trunk",
            "cadence storage cut",
            "--next-start-hint",
            "wire Do receipts",
        ],
    )?;
    let first: Value = serde_json::from_str(&first_output)
        .context("nota checkpoint output should be valid JSON")?;
    assert_eq!(first["checkpoint"]["cadence_kind"], "CADENCE_CHECKPOINT");
    assert_eq!(
        first["checkpoint"]["payload"]["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host"
    );
    assert_eq!(first["superseded_checkpoint_id"], Value::Null);

    let second_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "checkpoint",
            "--title",
            "Second checkpoint",
            "--stable-level",
            "single-ingress, checkpointed, DB-first NOTA host",
            "--landed",
            "cadence supersession relation",
            "--remaining",
            "Do automatic checkpoint/receipt",
            "--human-continuity-bus",
            "reduced but not eliminated",
            "--selected-trunk",
            "Do automatic checkpoint/receipt",
            "--next-start-hint",
            "persist runtime transactions",
        ],
    )?;
    let second: Value = serde_json::from_str(&second_output)
        .context("second nota checkpoint output should be valid JSON")?;
    assert_eq!(
        second["supersession_link"]["relation_type"],
        "superseded_by"
    );

    let list_output = run_nota_cli(&app_data_dir, &["nota", "checkpoints"])?;
    let listed: Value = serde_json::from_str(&list_output)
        .context("nota checkpoints output should be valid JSON")?;
    assert_eq!(listed["checkpoint_count"], 2);
    assert_eq!(listed["checkpoints"][0]["title"], "Second checkpoint");
    assert_eq!(listed["checkpoints"][0]["is_current"], true);
    assert_eq!(listed["checkpoints"][1]["is_current"], false);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(count_rows(&connection, "cadence_objects")?, 2);
    assert_eq!(count_rows(&connection, "cadence_links")?, 1);
    assert_eq!(count_rows(&connection, "memory_fragments")?, 0);

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
enabled = false
http_port = 9721

[plugins.vault]
enabled = false
"#,
    )?;

    Ok(())
}

fn run_nota_cli(app_data_dir: &Path, args: &[&str]) -> Result<String> {
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

fn count_rows(connection: &Connection, table: &str) -> Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&query, [], |row| row.get(0))?)
}
