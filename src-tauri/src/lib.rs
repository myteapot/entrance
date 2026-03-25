pub mod core;
mod plugins;

use std::{
    fs,
    io::{self, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, Manager};

use core::{
    action::ActorRole,
    anti_zeno_runtime::{build_anti_zeno_budget_report, AntiZenoBudgetReport},
    bootstrap_for_paths,
    bootstrap_mcp_cycle::{
        run_forge_bootstrap_dev_task, run_forge_bootstrap_mcp_cycle, ForgeBootstrapMcpCycleOptions,
        ForgeBootstrapMcpCycleReport,
    },
    chat_archive::{
        capture_chat_message, get_chat_archive_policy, list_chat_captures, set_chat_archive_policy,
        ChatArchivePolicyReport, ChatArchivePolicyRequest, ChatCaptureListReport,
        ChatCaptureRequest,
    },
    cold_docs_runtime::{
        canonicalize_cold_docs_from_repo, export_cold_docs_to_repo, list_cold_documents,
        NotaColdDocExportReport, NotaColdDocListReport,
    },
    data_store::{
        StoredDecisionRecord, StoredNotaRuntimeReceipt, StoredNotaRuntimeTransaction,
        StoredSourceIngestRun, StoredTodoRecord, StoredVisionRecord,
    },
    design_governance::{
        list_design_decisions, record_design_decision, DesignDecisionListReport,
        DesignDecisionRequest,
    },
    environment_runtime::{
        current_runtime_host, list_owned_worktrees, OwnedWorktreeRegistryReport,
    },
    event_bus::EventBus,
    hotkey,
    hygiene::{list_spec_hygiene_v0, run_spec_hygiene_v0, SpecHygieneReport},
    invariant_runtime::{
        project_runtime_invariants, refresh_runtime_invariants, RepairLaneReport,
        RuntimeInvariantReport,
    },
    landing::{
        import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
        list_landing_planning_items, list_landing_unreconciled_items, LandingImportReport,
        LandingMirrorSummary, LandingPlanningItemSummary,
    },
    logging::LoggingSystem,
    mcp_server::{McpPluginSet, McpServer, McpTransport},
    nota_runtime::{
        active_checkpoint_scope_ids, derive_anti_zeno_projection,
        derive_current_runtime_acceptance_bundle, derive_current_runtime_handout,
        derive_current_runtime_human_round, derive_current_runtime_wake_request,
        derive_nota_runtime_finalize, derive_nota_runtime_integrate, derive_nota_runtime_next_step,
        derive_nota_runtime_review, derive_runtime_round_state_projection,
        list_nota_runtime_allocations, list_nota_runtime_receipts, list_nota_runtime_transactions,
        list_runtime_acceptance_bundles, list_runtime_checkpoints, list_runtime_human_rounds,
        materialize_runtime_closure_checkpoint, recommend_runtime_closure_checkpoint,
        record_dev_return_finalize, record_dev_return_integration, record_dev_return_review,
        run_nota_dev_dispatch, run_nota_do_agent_dispatch, write_runtime_checkpoint,
        NotaAcceptanceBundleListReport, NotaAntiZenoProjection, NotaCheckpointListReport,
        NotaCheckpointRequest, NotaDevDispatchRequest, NotaDevReturnFinalizeRequest,
        NotaDevReturnIntegrateRequest, NotaDevReturnReviewRequest, NotaDispatchExecutionHost,
        NotaDoAgentDispatchRequest, NotaHandoutRecord, NotaHumanRoundListReport,
        NotaRoundStateProjection, NotaRuntimeAllocationReadRecord, NotaRuntimeAllocationsReport,
        NotaRuntimeFinalize, NotaRuntimeIntegrate, NotaRuntimeNextStep, NotaRuntimeReview,
        NotaRuntimeTransactionsReport, NotaWakeRequestRecord,
    },
    plugin_manager::PluginManager,
    projection_runtime::{
        build_projection_status_report, record_projection_failure, record_projection_success,
        ProjectionStatusReport, ProjectionTargetSpec, ProjectionTruthRevision,
        HOT_ROOT_PROJECTION_CLASS, OPTIONAL_PROJECTION_POLICY, ORACLE_PROJECTION_CLASS,
        REQUIRED_PROJECTION_POLICY,
    },
    recovery::{
        build_recovery_status_report, import_recovery_seed, list_recovery_seed_rows,
        list_recovery_seed_runs, RecoveryImportOnlyStatusReport, RecoverySeedRowsQuery,
    },
    resolve_app_data_dir,
    theme::ThemeSystem,
    AppPaths, StartupState,
};
use plugins::{
    forge::commands::{
        forge_cancel_task, forge_create_task, forge_dispatch_agent, forge_get_task,
        forge_get_task_details, forge_list_tasks, forge_prepare_agent_dispatch,
    },
    forge::{
        prepare_agent_dispatch_blocking, verify_agent_dispatch, ForgeDispatchVerificationReport,
        PreparedAgentDispatch,
    },
    launcher::{launcher_launch, launcher_pin, launcher_search, LauncherPlugin},
    vault::{
        commands::{
            vault_add_token, vault_delete_token, vault_get_token, vault_get_token_by_provider,
            vault_list_mcp, vault_list_tokens, vault_update_mcp, vault_upsert_token,
        },
        VaultPlugin,
    },
    AppContext,
};

#[derive(Clone, Serialize)]
struct LauncherUiState {
    hotkey: Option<String>,
}

#[derive(Clone)]
struct DashboardUiState {
    app_version: String,
    launcher_hotkey: Option<String>,
    enabled_plugin_count: usize,
    launcher_enabled: bool,
    forge_enabled: bool,
    vault_enabled: bool,
}

#[derive(Clone, Serialize)]
struct DashboardSummary {
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
pub(crate) struct NotaRuntimeOverview {
    chat_policy: ChatArchivePolicyReport,
    checkpoints: NotaCheckpointListReport,
    human_rounds: NotaHumanRoundListReport,
    acceptance_bundles: NotaAcceptanceBundleListReport,
    transactions: NotaRuntimeTransactionsReport,
    allocations: NotaRuntimeAllocationsReport,
    visions: NotaVisionListReport,
    todos: NotaTodoListReport,
    cold_docs: NotaColdDocListReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<core::data_store::StoredRuntimeHost>,
    worktrees: OwnedWorktreeRegistryReport,
    recovery: RecoveryImportOnlyStatusReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    recommended_checkpoint: Option<NotaCheckpointRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    handout: Option<NotaHandoutRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wake_request: Option<NotaWakeRequestRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<NotaRuntimeReview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrate: Option<NotaRuntimeIntegrate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalize: Option<NotaRuntimeFinalize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_step: Option<NotaRuntimeNextStep>,
    round_state: NotaRoundStateProjection,
    anti_zeno: NotaAntiZenoProjection,
    anti_zeno_budget: AntiZenoBudgetReport,
    front_door: NotaFrontDoorProjection,
    projections: ProjectionStatusReport,
    invariants: RuntimeInvariantReport,
    repair_lane: RepairLaneReport,
    decisions: DesignDecisionListReport,
    chat_captures: ChatCaptureListReport,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaRuntimeStatus {
    chat_policy: ChatArchivePolicyReport,
    human_round_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_human_round: Option<core::nota_runtime::NotaHumanRoundRecord>,
    checkpoint_count: usize,
    current_checkpoint_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_checkpoint: Option<core::nota_runtime::NotaCheckpointRecord>,
    acceptance_bundle_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_acceptance_bundle: Option<core::nota_runtime::NotaAcceptanceBundleRecord>,
    transaction_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_transaction: Option<StoredNotaRuntimeTransaction>,
    allocation_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_allocation: Option<NotaRuntimeAllocationReadRecord>,
    receipt_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_receipt: Option<StoredNotaRuntimeReceipt>,
    decision_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_decision: Option<StoredDecisionRecord>,
    chat_capture_count: usize,
    vision_count: usize,
    todo_count: usize,
    cold_doc_count: usize,
    cold_docs: NotaColdDocListReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<core::data_store::StoredRuntimeHost>,
    worktree_count: usize,
    worktrees: OwnedWorktreeRegistryReport,
    recovery: RecoveryImportOnlyStatusReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    recommended_checkpoint: Option<NotaCheckpointRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    handout: Option<NotaHandoutRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wake_request: Option<NotaWakeRequestRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<NotaRuntimeReview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integrate: Option<NotaRuntimeIntegrate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finalize: Option<NotaRuntimeFinalize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_step: Option<NotaRuntimeNextStep>,
    round_state: NotaRoundStateProjection,
    anti_zeno: NotaAntiZenoProjection,
    anti_zeno_budget: AntiZenoBudgetReport,
    front_door: NotaFrontDoorProjection,
    projections: ProjectionStatusReport,
    invariants: RuntimeInvariantReport,
    repair_lane: RepairLaneReport,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaTodoListReport {
    todo_count: usize,
    todos: Vec<StoredTodoRecord>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaVisionListReport {
    vision_count: usize,
    visions: Vec<StoredVisionRecord>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaFrontDoorProjection {
    posture: String,
    summary: String,
    next_action_label: String,
    next_action_detail: String,
    dashboard_hook: String,
    progress_tracks: Vec<NotaFrontDoorProgressTrack>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaFrontDoorProgressTrack {
    id: String,
    label: String,
    value: u8,
    tone: String,
    summary: String,
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

fn build_projection_truth_revision(
    current_checkpoint_id: Option<i64>,
    current_human_round_id: Option<i64>,
    current_acceptance_bundle_id: Option<i64>,
) -> ProjectionTruthRevision {
    ProjectionTruthRevision {
        checkpoint_id: current_checkpoint_id,
        human_round_id: current_human_round_id,
        acceptance_bundle_id: current_acceptance_bundle_id,
    }
}

fn build_nota_front_door_projection(
    current_checkpoint: Option<&core::nota_runtime::NotaCheckpointRecord>,
    decision_count: usize,
    transaction_count: usize,
    allocation_count: usize,
    receipt_count: usize,
    anti_zeno: &NotaAntiZenoProjection,
    recommended_checkpoint: Option<&NotaCheckpointRequest>,
    review: Option<&NotaRuntimeReview>,
    integrate: Option<&NotaRuntimeIntegrate>,
    finalize: Option<&NotaRuntimeFinalize>,
    next_step: Option<&NotaRuntimeNextStep>,
) -> NotaFrontDoorProjection {
    let posture = if current_checkpoint.is_some() {
        "Checkpoint-backed native front door".to_string()
    } else {
        "Native front door waiting for first checkpoint".to_string()
    };

    let summary = if let Some(checkpoint) = current_checkpoint {
        checkpoint.cadence_object.summary.clone()
    } else if let Some(checkpoint) = recommended_checkpoint {
        checkpoint.stable_level.clone()
    } else {
        "Write the first checkpoint so the GUI can resume from runtime truth instead of terminal recap."
            .to_string()
    };

    let (next_action_label, next_action_detail) = if let Some(step) = next_step {
        (
            "Next runtime move".to_string(),
            describe_nota_front_door_next_step(step),
        )
    } else if let Some(checkpoint) = current_checkpoint {
        (
            "Current slice".to_string(),
            checkpoint
                .payload
                .selected_trunk
                .clone()
                .unwrap_or_else(|| checkpoint.cadence_object.title.clone()),
        )
    } else if let Some(checkpoint) = recommended_checkpoint {
        (
            "Suggested checkpoint".to_string(),
            checkpoint.remaining.first().cloned().unwrap_or_else(|| {
                checkpoint
                    .selected_trunk
                    .clone()
                    .unwrap_or_else(|| "Checkpoint the current closure boundary.".to_string())
            }),
        )
    } else if let Some(finalize) = finalize {
        (
            "Latest closure".to_string(),
            finalize.summary.clone().unwrap_or_else(|| {
                format!(
                    "Finalize closed allocation {} on lineage {}.",
                    finalize.allocation_id, finalize.lineage_ref
                )
            }),
        )
    } else if let Some(integrate) = integrate {
        (
            "Latest integration".to_string(),
            integrate.summary.clone().unwrap_or_else(|| {
                format!(
                    "Integration recorded {} on allocation {}.",
                    integrate
                        .outcome
                        .clone()
                        .unwrap_or_else(|| integrate.state.clone()),
                    integrate.allocation_id
                )
            }),
        )
    } else if let Some(review) = review {
        (
            "Latest review".to_string(),
            review.summary.clone().unwrap_or_else(|| {
                format!(
                    "Review is tracking allocation {} on lineage {}.",
                    review.allocation_id, review.lineage_ref
                )
            }),
        )
    } else {
        (
            "Current slice".to_string(),
            "No active checkpoint is recorded yet.".to_string(),
        )
    };

    let truth_spine_value = front_door_truth_spine_value(
        current_checkpoint.is_some(),
        decision_count,
        transaction_count,
        allocation_count,
        receipt_count,
    );
    let shell_reach_value = if current_checkpoint.is_some() { 82 } else { 72 };
    let relay_relief_summary = current_checkpoint
        .map(|checkpoint| checkpoint.payload.human_continuity_bus.clone())
        .or_else(|| {
            recommended_checkpoint.map(|checkpoint| checkpoint.human_continuity_bus.clone())
        })
        .unwrap_or_else(|| {
            "Human relay is still heavy because no checkpoint is active yet.".to_string()
        });
    let relay_relief_value = front_door_relay_relief_value(
        relay_relief_summary.as_str(),
        next_step.is_some(),
        recommended_checkpoint.is_some(),
    );

    NotaFrontDoorProjection {
        posture,
        summary,
        next_action_label,
        next_action_detail,
        dashboard_hook:
            "Dashboard now reads the same runtime truth plane as Chat, with acceptance-backed anti-Zeno progress and bounded continuity detail."
                .to_string(),
        progress_tracks: vec![
            NotaFrontDoorProgressTrack {
                id: "truth-spine".to_string(),
                label: "Grounded in truth".to_string(),
                value: truth_spine_value,
                tone: if truth_spine_value >= 80 {
                    "steady".to_string()
                } else {
                    "warming".to_string()
                },
                summary:
                    "Checkpoint, decision, transaction, and receipt reads are all coming from the NOTA runtime."
                        .to_string(),
            },
            NotaFrontDoorProgressTrack {
                id: "front-door-slice".to_string(),
                label: "Front-door reach".to_string(),
                value: shell_reach_value,
                tone: "active".to_string(),
                summary:
                    "This build exposes a Chat-first shell, a live state rail, mission progress, and a real import entry."
                        .to_string(),
            },
            NotaFrontDoorProgressTrack {
                id: "anti-zeno".to_string(),
                label: "Anti-Zeno progress".to_string(),
                value: anti_zeno.value,
                tone: if anti_zeno.fully_settled {
                    "steady".to_string()
                } else if anti_zeno.acceptance_present {
                    "active".to_string()
                } else {
                    "caution".to_string()
                },
                summary: anti_zeno.summary.clone(),
            },
            NotaFrontDoorProgressTrack {
                id: "relay-relief".to_string(),
                label: "Human relay relief".to_string(),
                value: relay_relief_value,
                tone: if relay_relief_value >= 70 {
                    "steady".to_string()
                } else {
                    "caution".to_string()
                },
                summary: relay_relief_summary,
            },
        ],
    }
}

fn describe_nota_front_door_next_step(step: &NotaRuntimeNextStep) -> String {
    let action = match step.step.as_str() {
        "review" => "Review the returned boundary",
        "integrate" => "Record the integration result",
        "finalize" => "Close the integrated boundary",
        other => return format!("Follow `{other}` for allocation {}.", step.allocation_id),
    };

    format!(
        "{action} for allocation {} on lineage {}.",
        step.allocation_id, step.lineage_ref
    )
}

fn front_door_truth_spine_value(
    has_checkpoint: bool,
    decision_count: usize,
    transaction_count: usize,
    allocation_count: usize,
    receipt_count: usize,
) -> u8 {
    let mut value = 18_u8;
    if has_checkpoint {
        value = value.saturating_add(30);
    }
    if decision_count > 0 {
        value = value.saturating_add(14);
    }
    if transaction_count > 0 {
        value = value.saturating_add(14);
    }
    if allocation_count > 0 {
        value = value.saturating_add(12);
    }
    if receipt_count > 0 {
        value = value.saturating_add(12);
    }
    value.min(100)
}

fn front_door_relay_relief_value(
    human_continuity_bus: &str,
    has_next_step: bool,
    has_recommended_checkpoint: bool,
) -> u8 {
    let normalized = human_continuity_bus.to_ascii_lowercase();
    let mut value: u8 = if normalized.contains("further reduced") {
        78
    } else if normalized.contains("reduced") {
        64
    } else {
        42
    };

    if has_next_step {
        value = value.saturating_sub(8);
    }
    if has_recommended_checkpoint {
        value = value.saturating_sub(6);
    }

    value
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
    app.manage(data_store.clone());
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
            if core::event_bus::match_topic("forge:*", &event.topic) {
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

#[tauri::command]
fn launcher_hotkey(state: tauri::State<'_, LauncherUiState>) -> Option<String> {
    state.hotkey.clone()
}

const ROOT_CLI_HELP: &str = r#"Entrance V0 headless alpha runtime shell

Usage:
  entrance
  entrance <command> [args...]
  entrance --help

Commands:
  nota       Read or write NOTA runtime continuity surfaces
  mcp        Serve Entrance as an MCP server over stdio or HTTP
  forge      Run Forge dispatch and bootstrap helpers
  landing    Import and inspect landing snapshots
  recovery   Inspect import-only recovery seed data
  hygiene    Run runtime and spec hygiene checks

Notes:
  Running `entrance` with no command starts the GUI shell.
  Run `entrance <command> --help` for command-specific usage.
"#;

const LANDING_CLI_HELP: &str = r#"Usage:
  entrance landing import --file <path>
  entrance landing import <path>
  entrance landing runs
  entrance landing mirrors
  entrance landing planning
  entrance landing unreconciled
"#;

const RECOVERY_CLI_HELP: &str = r#"Usage:
  entrance recovery status
  entrance recovery import-seed --file <path>
  entrance recovery import-seed <path>
  entrance recovery runs
  entrance recovery rows [--ingest-run-id <id>] [--table <name>] [--limit <n>]
"#;

const HYGIENE_CLI_HELP: &str = r#"Usage:
  entrance hygiene spec-v0
  entrance hygiene list-spec-v0
"#;

const FORGE_CLI_HELP: &str = r#"Usage:
  entrance forge prepare-dispatch
  entrance forge prepare-dispatch --project-dir <path>
  entrance forge verify-dispatch
  entrance forge verify-dispatch --project-dir <path>
  entrance forge bootstrap-mcp-cycle [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--agent-count <n>]
  entrance forge run-bootstrap-dev-plan
  entrance forge supervise-task --task-id <id>
"#;

const NOTA_CLI_HELP: &str = r#"Usage:
  entrance nota overview
  entrance nota status
  entrance nota chat-policy [--policy <off|summary|full>]
  entrance nota chat-captures
  entrance nota checkpoints
  entrance nota rounds
  entrance nota acceptance-bundles
  entrance nota projections
  entrance nota anti-zeno
  entrance nota invariants
  entrance nota repair
  entrance nota cold-docs
  entrance nota host
  entrance nota worktrees
  entrance nota canonicalize-cold-docs --project-dir <path>
  entrance nota export-cold-docs --project-dir <path>
  entrance nota export-hot-root [--project-dir <path>]
  entrance nota rebuild-projections [--project-dir <path>]
  entrance nota decisions
  entrance nota visions
  entrance nota todos
  entrance nota allocations
  entrance nota receipts [--transaction-id <id>]
  entrance nota transactions
  entrance nota do [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>]
  entrance nota dev [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>] [--repair-of-allocation-id <id>]
  entrance nota review --transaction-id <id> --allocation-id <id> --verdict <approved|changes_requested> [--summary <text>]
  entrance nota integrate --transaction-id <id> --allocation-id <id> --state <started|integrated|repair_requested> [--summary <text>]
  entrance nota finalize --transaction-id <id> --allocation-id <id> [--summary <text>]
  entrance nota decision --title <text> --statement <text> [--rationale <text>] [--decision-type <text>] [--scope-type <text>] [--scope-ref <text>] [--source-ref <text>] [--decided-by <text>] [--enforcement-level <text>] [--actor-scope <text>] [--confidence <float>] [--supersedes <id> ...] [--conflicts-with <id> ...]
  entrance nota capture-chat --role <human|nota> --content <text> [--summary <text>] [--session-ref <id>] [--scope-type <text>] [--scope-ref <text>] [--linked-decision-id <id>]
  entrance nota checkpoint --stable-level <text> --landed <text> [--landed <text> ...] --remaining <text> [--remaining <text> ...] --human-continuity-bus <text> [--selected-trunk <text>] [--next-start-hint <text> ...] [--title <text>] [--project-dir <path>]
  entrance nota checkpoint-runtime-closure
"#;

const MCP_CLI_HELP: &str = r#"Usage:
  entrance mcp stdio [--actor-role <nota|arch|dev>]
  entrance mcp http [--port <port>] [--endpoint <path>] [--actor-role <nota|arch|dev>]
"#;

fn is_help_flag(value: &str) -> bool {
    matches!(value, "help" | "-h" | "--help")
}

fn cli_help_for_args(args: &[String]) -> Option<&'static str> {
    match args {
        [flag] if is_help_flag(flag) => Some(ROOT_CLI_HELP),
        [command, flag] if command == "landing" && is_help_flag(flag) => Some(LANDING_CLI_HELP),
        [command, flag] if command == "recovery" && is_help_flag(flag) => Some(RECOVERY_CLI_HELP),
        [command, flag] if command == "hygiene" && is_help_flag(flag) => Some(HYGIENE_CLI_HELP),
        [command, flag] if command == "nota" && is_help_flag(flag) => Some(NOTA_CLI_HELP),
        [command, flag] if command == "forge" && is_help_flag(flag) => Some(FORGE_CLI_HELP),
        [command, flag] if command == "mcp" && is_help_flag(flag) => Some(MCP_CLI_HELP),
        [command, transport, flag]
            if command == "mcp"
                && matches!(transport.as_str(), "stdio" | "http")
                && is_help_flag(flag) =>
        {
            Some(MCP_CLI_HELP)
        }
        _ => None,
    }
}

fn print_cli_help(help: &str) {
    println!("{help}");
}

pub fn dispatch_cli_or_run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(help) = cli_help_for_args(&args) {
        print_cli_help(help);
        return Ok(());
    }

    match args.as_slice() {
        [command, rest @ ..] if command == "landing" => run_landing_cli(rest),
        [command, rest @ ..] if command == "recovery" => run_recovery_cli(rest),
        [command, rest @ ..] if command == "hygiene" => run_hygiene_cli(rest),
        [command, rest @ ..] if command == "nota" => run_nota_cli(rest),
        [command, rest @ ..] if command == "forge" => run_forge_cli(rest),
        [command, transport, rest @ ..] if command == "mcp" && transport == "stdio" => {
            run_mcp_stdio(rest)
        }
        [command, transport, rest @ ..] if command == "mcp" && transport == "http" => {
            run_mcp_http(rest)
        }
        [command, ..] if command == "mcp" => {
            bail!("unsupported MCP transport, expected `entrance mcp stdio` or `entrance mcp http`")
        }
        _ => {
            run();
            Ok(())
        }
    }
}

fn run_recovery_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "status" => {
            print_json(&build_recovery_status_report(&startup.data_store())?)
        }
        [command, flag, value] if command == "import-seed" && flag == "--file" => {
            let report = import_recovery_seed(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command, value] if command == "import-seed" => {
            let report = import_recovery_seed(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command] if command == "runs" => print_json(&list_recovery_seed_runs(&startup.data_store())?),
        [command, rest @ ..] if command == "rows" => {
            let query = parse_recovery_rows_args(rest)?;
            print_json(&list_recovery_seed_rows(&startup.data_store(), query)?)
        }
        [command, rest @ ..] if command == "promote-safe-v0" => {
            let suffix = if rest.is_empty() {
                String::new()
            } else {
                format!(" ({})", rest.join(" "))
            };
            bail!(
                "recovery promotion is permanently disabled; `entrance recovery promote-safe-v0{suffix}` is no longer available because recovery is import-only"
            )
        }
        [command, rest @ ..] if command == "promote-remaining-v0" => {
            let suffix = if rest.is_empty() {
                String::new()
            } else {
                format!(" ({})", rest.join(" "))
            };
            bail!(
                "recovery promotion is permanently disabled; `entrance recovery promote-remaining-v0{suffix}` is no longer available because recovery is import-only"
            )
        }
        _ => bail!(
            "unsupported recovery command, expected `entrance recovery status`, `entrance recovery import-seed --file <path>`, `entrance recovery runs`, or `entrance recovery rows [--ingest-run-id <id>] [--table <name>] [--limit <n>]`"
        ),
    }
}

fn parse_recovery_rows_args(args: &[String]) -> Result<RecoverySeedRowsQuery> {
    let mut query = RecoverySeedRowsQuery::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--ingest-run-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance recovery rows --ingest-run-id` requires a value")?;
                query.ingest_run_id = Some(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid recovery ingest run id `{value}`"))?,
                );
                index += 2;
            }
            "--table" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance recovery rows --table` requires a value")?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance recovery rows --table` must not be empty");
                }
                query.table_name = Some(trimmed.to_string());
                index += 2;
            }
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance recovery rows --limit` requires a value")?;
                let limit = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid recovery row limit `{value}`"))?;
                if limit == 0 {
                    bail!("`entrance recovery rows --limit` must be >= 1");
                }
                query.limit = Some(limit);
                index += 2;
            }
            other => bail!("unsupported recovery rows argument `{other}`"),
        }
    }

    Ok(query)
}

fn run_landing_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command, flag, value] if command == "import" && flag == "--file" => {
            let report = import_linear_entrance_snapshot(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command, value] if command == "import" => {
            let report = import_linear_entrance_snapshot(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command] if command == "runs" => print_json(&list_landing_ingest_runs(&startup.data_store())?),
        [command] if command == "mirrors" => {
            print_json(&list_landing_mirror_items(&startup.data_store())?)
        }
        [command] if command == "planning" => {
            print_json(&list_landing_planning_items(&startup.data_store())?)
        }
        [command] if command == "unreconciled" => {
            print_json(&list_landing_unreconciled_items(&startup.data_store())?)
        }
        _ => bail!(
            "unsupported landing command, expected one of `entrance landing import --file <path>`, `entrance landing runs`, `entrance landing mirrors`, `entrance landing planning`, or `entrance landing unreconciled`"
        ),
    }
}

fn run_forge_cli(args: &[String]) -> Result<()> {
    match args {
        [command] if command == "prepare-dispatch" => {
            print_json(&prepare_forge_dispatch_cli(None)?)
        }
        [command, flag, value] if command == "prepare-dispatch" && flag == "--project-dir" => {
            print_json(&prepare_forge_dispatch_cli(Some(value.to_string()))?)
        }
        [command] if command == "verify-dispatch" => {
            print_json(&verify_forge_dispatch_cli(None)?)
        }
        [command, flag, value] if command == "verify-dispatch" && flag == "--project-dir" => {
            print_json(&verify_forge_dispatch_cli(Some(value.to_string()))?)
        }
        [command, rest @ ..] if command == "bootstrap-mcp-cycle" => {
            print_json(&bootstrap_forge_mcp_cycle_cli(parse_forge_bootstrap_mcp_cycle_args(
                rest,
            )?)?)
        }
        [command] if command == "run-bootstrap-dev-plan" => {
            print_json(&run_forge_bootstrap_dev_plan_cli()?)
        }
        [command, rest @ ..] if command == "supervise-task" => {
            run_forge_supervise_task_cli(parse_forge_supervise_task_args(rest)?)
        }
        _ => bail!(
            "unsupported forge command, expected `entrance forge prepare-dispatch`, `entrance forge prepare-dispatch --project-dir <path>`, `entrance forge verify-dispatch`, `entrance forge verify-dispatch --project-dir <path>`, `entrance forge bootstrap-mcp-cycle [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--agent-count <n>]`, `entrance forge run-bootstrap-dev-plan`, or `entrance forge supervise-task --task-id <id>`"
        ),
    }
}

fn run_hygiene_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "spec-v0" => print_json(&run_spec_hygiene_v0(&startup.data_store())?),
        [command] if command == "list-spec-v0" => {
            print_json(&list_spec_hygiene_v0(&startup.data_store())?)
        }
        _ => bail!(
            "unsupported hygiene command, expected `entrance hygiene spec-v0` or `entrance hygiene list-spec-v0`"
        ),
    }
}

fn run_nota_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "overview" => {
            print_json(&build_nota_runtime_overview(&startup.data_store())?)
        }
        [command] if command == "status" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?)
        }
        [command] if command == "chat-policy" => {
            print_json(&get_chat_archive_policy(&startup.data_store(), None, None)?)
        }
        [command] if command == "chat-captures" => {
            print_json(&list_chat_captures(&startup.data_store())?)
        }
        [command] if command == "checkpoints" => {
            print_json(&list_runtime_checkpoints(&startup.data_store())?)
        }
        [command] if command == "rounds" => {
            print_json(&list_runtime_human_rounds(&startup.data_store())?)
        }
        [command] if command == "acceptance-bundles" => {
            print_json(&list_runtime_acceptance_bundles(&startup.data_store())?)
        }
        [command] if command == "projections" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.projections)
        }
        [command] if command == "anti-zeno" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.anti_zeno_budget)
        }
        [command] if command == "invariants" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.invariants)
        }
        [command] if command == "repair" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.repair_lane)
        }
        [command] if command == "cold-docs" => {
            let status = build_nota_runtime_status(&startup.data_store())?;
            print_json(&status.cold_docs)
        }
        [command] if command == "host" => {
            print_json(&current_runtime_host(&startup.data_store())?)
        }
        [command] if command == "worktrees" => {
            let host = current_runtime_host(&startup.data_store())?;
            print_json(&list_owned_worktrees(
                &startup.data_store(),
                host.as_ref().map(|value| value.host_key.as_str()),
            )?)
        }
        [command, flag, value] if command == "canonicalize-cold-docs" && flag == "--project-dir" => {
            let report = canonicalize_cold_docs_from_repo(&startup.data_store(), value)?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            print_json(&report)
        }
        [command, flag, value] if command == "export-cold-docs" && flag == "--project-dir" => {
            let status = build_nota_runtime_status(&startup.data_store())?;
            let report = export_cold_docs_to_repo(&startup.data_store(), value, &status.projections.current_truth_revision)?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            print_json(&report)
        }
        [command] if command == "export-hot-root" => {
            print_json(&write_hot_root_projection(&startup, None)?)
        }
        [command, flag, value] if command == "export-hot-root" && flag == "--project-dir" => {
            print_json(&write_hot_root_projection(&startup, Some(value))?)
        }
        [command] if command == "rebuild-projections" => {
            print_json(&rebuild_nota_projections(&startup, None)?)
        }
        [command, flag, value] if command == "rebuild-projections" && flag == "--project-dir" => {
            print_json(&rebuild_nota_projections(&startup, Some(value))?)
        }
        [command] if command == "decisions" => {
            print_json(&list_design_decisions(&startup.data_store())?)
        }
        [command] if command == "visions" => print_json(&list_nota_visions(&startup.data_store())?),
        [command] if command == "todos" => print_json(&list_nota_todos(&startup.data_store())?),
        [command] if command == "allocations" => {
            print_json(&list_nota_runtime_allocations(&startup.data_store())?)
        }
        [command] if command == "receipts" => {
            print_json(&list_nota_runtime_receipts(&startup.data_store(), None)?)
        }
        [command] if command == "transactions" => {
            print_json(&list_nota_runtime_transactions(&startup.data_store())?)
        }
        [command, rest @ ..] if command == "receipts" => {
            let transaction_id = parse_nota_receipts_args(rest)?;
            print_json(&list_nota_runtime_receipts(
                &startup.data_store(),
                transaction_id,
            )?)
        }
        [command, rest @ ..] if command == "chat-policy" => {
            let request = parse_nota_chat_policy_args(rest)?;
            print_json(&set_chat_archive_policy(&startup.data_store(), request)?)
        }
        [command, rest @ ..] if command == "capture-chat" => {
            let request = parse_nota_chat_capture_args(rest)?;
            let report = capture_chat_message(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "decision" => {
            let request = parse_nota_decision_args(rest)?;
            let report = record_design_decision(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "do" => {
            if !startup.forge_enabled() {
                bail!("Forge is disabled in entrance.toml");
            }

            let request = parse_nota_dispatch_args(rest, "do")?;
            let config = startup.config_store();
            let forge_config = &config.config().plugins.forge;
            let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
            let project_dir = request.project_dir.or_else(|| forge_config.project_dir.clone());
            let agent_command = request
                .agent_command
                .or_else(|| forge_config.agent_command.clone());

            let report = run_nota_do_agent_dispatch(
                &startup.data_store(),
                &forge_plugin,
                NotaDoAgentDispatchRequest {
                    project_dir,
                    model: request.model,
                    agent_command,
                    title: request.title,
                    repair_of_allocation_id: request.repair_of_allocation_id,
                    execution_host: NotaDispatchExecutionHost::DetachedForgeCliSupervisor,
                },
            )?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "dev" => {
            if !startup.forge_enabled() {
                bail!("Forge is disabled in entrance.toml");
            }

            let request = parse_nota_dispatch_args(rest, "dev")?;
            let config = startup.config_store();
            let forge_config = &config.config().plugins.forge;
            let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
            let project_dir = request.project_dir.or_else(|| forge_config.project_dir.clone());
            let agent_command = request
                .agent_command
                .or_else(|| forge_config.agent_command.clone());

            let report = run_nota_dev_dispatch(
                &startup.data_store(),
                &forge_plugin,
                NotaDevDispatchRequest {
                    project_dir,
                    model: request.model,
                    agent_command,
                    title: request.title,
                    repair_of_allocation_id: request.repair_of_allocation_id,
                    execution_host: NotaDispatchExecutionHost::DetachedForgeCliSupervisor,
                },
            )?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "checkpoint" => {
            let request = parse_nota_checkpoint_args(rest)?;
            let mirror_project_dir = request.project_dir.clone();
            let report = write_runtime_checkpoint(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, mirror_project_dir.as_deref())?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "review" => {
            let request = parse_nota_review_args(rest)?;
            let report = record_dev_return_review(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "integrate" => {
            let request = parse_nota_integrate_args(rest)?;
            let report = record_dev_return_integration(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "finalize" => {
            let request = parse_nota_finalize_args(rest)?;
            let report = record_dev_return_finalize(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command] if command == "checkpoint-runtime-closure" => {
            let report = materialize_runtime_closure_checkpoint(&startup.data_store())?;
            let mirror_project_dir = report
                .checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.payload.repo_context.as_ref())
                .map(|context| context.project_dir.as_str());
            write_hot_root_projection(&startup, mirror_project_dir)?;
            print_json(&report)
        }
        _ => bail!(
            "unsupported nota command, expected `entrance nota overview`, `entrance nota status`, `entrance nota do [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>]`, `entrance nota dev [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>] [--repair-of-allocation-id <id>]`, `entrance nota review --transaction-id <id> --allocation-id <id> --verdict <approved|changes_requested> [--summary <text>]`, `entrance nota integrate --transaction-id <id> --allocation-id <id> --state <started|integrated|repair_requested> [--summary <text>]`, `entrance nota finalize --transaction-id <id> --allocation-id <id> [--summary <text>]`, `entrance nota decision --title <text> --statement <text> [--rationale <text>] [--decision-type <text>] [--scope-type <text>] [--scope-ref <text>] [--source-ref <text>] [--decided-by <text>] [--enforcement-level <text>] [--actor-scope <text>] [--confidence <float>] [--supersedes <id> ...] [--conflicts-with <id> ...]`, `entrance nota chat-policy [--policy <off|summary|full>]`, `entrance nota capture-chat --role <human|nota> --content <text> [--summary <text>] [--session-ref <id>] [--scope-type <text>] [--scope-ref <text>] [--linked-decision-id <id>]`, `entrance nota checkpoint --stable-level <text> --landed <text> [--landed <text> ...] --remaining <text> [--remaining <text> ...] --human-continuity-bus <text> [--selected-trunk <text>] [--next-start-hint <text> ...] [--title <text>] [--project-dir <path>]`, `entrance nota checkpoint-runtime-closure`, `entrance nota checkpoints`, `entrance nota rounds`, `entrance nota acceptance-bundles`, `entrance nota projections`, `entrance nota anti-zeno`, `entrance nota invariants`, `entrance nota repair`, `entrance nota cold-docs`, `entrance nota host`, `entrance nota worktrees`, `entrance nota canonicalize-cold-docs --project-dir <path>`, `entrance nota export-cold-docs --project-dir <path>`, `entrance nota export-hot-root [--project-dir <path>]`, `entrance nota rebuild-projections [--project-dir <path>]`, `entrance nota decisions`, `entrance nota visions`, `entrance nota todos`, `entrance nota chat-captures`, `entrance nota allocations`, `entrance nota receipts [--transaction-id <id>]`, or `entrance nota transactions`"
        ),
    }
}

fn parse_nota_receipts_args(args: &[String]) -> Result<Option<i64>> {
    let mut transaction_id = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota receipts --transaction-id` requires a value")?;
                let parsed = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if parsed <= 0 {
                    bail!("`entrance nota receipts --transaction-id` must be >= 1");
                }
                transaction_id = Some(parsed);
                index += 2;
            }
            other => bail!("unsupported nota receipts argument `{other}`"),
        }
    }

    Ok(transaction_id)
}

fn run_mcp_stdio(args: &[String]) -> Result<()> {
    let actor_role = parse_mcp_actor_role_args(args)?;
    let startup = bootstrap_headless()?;
    let server = build_mcp_server(&startup, McpTransport::Stdio, actor_role)?;
    server.serve_stdio()
}

fn run_mcp_http(args: &[String]) -> Result<()> {
    let mut port = 9720u16;
    let mut endpoint = "/mcp".to_string();
    let mut actor_role = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--port" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --port` requires a value")?;
                port = value
                    .parse::<u16>()
                    .with_context(|| format!("invalid MCP HTTP port `{value}`"))?;
                index += 2;
            }
            "--endpoint" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --endpoint` requires a value")?;
                endpoint = normalize_http_endpoint(value)?;
                index += 2;
            }
            "--actor-role" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --actor-role` requires a value")?;
                actor_role = Some(parse_mcp_actor_role(value)?);
                index += 2;
            }
            other => bail!("unsupported MCP HTTP argument `{other}`"),
        }
    }

    let startup = bootstrap_headless()?;
    let server = build_mcp_server(
        &startup,
        McpTransport::Http {
            endpoint: endpoint.clone(),
        },
        actor_role,
    )?;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime for MCP HTTP transport")?;

    runtime.block_on(server.serve_http(address))
}

