use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use entrance_core::{
    compiler,
    data_store::{DataStore, MigrationPlan, MigrationStep},
};

use crate::{
    config::{ConfigStore, EntranceConfig},
    environment_runtime,
    paths::HarnessPaths,
    plugins::{forge, launcher, vault},
};

#[derive(Debug, Clone)]
pub struct RuntimeServices {
    paths: HarnessPaths,
    config: EntranceConfig,
    config_store: ConfigStore,
    data_store: DataStore,
}

impl RuntimeServices {
    pub fn paths(&self) -> &HarnessPaths {
        &self.paths
    }

    pub fn data_store(&self) -> DataStore {
        self.data_store.clone()
    }

    pub fn config_store(&self) -> ConfigStore {
        self.config_store.clone()
    }

    pub fn config(&self) -> &EntranceConfig {
        &self.config
    }

    pub fn theme(&self) -> &str {
        &self.config.shell.gui.theme
    }

    pub fn log_level(&self) -> &str {
        &self.config.core.log_level
    }

    pub fn mcp_enabled(&self) -> bool {
        self.config.shell.mcp.enabled
    }

    pub fn launcher_enabled(&self) -> bool {
        self.config.plugins.launcher.enabled
    }

    pub fn forge_enabled(&self) -> bool {
        self.config.plugins.forge.enabled
    }

    pub fn forge_http_port(&self) -> u16 {
        self.config.plugins.forge.http_port
    }

    pub fn vault_enabled(&self) -> bool {
        self.config.plugins.vault.enabled
    }

    pub fn launcher_hotkey(&self) -> Option<&str> {
        self.config
            .plugins
            .launcher
            .enabled
            .then_some(self.config.shell.gui.global_hotkey.as_str())
    }
}

pub fn resolve_app_data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ENTRANCE_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    dirs::home_dir()
        .map(|home| home.join(".entrance"))
        .context("failed to resolve Entrance owner root under the user home directory")
}

pub fn boot() -> Result<RuntimeServices> {
    boot_for_root(resolve_app_data_dir()?)
}

pub fn boot_for_root(root: impl AsRef<Path>) -> Result<RuntimeServices> {
    boot_for_paths(HarnessPaths::new(root.as_ref().to_path_buf()))
}

pub fn boot_for_paths(paths: HarnessPaths) -> Result<RuntimeServices> {
    std::fs::create_dir_all(paths.app_data_dir())?;
    let config_store = ConfigStore::load_or_create(paths.config_path())?;
    let config = config_store.config().clone();
    let paths = build_resolved_paths(paths.app_data_dir(), &config)?;
    migrate_legacy_runtime_db(paths.app_data_dir(), paths.db_path())?;
    paths.ensure_layout()?;

    let plugin_migrations = enabled_plugin_migrations(&config);
    let migration_plan = MigrationPlan::new(plugin_migrations.as_slice());
    let data_store = DataStore::open(paths.db_path(), migration_plan)?;
    compiler::registry::seed_registry_snapshot(&data_store)?;
    environment_runtime::record_runtime_environment(&data_store, &paths)?;

    Ok(RuntimeServices {
        paths,
        config,
        config_store,
        data_store,
    })
}

pub fn resolve_runtime_paths() -> Result<HarnessPaths> {
    let root = resolve_app_data_dir()?;
    std::fs::create_dir_all(&root)?;
    let config_store = ConfigStore::load_or_create(root.join("entrance.toml"))?;
    let paths = build_resolved_paths(&root, config_store.config())?;
    paths.ensure_layout()?;
    Ok(paths)
}

fn build_resolved_paths(root: impl AsRef<Path>, config: &EntranceConfig) -> Result<HarnessPaths> {
    let root = root.as_ref().to_path_buf();
    let config_path = root.join("entrance.toml");
    let db_path = resolve_owned_relative_path(&root, &config.paths.runtime_db, "paths.runtime_db")?;
    let data_dir = db_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let log_dir = resolve_owned_relative_path(&root, &config.paths.logs, "paths.logs")?;
    let cache_dir = resolve_owned_relative_path(&root, &config.paths.cache, "paths.cache")?;
    let exports_dir = resolve_owned_relative_path(&root, &config.paths.exports, "paths.exports")?;
    let snapshots_dir =
        resolve_owned_relative_path(&root, &config.paths.snapshots, "paths.snapshots")?;
    let worktrees_dir =
        resolve_owned_relative_path(&root, &config.paths.worktrees, "paths.worktrees")?;

    Ok(HarnessPaths::resolved(
        root,
        config_path,
        data_dir,
        db_path,
        log_dir,
        cache_dir,
        exports_dir,
        snapshots_dir,
        worktrees_dir,
    ))
}

fn enabled_plugin_migrations(config: &EntranceConfig) -> Vec<MigrationStep> {
    let mut migrations = Vec::new();

    if config.plugins.launcher.enabled {
        migrations.extend_from_slice(launcher::migrations());
    }

    if config.plugins.forge.enabled {
        migrations.extend_from_slice(forge::migrations());
    }

    if config.plugins.vault.enabled {
        migrations.extend_from_slice(vault::migrations());
    }

    migrations
}

fn resolve_owned_relative_path(root: &Path, raw: &str, label: &str) -> Result<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("`{label}` must not be empty");
    }

    let candidate = Path::new(trimmed);
    if candidate.is_absolute() {
        bail!("`{label}` must stay under the Entrance owner root and cannot be absolute");
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                bail!("`{label}` must not escape the Entrance owner root with `..`");
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("`{label}` must stay under the Entrance owner root");
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        bail!("`{label}` must not resolve to the owner root itself");
    }

    Ok(root.join(normalized))
}

fn migrate_legacy_runtime_db(root: &Path, db_path: &Path) -> Result<()> {
    let legacy_db_path = root.join("entrance.db");
    if !legacy_db_path.exists() || db_path.exists() || legacy_db_path == db_path {
        return Ok(());
    }

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(&legacy_db_path, db_path).with_context(|| {
        format!(
            "failed to migrate legacy runtime database from {} to {}",
            legacy_db_path.display(),
            db_path.display()
        )
    })?;

    Ok(())
}
