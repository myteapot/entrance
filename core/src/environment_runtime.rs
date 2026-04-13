use anyhow::Result;
use serde::Serialize;

use crate::core::data_store::{DataStore, StoredOwnedWorktree, StoredRuntimeHost};

#[derive(Debug, Clone, Serialize)]
pub struct OwnedWorktreeRegistryReport {
    pub worktree_count: usize,
    pub observed_count: usize,
    pub missing_count: usize,
    pub worktrees: Vec<StoredOwnedWorktree>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeEnvironmentReport {
    pub host: StoredRuntimeHost,
    pub worktrees: OwnedWorktreeRegistryReport,
}

pub fn current_runtime_host(data_store: &DataStore) -> Result<Option<StoredRuntimeHost>> {
    Ok(data_store.list_runtime_hosts()?.into_iter().next())
}

pub fn list_owned_worktrees(
    data_store: &DataStore,
    host_key: Option<&str>,
) -> Result<OwnedWorktreeRegistryReport> {
    let mut worktrees = data_store.list_owned_worktrees()?;
    if let Some(host_key) = host_key {
        worktrees.retain(|worktree| worktree.host_key == host_key);
    }
    let observed_count = worktrees
        .iter()
        .filter(|worktree| worktree.status == "observed")
        .count();
    let missing_count = worktrees
        .iter()
        .filter(|worktree| worktree.status == "missing")
        .count();

    Ok(OwnedWorktreeRegistryReport {
        worktree_count: worktrees.len(),
        observed_count,
        missing_count,
        worktrees,
    })
}