fn bootstrap_headless() -> Result<StartupState> {
    let startup = bootstrap_cli_state()?;
    if !startup.mcp_enabled() {
        bail!("MCP server is disabled in entrance.toml");
    }

    let _logging_system = LoggingSystem::init(
        startup.paths().log_dir(),
        startup.log_level(),
        Some(startup.data_store()),
    )?;

    Ok(startup)
}

fn bootstrap_cli_state() -> Result<StartupState> {
    let app_paths = AppPaths::new(resolve_app_data_dir()?);
    bootstrap_for_paths(app_paths)
}

fn bootstrap_forge_cli_state() -> Result<StartupState> {
    let startup = bootstrap_cli_state()?;
    if !startup.forge_enabled() {
        bail!("Forge is disabled in entrance.toml");
    }

    Ok(startup)
}

fn prepare_forge_dispatch_with_startup(
    startup: &StartupState,
    project_dir: Option<String>,
) -> Result<PreparedAgentDispatch> {
    prepare_agent_dispatch_blocking(startup.data_store(), project_dir).map_err(anyhow::Error::msg)
}

fn prepare_forge_dispatch_cli(project_dir: Option<String>) -> Result<PreparedAgentDispatch> {
    let startup = bootstrap_forge_cli_state()?;
    prepare_forge_dispatch_with_startup(&startup, project_dir)
}

