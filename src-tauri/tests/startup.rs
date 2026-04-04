use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use entrance_lib::core::{
    bootstrap_for_paths,
    config_store::{render_config, EntranceConfig},
    AppPaths,
};
use rusqlite::Connection;

struct TempAppDir {
    path: PathBuf,
}

impl TempAppDir {
    fn new(name: &str) -> Result<Self> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time should be after UNIX_EPOCH")?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("entrance-startup-{name}-{suffix}"));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create temp dir at {}", path.display()))?;
        Ok(Self { path })
    }

    fn paths(&self) -> AppPaths {
        AppPaths::new(self.path.clone())
    }
}

impl Drop for TempAppDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn first_start_creates_default_config_and_database() -> Result<()> {
    let temp_dir = TempAppDir::new("default")?;
    let paths = temp_dir.paths();

    let startup = bootstrap_for_paths(paths.clone())?;

    assert!(paths.config_path().exists());
    assert!(paths.db_path().exists());
    assert_eq!(startup.theme(), "dark");
    assert_eq!(startup.log_level(), "info");
    assert!(startup.mcp_enabled());
    assert!(startup.launcher_enabled());
    assert_eq!(startup.launcher_hotkey(), Some("Alt+Space"));
    assert_eq!(startup.forge_http_port(), 9721);

    let config = fs::read_to_string(paths.config_path())?.replace("\r\n", "\n");
    assert_eq!(
        config,
        render_config(&EntranceConfig::default())?.replace("\r\n", "\n")
    );

    assert_tables_exist(
        paths.db_path(),
        &[
            "core_plugins",
            "core_hotkeys",
            "core_event_log",
            "plugin_launcher_apps",
        ],
    )?;

    Ok(())
}

#[test]
fn repository_default_template_matches_generated_default_config() -> Result<()> {
    let committed_template = include_str!("../../entrance.toml").replace("\r\n", "\n");
    assert_eq!(
        committed_template,
        render_config(&EntranceConfig::default())?.replace("\r\n", "\n")
    );
    Ok(())
}

#[test]
fn startup_only_runs_enabled_plugin_migrations() -> Result<()> {
    let temp_dir = TempAppDir::new("enabled-migrations")?;
    let paths = temp_dir.paths();

    fs::write(
        paths.config_path(),
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

    let startup = bootstrap_for_paths(paths.clone())?;

    assert!(!startup.mcp_enabled());
    assert!(!startup.launcher_enabled());
    assert_eq!(startup.launcher_hotkey(), None);
    assert_tables_exist(
        paths.db_path(),
        &["core_plugins", "core_hotkeys", "core_event_log"],
    )?;
    assert!(!table_exists(paths.db_path(), "plugin_launcher_apps")?);

    Ok(())
}

#[test]
fn config_changes_take_effect_after_restart() -> Result<()> {
    let temp_dir = TempAppDir::new("restart")?;
    let paths = temp_dir.paths();

    fs::write(
        paths.config_path(),
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

    let first_start = bootstrap_for_paths(paths.clone())?;
    assert_eq!(first_start.theme(), "dark");
    assert!(!first_start.mcp_enabled());
    assert!(!first_start.launcher_enabled());
    assert!(!table_exists(paths.db_path(), "plugin_launcher_apps")?);

    fs::write(
        paths.config_path(),
        r#"[core]
theme = "light"
log_level = "debug"
mcp_enabled = true

[plugins.launcher]
enabled = true
hotkey = "Ctrl+Space"
scan_paths = ["C:\\Tools"]

[plugins.forge]
enabled = true
http_port = 9833

[plugins.vault]
enabled = false
"#,
    )?;

    let second_start = bootstrap_for_paths(paths.clone())?;

    assert_eq!(second_start.theme(), "light");
    assert_eq!(second_start.log_level(), "debug");
    assert!(second_start.mcp_enabled());
    assert!(second_start.launcher_enabled());
    assert_eq!(second_start.launcher_hotkey(), Some("Ctrl+Space"));
    assert!(second_start.forge_enabled());
    assert_eq!(second_start.forge_http_port(), 9833);
    assert!(table_exists(paths.db_path(), "plugin_launcher_apps")?);
    assert!(table_exists(paths.db_path(), "plugin_forge_tasks")?);
    assert!(table_exists(paths.db_path(), "plugin_forge_task_logs")?);
    assert!(table_exists(
        paths.db_path(),
        "plugin_forge_dispatch_receipts"
    )?);

    Ok(())
}

#[test]
fn vault_migrations_run_when_plugin_is_enabled() -> Result<()> {
    let temp_dir = TempAppDir::new("vault-enabled")?;
    let paths = temp_dir.paths();

    fs::write(
        paths.config_path(),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = true

[plugins.launcher]
enabled = false
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = false
http_port = 9721

[plugins.vault]
enabled = true
"#,
    )?;

    let startup = bootstrap_for_paths(paths.clone())?;

    assert!(startup.vault_enabled());
    assert_tables_exist(
        paths.db_path(),
        &[
            "core_plugins",
            "core_hotkeys",
            "core_event_log",
            "plugin_vault_tokens",
            "plugin_vault_mcp_configs",
        ],
    )?;

    Ok(())
}

fn assert_tables_exist(db_path: &Path, tables: &[&str]) -> Result<()> {
    for table in tables {
        assert!(table_exists(db_path, table)?);
    }

    Ok(())
}

fn table_exists(db_path: &Path, table: &str) -> Result<bool> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(exists != 0)
}
