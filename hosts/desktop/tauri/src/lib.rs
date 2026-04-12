pub mod core;
pub mod hosts;
pub mod surfaces;

use std::sync::Arc;

use anyhow::Result;
use tauri::{Emitter, Manager};

use crate::{
    hosts::{
        desktop::{
            hotkey, instance_manager::InstanceManager, logging::LoggingSystem,
            plugin_manager::PluginManager, theme::ThemeSystem,
        },
        plugins::{self, launcher::LauncherPlugin, vault::VaultPlugin, AppContext},
    },
    core::{
        bootstrap_for_paths,
        event_bus::{install_runtime_emitters, EventBus},
        resolve_app_data_dir,
        system_heartbeat::{run_system_heartbeat, HeartbeatConfig},
        AppPaths,
    },
    surfaces::tauri::{DashboardUiState, LauncherUiState},
};

pub use surfaces::cli::dispatch_cli_or_run;

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
pub(crate) use surfaces::cli::{
    cli_help_for_args, prepare_forge_dispatch_cli, verify_forge_dispatch_cli, COMPILER_CLI_HELP,
    ELECTRON_BRIDGE_CLI_HELP, FORGE_CLI_HELP, MCP_CLI_HELP, NOTA_CLI_HELP, ROOT_CLI_HELP,
};
pub(crate) use core::overview::{build_nota_runtime_overview, build_nota_runtime_status};

#[cfg(test)]
static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn test_env_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("test environment lock should not be poisoned")
}

fn setup_application<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_paths = AppPaths::new(resolve_app_data_dir()?);
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
    install_runtime_emitters(event_bus.clone(), app.handle().clone());
    app.manage(data_store.clone());
    app.manage(InstanceManager::new(data_store.clone(), event_bus.clone()));
    tauri::async_runtime::spawn({
        let data_store = data_store.clone();
        let event_bus = event_bus.clone();
        async move {
            run_system_heartbeat(data_store, event_bus, HeartbeatConfig::default()).await;
        }
    });
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
            if core::event_bus::match_topic("forge:*", &event.topic)
                || core::event_bus::match_topic("system:*", &event.topic)
            {
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
        if let Err(error) = forge_plugin.start_http_server(startup.forge_http_port()) {
            tracing::warn!(
                ?error,
                "Forge HTTP server failed to start (port may be in use), continuing without it"
            );
        }
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
        if let Err(err) = hotkey::register_launcher_shortcut(app, shortcut) {
            tracing::warn!(
                "Failed to register launcher hotkey '{}': {}. Launcher shortcut disabled.",
                shortcut,
                err
            );
        }
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run_tauri_app() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(hotkey::plugin::<tauri::Wry>().expect("failed to initialize global hotkey plugin"))
        .setup(setup_application)
        .invoke_handler(tauri::generate_handler![
            surfaces::tauri::launcher_hotkey,
            surfaces::tauri::dashboard_summary,
            surfaces::tauri::list_agent_instances,
            surfaces::tauri::get_system_pulse,
            surfaces::tauri::get_parallel_budget_config,
            surfaces::tauri::create_agent_instance,
            surfaces::tauri::stop_agent_instance,
            surfaces::tauri::spawn_child_instances,
            surfaces::tauri::nota_runtime_overview,
            surfaces::tauri::nota_runtime_status,
            surfaces::tauri::landing_import_snapshot,
            surfaces::tauri::landing_list_ingest_runs,
            surfaces::tauri::landing_list_mirror_items,
            surfaces::tauri::landing_list_planning_items,
            surfaces::tauri::landing_list_unreconciled_items,
            surfaces::tauri::hygiene_list_spec_v0,
            surfaces::tauri::nota_prayer::nota_approve_prayer,
            surfaces::tauri::nota_prayer::nota_reject_prayer,
            hosts::desktop::theme::get_theme,
            hosts::desktop::theme::set_theme,
            hosts::plugins::launcher::launcher_search,
            hosts::plugins::launcher::launcher_launch,
            hosts::plugins::launcher::launcher_pin,
            hosts::plugins::forge::commands::forge_create_task,
            hosts::plugins::forge::commands::forge_dispatch_agent,
            hosts::plugins::forge::commands::forge_prepare_agent_dispatch,
            hosts::plugins::forge::commands::forge_list_tasks,
            hosts::plugins::forge::commands::forge_get_task,
            hosts::plugins::forge::commands::forge_get_task_details,
            hosts::plugins::forge::commands::forge_cancel_task,
            hosts::plugins::vault::commands::vault_list_tokens,
            hosts::plugins::vault::commands::vault_add_token,
            hosts::plugins::vault::commands::vault_upsert_token,
            hosts::plugins::vault::commands::vault_delete_token,
            hosts::plugins::vault::commands::vault_get_token,
            hosts::plugins::vault::commands::vault_get_token_by_provider,
            hosts::plugins::vault::commands::vault_list_mcp,
            hosts::plugins::vault::commands::vault_update_mcp,
            surfaces::tauri::issues::issue_list,
            surfaces::tauri::issues::issue_get,
            surfaces::tauri::issues::issue_create,
            surfaces::tauri::issues::issue_update_status,
            surfaces::tauri::issues::issue_update,
            surfaces::tauri::issues::issue_delete,
            surfaces::tauri::issues::issue_add_comment,
            surfaces::tauri::issues::issue_list_comments
        ])
        .run(tauri::generate_context!())
        .expect("error while running Entrance application");
}

#[cfg(test)]
mod tests;
