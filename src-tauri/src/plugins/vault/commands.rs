use tauri::State;

use crate::{
    core::data_store::{StoredVaultMcpConfig, StoredVaultToken, StoredVaultTokenSecret},
    plugins::vault::VaultPlugin,
};

#[tauri::command]
pub fn vault_list_tokens(vault: State<'_, VaultPlugin>) -> Result<Vec<StoredVaultToken>, String> {
    vault.list_tokens().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_add_token(
    name: String,
    provider: String,
    value: String,
    vault: State<'_, VaultPlugin>,
) -> Result<i64, String> {
    vault
        .add_token(&name, &provider, &value)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_upsert_token(
    name: String,
    provider: String,
    value: String,
    vault: State<'_, VaultPlugin>,
) -> Result<i64, String> {
    vault
        .upsert_token(&name, &provider, &value)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_delete_token(id: i64, vault: State<'_, VaultPlugin>) -> Result<(), String> {
    vault.delete_token(id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_get_token(
    id: i64,
    vault: State<'_, VaultPlugin>,
) -> Result<Option<StoredVaultTokenSecret>, String> {
    vault.get_token(id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_get_token_by_provider(
    provider: String,
    vault: State<'_, VaultPlugin>,
) -> Result<Option<StoredVaultTokenSecret>, String> {
    vault
        .get_token_by_provider(&provider)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_list_mcp(vault: State<'_, VaultPlugin>) -> Result<Vec<StoredVaultMcpConfig>, String> {
    vault.list_mcp_configs().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn vault_update_mcp(
    id: Option<i64>,
    name: String,
    transport: String,
    endpoint: String,
    enabled: bool,
    vault: State<'_, VaultPlugin>,
) -> Result<StoredVaultMcpConfig, String> {
    vault
        .update_mcp_config(id, &name, &transport, &endpoint, enabled)
        .map_err(|error| error.to_string())
}
