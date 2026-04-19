mod scanner;
mod search;

use std::process::Command;

use anyhow::{Context, Result};
use entrance_core::{LauncherEntry, LauncherEntryCreate, LauncherQuery, Plugin, PluginContext, Store};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSearchResult {
    pub id: i64,
    pub name: String,
    pub command: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub source: String,
    pub launch_count: i64,
    pub pinned: bool,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct LauncherPlugin {
    store: Store,
    hotkey: String,
}

impl LauncherPlugin {
    pub fn new(ctx: &PluginContext) -> Self {
        Self {
            store: ctx.store(),
            hotkey: ctx.kernel.config.launcher.hotkey.clone(),
        }
    }

    pub fn hotkey(&self) -> &str {
        &self.hotkey
    }

    pub fn refresh(&self, extra_scan_paths: &[String]) -> Result<usize> {
        let entries = scanner::scan(extra_scan_paths)?;
        self.store.upsert_launcher_entries(&entries)?;
        Ok(entries.len())
    }

    pub fn search(&self, query: LauncherQuery) -> Result<Vec<LauncherSearchResult>> {
        let mut results = self
            .store
            .search_launcher_entries(&query)?
            .into_iter()
            .filter_map(|entry| build_result(&query.query, entry))
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.score.total_cmp(&left.score))
                .then_with(|| right.launch_count.cmp(&left.launch_count))
                .then_with(|| left.name.cmp(&right.name))
        });

        let limit = if query.limit == 0 { 20 } else { query.limit };
        results.truncate(limit);
        Ok(results)
    }

    pub fn launch(
        &self,
        command: &str,
        arguments: Option<&str>,
        working_dir: Option<&str>,
    ) -> Result<()> {
        let mut child = Command::new(command);
        if let Some(arguments) = arguments.filter(|value| !value.trim().is_empty()) {
            child.args(arguments.split_whitespace());
        }
        if let Some(working_dir) = working_dir {
            child.current_dir(working_dir);
        }

        child
            .spawn()
            .with_context(|| format!("failed to launch `{command}`"))?;
        self.store.record_launcher_launch(command)?;
        Ok(())
    }

    pub fn pin(&self, command: &str, pinned: bool) -> Result<()> {
        self.store.set_launcher_pinned(command, pinned)
    }
}

impl Plugin for LauncherPlugin {
    fn name(&self) -> &'static str {
        "launcher"
    }
}

fn build_result(query: &str, entry: LauncherEntry) -> Option<LauncherSearchResult> {
    let score = search::score(query, &entry);
    if !query.trim().is_empty() && score <= 0.0 {
        return None;
    }

    Some(LauncherSearchResult {
        id: entry.id,
        name: entry.name,
        command: entry.command,
        arguments: entry.arguments,
        working_dir: entry.working_dir,
        source: entry.source,
        launch_count: entry.launch_count,
        pinned: entry.pinned,
        score,
    })
}
