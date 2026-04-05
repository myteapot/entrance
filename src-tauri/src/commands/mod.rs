use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Serialize;

pub mod nota_prayer;

use crate::core::{
    cold_docs_runtime::{export_cold_docs_to_repo, NotaColdDocExportReport},
    data_store::{DataStore, StoredAgentInstance, StoredSourceIngestRun},
    hygiene::{list_spec_hygiene_v0, SpecHygieneReport},
    instance_manager::{InstanceManager, InstanceRole},
    invariant_runtime::refresh_runtime_invariants,
    landing::{
        import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
        list_landing_planning_items, list_landing_unreconciled_items, LandingImportReport,
        LandingMirrorSummary, LandingPlanningItemSummary,
    },
    overview::{
        build_nota_runtime_overview, build_nota_runtime_status, build_projection_truth_revision,
        NotaRuntimeOverview, NotaRuntimeStatus,
    },
    parallel_budget::ParallelBudgetConfig,
    projection_runtime::{
        record_projection_failure, record_projection_success, ProjectionTargetSpec,
        ProjectionTruthRevision, HOT_ROOT_PROJECTION_CLASS, OPTIONAL_PROJECTION_POLICY,
        ORACLE_PROJECTION_CLASS, REQUIRED_PROJECTION_POLICY,
    },
    system_heartbeat::{compute_pulse, AgentTier, HeartbeatConfig, SystemPulse},
    StartupState,
};

#[derive(Clone)]
pub(crate) struct LauncherUiState {
    pub(crate) hotkey: Option<String>,
}

#[derive(Clone)]
pub(crate) struct DashboardUiState {
    pub(crate) app_version: String,
    pub(crate) launcher_hotkey: Option<String>,
    pub(crate) enabled_plugin_count: usize,
    pub(crate) launcher_enabled: bool,
    pub(crate) forge_enabled: bool,
    pub(crate) vault_enabled: bool,
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

#[derive(Clone, Serialize)]
pub(crate) struct HotRootProjectionWriteReport {
    export_root: String,
    files_written: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirrored_repo_top_dir: Option<String>,
    truth_revision: ProjectionTruthRevision,
}

#[derive(Clone, Serialize)]
pub(crate) struct ProjectionRebuildReport {
    status: String,
    truth_revision: ProjectionTruthRevision,
    hot_root: HotRootProjectionWriteReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    cold_docs: Option<NotaColdDocExportReport>,
    required_targets_fresh: bool,
    dirty_required_target_count: usize,
    repair_lane_open_count: usize,
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

pub(crate) fn write_hot_root_projection(
    startup: &StartupState,
    mirror_project_dir: Option<&str>,
) -> Result<HotRootProjectionWriteReport> {
    let status = build_nota_runtime_status(&startup.data_store())?;
    let hot_root_dir = startup.paths().exports_dir().join("hot-root");
    let truth_revision = build_projection_truth_revision(
        status.current_checkpoint_id,
        status
            .current_human_round
            .as_ref()
            .map(|round| round.cadence_object.id),
        status
            .current_acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.cadence_object.id),
    );
    let hot_root_dir_display = hot_root_dir.to_string_lossy().replace('\\', "/");
    let oracle_readme_path = hot_root_dir
        .join("README.md")
        .to_string_lossy()
        .replace('\\', "/");
    if let Err(error) = fs::create_dir_all(&hot_root_dir).with_context(|| {
        format!(
            "failed to create hot-root export directory at {}",
            hot_root_dir.display()
        )
    }) {
        record_hot_root_projection_failure(
            startup,
            &truth_revision,
            &hot_root_dir_display,
            &oracle_readme_path,
            &error.to_string(),
        )?;
        refresh_runtime_invariant_truth(&startup.data_store())?;
        return Err(error);
    }