fn verify_forge_dispatch_cli(
    project_dir: Option<String>,
) -> Result<ForgeDispatchVerificationReport> {
    let startup = bootstrap_forge_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    verify_agent_dispatch(&forge_plugin, project_dir).map_err(anyhow::Error::msg)
}

fn bootstrap_forge_mcp_cli_state() -> Result<StartupState> {
    let startup = bootstrap_headless()?;
    if !startup.forge_enabled() {
        bail!("Forge is disabled in entrance.toml");
    }

    Ok(startup)
}

fn parse_forge_bootstrap_mcp_cycle_args(args: &[String]) -> Result<ForgeBootstrapMcpCycleOptions> {
    let mut options = ForgeBootstrapMcpCycleOptions {
        project_dir: None,
        model: "codex".to_string(),
        agent_command: None,
        agent_count: 1,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--project-dir" => {
                let value = args.get(index + 1).context(
                    "`entrance forge bootstrap-mcp-cycle --project-dir` requires a value",
                )?;
                options.project_dir = Some(value.to_string());
                index += 2;
            }
            "--model" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance forge bootstrap-mcp-cycle --model` requires a value")?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance forge bootstrap-mcp-cycle --model` must not be empty");
                }
                options.model = trimmed.to_string();
                index += 2;
            }
            "--agent-command" => {
                let value = args.get(index + 1).context(
                    "`entrance forge bootstrap-mcp-cycle --agent-command` requires a value",
                )?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance forge bootstrap-mcp-cycle --agent-command` must not be empty");
                }
                options.agent_command = Some(trimmed.to_string());
                index += 2;
            }
            "--agent-count" => {
                let value = args.get(index + 1).context(
                    "`entrance forge bootstrap-mcp-cycle --agent-count` requires a value",
                )?;
                let parsed = value.parse::<usize>().with_context(|| {
                    format!(
                        "`entrance forge bootstrap-mcp-cycle --agent-count` received invalid value `{value}`"
                    )
                })?;
                if parsed == 0 {
                    bail!("`entrance forge bootstrap-mcp-cycle --agent-count` must be >= 1");
                }
                options.agent_count = parsed;
                index += 2;
            }
            other => bail!("unsupported forge bootstrap-mcp-cycle argument `{other}`"),
        }
    }

    Ok(options)
}

fn parse_forge_supervise_task_args(args: &[String]) -> Result<i64> {
    let mut task_id = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--task-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance forge supervise-task --task-id` requires a value")?;
                let parsed = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid forge task id `{value}`"))?;
                if parsed <= 0 {
                    bail!("`entrance forge supervise-task --task-id` must be >= 1");
                }
                task_id = Some(parsed);
                index += 2;
            }
            other => bail!("unsupported forge supervise-task argument `{other}`"),
        }
    }

    task_id.context("`entrance forge supervise-task --task-id` is required")
}

