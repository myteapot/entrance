use std::path::{Path, PathBuf};

use anyhow::Result;
use entrance_core::{
    DrawerEntry, DrawerEntryCreate, DrawerFilter, DrawerMode, FileSystem, Store,
};

use crate::slugify;

#[derive(Debug, Clone)]
pub struct DrawerStorage {
    store: Store,
    fs: FileSystem,
    root: PathBuf,
    mode: DrawerMode,
}

impl DrawerStorage {
    pub fn new(store: Store, fs: FileSystem, root: PathBuf, mode: DrawerMode) -> Self {
        Self {
            store,
            fs,
            root,
            mode,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn mode(&self) -> DrawerMode {
        self.mode
    }

    pub fn list(&self, filter: DrawerFilter) -> Result<Vec<DrawerEntry>> {
        self.store.list_drawer_entries(&filter)
    }

    pub fn entry(&self, id: i64) -> Result<Option<DrawerEntry>> {
        self.store.get_drawer_entry(id)
    }

    pub fn import_path(
        &self,
        source: PathBuf,
        kind: &str,
        tags: Vec<String>,
        encrypted: bool,
    ) -> Result<i64> {
        let file_name = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("entry")
            .to_string();

        self.fs.create_dir_all(&self.root)?;
        let storage_path = match self.mode {
            DrawerMode::FileSystem if source.is_file() => {
                let destination = self.root.join(&file_name);
                self.fs.copy(&source, &destination)?;
                Some(destination.display().to_string())
            }
            DrawerMode::FileSystem if source.is_dir() => Some(source.display().to_string()),
            DrawerMode::Database => None,
            _ => None,
        };

        self.store.insert_drawer_entry(DrawerEntryCreate {
            title: file_name,
            kind: kind.to_string(),
            source_path: Some(source.display().to_string()),
            storage_path,
            tags,
            encrypted,
        })
    }

    pub fn create_note(&self, title: String, body: String, tags: Vec<String>) -> Result<i64> {
        self.fs.create_dir_all(&self.root)?;
        let storage_path = match self.mode {
            DrawerMode::FileSystem => {
                let file_name = slugify(&title);
                let destination = self.root.join(format!("{file_name}.md"));
                self.fs.write_string(&destination, &body)?;
                Some(destination.display().to_string())
            }
            DrawerMode::Database => None,
        };

        self.store.insert_drawer_entry(DrawerEntryCreate {
            title,
            kind: "note".to_string(),
            source_path: None,
            storage_path,
            tags,
            encrypted: false,
        })
    }

    pub fn create_record(
        &self,
        title: String,
        body: String,
        kind: String,
        tags: Vec<String>,
        encrypted: bool,
    ) -> Result<i64> {
        self.fs.create_dir_all(&self.root)?;
        let storage_path = match self.mode {
            DrawerMode::FileSystem => {
                let file_name = slugify(&title);
                let extension = if encrypted { "vault" } else { "md" };
                let destination = self.root.join(format!("{file_name}.{extension}"));
                self.fs.write_string(&destination, &body)?;
                Some(destination.display().to_string())
            }
            DrawerMode::Database => None,
        };

        self.store.insert_drawer_entry(DrawerEntryCreate {
            title,
            kind,
            source_path: None,
            storage_path,
            tags,
            encrypted,
        })
    }

    pub fn relocate(&self, id: i64, destination: PathBuf) -> Result<()> {
        if let Some(entry) = self.entry(id)? {
            if let Some(current) = entry.storage_path {
                self.fs.move_path(current, &destination)?;
                let destination_path = destination.display().to_string();
                self.store.update_drawer_entry_paths(
                    id,
                    entry.source_path.as_deref(),
                    Some(destination_path.as_str()),
                )?;
            }
        }
        Ok(())
    }
}
