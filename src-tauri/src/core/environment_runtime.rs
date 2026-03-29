use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use serde::Serialize;
use walkdir::WalkDir;

use crate::core::data_store::{
    DataStore, StoredOwnedWorktree, StoredRuntimeHost, UpsertOwnedWorktree, UpsertRuntimeHost,
};
use crate::core::AppPaths;

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

pub fn record_runtime_environment(
    data_store: &DataStore,
    paths: &AppPaths,
) -> Result<RuntimeEnvironmentReport> {
    let host = record_runtime_host_snapshot(data_store, paths)?;
    let worktrees = synchronize_owned_worktrees(data_store, &host, paths.worktrees_dir())?;
    Ok(RuntimeEnvironmentReport { host, worktrees })
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

fn record_runtime_host_snapshot(
    data_store: &DataStore,
    paths: &AppPaths,
) -> Result<StoredRuntimeHost> {
    let host_label = env::var("HOSTNAME")
        .ok()
        .or_else(|| env::var("COMPUTERNAME").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "localhost".to_string());
    let os_family = env::consts::OS.to_string();
    let kernel_label = env::consts::FAMILY.to_string();
    let user_home = dirs::home_dir()
        .unwrap_or_else(|| paths.app_data_dir().to_path_buf())
        .to_string_lossy()
        .replace('\\', "/");
    let owner_root = paths.app_data_dir().to_string_lossy().replace('\\', "/");
    let config_path = paths.config_path().to_string_lossy().replace('\\', "/");
    let runtime_db_path = paths.db_path().to_string_lossy().replace('\\', "/");
    let exports_path = paths.exports_dir().to_string_lossy().replace('\\', "/");
    let worktrees_root = paths.worktrees_dir().to_string_lossy().replace('\\', "/");
    let host_key = format!("{os_family}:{host_label}:{owner_root}");
    let path_style = if cfg!(windows) { "windows" } else { "posix" };
    let wsl_distro_name = env::var("WSL_DISTRO_NAME").ok();

    data_store.upsert_runtime_host(UpsertRuntimeHost {
        host_key: &host_key,
        os_family: &os_family,
        host_label: &host_label,
        kernel_label: &kernel_label,
        user_home: &user_home,
        owner_root: &owner_root,
        config_path: &config_path,
        runtime_db_path: &runtime_db_path,
        exports_path: &exports_path,
        worktrees_root: &worktrees_root,
        wsl_distro_name: wsl_distro_name.as_deref(),
        path_style,
        status: "active",
    })
}

fn synchronize_owned_worktrees(
    data_store: &DataStore,
    host: &StoredRuntimeHost,
    worktrees_root: &Path,
) -> Result<OwnedWorktreeRegistryReport> {
    let mut observed_paths = Vec::new();

    if worktrees_root.exists() {
        for entry in WalkDir::new(worktrees_root)
            .follow_links(false)
            .min_depth(2)
            .max_depth(4)
        {
            let entry = entry?;
            if !entry.file_type().is_dir() {
                continue;
            }
            if !entry.path().join(".git").exists() {
                continue;
            }

            let worktree_path = normalize_path(entry.path());
            let relative = entry
                .path()
                .strip_prefix(worktrees_root)
                .unwrap_or(entry.path());
            let metadata = derive_worktree_metadata(relative, entry.path());

            data_store.upsert_owned_worktree(UpsertOwnedWorktree {
                host_key: &host.host_key,
                project_name: &metadata.project_name,
                issue_id: metadata.issue_id.as_deref(),
                branch_name: &metadata.branch_name,
                worktree_kind: &metadata.worktree_kind,
                worktree_path: &worktree_path,
                repo_root: metadata.repo_root.as_deref(),
                slot_name: metadata.slot_name.as_deref(),
                status: "observed",
            })?;
            observed_paths.push(worktree_path);
        }
    }

    data_store.mark_owned_worktrees_missing(&host.host_key, &observed_paths)?;
    list_owned_worktrees(data_store, Some(&host.host_key))
}

#[derive(Debug)]
struct ObservedWorktreeMetadata {
    project_name: String,
    issue_id: Option<String>,
    branch_name: String,
    worktree_kind: String,
    repo_root: Option<String>,
    slot_name: Option<String>,
}

fn derive_worktree_metadata(
    relative_path: &Path,
    worktree_path: &Path,
) -> ObservedWorktreeMetadata {
    let components = relative_path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let project_name = components.first().cloned().unwrap_or_default();
    let is_slot = components.get(1).map(String::as_str) == Some("slots");
    let repo_root = git_output(worktree_path, ["rev-parse", "--show-toplevel"]);
    let branch_name =
        git_output(worktree_path, ["branch", "--show-current"]).unwrap_or_else(|| {
            components
                .last()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string())
        });
    let issue_id = if is_slot {
        components.get(2).cloned()
    } else {
        parse_issue_id(&branch_name)
    };
    let slot_name = if is_slot {
        components.last().cloned()
    } else {
        None
    };

    ObservedWorktreeMetadata {
        project_name,
        issue_id,
        branch_name,
        worktree_kind: if is_slot {
            "agent_slot_worktree".to_string()
        } else {
            "managed_worktree".to_string()
        },
        repo_root,
        slot_name,
    }
}

