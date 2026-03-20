pub mod core;
mod plugins;

use std::sync::Arc;

use serde::Serialize;
use tauri::Manager;

use core::{bootstrap_for_paths, hotkey, event_bus::EventBus, plugin_manager::PluginManager, AppPaths};
use plugins::{
    launcher::{launcher_launch, launcher_pin, launcher_search, LauncherPlugin},
    forge::commands::{forge_create_task, forge_cancel_task, forge_get_task, forge_list_tasks},
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

    let data_store = startup.data_store();
    let event_bus = EventBus::new();
    app.manage(event_bus.clone());

    let app_context = AppContext::new(data_store.clone(), event_bus.clone());

    let mut plugin_manager = PluginManager::default();
    if startup.launcher_enabled() {
        let launcher_plugin = LauncherPlugin::new(data_store.clone());
        plugin_manager.register(Arc::new(launcher_plugin.clone()));
        app.manage(launcher_plugin);
    }
    
    if startup.forge_enabled() {
        let forge_plugin = plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone());
        plugin_manager.register(Arc::new(forge_plugin.clone()));
        app.manage(forge_plugin);
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(hotkey::plugin::<tauri::Wry>().expect("failed to initialize global hotkey plugin"))
        .setup(setup_application)
        .invoke_handler(tauri::generate_handler![
            launcher_hotkey,
            launcher_search,
            launcher_launch,
            launcher_pin,
            forge_create_task,
            forge_list_tasks,
            forge_get_task,
            forge_cancel_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running Entrance application");
}
