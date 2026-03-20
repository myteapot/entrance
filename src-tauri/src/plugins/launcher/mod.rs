pub mod scanner;
pub mod search;

use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;
use tauri::State;

use crate::{
    core::data_store::{DataStore, MigrationStep, StoredLauncherApp},
    plugins::{AppContext, Event, Manifest, McpToolDefinition, Plugin, TauriCommandDefinition},
};

use self::{
    scanner::scan_installed_apps,
    search::{normalize_text, score_launcher_app},
};

#[cfg(not(target_os = "windows"))]
use self::scanner::split_command_line_words;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const MANIFEST: Manifest = Manifest {
    name: "launcher",
    version: env!("CARGO_PKG_VERSION"),
    description: "Indexes local applications, performs fuzzy search, and launches desktop apps.",
};

const MIGRATIONS: [MigrationStep; 1] = [MigrationStep {
    name: "0001_create_plugin_launcher_apps",
    sql: include_str!("../../../migrations/0001_create_plugin_launcher_apps.sql"),
}];

#[derive(Debug, Clone, Serialize)]
pub struct LauncherSearchResult {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub icon_path: Option<String>,
    pub source: String,
    pub launch_count: i64,
    pub last_used: Option<String>,
    pub pinned: bool,
    pub score: f64,
}

#[derive(Clone)]
pub struct LauncherPlugin {
    manifest: Manifest,
    data_store: DataStore,
}

impl LauncherPlugin {
    pub fn new(data_store: DataStore) -> Self {
        Self {
            manifest: MANIFEST,
            data_store,
        }
    }

    pub fn refresh_index(&self) -> Result<usize> {
        let apps = scan_installed_apps()?;
        self.data_store
            .upsert_launcher_apps(&apps)
            .context("failed to persist scanned launcher apps")?;
        Ok(apps.len())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<LauncherSearchResult>> {
        if self.data_store.launcher_app_count()? == 0 {
            let _ = self.refresh_index();
        }

        let normalized_query = normalize_text(query);
        let mut results = self
            .data_store
            .list_launcher_apps()?
            .into_iter()
            .filter_map(|app| build_search_result(&normalized_query, app))
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.score.total_cmp(&left.score))
                .then_with(|| right.launch_count.cmp(&left.launch_count))
                .then_with(|| left.name.cmp(&right.name))
        });
        results.truncate(limit);

        Ok(results)
    }

    pub fn launch(
        &self,
        path: &str,
        arguments: Option<&str>,
        working_dir: Option<&str>,
    ) -> Result<()> {
        let record = self.data_store.get_launcher_app_by_path(path)?;
        let arguments = arguments
            .map(str::to_string)
            .or_else(|| record.as_ref().and_then(|app| app.arguments.clone()));
        let working_dir = working_dir
            .map(str::to_string)
            .or_else(|| record.as_ref().and_then(|app| app.working_dir.clone()));

        let mut command = Command::new(path);
        apply_command_arguments(&mut command, arguments.as_deref());

        if let Some(working_dir) = working_dir.as_deref() {
            command.current_dir(working_dir);
        }

        command
            .spawn()
            .with_context(|| format!("failed to launch application `{path}`"))?;
        self.data_store.record_launcher_launch(path)?;

        Ok(())
    }

    pub fn pin(&self, path: &str, pinned: bool) -> Result<()> {
        self.data_store.set_launcher_pinned(path, pinned)
    }
}

pub fn migrations() -> &'static [MigrationStep] {
    &MIGRATIONS
}

impl Plugin for LauncherPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn init(&self, ctx: &AppContext) -> Result<()> {
        let _ = ctx.data_store.launcher_app_count()?;
        self.refresh_index()?;
        Ok(())
    }

    fn on_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }

    fn register_commands(&self) -> Vec<TauriCommandDefinition> {
        vec![
            TauriCommandDefinition {
                name: "launcher_search",
                description: "Search indexed local applications with fuzzy matching.",
            },
            TauriCommandDefinition {
                name: "launcher_launch",
                description: "Launch a local application by executable path.",
            },
            TauriCommandDefinition {
                name: "launcher_pin",
                description: "Pin or unpin a launcher entry for ranking.",
            },
        ]
    }

    fn mcp_tools(&self) -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "launcher_search",
                description: "Search indexed local applications.",
            },
            McpToolDefinition {
                name: "launcher_launch",
                description: "Launch an indexed local application.",
            },
        ]
    }

    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tauri::command]
pub fn launcher_search(
    query: String,
    limit: Option<usize>,
    launcher: State<'_, LauncherPlugin>,
) -> Result<Vec<LauncherSearchResult>, String> {
    launcher
        .search(&query, limit.unwrap_or(20))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn launcher_launch(
    path: String,
    arguments: Option<String>,
    working_dir: Option<String>,
    launcher: State<'_, LauncherPlugin>,
) -> Result<(), String> {
    launcher
        .launch(&path, arguments.as_deref(), working_dir.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn launcher_pin(
    path: String,
    pinned: bool,
    launcher: State<'_, LauncherPlugin>,
) -> Result<(), String> {
    launcher
        .pin(&path, pinned)
        .map_err(|error| error.to_string())
}

fn build_search_result(query: &str, app: StoredLauncherApp) -> Option<LauncherSearchResult> {
    let score = if query.is_empty() {
        0.0
    } else {
        score_launcher_app(query, &app)
    };

    if !query.is_empty() && score <= 0.0 {
        return None;
    }

    Some(LauncherSearchResult {
        id: app.id,
        name: app.name,
        path: app.path,
        arguments: app.arguments,
        working_dir: app.working_dir,
        icon_path: app.icon_path,
        source: app.source,
        launch_count: app.launch_count,
        last_used: app.last_used,
        pinned: app.pinned,
        score,
    })
}

#[cfg(target_os = "windows")]
fn apply_command_arguments(command: &mut Command, arguments: Option<&str>) {
    if let Some(arguments) = arguments.filter(|arguments| !arguments.trim().is_empty()) {
        command.raw_arg(arguments);
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_command_arguments(command: &mut Command, arguments: Option<&str>) {
    if let Some(arguments) = arguments.filter(|arguments| !arguments.trim().is_empty()) {
        command.args(split_command_line_words(arguments));
    }
}
