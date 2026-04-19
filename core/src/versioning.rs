use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct Versioning {
    root: PathBuf,
}

impl Versioning {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn init(&self) -> Result<()> {
        let _ = &self.root;
        Ok(())
    }

    pub fn history(&self, _limit: usize) -> Result<Vec<CommitSummary>> {
        Ok(Vec::new())
    }

    pub fn commit(&self, summary: &str) -> Result<CommitSummary> {
        Ok(CommitSummary {
            id: "pending".to_string(),
            summary: summary.to_string(),
        })
    }

    pub fn rollback(&self, _target: &str) -> Result<()> {
        Ok(())
    }
}
