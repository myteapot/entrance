use crate::core::data_store::StoredForgeTask;
use crate::plugins::forge::{ForgePlugin, ForgeTaskDetails};
use tauri::State;

#[tauri::command]
pub fn forge_create_task(
    name: String,
    command: String,
    args: String, // Expected JSON array string
    forge: State<'_, ForgePlugin>,
) -> Result<i64, String> {
    let id = forge
        .create_task(&name, &command, &args)
        .map_err(|e| e.to_string())?;
    forge.engine().spawn_task(id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn forge_list_tasks(forge: State<'_, ForgePlugin>) -> Result<Vec<StoredForgeTask>, String> {
    forge.list_tasks().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn forge_get_task(
    id: i64,
    forge: State<'_, ForgePlugin>,
) -> Result<Option<StoredForgeTask>, String> {
    forge.get_task(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn forge_get_task_details(
    id: i64,
    forge: State<'_, ForgePlugin>,
) -> Result<Option<ForgeTaskDetails>, String> {
    forge.get_task_details(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn forge_cancel_task(id: i64, forge: State<'_, ForgePlugin>) -> Result<(), String> {
    forge.cancel_task(id).map_err(|e| e.to_string())
}
