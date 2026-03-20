use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::plugins::{forge, launcher};

use self::{
    config_store::{ConfigStore, EntranceConfig},
    data_store::{DataStore, MigrationPlan, MigrationStep},
};

pub mod config_store;
pub mod data_store;
pub mod event_bus;
pub mod hotkey;
pub mod logging;
pub mod mcp_server;
pub mod permission;
pub mod plugin_manager;
pub mod theme;
pub mod updater;
pub mod window;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    app_data_dir: PathBuf,
    config_path: PathBuf,
    db_path: PathBuf,
    log_dir: PathBuf,
}

impl AppPaths {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        let app_data_dir = app_data_dir.into();
        Self {
            config_path: app_data_dir.join("entrance.toml"),
            db_path: app_data_dir.join("entrance.db"),
            log_dir: app_data_dir.join("logs"),
            app_data_dir,
        }
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }
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

    pub fn launcher_enabled(&self) -> bool {
        self.config.plugins.launcher.enabled
    }

    pub fn forge_enabled(&self) -> bool {
        self.config.plugins.forge.enabled
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

    let plugin_migrations = enabled_plugin_migrations(&config);
    let migration_plan = MigrationPlan::new(plugin_migrations.as_slice());
    let data_store = DataStore::open(paths.db_path(), migration_plan)?;

    Ok(StartupState {
        paths,
        config,
        config_store,
        data_store,
    })
}

fn enabled_plugin_migrations(config: &EntranceConfig) -> Vec<MigrationStep> {
    let mut migrations = Vec::new();

    if config.plugins.launcher.enabled {
        migrations.extend_from_slice(launcher::migrations());
    }

    if config.plugins.forge.enabled {
        migrations.extend_from_slice(forge::migrations());
    }

    migrations
}
