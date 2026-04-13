use anyhow::Result;
use serde::Serialize;

pub mod issues;
pub mod nota_prayer;

use crate::core::{
    chat_archive::{
        capture_chat_message, get_chat_archive_policy, set_chat_archive_policy,
        ChatArchivePolicyReport, ChatArchivePolicyRequest, ChatCaptureReport, ChatCaptureRequest,
    },
    data_store::{DataStore, StoredAgentInstance, StoredSourceIngestRun},
    hygiene::{list_spec_hygiene_v0, SpecHygieneReport},
    landing::{
        import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
        list_landing_planning_items, list_landing_unreconciled_items, LandingImportReport,
        LandingMirrorSummary, LandingPlanningItemSummary,
    },
    overview::{
        build_nota_runtime_overview, build_nota_runtime_status, NotaRuntimeOverview,
        NotaRuntimeStatus,
    },
    parallel_budget::ParallelBudgetConfig,
    system_heartbeat::{compute_pulse, AgentTier, HeartbeatConfig, SystemPulse},
};
use crate::hosts::desktop::instance_manager::{InstanceManager, InstanceRole};

#[derive(Clone)]
pub struct LauncherUiState {
    pub hotkey: Option<String>,
}

#[derive(Clone)]
pub struct DashboardUiState {
    pub app_version: String,
    pub launcher_hotkey: Option<String>,
    pub enabled_plugin_count: usize,
    pub launcher_enabled: bool,
    pub forge_enabled: bool,
    pub vault_enabled: bool,
}

#[derive(Clone, Serialize)]
pub(crate) struct DashboardSummary {
    app_version: String,
    launcher_hotkey: Option<String>,
    enabled_plugin_count: usize,
    running_task_count: usize,
    last_activity_at: Option<String>,
    token_count: usize,
    mcp_config_count: usize,
    enabled_mcp_count: usize,
}

#[tauri::command]
pub(crate) fn launcher_hotkey(state: tauri::State<'_, LauncherUiState>) -> Option<String> {
    state.hotkey.clone()
}

#[tauri::command]
pub(crate) fn dashboard_summary(
    dashboard: tauri::State<'_, DashboardUiState>,
    data_store: tauri::State<'_, DataStore>,
) -> Result<DashboardSummary, String> {
    build_dashboard_summary(&dashboard, &data_store)
}

pub(crate) fn build_dashboard_summary(
    dashboard: &DashboardUiState,
    data_store: &DataStore,
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

#[tauri::command]
pub(crate) fn list_agent_instances(
    data_store: tauri::State<'_, DataStore>,
) -> Result<Vec<StoredAgentInstance>, String> {
    data_store
        .list_agent_instances()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_system_pulse(
    data_store: tauri::State<'_, DataStore>,
) -> Result<SystemPulse, String> {
    compute_pulse(&data_store, &HeartbeatConfig::default()).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_parallel_budget_config() -> ParallelBudgetConfig {
    ParallelBudgetConfig::default()
}

#[tauri::command]
pub(crate) fn create_agent_instance(
    instance_manager: tauri::State<'_, InstanceManager>,
    role: String,
    parent_instance_id: Option<i64>,
    display_name: String,
    config_json: String,
) -> Result<StoredAgentInstance, String> {
    let role: InstanceRole = role
        .parse()
        .map_err(|error: anyhow::Error| error.to_string())?;
    let tier = AgentTier::ArchNota;
    instance_manager
        .create_instance(
            role,
            parent_instance_id,
            &display_name,
            &config_json,
            None,
            tier,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn stop_agent_instance(
    instance_manager: tauri::State<'_, InstanceManager>,
    id: i64,
) -> Result<(), String> {
    instance_manager
        .stop_instance(id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn spawn_child_instances(
    instance_manager: tauri::State<'_, InstanceManager>,
    parent_id: i64,
    count: usize,
) -> Result<Vec<StoredAgentInstance>, String> {
    instance_manager
        .spawn_children(parent_id, count)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn nota_runtime_overview(
    data_store: tauri::State<'_, DataStore>,
) -> Result<NotaRuntimeOverview, String> {
    build_nota_runtime_overview(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn nota_runtime_status(
    data_store: tauri::State<'_, DataStore>,
) -> Result<NotaRuntimeStatus, String> {
    build_nota_runtime_status(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn nota_get_chat_archive_policy(
    data_store: tauri::State<'_, DataStore>,
) -> Result<ChatArchivePolicyReport, String> {
    get_chat_archive_policy(&data_store, None, None).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn nota_set_chat_archive_policy(
    archive_policy: String,
    data_store: tauri::State<'_, DataStore>,
) -> Result<ChatArchivePolicyReport, String> {
    set_chat_archive_policy(
        &data_store,
        ChatArchivePolicyRequest {
            scope_type: None,
            scope_ref: None,
            archive_policy,
        },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn nota_capture_chat_message(
    role: String,
    content: String,
    summary: Option<String>,
    session_ref: Option<String>,
    data_store: tauri::State<'_, DataStore>,
) -> Result<ChatCaptureReport, String> {
    capture_chat_message(
        &data_store,
        ChatCaptureRequest {
            session_ref,
            role,
            content,
            summary,
            scope_type: None,
            scope_ref: None,
            linked_decision_id: None,
        },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn landing_import_snapshot(
    path: String,
    data_store: tauri::State<'_, DataStore>,
) -> Result<LandingImportReport, String> {
    import_linear_entrance_snapshot(&data_store, path).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn landing_list_ingest_runs(
    data_store: tauri::State<'_, DataStore>,
) -> Result<Vec<StoredSourceIngestRun>, String> {
    list_landing_ingest_runs(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn landing_list_mirror_items(
    data_store: tauri::State<'_, DataStore>,
) -> Result<Vec<LandingMirrorSummary>, String> {
    list_landing_mirror_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn landing_list_planning_items(
    data_store: tauri::State<'_, DataStore>,
) -> Result<Vec<LandingPlanningItemSummary>, String> {
    list_landing_planning_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn landing_list_unreconciled_items(
    data_store: tauri::State<'_, DataStore>,
) -> Result<Vec<LandingPlanningItemSummary>, String> {
    list_landing_unreconciled_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn hygiene_list_spec_v0(
    data_store: tauri::State<'_, DataStore>,
) -> Result<SpecHygieneReport, String> {
    list_spec_hygiene_v0(&data_store).map_err(|error| error.to_string())
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
