use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    bus::Bus,
    config::AppConfig,
    crypto::Crypto,
    fs::FileSystem,
    persona::{Persona, PersonaProfile},
    scheduler::Scheduler,
    store::Store,
    supervision::Supervision,
    versioning::Versioning,
};

#[derive(Debug, Clone)]
pub struct AppKernel {
    pub root: PathBuf,
    pub config: AppConfig,
    pub store: Store,
    pub bus: Bus,
    pub versioning: Versioning,
    pub crypto: Crypto,
    pub fs: FileSystem,
    pub scheduler: Scheduler,
    pub supervision: Supervision,
    pub persona: Persona,
}

pub fn resolve_app_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ENTRANCE_APP_ROOT") {
        return Ok(PathBuf::from(path));
    }

    Ok(dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".entrance"))
}

pub fn boot() -> Result<AppKernel> {
    let root = resolve_app_root()?;
    boot_at(root)
}

pub fn boot_at(root: impl AsRef<Path>) -> Result<AppKernel> {
    let root = root.as_ref().to_path_buf();
    std::fs::create_dir_all(&root)?;
    let config = AppConfig::load_or_create(root.join("entrance.toml"))?;
    let store = Store::open(root.join("data/entrance.db"))?;
    let versioning = Versioning::new(config.drawer_root(&root));
    versioning.init()?;
    let bus = Bus::with_store(Some(store.clone()));

    Ok(AppKernel {
        root: root.clone(),
        config,
        store,
        bus,
        versioning,
        crypto: Crypto,
        fs: FileSystem,
        scheduler: Scheduler,
        supervision: Supervision,
        persona: Persona {
            active: PersonaProfile::new("operator", "Entrance Operator"),
        },
    })
}
