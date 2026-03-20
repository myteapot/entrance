pub mod core;
mod plugins;

use std::sync::Arc;

use anyhow::{bail, Result};
use serde::Serialize;
use tauri::{Emitter, Manager};

use core::{
    bootstrap_for_paths,
    event_bus::EventBus,
    hotkey,
    logging::LoggingSystem,
    mcp_server::{McpPluginSet, McpServer, McpTransport},
    plugin_manager::PluginManager,
    resolve_app_data_dir,
    theme::ThemeSystem,
    AppPaths,
};
use plugins::{
    forge::commands::{
        forge_cancel_task, forge_create_task, forge_get_task, forge_get_task_details,
        forge_list_tasks,
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
    app.manage(event_bus.clone());

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
    let mut launcher_plugin_state = None;
    if startup.launcher_enabled() {
        let launcher_plugin = LauncherPlugin::new(data_store.clone());
        plugin_manager.register(Arc::new(launcher_plugin.clone()));
        app.manage(launcher_plugin.clone());
        launcher_plugin_state = Some(launcher_plugin);
    }

    let mut forge_plugin_state = None;
    if startup.forge_enabled() {
        let forge_plugin = plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone());
        forge_plugin.start_http_server(startup.forge_http_port())?;
        plugin_manager.register(Arc::new(forge_plugin.clone()));
        app.manage(forge_plugin.clone());
        forge_plugin_state = Some(forge_plugin);
    }

    let mut vault_plugin_state = None;
    if startup.vault_enabled() {
        let vault_plugin = VaultPlugin::new(data_store.clone())?;
        plugin_manager.register(Arc::new(vault_plugin.clone()));
        app.manage(vault_plugin.clone());
        vault_plugin_state = Some(vault_plugin);
    }

    plugin_manager.init_all(&app_context)?;
    app.manage(plugin_manager);

    if startup.mcp_enabled() {
        app.manage(McpServer::new(
            McpTransport::InProcess,
            McpPluginSet {
                forge: forge_plugin_state,
                launcher: launcher_plugin_state,
                vault: vault_plugin_state,
            },
        ));
    }

    if let Some(shortcut) = launcher_hotkey.as_deref() {
        hotkey::register_launcher_shortcut(app, shortcut)?;
    }

    Ok(())
}

#[tauri::command]
fn launcher_hotkey(state: tauri::State<'_, LauncherUiState>) -> Option<String> {
    state.hotkey.clone()
}

pub fn dispatch_cli_or_run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if matches!(args.as_slice(), [command, transport] if command == "mcp" && transport == "stdio") {
        return run_mcp_stdio();
    }
    if matches!(args.first().map(String::as_str), Some("mcp")) {
        bail!("unsupported MCP transport, expected `entrance mcp stdio`");
    }

    run();
    Ok(())
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
            core::theme::get_theme,
            core::theme::set_theme,
            launcher_search,
            launcher_launch,
            launcher_pin,
            forge_create_task,
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

fn run_mcp_stdio() -> Result<()> {
    let startup = bootstrap_for_paths(AppPaths::new(resolve_app_data_dir()?))?;
    if !startup.mcp_enabled() {
        bail!("MCP server is disabled in entrance.toml");
    }

    let _logging_system = LoggingSystem::init(
        startup.paths().log_dir(),
        startup.log_level(),
        Some(startup.data_store()),
    )?;
    let data_store = startup.data_store();
    let event_bus = EventBus::new();

    let server = McpServer::new(
        McpTransport::Stdio,
        McpPluginSet {
            forge: startup
                .forge_enabled()
                .then(|| plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone())),
            launcher: startup
                .launcher_enabled()
                .then(|| LauncherPlugin::new(data_store.clone())),
            vault: if startup.vault_enabled() {
                Some(VaultPlugin::new(data_store)?)
            } else {
                None
            },
        },
    );

    server.serve_stdio()
}