    let files = render_hot_root_files(startup, &status);
    let mut files_written = Vec::new();
    for (name, content) in &files {
        let path = hot_root_dir.join(name);
        if let Err(error) = fs::write(&path, content)
            .with_context(|| format!("failed to write hot-root export at {}", path.display()))
        {
            record_hot_root_projection_failure(
                startup,
                &truth_revision,
                &hot_root_dir_display,
                &oracle_readme_path,
                &error.to_string(),
            )?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            return Err(error);
        }
        files_written.push(path.display().to_string());
    }
    record_hot_root_projection_success(
        startup,
        &truth_revision,
        &hot_root_dir_display,
        &oracle_readme_path,
    )?;

    let mirrored_repo_top_dir = mirror_project_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|project_dir| project_dir.to_string());
    if let Some(project_dir) = mirrored_repo_top_dir.as_deref() {
        let repo_top_dir = Path::new(project_dir).join("specs").join("top");
        let repo_top_dir_display = repo_top_dir.to_string_lossy().replace('\\', "/");
        let mirror_target_key = format!("mirror:{repo_top_dir_display}");
        if let Err(error) = fs::create_dir_all(&repo_top_dir).with_context(|| {
            format!(
                "failed to create mirrored repo top directory at {}",
                repo_top_dir.display()
            )
        }) {
            record_projection_failure(
                &startup.data_store(),
                ProjectionTargetSpec {
                    projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                    target_key: mirror_target_key.as_str().into(),
                    title: "Mirrored repo hot root".into(),
                    target_path: repo_top_dir_display.as_str().into(),
                    source_scope: "runtime:Entrance".into(),
                    repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                    projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                    is_required: false,
                },
                &truth_revision,
                "hot_root_export",
                "failed to create mirrored repo top directory",
                &error.to_string(),
            )?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            return Err(error);
        }
        for (name, content) in &files {
            let path = repo_top_dir.join(name);
            if let Err(error) = fs::write(&path, content).with_context(|| {
                format!(
                    "failed to write mirrored hot-root projection at {}",
                    path.display()
                )
            }) {
                record_projection_failure(
                    &startup.data_store(),
                    ProjectionTargetSpec {
                        projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                        target_key: mirror_target_key.as_str().into(),
                        title: "Mirrored repo hot root".into(),
                        target_path: repo_top_dir_display.as_str().into(),
                        source_scope: "runtime:Entrance".into(),
                        repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                        projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                        is_required: false,
                    },
                    &truth_revision,
                    "hot_root_export",
                    "failed to write mirrored repo hot root",
                    &error.to_string(),
                )?;
                refresh_runtime_invariant_truth(&startup.data_store())?;
                return Err(error);
            }
        }
        record_projection_success(
            &startup.data_store(),
            ProjectionTargetSpec {
                projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                target_key: mirror_target_key.as_str().into(),
                title: "Mirrored repo hot root".into(),
                target_path: repo_top_dir_display.as_str().into(),
                source_scope: "runtime:Entrance".into(),
                repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                is_required: false,
            },
            &truth_revision,
            "hot_root_export",
            "Mirrored repo hot root is current with runtime truth.",
        )?;
    }

    refresh_runtime_invariant_truth(&startup.data_store())?;

    Ok(HotRootProjectionWriteReport {
        export_root: hot_root_dir.display().to_string(),
        files_written,
        mirrored_repo_top_dir: mirrored_repo_top_dir
            .map(|project_dir| Path::new(&project_dir).join("specs").join("top"))
            .map(|path| path.display().to_string()),
        truth_revision,
    })
}