fn git_output<const N: usize>(path: &Path, args: [&str; N]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
}

fn parse_issue_id(branch_name: &str) -> Option<String> {
    for segment in branch_name.split(&['/', '_', ' '][..]) {
        let parts = segment
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[parts.len() - 2];
        let number = parts[parts.len() - 1];
        if key.chars().all(|value| value.is_ascii_uppercase())
            && number.chars().all(|value| value.is_ascii_digit())
        {
            return Some(format!("{key}-{number}"));
        }
    }
    None
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context, Result};

    use crate::core::{data_store::MigrationPlan, AppPaths};

    use super::{
        current_runtime_host, list_owned_worktrees, normalize_path, record_runtime_environment,
    };

    struct TempRoot {
        root: PathBuf,
        app_data_dir: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-environment-runtime-{label}-{}-{suffix}",
                std::process::id()
            ));
            let app_data_dir = root.join("appdata");
            fs::create_dir_all(&app_data_dir)?;
            Ok(Self { root, app_data_dir })
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn runtime_environment_tracks_host_and_worktree_registry() -> Result<()> {
        let temp = TempRoot::new("host-worktree")?;
        let paths = AppPaths::new(&temp.app_data_dir);
        paths.ensure_layout()?;
        let migration_plan = MigrationPlan::new(crate::plugins::forge::migrations());
        let store = crate::core::data_store::DataStore::open(paths.db_path(), migration_plan)?;

        let managed_worktree = temp
            .app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree)?;

        let slot_worktree = temp
            .app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("slots")
            .join("MYT-48")
            .join("agent-1");
        fs::create_dir_all(&slot_worktree)?;
        init_git_repo(&slot_worktree)?;

        let report = record_runtime_environment(&store, &paths)?;
        assert_eq!(report.host.owner_root, normalize_path(paths.app_data_dir()));
        assert_eq!(report.worktrees.worktree_count, 2);
        assert_eq!(report.worktrees.observed_count, 2);

        let current_host = current_runtime_host(&store)?.context("host should exist")?;
        assert_eq!(current_host.host_key, report.host.host_key);

        let worktrees = list_owned_worktrees(&store, Some(&report.host.host_key))?;
        assert_eq!(worktrees.worktree_count, 2);
        assert!(worktrees
            .worktrees
            .iter()
            .any(|worktree| worktree.worktree_kind == "managed_worktree"));
        assert!(worktrees
            .worktrees
            .iter()
            .any(|worktree| worktree.worktree_kind == "agent_slot_worktree"));

        Ok(())
    }

    fn init_git_repo(path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(path)
            .output()
            .with_context(|| format!("failed to initialize git repo at {}", path.display()))?;
        if !output.status.success() {
            anyhow::bail!(
                "git init failed for {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }
}
