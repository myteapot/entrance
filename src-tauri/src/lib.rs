mod core;
mod plugins;

use std::sync::Arc;

use tauri::Manager;

use core::{data_store::DataStore, hotkey, plugin_manager::PluginManager};
use plugins::{
    launcher::{launcher_launch, launcher_pin, launcher_search, LauncherPlugin},
    AppContext,
};

fn setup_application<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let data_store = DataStore::open_default()?;
    let app_context = AppContext::new(data_store.clone());

    let launcher_plugin = LauncherPlugin::new(data_store);
    let mut plugin_manager = PluginManager::default();
    plugin_manager.register(Arc::new(launcher_plugin.clone()));
    plugin_manager.init_all(&app_context)?;

    app.manage(plugin_manager);
    app.manage(launcher_plugin);

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(hotkey::plugin::<tauri::Wry>().expect("failed to initialize global hotkey plugin"))
        .setup(setup_application)
        .invoke_handler(tauri::generate_handler![
            launcher_search,
            launcher_launch,
            launcher_pin
        ])
        .run(tauri::generate_context!())
        .expect("error while running Entrance application");
}
