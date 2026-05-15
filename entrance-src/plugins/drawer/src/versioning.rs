use anyhow::Result;
use entrance_core::{CommitSummary, Versioning};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerHistory {
    pub commits: Vec<CommitSummary>,
}

pub fn history(versioning: &Versioning, limit: usize) -> Result<DrawerHistory> {
    Ok(DrawerHistory {
        commits: versioning.history(limit)?,
    })
}

pub fn snapshot(versioning: &Versioning, summary: &str) -> Result<CommitSummary> {
    versioning.commit(summary)
}

pub fn rollback(versioning: &Versioning, target: &str) -> Result<()> {
    versioning.rollback(target)
}