fn bootstrap_forge_mcp_cycle_cli(
    options: ForgeBootstrapMcpCycleOptions,
) -> Result<ForgeBootstrapMcpCycleReport> {
    let startup = bootstrap_forge_mcp_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    run_forge_bootstrap_mcp_cycle(&forge_plugin, startup.paths().app_data_dir(), options)
}

fn run_forge_supervise_task_cli(task_id: i64) -> Result<()> {
    let startup = bootstrap_forge_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    forge_plugin.engine().spawn_task(task_id)?;

    loop {
        let task = forge_plugin.get_task(task_id)?.ok_or_else(|| {
            anyhow::anyhow!("forge task `{task_id}` disappeared during supervision")
        })?;
        if matches!(
            task.status.as_str(),
            "Done" | "Failed" | "Cancelled" | "Blocked"
        ) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn parse_nota_checkpoint_args(args: &[String]) -> Result<NotaCheckpointRequest> {
    let mut request = NotaCheckpointRequest {
        title: None,
        stable_level: String::new(),
        landed: Vec::new(),
        remaining: Vec::new(),
        human_continuity_bus: String::new(),
        selected_trunk: None,
        next_start_hints: Vec::new(),
        project_dir: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--title" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --title` requires a value")?;
                request.title = Some(value.to_string());
                index += 2;
            }
            "--stable-level" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --stable-level` requires a value")?;
                request.stable_level = value.to_string();
                index += 2;
            }
            "--landed" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --landed` requires a value")?;
                request.landed.push(value.to_string());
                index += 2;
            }
            "--remaining" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --remaining` requires a value")?;
                request.remaining.push(value.to_string());
                index += 2;
            }
            "--human-continuity-bus" => {
                let value = args.get(index + 1).context(
                    "`entrance nota checkpoint --human-continuity-bus` requires a value",
                )?;
                request.human_continuity_bus = value.to_string();
                index += 2;
            }
            "--selected-trunk" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --selected-trunk` requires a value")?;
                request.selected_trunk = Some(value.to_string());
                index += 2;
            }
            "--next-start-hint" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --next-start-hint` requires a value")?;
                request.next_start_hints.push(value.to_string());
                index += 2;
            }
            "--project-dir" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --project-dir` requires a value")?;
                request.project_dir = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota checkpoint argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_review_args(args: &[String]) -> Result<NotaDevReturnReviewRequest> {
    let mut request = NotaDevReturnReviewRequest {
        transaction_id: 0,
        allocation_id: 0,
        verdict: String::new(),
        summary: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --transaction-id` requires a value")?;
                request.transaction_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if request.transaction_id <= 0 {
                    bail!("`entrance nota review --transaction-id` must be >= 1");
                }
                index += 2;
            }
            "--allocation-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --allocation-id` requires a value")?;
                request.allocation_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if request.allocation_id <= 0 {
                    bail!("`entrance nota review --allocation-id` must be >= 1");
                }
                index += 2;
            }
            "--verdict" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --verdict` requires a value")?;
                request.verdict = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota review argument `{other}`"),
        }
    }

    if request.transaction_id <= 0 {
        bail!("`entrance nota review --transaction-id` is required");
    }
    if request.allocation_id <= 0 {
        bail!("`entrance nota review --allocation-id` is required");
    }
    if request.verdict.trim().is_empty() {
        bail!("`entrance nota review --verdict` is required");
    }

    Ok(request)
}

fn parse_nota_integrate_args(args: &[String]) -> Result<NotaDevReturnIntegrateRequest> {
    let mut request = NotaDevReturnIntegrateRequest {
        transaction_id: 0,
        allocation_id: 0,
        state: String::new(),
        summary: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --transaction-id` requires a value")?;
                request.transaction_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if request.transaction_id <= 0 {
                    bail!("`entrance nota integrate --transaction-id` must be >= 1");
                }
                index += 2;
            }
            "--allocation-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --allocation-id` requires a value")?;
                request.allocation_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if request.allocation_id <= 0 {
                    bail!("`entrance nota integrate --allocation-id` must be >= 1");
                }
                index += 2;
            }
            "--state" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --state` requires a value")?;
                request.state = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota integrate argument `{other}`"),
        }
    }

    if request.transaction_id <= 0 {
        bail!("`entrance nota integrate --transaction-id` is required");
    }
    if request.allocation_id <= 0 {
        bail!("`entrance nota integrate --allocation-id` is required");
    }
    if request.state.trim().is_empty() {
        bail!("`entrance nota integrate --state` is required");
    }

    Ok(request)
}

fn parse_nota_finalize_args(args: &[String]) -> Result<NotaDevReturnFinalizeRequest> {
    let mut request = NotaDevReturnFinalizeRequest {
        transaction_id: 0,
        allocation_id: 0,
        summary: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota finalize --transaction-id` requires a value")?;
                request.transaction_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if request.transaction_id <= 0 {
                    bail!("`entrance nota finalize --transaction-id` must be >= 1");
                }
                index += 2;
            }
            "--allocation-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota finalize --allocation-id` requires a value")?;
                request.allocation_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if request.allocation_id <= 0 {
                    bail!("`entrance nota finalize --allocation-id` must be >= 1");
                }
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota finalize --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota finalize argument `{other}`"),
        }
    }

    if request.transaction_id <= 0 {
        bail!("`entrance nota finalize --transaction-id` is required");
    }
    if request.allocation_id <= 0 {
        bail!("`entrance nota finalize --allocation-id` is required");
    }

    Ok(request)
}

