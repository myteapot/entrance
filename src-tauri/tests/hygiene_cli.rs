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
        let path = std::env::temp_dir().join(format!("entrance-hygiene-cli-{name}-{suffix}"));
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
fn hygiene_spec_v0_cli_persists_and_lists_spec_self_clean_findings() -> Result<()> {
    let temp_dir = TempDir::new("spec-v0")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let run_output = run_hygiene_cli(&app_data_dir, &["hygiene", "spec-v0"])?;
    let report: Value =
        serde_json::from_str(&run_output).context("hygiene spec-v0 output should be valid JSON")?;
    assert_eq!(report["workflow"], "spec_hygiene_v0");
    assert_eq!(report["finding_count"], 9);
    assert_eq!(report["relation_count"], 8);
    assert!(report["findings"]
        .as_array()
        .context("findings should be an array")?
        .iter()
        .any(|finding| {
            finding["target_ref"] == "specs/chore/top_self_cycle_handout.md"
                && finding["status"] == "archived"
        }));

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(
        count_where(
            &connection,
            "memory_fragments",
            "kind = 'spec_hygiene' AND source_type = 'runtime_hygiene'"
        )?,
        9
    );
    assert_eq!(count_rows(&connection, "memory_links")?, 8);

    let list_output = run_hygiene_cli(&app_data_dir, &["hygiene", "list-spec-v0"])?;
    let listed: Value = serde_json::from_str(&list_output)
        .context("hygiene list-spec-v0 output should be valid JSON")?;
    assert_eq!(listed["finding_count"], 9);
    assert_eq!(listed["relation_count"], 8);
    assert!(listed["relations"]
        .as_array()
        .context("relations should be an array")?
        .iter()
        .any(|relation| {
            relation["relation_type"] == "superseded_by"
                && relation["source_target_ref"] == "specs/chore/top_self_cycle_handout.md"
                && relation["target_target_ref"]
                    == "specs/chore/entrance_v0_headless_system_roadmap.md"
        }));

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

fn run_hygiene_cli(app_data_dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(args)
        .env("ENTRANCE_APP_DATA_DIR", app_data_dir)
        .output()
        .with_context(|| format!("failed to spawn `{}`", args.join(" ")))?;

    if !output.status.success() {
        anyhow::bail!(
            "`{}` failed: {}",
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

fn count_where(connection: &Connection, table: &str, filter: &str) -> Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table} WHERE {filter}");
    Ok(connection.query_row(&query, [], |row| row.get(0))?)
}
