use std::path::PathBuf;

use anyhow::Result;
use entrance_core::{
    DrawerEntry, DrawerEntryCreate, DrawerFilter, DrawerMode, Plugin, PluginContext, Store,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerSummary {
    pub mode: String,
    pub root: String,
    pub items: usize,
}

#[derive(Debug, Clone)]
pub struct DrawerPlugin {
    store: Store,
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
            store: ctx.store(),
            root,
            mode,
        }
    }

    pub fn summary(&self) -> Result<DrawerSummary> {
        let items = self.list(DrawerFilter::default())?.len();
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
        self.store.list_drawer_entries(&filter)
    }

    pub fn import_path(&self, source: PathBuf, tags: Vec<String>) -> Result<i64> {
        let file_name = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("entry")
            .to_string();

        let destination = self.root.join(&file_name);
        std::fs::create_dir_all(&self.root)?;
        if source.is_file() {
            std::fs::copy(&source, &destination)?;
        }

        self.store.insert_drawer_entry(DrawerEntryCreate {
            title: file_name,
            kind: "import".to_string(),
            source_path: Some(source.display().to_string()),
            storage_path: Some(destination.display().to_string()),
            tags,
            encrypted: false,
        })
    }

    pub fn add_note(&self, title: String, body: String, tags: Vec<String>) -> Result<i64> {
        std::fs::create_dir_all(&self.root)?;
        let file_name = slugify(&title);
        let destination = self.root.join(format!("{file_name}.md"));
        std::fs::write(&destination, body)?;

        self.store.insert_drawer_entry(DrawerEntryCreate {
            title,
            kind: "note".to_string(),
            source_path: None,
            storage_path: Some(destination.display().to_string()),
            tags,
            encrypted: false,
        })
    }
}

impl Plugin for DrawerPlugin {
    fn name(&self) -> &'static str {
        "drawer"
    }
}

fn slugify(value: &str) -> String {
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