fn parse_nota_dispatch_args(
    args: &[String],
    command_name: &str,
) -> Result<NotaDoAgentDispatchRequest> {
    let mut request = NotaDoAgentDispatchRequest {
        project_dir: None,
        model: "codex".to_string(),
        agent_command: None,
        title: None,
        repair_of_allocation_id: None,
        execution_host: NotaDispatchExecutionHost::InProcess,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--project-dir" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --project-dir` requires a value")
                })?;
                request.project_dir = Some(value.to_string());
                index += 2;
            }
            "--model" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --model` requires a value")
                })?;
                request.model = value.to_string();
                index += 2;
            }
            "--agent-command" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --agent-command` requires a value")
                })?;
                request.agent_command = Some(value.to_string());
                index += 2;
            }
            "--title" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --title` requires a value")
                })?;
                request.title = Some(value.to_string());
                index += 2;
            }
            "--repair-of-allocation-id" => {
                if command_name != "dev" {
                    bail!(
                        "`entrance nota {command_name}` does not support `--repair-of-allocation-id`"
                    );
                }
                let value = args.get(index + 1).with_context(|| {
                    format!(
                        "`entrance nota {command_name} --repair-of-allocation-id` requires a value"
                    )
                })?;
                let parsed = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if parsed <= 0 {
                    bail!("`entrance nota {command_name} --repair-of-allocation-id` must be >= 1");
                }
                request.repair_of_allocation_id = Some(parsed);
                index += 2;
            }
            other => bail!("unsupported nota {command_name} argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_decision_args(args: &[String]) -> Result<DesignDecisionRequest> {
    let mut request = DesignDecisionRequest {
        title: String::new(),
        statement: String::new(),
        rationale: String::new(),
        decision_type: String::new(),
        decision_status: "accepted".to_string(),
        scope_type: String::new(),
        scope_ref: String::new(),
        source_ref: String::new(),
        decided_by: "NOTA".to_string(),
        enforcement_level: "runtime_canonical".to_string(),
        actor_scope: "system".to_string(),
        confidence: 1.0,
        supersedes: Vec::new(),
        conflicts_with: Vec::new(),
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--title" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --title` requires a value")?;
                request.title = value.to_string();
                index += 2;
            }
            "--statement" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --statement` requires a value")?;
                request.statement = value.to_string();
                index += 2;
            }
            "--rationale" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --rationale` requires a value")?;
                request.rationale = value.to_string();
                index += 2;
            }
            "--decision-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --decision-type` requires a value")?;
                request.decision_type = value.to_string();
                index += 2;
            }
            "--decision-status" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --decision-status` requires a value")?;
                request.decision_status = value.to_string();
                index += 2;
            }
            "--scope-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --scope-type` requires a value")?;
                request.scope_type = value.to_string();
                index += 2;
            }
            "--scope-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --scope-ref` requires a value")?;
                request.scope_ref = value.to_string();
                index += 2;
            }
            "--source-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --source-ref` requires a value")?;
                request.source_ref = value.to_string();
                index += 2;
            }
            "--decided-by" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --decided-by` requires a value")?;
                request.decided_by = value.to_string();
                index += 2;
            }
            "--enforcement-level" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --enforcement-level` requires a value")?;
                request.enforcement_level = value.to_string();
                index += 2;
            }
            "--actor-scope" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --actor-scope` requires a value")?;
                request.actor_scope = value.to_string();
                index += 2;
            }
            "--confidence" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --confidence` requires a value")?;
                request.confidence = value
                    .parse::<f64>()
                    .with_context(|| format!("invalid nota decision confidence `{value}`"))?;
                index += 2;
            }
            "--supersedes" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --supersedes` requires a value")?;
                request.supersedes.push(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid superseded decision id `{value}`"))?,
                );
                index += 2;
            }
            "--conflicts-with" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --conflicts-with` requires a value")?;
                request.conflicts_with.push(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid conflicted decision id `{value}`"))?,
                );
                index += 2;
            }
            other => bail!("unsupported nota decision argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_chat_policy_args(args: &[String]) -> Result<ChatArchivePolicyRequest> {
    let mut request = ChatArchivePolicyRequest {
        scope_type: None,
        scope_ref: None,
        archive_policy: "off".to_string(),
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--policy" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota chat-policy --policy` requires a value")?;
                request.archive_policy = value.to_string();
                index += 2;
            }
            "--scope-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota chat-policy --scope-type` requires a value")?;
                request.scope_type = Some(value.to_string());
                index += 2;
            }
            "--scope-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota chat-policy --scope-ref` requires a value")?;
                request.scope_ref = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota chat-policy argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_chat_capture_args(args: &[String]) -> Result<ChatCaptureRequest> {
    let mut request = ChatCaptureRequest {
        session_ref: None,
        role: String::new(),
        content: String::new(),
        summary: None,
        scope_type: None,
        scope_ref: None,
        linked_decision_id: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --session-ref` requires a value")?;
                request.session_ref = Some(value.to_string());
                index += 2;
            }
            "--role" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --role` requires a value")?;
                request.role = value.to_string();
                index += 2;
            }
            "--content" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --content` requires a value")?;
                request.content = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            "--scope-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --scope-type` requires a value")?;
                request.scope_type = Some(value.to_string());
                index += 2;
            }
            "--scope-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --scope-ref` requires a value")?;
                request.scope_ref = Some(value.to_string());
                index += 2;
            }
            "--linked-decision-id" => {
                let value = args.get(index + 1).context(
                    "`entrance nota capture-chat --linked-decision-id` requires a value",
                )?;
                request.linked_decision_id = Some(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid linked decision id `{value}`"))?,
                );
                index += 2;
            }
            other => bail!("unsupported nota capture-chat argument `{other}`"),
        }
    }

    Ok(request)
}