pub(crate) fn rebuild_nota_projections(
    startup: &StartupState,
    project_dir: Option<&str>,
) -> Result<ProjectionRebuildReport> {
    let before = build_nota_runtime_status(&startup.data_store())?;
    let truth_revision = before.projections.current_truth_revision.clone();
    let hot_root = write_hot_root_projection(startup, project_dir)?;
    let cold_docs = if let Some(project_dir) = project_dir {
        Some(export_cold_docs_to_repo(
            &startup.data_store(),
            project_dir,
            &truth_revision,
        )?)
    } else {
        None
    };
    refresh_runtime_invariant_truth(&startup.data_store())?;
    let after = build_nota_runtime_status(&startup.data_store())?;

    Ok(ProjectionRebuildReport {
        status: if after.projections.required_targets_fresh {
            "rebuilt".to_string()
        } else {
            "repair_required".to_string()
        },
        truth_revision,
        hot_root,
        cold_docs,
        required_targets_fresh: after.projections.required_targets_fresh,
        dirty_required_target_count: after.projections.dirty_required_target_count,
        repair_lane_open_count: after.repair_lane.open_count,
    })
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

fn refresh_runtime_invariant_truth(data_store: &DataStore) -> Result<()> {
    refresh_runtime_invariants(data_store).map(|_| ())
}

fn record_hot_root_projection_success(
    startup: &StartupState,
    truth_revision: &ProjectionTruthRevision,
    hot_root_dir: &str,
    oracle_readme_path: &str,
) -> Result<()> {
    record_projection_success(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root".into(),
            title: "Hot-root export".into(),
            target_path: hot_root_dir.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Hot-root export is current with runtime truth.",
    )?;
    record_projection_success(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: ORACLE_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root/README.md".into(),
            title: "Oracle README export".into(),
            target_path: oracle_readme_path.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Oracle README projection is current with runtime truth.",
    )?;
    Ok(())
}

fn record_hot_root_projection_failure(
    startup: &StartupState,
    truth_revision: &ProjectionTruthRevision,
    hot_root_dir: &str,
    oracle_readme_path: &str,
    error_message: &str,
) -> Result<()> {
    record_projection_failure(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root".into(),
            title: "Hot-root export".into(),
            target_path: hot_root_dir.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Hot-root export failed.",
        error_message,
    )?;
    record_projection_failure(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: ORACLE_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root/README.md".into(),
            title: "Oracle README export".into(),
            target_path: oracle_readme_path.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Oracle README export failed.",
        error_message,
    )?;
    Ok(())
}

fn render_hot_root_files(
    startup: &StartupState,
    status: &NotaRuntimeStatus,
) -> Vec<(&'static str, String)> {
    let checkpoint_label = status
        .current_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.cadence_object.title.clone())
        .unwrap_or_else(|| "No active checkpoint".to_string());
    let checkpoint_level = status
        .current_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.payload.stable_level.clone())
        .unwrap_or_else(|| {
            "Checkpoint the current human round before relying on exported views.".to_string()
        });
    let human_round_line = status
        .current_human_round
        .as_ref()
        .map(|round| {
            format!(
                "{} / {} on checkpoint {}",
                round.payload.round_state,
                round
                    .payload
                    .detail_round_state
                    .as_deref()
                    .unwrap_or("unknown"),
                round.payload.checkpoint_id
            )
        })
        .unwrap_or_else(|| "No current human round is materialized yet.".to_string());
    let acceptance_line = status
        .current_acceptance_bundle
        .as_ref()
        .map(|bundle| {
            if bundle.payload.allocation_id == 0 {
                format!(
                    "{} on checkpoint {} ({})",
                    bundle.payload.acceptance_kind,
                    bundle.payload.checkpoint_id,
                    bundle.payload.round_state
                )
            } else {
                format!(
                    "{} on allocation {} ({})",
                    bundle.payload.acceptance_kind,
                    bundle.payload.allocation_id,
                    bundle.payload.round_state
                )
            }
        })
        .unwrap_or_else(|| "No formal acceptance bundle is current.".to_string());
    let next_step_line = status
        .next_step
        .as_ref()
        .map(|step| {
            if step.allocation_id == 0 {
                format!("{} on checkpoint {}", step.step, step.target_ref)
            } else {
                format!("{} for allocation {}", step.step, step.allocation_id)
            }
        })
        .unwrap_or_else(|| "No follow-on runtime step is currently open.".to_string());
    let round_state_line = format!(
        "{} / {} ({})",
        status.round_state.state, status.round_state.detail_state, status.round_state.summary
    );
    let projection_line = format!(
        "{}/{} required projections fresh",
        status.projections.fresh_required_target_count, status.projections.required_target_count
    );
    let invariant_line = format!(
        "{} passed, {} repairable, {} blocked",
        status.invariants.passed_count,
        status.invariants.repairable_count,
        status.invariants.blocked_count
    );
    let repair_lane_line = format!(
        "{} open, {} resolved",
        status.repair_lane.open_count, status.repair_lane.resolved_count
    );
    let recovery_line = format!("{} ({})", status.recovery.mode, status.recovery.summary);
    let supervision_line = match (
        status.current_supervision.as_ref(),
        status.current_supervision_incident.as_ref(),
    ) {
        (Some(projection), Some(incident)) => {
            let exhaustion_suffix = if incident.budget_exhausted {
                ", budget exhausted"
            } else {
                ""
            };
            format!(
                "{:?} (attempt {}/{}, action: {:?}{})",
                projection.current_supervision_state,
                incident.retry_count,
                incident.max_restarts,
                incident.last_supervisor_action,
                exhaustion_suffix,
            )
        }
        (Some(projection), None) => format!(
            "{:?} via {:?}",
            projection.current_supervision_state, projection.last_supervisor_action
        ),
        (None, _) => "No supervised allocation is currently active.".to_string(),
    };
    let owner_root = startup.paths().app_data_dir().display().to_string();
    let config_path = startup.paths().config_path().display().to_string();
    let db_path = startup.paths().db_path().display().to_string();
    let host_line = status
        .host
        .as_ref()
        .map(|host| format!("{} on {}", host.host_label, host.os_family))
        .unwrap_or_else(|| "No host snapshot has been recorded yet.".to_string());

    let readme = format!(
        "# Top Layer\n\n> Status: exported hot root from DB-first runtime truth\n\nThe top layer is a retained projection, not an authoring authority.\n\nActive hot-root files:\n\n- [machine.md](./machine.md)\n- [control.md](./control.md)\n- [truth.md](./truth.md)\n- [phase-todo.md](./phase-todo.md)\n- [pending.md](./pending.md)\n\nCurrent owner root:\n\n- `{owner_root}`\n- host: {host_line}\n- config: `{config_path}`\n- runtime DB: `{db_path}`\n- exported hot root: `{}`\n- observed worktrees: {}\n\nCurrent round:\n\n- human round: {human_round_line}\n- round state: {round_state_line}\n- checkpoint: {checkpoint_label}\n- stable level: {checkpoint_level}\n- acceptance: {acceptance_line}\n- anti-Zeno: {} / {} ({})\n- anti-Zeno budget: {} ({})\n- supervision: {supervision_line}\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- recovery: {recovery_line}\n- next step: {next_step_line}\n- projection freshness: {projection_line}\n\nProjection law:\n\n- DB is the only canonical writer.\n- README, hot root, cold docs, GUI, CLI, and MCP are projections from DB truth.\n- `passed human round = acceptance`.\n- canonical state and detail state are both derived from DB truth rather than hand-authored status prose.\n- `fully settled round = acceptance + no next_step + checkpoint carry-forward`.\n",
        startup.paths().exports_dir().join("hot-root").display(),
        status.worktree_count,
        status.anti_zeno.state,
        status.anti_zeno.detail_state,
        status.anti_zeno.summary,
        status.anti_zeno_budget.summary,
        status.anti_zeno_budget.state
    );

    let machine = format!(
        "# Machine\n\n> Status: hot root projection\n\n## Current Runtime Cut\n\n- current human round: {human_round_line}\n- round state: {round_state_line}\n- current checkpoint: {checkpoint_label}\n- stable level: {checkpoint_level}\n- acceptance bundle count: {}\n- current acceptance: {acceptance_line}\n- anti-Zeno state: {} / {} ({})\n- supervision: {supervision_line}\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n\n## State Law\n\n- runtime continuity is resumed from checkpoint, human-round, allocation, receipt, and cadence-object truth\n- `passed human round` is formalized as `CADENCE_ACCEPTANCE_BUNDLE`\n- canonical round ladder is `opened -> checkpointed -> accepted -> settling -> fully_settled`\n- detail state remains a finer runtime projection over that ladder rather than replacing it\n- `fully settled round` is stricter than acceptance and only holds after follow-on closure has been carried forward\n- phase is projection, not a peer truth plane\n",
        status.acceptance_bundle_count,
        status.anti_zeno.state,
        status.anti_zeno.detail_state,
        status.anti_zeno.summary
    );

    let control = format!(
        "# Control\n\n> Status: hot root projection\n\n## Runtime Authority\n\n- Human is the final sovereign.\n- NOTA is the only normal semantic ingress and egress.\n- Policy is the highest internal writer.\n- Arch / Dev / Agent are bounded execution lanes, not peer continuation authorities.\n\n## Active Control Boundary\n\n- current checkpoint: {checkpoint_label}\n- next step: {next_step_line}\n- review surface active: {}\n- integrate surface active: {}\n- finalize surface active: {}\n",
        status.review.is_some(),
        status.integrate.is_some(),
        status.finalize.is_some()
    );

    let truth = format!(
        "# Truth\n\n> Status: hot root projection\n\n## Canonical Law\n\n- DB-first is mandatory.\n- Every operation must write runtime truth before any projection is considered valid.\n- Files are preserved projections and may be regenerated from DB truth.\n- Cold docs remain canonicalized in DB and may be periodically projected back to files.\n\n## Projection Boundary\n\n- owner root: `{owner_root}`\n- host visibility: {host_line}\n- config TOML: `{config_path}`\n- runtime DB: `{db_path}`\n- files are downstream of truth, never upstream of truth\n- anti-Zeno is a derived progress discipline, not a second truth plane\n- recovery mode: {recovery_line}\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- required projection freshness: {projection_line}\n- required dirty projections: {}\n- owned worktree count: {}\n",
        status.projections.dirty_required_target_count,
        status.worktree_count
    );

    let phase_todo = format!(
        "# Phase Todo\n\n> Status: hot root projection\n\n## Current Focus\n\n- current checkpoint: {checkpoint_label}\n- acceptance: {acceptance_line}\n- anti-Zeno: {} / {} ({})\n- anti-Zeno budget: {} ({})\n- supervision: {supervision_line}\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- next step: {next_step_line}\n- projection freshness: {projection_line}\n\n## Ordered Work\n\n- keep runtime truth sharper than file projections\n- keep acceptance formalized as a cadence object rather than chat implication\n- keep anti-Zeno visible in status, overview, and exported hot root\n- keep canonical state and detail state both reconstructable from DB truth\n- keep supervision projection derived from runtime facts instead of task-label folklore\n- keep invariant failure and repair lane truth explicit in DB\n- keep hot-root export synchronized from DB truth after human-round writes\n",
        status.anti_zeno.state,
        status.anti_zeno.detail_state,
        status.anti_zeno.summary,
        status.anti_zeno_budget.state,
        status.anti_zeno_budget.summary
    );

    let pending = format!(
        "# Pending\n\n> Status: hot utility projection\n\n## Current Pending Boundary\n\n- recommended checkpoint present: {}\n- current next step: {next_step_line}\n- fully settled: {}\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- recovery mode: {recovery_line}\n\n## Rule\n\n- pending only holds unresolved items that are not yet oracle truth\n- once a pending item becomes truth, it must be carried by DB and then projected back out\n- do not let file-local TODOs outrank runtime truth\n",
        status.recommended_checkpoint.is_some(),
        status.anti_zeno.fully_settled
    );

    vec![
        ("README.md", readme),
        ("machine.md", machine),
        ("control.md", control),
        ("truth.md", truth),
        ("phase-todo.md", phase_todo),
        ("pending.md", pending),
    ]
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
