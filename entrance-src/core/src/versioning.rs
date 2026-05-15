use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::FileSystem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct Versioning {
    root: PathBuf,
    fs: FileSystem,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct VersionLedger {
    current: Option<String>,
    commits: Vec<CommitSummary>,
}

impl Versioning {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            fs: FileSystem,
        }
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(self.ledger_root())?;
        if !self.ledger_path().exists() {
            self.save_ledger(&VersionLedger::default())?;
        }
        Ok(())
    }

    pub fn history(&self, limit: usize) -> Result<Vec<CommitSummary>> {
        let ledger = self.load_ledger()?;
        let take = if limit == 0 {
            ledger.commits.len()
        } else {
            limit
        };
        Ok(ledger
            .commits
            .into_iter()
            .rev()
            .take(take)
            .collect::<Vec<_>>())
    }

    pub fn commit(&self, summary: &str) -> Result<CommitSummary> {
        self.init()?;
        let mut ledger = self.load_ledger()?;
        let commit = CommitSummary {
            id: Utc::now().format("%Y%m%d%H%M%S").to_string(),
            summary: summary.to_string(),
        };
        self.write_snapshot_manifest(&commit.id)?;
        ledger.current = Some(commit.id.clone());
        ledger.commits.push(commit.clone());
        self.save_ledger(&ledger)?;
        Ok(commit)
    }

    pub fn rollback(&self, target: &str) -> Result<()> {
        self.init()?;
        let mut ledger = self.load_ledger()?;
        if !ledger.commits.iter().any(|commit| commit.id == target) {
            bail!("unknown version target `{target}`");
        }
        let snapshot_path = self.snapshot_root().join(target).join("snapshot.json");
        if !snapshot_path.exists() {
            bail!("missing snapshot manifest for `{target}`");
        }
        ledger.current = Some(target.to_string());
        self.save_ledger(&ledger)?;
        Ok(())
    }

    fn ledger_root(&self) -> PathBuf {
        self.root.join(".versions")
    }

    fn snapshot_root(&self) -> PathBuf {
        self.ledger_root().join("snapshots")
    }

    fn ledger_path(&self) -> PathBuf {
        self.ledger_root().join("history.json")
    }

    fn load_ledger(&self) -> Result<VersionLedger> {
        let path = self.ledger_path();
        if !path.exists() {
            return Ok(VersionLedger::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read version ledger {}", path.display()))?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    fn save_ledger(&self, ledger: &VersionLedger) -> Result<()> {
        fs::create_dir_all(self.ledger_root())?;
        fs::write(self.ledger_path(), serde_json::to_string_pretty(ledger)?).with_context(|| {
            format!(
                "failed to write version ledger under {}",
                self.root.display()
            )
        })
    }

    fn write_snapshot_manifest(&self, commit_id: &str) -> Result<()> {
        let snapshot_root = self.snapshot_root().join(commit_id);
        fs::create_dir_all(&snapshot_root)?;

        let files = self
            .fs
            .watch_snapshot(&self.root)?
            .into_iter()
            .filter(|change| !change.path.starts_with(self.ledger_root()))
            .map(|change| {
                serde_json::json!({
                    "path": change.path.strip_prefix(&self.root).unwrap_or(&change.path).display().to_string(),
                    "kind": change.kind,
                })
            })
            .collect::<Vec<_>>();

        fs::write(
            snapshot_root.join("snapshot.json"),
            serde_json::to_string_pretty(&files)?,
        )
        .with_context(|| format!("failed to write snapshot manifest for {}", commit_id))
    }
}
