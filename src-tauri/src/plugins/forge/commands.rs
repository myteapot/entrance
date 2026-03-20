use crate::core::data_store::StoredForgeTask;
use crate::plugins::forge::ForgePlugin;
use tauri::State;

#[tauri::command]
pub fn forge_create_task(
    name: String,
    command: String,
    args: String, // Expected JSON array string
    required_tokens: Option<Vec<String>>,
    forge: State<'_, ForgePlugin>,
) -> Result<i64, String> {
    let required_tokens =
        serde_json::to_string(&required_tokens.unwrap_or_default()).map_err(|e| e.to_string())?;
    let id = forge
        .create_task(&name, &command, &args, &required_tokens)
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
pub fn forge_cancel_task(id: i64, forge: State<'_, ForgePlugin>) -> Result<(), String> {
    forge.cancel_task(id).map_err(|e| e.to_string())
}
