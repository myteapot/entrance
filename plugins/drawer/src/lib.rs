mod landing;
mod memory;
mod organizer;
mod storage;
mod vault;
mod versioning;

use std::path::PathBuf;

use anyhow::Result;
use entrance_core::{
    CommitSummary, Crypto, DrawerEntry, DrawerFilter, DrawerMode, FileSystem, Plugin,
    PluginContext, Versioning,
};
use serde::{Deserialize, Serialize};

pub use landing::LandingImportReport;
pub use memory::MemoryImportReport;
pub use organizer::{DrawerAction, DrawerActionKind, ReorganizationPlan};
pub use storage::DrawerStorage;
pub use vault::{VaultSecret, VaultSecretRecord};
pub use versioning::DrawerHistory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerSummary {
    pub mode: String,
    pub root: String,
    pub items: usize,
}

#[derive(Debug, Clone)]
pub struct DrawerPlugin {
    storage: DrawerStorage,
    versioning: Versioning,
    crypto: Crypto,
    fs: FileSystem,
    root: PathBuf,
    mode: DrawerMode,
}

impl DrawerPlugin {
    pub fn new(ctx: &PluginContext) -> Self {
        let root = ctx.kernel.config.drawer_root(&ctx.kernel.root);
        let mode = match ctx.kernel.config.drawer.mode.as_str() {
            "database" => DrawerMode::Database,
            _ => DrawerMode::FileSystem,
        };

        Self {
            storage: DrawerStorage::new(ctx.store(), ctx.fs(), root.clone(), mode),
            versioning: ctx.versioning(),
            crypto: ctx.crypto(),
            fs: ctx.fs(),
            root,
            mode,
        }
    }

    pub fn summary(&self) -> Result<DrawerSummary> {
        let items = self.storage.list(DrawerFilter::default())?.len();
        Ok(DrawerSummary {
            mode: match self.mode {
                DrawerMode::FileSystem => "filesystem".to_string(),
                DrawerMode::Database => "database".to_string(),
            },
            root: self.root.display().to_string(),
            items,
        })
    }

    pub fn list(&self, filter: DrawerFilter) -> Result<Vec<DrawerEntry>> {
        self.storage.list(filter)
    }

    pub fn import_path(&self, source: PathBuf, tags: Vec<String>) -> Result<i64> {
        Ok(landing::import_path(&self.storage, source, tags)?.id)
    }

    pub fn import_path_report(
        &self,
        source: PathBuf,
        tags: Vec<String>,
    ) -> Result<LandingImportReport> {
        landing::import_path(&self.storage, source, tags)
    }

    pub fn add_note(&self, title: String, body: String, tags: Vec<String>) -> Result<i64> {
        self.storage.create_note(title, body, tags)
    }

    pub fn import_memory(
        &self,
        title: String,
        body: String,
        tags: Vec<String>,
    ) -> Result<MemoryImportReport> {
        memory::import_memory(&self.storage, title, body, tags)
    }

    pub fn plan_reorganization(&self) -> Result<ReorganizationPlan> {
        organizer::plan(&self.storage)
    }

    pub fn apply_reorganization(&self, plan: ReorganizationPlan) -> Result<usize> {
        organizer::apply(&self.storage, &self.fs, plan)
    }

    pub fn history(&self, limit: usize) -> Result<DrawerHistory> {
        versioning::history(&self.versioning, limit)
    }

    pub fn snapshot(&self, summary: &str) -> Result<CommitSummary> {
        versioning::snapshot(&self.versioning, summary)
    }

    pub fn rollback(&self, target: &str) -> Result<()> {
        versioning::rollback(&self.versioning, target)
    }

    pub fn store_secret(&self, secret: VaultSecret) -> Result<VaultSecretRecord> {
        vault::store_secret(&self.storage, &self.crypto, secret)
    }

    pub fn list_secrets(&self) -> Result<Vec<VaultSecretRecord>> {
        vault::list_secrets(&self.storage)
    }
}

impl Plugin for DrawerPlugin {
    fn name(&self) -> &'static str {
        "drawer"
    }

    fn init(&self, _ctx: &PluginContext) -> Result<()> {
        self.fs.create_dir_all(&self.root)?;
        self.versioning.init()?;
        Ok(())
    }
}

pub(crate) fn slugify(value: &str) -> String {
    let normalized = value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn secret_tag() -> &'static str {
    "vault-secret"
}

pub(crate) fn memory_tag() -> &'static str {
    "memory"
}
