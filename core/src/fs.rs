use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct FileSystem;

impl FileSystem {
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> Result<String> {
        fs::read_to_string(path.as_ref())
            .with_context(|| format!("failed to read {}", path.as_ref().display()))
    }

    pub fn write_string(&self, path: impl AsRef<Path>, content: &str) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
    }

    pub fn copy(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<u64> {
        let to = to.as_ref();
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from.as_ref(), to)
            .with_context(|| format!("failed to copy into {}", to.display()))
    }

    pub fn move_path(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        let to = to.as_ref();
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(from.as_ref(), to)
            .with_context(|| format!("failed to move into {}", to.display()))
    }

    pub fn walk_files(&self, root: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for entry in WalkDir::new(root.as_ref()) {
            let entry = match entry {
                Ok(value) => value,
                Err(_) => continue,
            };
            if entry.file_type().is_file() {
                files.push(entry.path().to_path_buf());
            }
        }
        Ok(files)
    }
}
