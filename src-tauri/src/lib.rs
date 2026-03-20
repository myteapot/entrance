pub mod core;
mod plugins;

use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager};

use core::{
    bootstrap_for_paths, event_bus::EventBus, hotkey, logging::LoggingSystem,
    plugin_manager::PluginManager, theme::ThemeSystem, AppPaths,
};
use plugins::{
    forge::commands::{
        forge_cancel_task, forge_create_task, forge_dispatch_agent, forge_get_task,
        forge_get_task_details, forge_list_tasks,
    },
    launcher::{launcher_launch, launcher_pin, launcher_search, LauncherPlugin},
    vault::{
        commands::{
            vault_add_token, vault_delete_token, vault_get_token, vault_list_mcp,
            vault_list_tokens, vault_update_mcp,
        },
        VaultPlugin,
    },
    AppContext,
};

#[derive(Clone, Serialize)]
struct LauncherUiState {
    hotkey: Option<String>,
}

#[derive(Clone)]
struct DashboardUiState {
    app_version: String,
    launcher_hotkey: Option<String>,
    enabled_plugin_count: usize,
    launcher_enabled: bool,
    forge_enabled: bool,
    vault_enabled: bool,
}

#[derive(Clone, Serialize)]
struct DashboardSummary {
    app_version: String,
    launcher_hotkey: Option<String>,
    enabled_plugin_count: usize,
    running_task_count: usize,
    last_activity_at: Option<String>,
    token_count: usize,
    mcp_config_count: usize,
    enabled_mcp_count: usize,
}

fn setup_application<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_paths = AppPaths::new(app.path().app_data_dir()?);
    let startup = bootstrap_for_paths(app_paths)?;
    let launcher_hotkey = startup.launcher_hotkey().map(str::to_owned);
    app.manage(LauncherUiState {
        hotkey: launcher_hotkey.clone(),
    });

    let logging_system = LoggingSystem::init(
        startup.paths().log_dir(),
        startup.log_level(),
        Some(startup.data_store()),
    )?;
    app.manage(logging_system);

    let theme_system = ThemeSystem::new(startup.config_store());
    let app_handle = app.handle().clone();
    theme_system.emit_current_theme(&app_handle)?;
    app.manage(theme_system);

    let data_store = startup.data_store();
    let event_bus = EventBus::new();
    let enabled_plugin_count = [
        startup.launcher_enabled(),
        startup.forge_enabled(),
        startup.vault_enabled(),
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count();

    app.manage(event_bus.clone());
    app.manage(data_store.clone());
    app.manage(DashboardUiState {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        launcher_hotkey: launcher_hotkey.clone(),
        enabled_plugin_count,
        launcher_enabled: startup.launcher_enabled(),
        forge_enabled: startup.forge_enabled(),
        vault_enabled: startup.vault_enabled(),
    });

    let app_handle_for_events = app.handle().clone();
    let mut rx = event_bus.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if core::event_bus::match_topic("forge:*", &event.topic) {
                let _ = app_handle_for_events.emit(&event.topic, event.payload);
            }
        }
    });

    let app_context = AppContext::new(data_store.clone(), event_bus.clone());

    let mut plugin_manager = PluginManager::default();
    if startup.launcher_enabled() {
        let launcher_plugin = LauncherPlugin::new(data_store.clone());
        plugin_manager.register(Arc::new(launcher_plugin.clone()));
        app.manage(launcher_plugin);
    }

    if startup.forge_enabled() {
        let forge_plugin = plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone());
        forge_plugin.start_http_server(startup.forge_http_port())?;
        plugin_manager.register(Arc::new(forge_plugin.clone()));
        app.manage(forge_plugin);
    }

    if startup.vault_enabled() {
        let vault_plugin = VaultPlugin::new(data_store.clone())?;
        plugin_manager.register(Arc::new(vault_plugin.clone()));
        app.manage(vault_plugin);
    }

    plugin_manager.init_all(&app_context)?;
    app.manage(plugin_manager);

    if let Some(shortcut) = launcher_hotkey.as_deref() {
        hotkey::register_launcher_shortcut(app, shortcut)?;
    }

    Ok(())
}

#[tauri::command]
fn launcher_hotkey(state: tauri::State<'_, LauncherUiState>) -> Option<String> {
    state.hotkey.clone()
}

#[tauri::command]
fn dashboard_summary(
    dashboard: tauri::State<'_, DashboardUiState>,
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<DashboardSummary, String> {
    let tasks = if dashboard.forge_enabled {
        data_store
            .list_forge_tasks()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let tokens = if dashboard.vault_enabled {
        data_store
            .list_vault_tokens()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let mcp_configs = if dashboard.vault_enabled {
        data_store
            .list_vault_mcp_configs()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let launcher_apps = if dashboard.launcher_enabled {
        data_store
            .list_launcher_apps()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };

    let mut last_activity_at = None;
    for task in &tasks {
        update_latest_timestamp(&mut last_activity_at, Some(task.created_at.as_str()));
        update_latest_timestamp(&mut last_activity_at, task.finished_at.as_deref());
    }
    for token in &tokens {
        update_latest_timestamp(&mut last_activity_at, Some(token.updated_at.as_str()));
    }
    for config in &mcp_configs {
        update_latest_timestamp(&mut last_activity_at, Some(config.updated_at.as_str()));
    }
    for app in &launcher_apps {
        update_latest_timestamp(&mut last_activity_at, app.last_used.as_deref());
        update_latest_timestamp(&mut last_activity_at, Some(app.updated_at.as_str()));
    }

    Ok(DashboardSummary {
        app_version: dashboard.app_version.clone(),
        launcher_hotkey: dashboard.launcher_hotkey.clone(),
        enabled_plugin_count: dashboard.enabled_plugin_count,
        running_task_count: tasks.iter().filter(|task| task.status == "Running").count(),
        last_activity_at,
        token_count: tokens.len(),
        mcp_config_count: mcp_configs.len(),
        enabled_mcp_count: mcp_configs.iter().filter(|config| config.enabled).count(),
    })
}

fn update_latest_timestamp(current: &mut Option<String>, candidate: Option<&str>) {
    let Some(candidate) = candidate.filter(|value| !value.is_empty()) else {
        return;
    };

    let should_replace = current
        .as_deref()
        .map(|value| candidate > value)
        .unwrap_or(true);
    if should_replace {
        *current = Some(candidate.to_string());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(hotkey::plugin::<tauri::Wry>().expect("failed to initialize global hotkey plugin"))
        .setup(setup_application)
        .invoke_handler(tauri::generate_handler![
            launcher_hotkey,
            dashboard_summary,
            core::theme::get_theme,
            core::theme::set_theme,
            launcher_search,
            launcher_launch,
            launcher_pin,
            forge_create_task,
            forge_dispatch_agent,
            forge_list_tasks,
            forge_get_task,
            forge_get_task_details,
            forge_cancel_task,
            vault_list_tokens,
            vault_add_token,
            vault_delete_token,
            vault_get_token,
            vault_list_mcp,
            vault_update_mcp
        ])
        .run(tauri::generate_context!())
        .expect("error while running Entrance application");
}