fn run_forge_bootstrap_dev_plan_cli() -> Result<Value> {
    let startup = bootstrap_forge_mcp_cli_state()?;
    let mut raw_plan = String::new();
    io::stdin()
        .read_to_string(&mut raw_plan)
        .context("failed to read bootstrap dev task plan from stdin")?;
    run_forge_bootstrap_dev_task(startup.paths().app_data_dir(), &raw_plan)
}

fn build_mcp_server(
    startup: &StartupState,
    transport: McpTransport,
    actor_role: Option<ActorRole>,
) -> Result<McpServer> {
    let data_store = startup.data_store();
    let event_bus = EventBus::new();

    Ok(McpServer::with_actor_role(
        transport,
        McpPluginSet {
            core_data_store: Some(data_store.clone()),
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
        actor_role,
    ))
}

fn normalize_http_endpoint(raw: &str) -> Result<String> {
    let endpoint = raw.trim();
    if endpoint.is_empty() {
        bail!("MCP HTTP endpoint must not be empty");
    }

    if endpoint.starts_with('/') {
        Ok(endpoint.to_string())
    } else {
        Ok(format!("/{endpoint}"))
    }
}

fn parse_mcp_actor_role_args(args: &[String]) -> Result<Option<ActorRole>> {
    match args {
        [] => Ok(None),
        [flag, value] if flag == "--actor-role" => Ok(Some(parse_mcp_actor_role(value)?)),
        [other, ..] => bail!("unsupported MCP stdio argument `{other}`"),
    }
}

fn parse_mcp_actor_role(value: &str) -> Result<ActorRole> {
    match value.trim() {
        "nota" => Ok(ActorRole::Nota),
        "arch" => Ok(ActorRole::Arch),
        "dev" => Ok(ActorRole::Dev),
        other => bail!("unsupported MCP actor role `{other}`, expected `nota`, `arch`, or `dev`"),
    }
}

#[tauri::command]
fn dashboard_summary(
    dashboard: tauri::State<'_, DashboardUiState>,
    data_store: tauri::State<'_, core::data_store::DataStore>,
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

pub(crate) fn build_nota_runtime_overview(
    data_store: &core::data_store::DataStore,
) -> Result<NotaRuntimeOverview> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current);
    let checkpoint_scope_ids = active_checkpoint_scope_ids(data_store, current_checkpoint)?;
    let transactions = list_nota_runtime_transactions(data_store)?;
    let allocations = list_nota_runtime_allocations(data_store)?;
    let receipts = list_nota_runtime_receipts(data_store, None)?;
    let human_rounds = list_runtime_human_rounds(data_store)?;
    let acceptance_bundles = list_runtime_acceptance_bundles(data_store)?;
    let current_human_round = derive_current_runtime_human_round(data_store)?;
    let visions = list_nota_visions(data_store)?;
    let todos = list_nota_todos(data_store)?;
    let recommended_checkpoint = recommend_runtime_closure_checkpoint(
        data_store,
        allocations.stored_allocations(),
        current_checkpoint,
    )?;
    let handout = derive_current_runtime_handout(data_store)?;
    let wake_request = derive_current_runtime_wake_request(data_store)?;
    let review = derive_nota_runtime_review(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let integrate = derive_nota_runtime_integrate(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let finalize = derive_nota_runtime_finalize(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let next_step = derive_nota_runtime_next_step(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let current_acceptance_bundle =
        derive_current_runtime_acceptance_bundle(data_store, &checkpoint_scope_ids)?;
    let projection_truth_revision = build_projection_truth_revision(
        current_checkpoint.map(|checkpoint| checkpoint.cadence_object.id),
        current_human_round
            .as_ref()
            .map(|round| round.cadence_object.id),
        current_acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.cadence_object.id),
    );
    let projections =
        build_projection_status_report(data_store, projection_truth_revision.clone())?;
    let cold_docs = list_cold_documents(data_store, projection_truth_revision)?;
    let host = current_runtime_host(data_store)?;
    let worktrees = list_owned_worktrees(
        data_store,
        host.as_ref().map(|value| value.host_key.as_str()),
    )?;
    let recovery = build_recovery_status_report(data_store)?;
    let round_state = derive_runtime_round_state_projection(
        current_checkpoint,
        current_acceptance_bundle.as_ref(),
        next_step.as_ref(),
    );
    let anti_zeno = derive_anti_zeno_projection(
        current_checkpoint,
        current_acceptance_bundle.as_ref(),
        next_step.as_ref(),
        recommended_checkpoint.as_ref(),
    );
    let anti_zeno_budget = build_anti_zeno_budget_report(
        data_store,
        round_state.checkpoint_id,
        round_state.acceptance_bundle_id,
        round_state.acceptance_present,
        round_state.fully_settled,
        round_state.next_step_open,
        projections.dirty_required_target_count,
    )?;
    let (invariants, repair_lane) = project_runtime_invariants(data_store)?;
    let decisions = list_design_decisions(data_store)?;
    let front_door = build_nota_front_door_projection(
        current_checkpoint,
        decisions.decision_count,
        transactions.transaction_count,
        allocations.allocation_count,
        receipts.receipt_count,
        &anti_zeno,
        recommended_checkpoint.as_ref(),
        review.as_ref(),
        integrate.as_ref(),
        finalize.as_ref(),
        next_step.as_ref(),
    );

    Ok(NotaRuntimeOverview {
        chat_policy: get_chat_archive_policy(data_store, None, None)?,
        checkpoints,
        human_rounds,
        acceptance_bundles,
        transactions,
        allocations,
        visions,
        todos,
        cold_docs,
        host,
        worktrees,
        recovery,
        recommended_checkpoint,
        handout,
        wake_request,
        review,
        integrate,
        finalize,
        next_step,
        round_state,
        anti_zeno,
        anti_zeno_budget,
        front_door,
        projections,
        invariants,
        repair_lane,
        decisions,
        chat_captures: list_chat_captures(data_store)?,
    })
}

pub(crate) fn build_nota_runtime_status(
    data_store: &core::data_store::DataStore,
) -> Result<NotaRuntimeStatus> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .cloned();
    let checkpoint_scope_ids =
        active_checkpoint_scope_ids(data_store, current_checkpoint.as_ref())?;
    let transactions = list_nota_runtime_transactions(data_store)?;
    let allocations = list_nota_runtime_allocations(data_store)?;
    let receipts = list_nota_runtime_receipts(data_store, None)?;
    let human_rounds = list_runtime_human_rounds(data_store)?;
    let acceptance_bundles = list_runtime_acceptance_bundles(data_store)?;
    let decisions = list_design_decisions(data_store)?;
    let chat_captures = list_chat_captures(data_store)?;
    let visions = list_nota_visions(data_store)?;
    let todos = list_nota_todos(data_store)?;
    let recommended_checkpoint = recommend_runtime_closure_checkpoint(
        data_store,
        allocations.stored_allocations(),
        current_checkpoint.as_ref(),
    )?;
    let handout = derive_current_runtime_handout(data_store)?;
    let wake_request = derive_current_runtime_wake_request(data_store)?;
    let review = derive_nota_runtime_review(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let integrate = derive_nota_runtime_integrate(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let finalize = derive_nota_runtime_finalize(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let next_step = derive_nota_runtime_next_step(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let current_human_round = derive_current_runtime_human_round(data_store)?;
    let current_acceptance_bundle =
        derive_current_runtime_acceptance_bundle(data_store, &checkpoint_scope_ids)?;
    let projection_truth_revision = build_projection_truth_revision(
        current_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.cadence_object.id),
        current_human_round
            .as_ref()
            .map(|round| round.cadence_object.id),
        current_acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.cadence_object.id),
    );
    let projections =
        build_projection_status_report(data_store, projection_truth_revision.clone())?;
    let cold_docs = list_cold_documents(data_store, projection_truth_revision)?;
    let host = current_runtime_host(data_store)?;
    let worktrees = list_owned_worktrees(
        data_store,
        host.as_ref().map(|value| value.host_key.as_str()),
    )?;
    let recovery = build_recovery_status_report(data_store)?;
    let round_state = derive_runtime_round_state_projection(
        current_checkpoint.as_ref(),
        current_acceptance_bundle.as_ref(),
        next_step.as_ref(),
    );
    let anti_zeno = derive_anti_zeno_projection(
        current_checkpoint.as_ref(),
        current_acceptance_bundle.as_ref(),
        next_step.as_ref(),
        recommended_checkpoint.as_ref(),
    );
    let anti_zeno_budget = build_anti_zeno_budget_report(
        data_store,
        round_state.checkpoint_id,
        round_state.acceptance_bundle_id,
        round_state.acceptance_present,
        round_state.fully_settled,
        round_state.next_step_open,
        projections.dirty_required_target_count,
    )?;
    let (invariants, repair_lane) = project_runtime_invariants(data_store)?;
    let front_door = build_nota_front_door_projection(
        current_checkpoint.as_ref(),
        decisions.decision_count,
        transactions.transaction_count,
        allocations.allocation_count,
        receipts.receipt_count,
        &anti_zeno,
        recommended_checkpoint.as_ref(),
        review.as_ref(),
        integrate.as_ref(),
        finalize.as_ref(),
        next_step.as_ref(),
    );

    Ok(NotaRuntimeStatus {
        chat_policy: get_chat_archive_policy(data_store, None, None)?,
        human_round_count: human_rounds.human_round_count,
        current_human_round,
        checkpoint_count: checkpoints.checkpoint_count,
        current_checkpoint_id: checkpoints.current_checkpoint_id,
        current_checkpoint,
        acceptance_bundle_count: acceptance_bundles.acceptance_bundle_count,
        current_acceptance_bundle,
        transaction_count: transactions.transaction_count,
        latest_transaction: transactions.transactions.first().cloned(),
        allocation_count: allocations.allocation_count,
        latest_allocation: allocations.allocations.first().cloned(),
        receipt_count: receipts.receipt_count,
        latest_receipt: receipts.receipts.last().cloned(),
        decision_count: decisions.decision_count,
        latest_decision: decisions.decisions.first().cloned(),
        chat_capture_count: chat_captures.capture_count,
        vision_count: visions.vision_count,
        todo_count: todos.todo_count,
        cold_doc_count: cold_docs.cold_doc_count,
        cold_docs,
        host,
        worktree_count: worktrees.worktree_count,
        worktrees,
        recovery,
        recommended_checkpoint,
        handout,
        wake_request,
        review,
        integrate,
        finalize,
        next_step,
        round_state,
        anti_zeno,
        anti_zeno_budget,
        front_door,
        projections,
        invariants,
        repair_lane,
    })
}

fn write_hot_root_projection(
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

fn rebuild_nota_projections(
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

fn refresh_runtime_invariant_truth(data_store: &core::data_store::DataStore) -> Result<()> {
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
                "{} on checkpoint {}",
                round.payload.round_state, round.payload.checkpoint_id
            )
        })
        .unwrap_or_else(|| "No current human round is materialized yet.".to_string());
    let acceptance_line = status
        .current_acceptance_bundle
        .as_ref()
        .map(|bundle| {
            format!(
                "{} on allocation {} ({})",
                bundle.payload.acceptance_kind,
                bundle.payload.allocation_id,
                bundle.payload.round_state
            )
        })
        .unwrap_or_else(|| "No formal acceptance bundle is current.".to_string());
    let next_step_line = status
        .next_step
        .as_ref()
        .map(|step| format!("{} for allocation {}", step.step, step.allocation_id))
        .unwrap_or_else(|| "No follow-on runtime step is currently open.".to_string());
    let round_state_line = format!(
        "{} ({})",
        status.round_state.state, status.round_state.summary
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
    let owner_root = startup.paths().app_data_dir().display().to_string();
    let config_path = startup.paths().config_path().display().to_string();
    let db_path = startup.paths().db_path().display().to_string();
    let host_line = status
        .host
        .as_ref()
        .map(|host| format!("{} on {}", host.host_label, host.os_family))
        .unwrap_or_else(|| "No host snapshot has been recorded yet.".to_string());

    let readme = format!(
        "# Top Layer\n\n> Status: exported hot root from DB-first runtime truth\n\nThe top layer is a retained projection, not an authoring authority.\n\nActive hot-root files:\n\n- [machine.md](./machine.md)\n- [control.md](./control.md)\n- [truth.md](./truth.md)\n- [phase-todo.md](./phase-todo.md)\n- [pending.md](./pending.md)\n\nCurrent owner root:\n\n- `{owner_root}`\n- host: {host_line}\n- config: `{config_path}`\n- runtime DB: `{db_path}`\n- exported hot root: `{}`\n- observed worktrees: {}\n\nCurrent round:\n\n- human round: {human_round_line}\n- round state: {round_state_line}\n- checkpoint: {checkpoint_label}\n- stable level: {checkpoint_level}\n- acceptance: {acceptance_line}\n- anti-Zeno: {} ({})\n- anti-Zeno budget: {} ({})\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- recovery: {recovery_line}\n- next step: {next_step_line}\n- projection freshness: {projection_line}\n\nProjection law:\n\n- DB is the only canonical writer.\n- README, hot root, cold docs, GUI, CLI, and MCP are projections from DB truth.\n- `passed human round = acceptance`.\n- `fully settled round = acceptance + no next_step + checkpoint carry-forward`.\n",
        startup.paths().exports_dir().join("hot-root").display(),
        status.worktree_count,
        status.anti_zeno.summary,
        status.anti_zeno.state,
        status.anti_zeno_budget.summary,
        status.anti_zeno_budget.state
    );

    let machine = format!(
        "# Machine\n\n> Status: hot root projection\n\n## Current Runtime Cut\n\n- current human round: {human_round_line}\n- round state: {round_state_line}\n- current checkpoint: {checkpoint_label}\n- stable level: {checkpoint_level}\n- acceptance bundle count: {}\n- current acceptance: {acceptance_line}\n- anti-Zeno state: {} ({})\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n\n## State Law\n\n- runtime continuity is resumed from checkpoint, human-round, allocation, receipt, and cadence-object truth\n- `passed human round` is formalized as `CADENCE_ACCEPTANCE_BUNDLE`\n- `fully settled round` is stricter than acceptance and only holds after follow-on closure has been carried forward\n- phase is projection, not a peer truth plane\n",
        status.acceptance_bundle_count, status.anti_zeno.state, status.anti_zeno.summary
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
        "# Phase Todo\n\n> Status: hot root projection\n\n## Current Focus\n\n- current checkpoint: {checkpoint_label}\n- acceptance: {acceptance_line}\n- anti-Zeno: {} ({})\n- anti-Zeno budget: {} ({})\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- next step: {next_step_line}\n- projection freshness: {projection_line}\n\n## Ordered Work\n\n- keep runtime truth sharper than file projections\n- keep acceptance formalized as a cadence object rather than chat implication\n- keep anti-Zeno visible in status, overview, and exported hot root\n- keep invariant failure and repair lane truth explicit in DB\n- keep hot-root export synchronized from DB truth after human-round writes\n",
        status.anti_zeno.state,
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

pub(crate) fn list_nota_todos(
    data_store: &core::data_store::DataStore,
) -> Result<NotaTodoListReport> {
    let todos = data_store.list_todo_records()?;
    Ok(NotaTodoListReport {
        todo_count: todos.len(),
        todos,
    })
}

pub(crate) fn list_nota_visions(
    data_store: &core::data_store::DataStore,
) -> Result<NotaVisionListReport> {
    let visions = data_store.list_vision_records()?;
    Ok(NotaVisionListReport {
        vision_count: visions.len(),
        visions,
    })
}

#[tauri::command]
fn nota_runtime_overview(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<NotaRuntimeOverview, String> {
    build_nota_runtime_overview(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn nota_runtime_status(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<NotaRuntimeStatus, String> {
    build_nota_runtime_status(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_import_snapshot(
    path: String,
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<LandingImportReport, String> {
    import_linear_entrance_snapshot(&data_store, path).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_ingest_runs(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<StoredSourceIngestRun>, String> {
    list_landing_ingest_runs(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_mirror_items(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<LandingMirrorSummary>, String> {
    list_landing_mirror_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_planning_items(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<LandingPlanningItemSummary>, String> {
    list_landing_planning_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_unreconciled_items(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<LandingPlanningItemSummary>, String> {
    list_landing_unreconciled_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn hygiene_list_spec_v0(
    data_store: tauri::State<'_, core::data_store::DataStore>,
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

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("failed to serialize CLI output")?
    );
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
            dashboard_summary,
            nota_runtime_overview,
            nota_runtime_status,
            landing_import_snapshot,
            landing_list_ingest_runs,
            landing_list_mirror_items,
            landing_list_planning_items,
            landing_list_unreconciled_items,
            hygiene_list_spec_v0,
            core::theme::get_theme,
            core::theme::set_theme,
            launcher_search,
            launcher_launch,
            launcher_pin,
            forge_create_task,
            forge_dispatch_agent,
            forge_prepare_agent_dispatch,
            forge_list_tasks,
            forge_get_task,
            forge_get_task_details,
            forge_cancel_task,
            vault_list_tokens,
            vault_add_token,
            vault_upsert_token,
            vault_delete_token,
            vault_get_token,
            vault_get_token_by_provider,
            vault_list_mcp,
            vault_update_mcp
        ])
        .run(tauri::generate_context!())
        .expect("error while running Entrance application");
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::{OsStr, OsString},
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::core::config_store::{render_config, EntranceConfig};
    use crate::core::data_store::{DataStore, MigrationPlan};

    use super::{
        build_nota_runtime_status, cli_help_for_args, prepare_forge_dispatch_cli,
        verify_forge_dispatch_cli, FORGE_CLI_HELP, MCP_CLI_HELP, NOTA_CLI_HELP, ROOT_CLI_HELP,
    };

    static CLI_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TestDir {
        path: PathBuf,
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "entrance-lib-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test temp directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
            Self { key, original }
        }

        fn remove(key: &'static str) -> Self {
            let original = env::var_os(key);
            env::remove_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    fn cli_test_guard() -> std::sync::MutexGuard<'static, ()> {
        CLI_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("CLI test lock should not be poisoned")
    }

    #[test]
    fn cli_help_is_available_without_falling_back_to_gui() {
        let root = vec!["--help".to_string()];
        assert_eq!(cli_help_for_args(&root), Some(ROOT_CLI_HELP));

        let nota = vec!["nota".to_string(), "--help".to_string()];
        assert_eq!(cli_help_for_args(&nota), Some(NOTA_CLI_HELP));

        let mcp = vec!["mcp".to_string(), "--help".to_string()];
        assert_eq!(cli_help_for_args(&mcp), Some(MCP_CLI_HELP));

        let mcp_stdio = vec!["mcp".to_string(), "stdio".to_string(), "--help".to_string()];
        assert_eq!(cli_help_for_args(&mcp_stdio), Some(MCP_CLI_HELP));

        let forge = vec!["forge".to_string(), "--help".to_string()];
        assert_eq!(cli_help_for_args(&forge), Some(FORGE_CLI_HELP));
    }

    #[test]
    fn nota_status_can_project_runtime_invariants_on_readonly_store() -> Result<()> {
        let temp_dir = TestDir::new("nota-status-readonly");
        let db_path = temp_dir.path().join("data").join("entrance.db");
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let migration_plan = MigrationPlan::new(crate::plugins::forge::migrations());
        let writable_store = DataStore::open(&db_path, migration_plan)?;
        drop(writable_store);

        let migration_plan = MigrationPlan::new(crate::plugins::forge::migrations());
        let readonly_store = DataStore::open_read_only(&db_path, migration_plan)?;
        let status = build_nota_runtime_status(&readonly_store)?;

        assert_eq!(status.invariants.failed_count, 1);
        assert_eq!(status.repair_lane.open_count, 1);
        assert!(status
            .invariants
            .invariants
            .iter()
            .any(|invariant| invariant.invariant_key == "runtime_host_snapshot"));

        Ok(())
    }

    fn init_git_repo(path: &Path) {
        let output = std::process::Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(path)
            .output()
            .expect("git init should run");
        assert!(
            output.status.success(),
            "git init should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn prepare_forge_dispatch_cli_works_without_agents_runtime() -> Result<()> {
        let _guard = cli_test_guard();

        let temp_dir = TestDir::new("forge-cli-no-agents");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let dispatch = prepare_forge_dispatch_cli(Some(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8")
                .to_string(),
        ))?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.issue_status, "Todo");
        assert_eq!(dispatch.issue_status_source, "fallback");
        assert!(dispatch.issue_title.is_none());
        assert_eq!(
            dispatch.prompt_source,
            "Entrance-owned harness/bootstrap prompt"
        );
        assert_eq!(
            dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(dispatch.prompt.contains("harness/bootstrap/duet/SKILL.md"));
        assert!(!dispatch.prompt.contains(".agents"));

        Ok(())
    }

    #[test]
    fn prepare_forge_dispatch_cli_requires_enabled_forge_plugin() -> Result<()> {
        let _guard = cli_test_guard();

        let temp_dir = TestDir::new("forge-cli-disabled");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);

        fs::create_dir_all(&app_data_dir)?;
        fs::write(
            app_data_dir.join("entrance.toml"),
            render_config(&EntranceConfig::default())?,
        )?;

        let error = prepare_forge_dispatch_cli(None).expect_err("forge-disabled CLI should fail");
        assert!(error.to_string().contains("Forge is disabled"));

        Ok(())
    }

    #[test]
    fn verify_forge_dispatch_cli_persists_task_without_agents_runtime() -> Result<()> {
        let _guard = cli_test_guard();

        let temp_dir = TestDir::new("forge-cli-verify-no-agents");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let report = verify_forge_dispatch_cli(Some(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8")
                .to_string(),
        ))?;

        assert_eq!(report.dispatch.issue_id, "MYT-48");
        assert_eq!(report.dispatch.issue_status, "Todo");
        assert_eq!(
            report.dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(!report.dispatch.prompt.contains(".agents"));
        assert!(report.task_id > 0);
        assert_eq!(report.task_status, "Pending");
        assert_eq!(report.task_command, "codex");
        assert_eq!(
            report.task_working_dir.as_deref(),
            Some(report.dispatch.worktree_path.as_str())
        );
        assert!(report.prompt_via_stdin);

        Ok(())
    }
}
