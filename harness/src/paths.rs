use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessPaths {
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

impl HarnessPaths {
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

    #[allow(clippy::too_many_arguments)]
    pub fn resolved(
        app_data_dir: PathBuf,
        config_path: PathBuf,
        data_dir: PathBuf,
        db_path: PathBuf,
        log_dir: PathBuf,
        cache_dir: PathBuf,
        exports_dir: PathBuf,
        snapshots_dir: PathBuf,
        worktrees_dir: PathBuf,
    ) -> Self {
        Self {
            app_data_dir,
            config_path,
            data_dir,
            db_path,
            log_dir,
            cache_dir,
            exports_dir,
            snapshots_dir,
            worktrees_dir,
        }
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

    pub fn ensure_layout(&self) -> anyhow::Result<()> {
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
