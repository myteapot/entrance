use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::plugins::{forge, launcher, vault};

use self::{
    config_store::{ConfigStore, EntranceConfig},
    data_store::{DataStore, MigrationPlan, MigrationStep},
};

pub mod action;
pub mod anti_zeno_runtime;
pub mod bootstrap_mcp_cycle;
pub mod chat_archive;
pub mod cold_docs_runtime;
pub mod compiler;
pub mod config_store;
pub mod data_store;
pub mod design_governance;
pub mod environment_runtime;
pub mod event_bus;
pub mod front_door;
pub mod hotkey;
pub mod hygiene;
pub mod invariant_runtime;
pub mod landing;
pub mod logging;
pub mod mcp_server;
pub mod mcp_stdio_client;
pub mod nota;
pub use nota as nota_runtime;
pub mod overview;
pub mod permission;
pub mod plugin_manager;
pub mod projection_runtime;
pub mod recovery;
pub mod supervision;
pub mod theme;
pub mod updater;
pub mod window;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    app_data_dir: PathBuf,
    config_path: PathBuf,
    data_dir: PathBuf,
    db_path: PathBuf,
    log_dir: PathBuf,
    cache_dir: PathBuf,
    exports_dir: PathBuf,
    snapshots_dir: PathBuf,
    worktrees_dir: PathBuf,
}

impl AppPaths {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        let app_data_dir = app_data_dir.into();
        Self {
            config_path: app_data_dir.join("entrance.toml"),
            data_dir: app_data_dir.join("data"),
            db_path: app_data_dir.join("data").join("entrance.db"),
            log_dir: app_data_dir.join("logs"),
            cache_dir: app_data_dir.join("cache"),
            exports_dir: app_data_dir.join("exports"),
            snapshots_dir: app_data_dir.join("snapshots"),
            worktrees_dir: app_data_dir.join("worktrees"),
            app_data_dir,
        }
    }

    pub fn from_config(app_data_dir: impl Into<PathBuf>, config: &EntranceConfig) -> Result<Self> {
        let app_data_dir = app_data_dir.into();
        let config_path = app_data_dir.join("entrance.toml");
        let db_path = resolve_owned_relative_path(
            &app_data_dir,
            &config.paths.runtime_db,
            "paths.runtime_db",
        )?;
        let data_dir = db_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| app_data_dir.clone());
        let log_dir = resolve_owned_relative_path(&app_data_dir, &config.paths.logs, "paths.logs")?;
        let cache_dir =
            resolve_owned_relative_path(&app_data_dir, &config.paths.cache, "paths.cache")?;
        let exports_dir =
            resolve_owned_relative_path(&app_data_dir, &config.paths.exports, "paths.exports")?;
        let snapshots_dir =
            resolve_owned_relative_path(&app_data_dir, &config.paths.snapshots, "paths.snapshots")?;
        let worktrees_dir =
            resolve_owned_relative_path(&app_data_dir, &config.paths.worktrees, "paths.worktrees")?;

        Ok(Self {
            app_data_dir,
            config_path,
            data_dir,
            db_path,
            log_dir,
            cache_dir,
            exports_dir,
            snapshots_dir,
            worktrees_dir,
        })
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn exports_dir(&self) -> &Path {
        &self.exports_dir
    }

    pub fn snapshots_dir(&self) -> &Path {
        &self.snapshots_dir
    }

    pub fn worktrees_dir(&self) -> &Path {
        &self.worktrees_dir
    }

    pub fn ensure_layout(&self) -> Result<()> {
        std::fs::create_dir_all(&self.app_data_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.log_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.exports_dir)?;
        std::fs::create_dir_all(&self.snapshots_dir)?;
        std::fs::create_dir_all(&self.worktrees_dir)?;
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

pub fn resolve_app_data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ENTRANCE_HOME") {
        return Ok(PathBuf::from(path));
    }

    if let Some(path) = std::env::var_os("ENTRANCE_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    dirs::home_dir()
        .map(|home| home.join(".entrance"))
        .context("failed to resolve Entrance owner root under the user home directory")
}

#[derive(Debug, Clone)]
pub struct StartupState {
    paths: AppPaths,
    config: EntranceConfig,
    config_store: ConfigStore,
    data_store: DataStore,
}

impl StartupState {
    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn data_store(&self) -> DataStore {
        self.data_store.clone()
    }

    pub fn config_store(&self) -> ConfigStore {
        self.config_store.clone()
    }

    pub fn theme(&self) -> &str {
        &self.config.core.theme
    }

    pub fn log_level(&self) -> &str {
        &self.config.core.log_level
    }

    pub fn mcp_enabled(&self) -> bool {
        self.config.core.mcp_enabled
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
            .then_some(self.config.plugins.launcher.hotkey.as_str())
    }
}

pub fn bootstrap_for_paths(paths: AppPaths) -> Result<StartupState> {
    std::fs::create_dir_all(paths.app_data_dir())?;
    let config_store = ConfigStore::load_or_create(paths.config_path())?;
    let config = config_store.config().clone();
    let paths = AppPaths::from_config(paths.app_data_dir().to_path_buf(), &config)?;
    migrate_legacy_runtime_db(paths.app_data_dir(), paths.db_path())?;
    paths.ensure_layout()?;

    let plugin_migrations = enabled_plugin_migrations(&config);
    let migration_plan = MigrationPlan::new(plugin_migrations.as_slice());
    let data_store = DataStore::open(paths.db_path(), migration_plan)?;
    compiler::registry::seed_registry_snapshot(&data_store)?;
    environment_runtime::record_runtime_environment(&data_store, &paths)?;

    Ok(StartupState {
        paths,
        config,
        config_store,
        data_store,
    })
}

pub fn resolve_runtime_paths() -> Result<AppPaths> {
    let root = resolve_app_data_dir()?;
    std::fs::create_dir_all(&root)?;
    let config_store = ConfigStore::load_or_create(root.join("entrance.toml"))?;
    let paths = AppPaths::from_config(root, config_store.config())?;
    paths.ensure_layout()?;
    Ok(paths)
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
