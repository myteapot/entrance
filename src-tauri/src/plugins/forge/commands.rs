use crate::core::data_store::DataStore;
use crate::core::data_store::StoredForgeTask;
use crate::plugins::forge::{
    build_agent_task_request, prepare_agent_dispatch, CreateTaskRequest, ForgePlugin,
    ForgeTaskDetails, PreparedAgentDispatch,
};
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
        .create_task(CreateTaskRequest {
            name,
            command,
            args,
            working_dir: None,
            stdin_text: None,
            required_tokens,
            metadata: "{}".to_string(),
            dispatch_receipt: None,
        })
        .map_err(|e| e.to_string())?;
    forge.engine().spawn_task(id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn forge_dispatch_agent(
    issue_id: String,
    worktree_path: String,
    model: String,
    prompt: String,
    required_tokens: Option<Vec<String>>,
    agent_command: Option<String>,
    forge: State<'_, ForgePlugin>,
) -> Result<i64, String> {
    let request = build_agent_task_request(
        issue_id,
        worktree_path,
        model,
        prompt,
        required_tokens.unwrap_or_default(),
        agent_command,
    )?;
    let id = forge.create_task(request).map_err(|e| e.to_string())?;
    forge.engine().spawn_task(id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn forge_prepare_agent_dispatch(
    project_dir: Option<String>,
    data_store: State<'_, DataStore>,
) -> Result<PreparedAgentDispatch, String> {
    prepare_agent_dispatch(data_store.inner().clone(), project_dir).await
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
