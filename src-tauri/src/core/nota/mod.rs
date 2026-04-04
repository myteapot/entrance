mod helpers;
mod policy;
mod types;

use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;

use crate::core::action::{ActionPrimitive, ActionRecord, ActionRoom, ActorRole, KnowledgeLayer};
use crate::core::compiler::{
    admission::{admit_dispatch, AdmittedDispatch},
    evidence::{collect_task_evidence, derive_verdict},
    lowering::{
        lower_dispatch, DispatchLineage, DispatchRouting, LoweredDispatch, LoweringContext,
        SandboxConfig,
    },
    packet::TypedActionPacket,
    routing::{resolve_return_route, ReturnBoundary, TerminalStatus},
};
use crate::core::data_store::{
    DataStore, DataStoreTransaction, NewCadenceLink, NewCadenceObject, NewNotaRuntimeAllocation,
    NewNotaRuntimeReceipt, NewNotaRuntimeTransaction, NotaRuntimeAllocationUpdate,
    NotaRuntimeTransactionUpdate, StoredCadenceObject, StoredForgeTask,
    StoredNotaRuntimeAllocation, StoredNotaRuntimeReceipt, StoredNotaRuntimeTransaction,
};
use crate::core::invariant_runtime::refresh_runtime_invariants;
use crate::core::supervision::{
    build_runtime_supervision_incident_summary, derive_runtime_supervision_projection_with_budget,
};
use crate::plugins::forge::ForgePlugin;

use helpers::*;
use policy::*;
pub use types::*;
use types::{
    AgentReturnAcceptedReceiptPayload, AllocationTerminalOutcomeReceiptPayload, DevRepairOrigin,
    DevReturnAcceptedReceiptPayload, DevReturnFinalizeRecordedReceiptPayload,
    DevReturnIntegrateRecordedReceiptPayload, DevReturnReviewReadyReceiptPayload,
    DevReturnReviewRecordedReceiptPayload, DoAskRecordedReceiptPayload,
    DoClarificationRecordedReceiptPayload, HumanRoundCanonicalState, HumanRoundDetailState,
    NotaDispatchLane, RecommendedCheckpointCandidate, RecommendedCheckpointCandidateKind,
    RuntimeBoundaryLane,
};

const CADENCE_CHECKPOINT_KIND: &str = "CADENCE_CHECKPOINT";
const CADENCE_HUMAN_ROUND_KIND: &str = "CADENCE_HUMAN_ROUND";
const CADENCE_ACCEPTANCE_BUNDLE_KIND: &str = "CADENCE_ACCEPTANCE_BUNDLE";
const CADENCE_HANDOUT_KIND: &str = "CADENCE_HANDOUT";
const CADENCE_WAKE_REQUEST_KIND: &str = "CADENCE_WAKE_REQUEST";
const CADENCE_POLICY_NOTE_KIND: &str = "CADENCE_POLICY_NOTE";
const NOTA_RUNTIME_SOURCE_TYPE: &str = "nota_runtime";
const NOTA_RUNTIME_SCOPE_TYPE: &str = "runtime";
const NOTA_RUNTIME_SCOPE_REF: &str = "Entrance";
const CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND: &str = "CADENCE_CHECKPOINT_WRITTEN";
const AGENT_RETURN_ACCEPTED_RECEIPT_KIND: &str = "AGENT_RETURN_ACCEPTED";
const DEV_RETURN_ACCEPTED_RECEIPT_KIND: &str = "DEV_RETURN_ACCEPTED";
const DO_CLARIFICATION_RECORDED_RECEIPT_KIND: &str = "DO_CLARIFICATION_RECORDED";
const DO_ASK_RECORDED_RECEIPT_KIND: &str = "DO_ASK_RECORDED";
const DEV_RETURN_REVIEW_READY_RECEIPT_KIND: &str = "DEV_RETURN_REVIEW_READY";
const DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND: &str = "DEV_RETURN_REVIEW_RECORDED";
const DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND: &str = "DEV_RETURN_INTEGRATE_RECORDED";
const DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND: &str = "DEV_RETURN_FINALIZE_RECORDED";
const DEV_REPAIR_FOLLOWUP_RECORDED_RECEIPT_KIND: &str = "DEV_REPAIR_FOLLOWUP_RECORDED";
const ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND: &str =
    "ALLOCATION_TERMINAL_OUTCOME_RECORDED";
const DEV_RETURN_REVIEW_APPROVED_VERDICT: &str = "approved";
const DEV_RETURN_REVIEW_CHANGES_REQUESTED_VERDICT: &str = "changes_requested";
const DEV_RETURN_INTEGRATE_STARTED_STATE: &str = "started";
const DEV_RETURN_INTEGRATE_INTEGRATED_STATE: &str = "integrated";
const DEV_RETURN_INTEGRATE_REPAIR_REQUESTED_STATE: &str = "repair_requested";
const DEV_RETURN_INTEGRATE_STARTED_RUNTIME_STATE: &str = "integrate_started";
const DEV_RETURN_INTEGRATE_RECORDED_RUNTIME_STATE: &str = "integrate_recorded";
const DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE: &str = "closed";
const HUMAN_ROUND_ACCEPTANCE_KIND: &str = "human_round_acceptance";
const NOTA_DO_CLARIFICATION_TRANSACTION_KIND: &str = "nota_do_clarification";
const NOTA_DO_ASK_TRANSACTION_KIND: &str = "nota_do_ask";
const NOTA_BOUNDARY_EXECUTION_HOST: &str = "boundary";
const CLARIFICATION_OPEN_TRANSACTION_STATUS: &str = "clarification_open";
const ASK_OPEN_TRANSACTION_STATUS: &str = "ask_open";
const BOUNDARY_INTAKE_SUPERSEDED_TRANSACTION_STATUS: &str = "superseded";

// All struct/enum/impl type definitions moved to types.rs

pub fn write_runtime_checkpoint(
    data_store: &DataStore,
    request: NotaCheckpointRequest,
) -> Result<NotaCheckpointWriteReport> {
    let report = data_store.with_immediate_transaction(|transaction| {
        write_runtime_checkpoint_in_transaction(transaction, request)
    })?;
    sync_runtime_truth(data_store, None)?;
    Ok(report)
}

fn write_runtime_checkpoint_in_transaction(
    transaction: &DataStoreTransaction<'_>,
    request: NotaCheckpointRequest,
) -> Result<NotaCheckpointWriteReport> {
    let stable_level = request.stable_level.trim().to_string();
    if stable_level.is_empty() {
        return Err(anyhow!("`stable_level` must not be empty"));
    }

    let landed = normalize_list(request.landed);
    if landed.is_empty() {
        return Err(anyhow!("at least one `landed` item is required"));
    }

    let remaining = normalize_list(request.remaining);
    let human_continuity_bus = request.human_continuity_bus.trim().to_string();
    if human_continuity_bus.is_empty() {
        return Err(anyhow!("`human_continuity_bus` must not be empty"));
    }

    let selected_trunk = normalize_optional(request.selected_trunk.as_deref());
    let next_start_hints = normalize_list(request.next_start_hints);
    let repo_context = request
        .project_dir
        .as_deref()
        .map(capture_repo_context)
        .transpose()?;

    let payload = NotaCheckpointPayload {
        stable_level: stable_level.clone(),
        landed: landed.clone(),
        remaining,
        human_continuity_bus,
        selected_trunk,
        next_start_hints,
        repo_context,
    };
    let payload_json =
        serde_json::to_string(&payload).context("failed to serialize nota checkpoint payload")?;

    let superseded_checkpoint = transaction
        .list_cadence_objects_by_kind(CADENCE_CHECKPOINT_KIND)?
        .into_iter()
        .find(|object| object.is_current);

    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("NOTA runtime checkpoint: {stable_level}"));
    let summary = build_checkpoint_summary(&stable_level, &landed);
    let cadence_object = transaction.insert_cadence_object(NewCadenceObject {
        cadence_kind: CADENCE_CHECKPOINT_KIND,
        title: &title,
        summary: &summary,
        payload_json: &payload_json,
        scope_type: NOTA_RUNTIME_SCOPE_TYPE,
        scope_ref: NOTA_RUNTIME_SCOPE_REF,
        source_type: NOTA_RUNTIME_SOURCE_TYPE,
        source_ref: "nota_cli:checkpoint",
        admission_policy: admission_policy_for_kind(CADENCE_CHECKPOINT_KIND),
        projection_policy: projection_policy_for_kind(CADENCE_CHECKPOINT_KIND),
        status: "active",
        is_current: true,
    })?;

    let supersession_link = if let Some(previous) = superseded_checkpoint.as_ref() {
        Some(transaction.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: previous.id,
            dst_cadence_object_id: cadence_object.id,
            relation_type: "superseded_by",
            status: "active",
        })?)
    } else {
        None
    };

    transaction.insert_anti_zeno_event(crate::core::data_store::NewAntiZenoEvent {
        checkpoint_id: Some(cadence_object.id),
        acceptance_bundle_id: None,
        event_kind: "checkpoint_written",
        boundary_ref: "checkpoint",
        budget_axis: "semantic",
        event_weight: 1,
        summary: &summary,
    })?;

    Ok(NotaCheckpointWriteReport {
        checkpoint: NotaCheckpointRecord {
            cadence_object,
            payload,
        },
        superseded_checkpoint_id: superseded_checkpoint.map(|object| object.id),
        supersession_link,
    })
}

pub fn list_runtime_checkpoints(data_store: &DataStore) -> Result<NotaCheckpointListReport> {
    let checkpoints = data_store
        .list_cadence_objects_by_kind(CADENCE_CHECKPOINT_KIND)?
        .into_iter()
        .map(parse_checkpoint_record)
        .collect::<Result<Vec<_>>>()?;
    let current_checkpoint_id = checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .map(|checkpoint| checkpoint.cadence_object.id);

    Ok(NotaCheckpointListReport {
        checkpoint_count: checkpoints.len(),
        current_checkpoint_id,
        checkpoints,
    })
}

pub fn list_runtime_acceptance_bundles(
    data_store: &DataStore,
) -> Result<NotaAcceptanceBundleListReport> {
    let acceptance_bundles = data_store
        .list_cadence_objects_by_kind(CADENCE_ACCEPTANCE_BUNDLE_KIND)?
        .into_iter()
        .map(parse_acceptance_bundle_record)
        .collect::<Result<Vec<_>>>()?;
    let current_acceptance_bundle_id = acceptance_bundles
        .iter()
        .find(|bundle| bundle.cadence_object.is_current)
        .map(|bundle| bundle.cadence_object.id);

    Ok(NotaAcceptanceBundleListReport {
        acceptance_bundle_count: acceptance_bundles.len(),
        current_acceptance_bundle_id,
        acceptance_bundles,
    })
}

pub fn list_runtime_human_rounds(data_store: &DataStore) -> Result<NotaHumanRoundListReport> {
    let human_rounds = data_store
        .list_cadence_objects_by_kind(CADENCE_HUMAN_ROUND_KIND)?
        .into_iter()
        .map(parse_human_round_record)
        .collect::<Result<Vec<_>>>()?;
    let current_human_round_id = human_rounds
        .iter()
        .find(|round| round.cadence_object.is_current)
        .map(|round| round.cadence_object.id);

    Ok(NotaHumanRoundListReport {
        human_round_count: human_rounds.len(),
        current_human_round_id,
        human_rounds,
    })
}

pub fn derive_current_runtime_human_round(
    data_store: &DataStore,
) -> Result<Option<NotaHumanRoundRecord>> {
    Ok(list_runtime_human_rounds(data_store)?
        .human_rounds
        .into_iter()
        .find(|round| round.cadence_object.is_current))
}

pub fn derive_current_runtime_handout(data_store: &DataStore) -> Result<Option<NotaHandoutRecord>> {
    data_store
        .list_cadence_objects_by_kind(CADENCE_HANDOUT_KIND)?
        .into_iter()
        .find(|object| object.is_current)
        .map(parse_handout_record)
        .transpose()
}

pub fn derive_current_runtime_wake_request(
    data_store: &DataStore,
) -> Result<Option<NotaWakeRequestRecord>> {
    Ok(data_store
        .list_cadence_objects_by_kind(CADENCE_WAKE_REQUEST_KIND)?
        .into_iter()
        .find(|object| object.is_current)
        .map(parse_wake_request_record)
        .transpose()?
        .filter(|record| record.cadence_object.status != "resolved"))
}

pub fn derive_current_runtime_acceptance_bundle(
    data_store: &DataStore,
    checkpoint_scope_ids: &[i64],
) -> Result<Option<NotaAcceptanceBundleRecord>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    Ok(list_runtime_acceptance_bundles(data_store)?
        .acceptance_bundles
        .into_iter()
        .filter(|bundle| {
            checkpoint_scope_contains(checkpoint_scope_ids, bundle.payload.checkpoint_id)
        })
        .max_by_key(|bundle| (bundle.cadence_object.is_current, bundle.cadence_object.id)))
}

pub(crate) fn active_checkpoint_scope_ids(
    data_store: &DataStore,
    current_checkpoint: Option<&NotaCheckpointRecord>,
) -> Result<Vec<i64>> {
    let Some(current_checkpoint) = current_checkpoint else {
        return Ok(Vec::new());
    };

    let links = data_store.list_cadence_links()?;
    let mut scope_ids = vec![current_checkpoint.cadence_object.id];
    let mut seen = HashSet::from([current_checkpoint.cadence_object.id]);
    let mut frontier = vec![current_checkpoint.cadence_object.id];

    while let Some(checkpoint_id) = frontier.pop() {
        for link in links.iter().filter(|link| {
            link.status == "active"
                && link.relation_type == "superseded_by"
                && link.dst_cadence_object_id == checkpoint_id
        }) {
            if seen.insert(link.src_cadence_object_id) {
                scope_ids.push(link.src_cadence_object_id);
                frontier.push(link.src_cadence_object_id);
            }
        }
    }

    Ok(scope_ids)
}

fn resolve_dev_repair_origin(
    data_store: &DataStore,
    allocation_id: i64,
) -> Result<DevRepairOrigin> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .cloned()
        .context("dev repair follow-up requires a current runtime checkpoint")?;
    let checkpoint_scope_ids = active_checkpoint_scope_ids(data_store, Some(&current_checkpoint))?;
    let transactions = list_nota_runtime_transactions(data_store)?;
    let allocations = list_nota_runtime_allocations(data_store)?;
    let receipts = list_nota_runtime_receipts(data_store, None)?;
    let next_step = derive_nota_runtime_next_step(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?
    .context("dev repair follow-up requires an active repair next step")?;

    if next_step.step != "repair" || next_step.allocation_id != allocation_id {
        bail!(
            "runtime allocation `{allocation_id}` is not the active repair boundary on the current checkpoint"
        );
    }

    let allocation = allocations
        .stored_allocations()
        .iter()
        .find(|allocation| allocation.id == allocation_id)
        .cloned()
        .with_context(|| format!("runtime allocation `{allocation_id}` was not found"))?;
    if allocation.allocation_kind != "forge_dev_dispatch" {
        bail!("runtime allocation `{allocation_id}` is not a dev dispatch boundary");
    }
    if allocation.status != "return_ready" {
        bail!(
            "runtime allocation `{allocation_id}` cannot seed a repair follow-up because status is `{}`",
            allocation.status
        );
    }

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse dev repair origin payload for allocation {}",
                allocation.id
            )
        })?;
    let outcome = payload
        .terminal_outcome
        .as_ref()
        .context("dev repair follow-up requires a terminal outcome on the source allocation")?;
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        bail!("runtime allocation `{allocation_id}` is not a returned Done dev boundary");
    }

    Ok(DevRepairOrigin {
        allocation_id: allocation.id,
        transaction_id: allocation.source_transaction_id,
        lineage_ref: allocation.lineage_ref,
        project_dir: payload.project_root,
    })
}

pub fn run_nota_do_agent_dispatch(
    data_store: &DataStore,
    forge: &ForgePlugin,
    request: NotaDoAgentDispatchRequest,
) -> Result<NotaDoDispatchReport> {
    run_nota_dispatch(data_store, forge, request, NotaDispatchLane::Agent)
}

pub fn run_nota_dev_dispatch(
    data_store: &DataStore,
    forge: &ForgePlugin,
    request: NotaDevDispatchRequest,
) -> Result<NotaDevDispatchReport> {
    run_nota_dispatch(data_store, forge, request, NotaDispatchLane::Dev)
}

fn run_nota_dispatch(
    data_store: &DataStore,
    forge: &ForgePlugin,
    request: NotaDoAgentDispatchRequest,
    lane: NotaDispatchLane,
) -> Result<NotaDoDispatchReport> {
    let model = request.model.trim().to_string();
    if model.is_empty() {
        return Err(anyhow!("`model` must not be empty"));
    }

    let repair_origin = match (lane, request.repair_of_allocation_id) {
        (NotaDispatchLane::Agent, Some(_)) => {
            bail!("repair follow-up is only supported on `entrance nota dev`")
        }
        (NotaDispatchLane::Dev, Some(allocation_id)) => {
            Some(resolve_dev_repair_origin(data_store, allocation_id)?)
        }
        (_, None) => None,
    };
    let dispatch_project_dir = request.project_dir.clone().or_else(|| {
        repair_origin
            .as_ref()
            .map(|origin| origin.project_dir.clone())
    });
    let dispatch = lane.prepare_dispatch(data_store, dispatch_project_dir.clone())?;
    let payload = NotaDoDispatchPayload {
        issue_id: dispatch.issue_id.clone(),
        issue_status: dispatch.issue_status.clone(),
        issue_status_source: dispatch.issue_status_source.clone(),
        issue_title: dispatch.issue_title.clone(),
        project_root: dispatch.project_root.clone(),
        worktree_path: dispatch.worktree_path.clone(),
        prompt_source: dispatch.prompt_source.clone(),
        model: model.clone(),
        agent_command: request.agent_command.clone(),
        repair_of_allocation_id: repair_origin.as_ref().map(|origin| origin.allocation_id),
        repair_of_transaction_id: repair_origin.as_ref().map(|origin| origin.transaction_id),
        repair_of_lineage_ref: repair_origin
            .as_ref()
            .map(|origin| origin.lineage_ref.clone()),
        execution_host: request.execution_host.as_str().to_string(),
    };
    let payload_json =
        serde_json::to_string(&payload).context("failed to serialize nota do payload")?;

    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if repair_origin.is_some() && lane == NotaDispatchLane::Dev {
                format!("Repair dispatch {}", dispatch.issue_id)
            } else {
                lane.default_title(&dispatch.issue_id)
            }
        });

    let mut receipts = Vec::new();
    let transaction = data_store.with_immediate_transaction(|tx| {
        let transaction = tx.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: lane.surface_action(),
            transaction_kind: lane.transaction_kind(),
            title: &title,
            payload_json: &payload_json,
            status: "accepted",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let accepted_receipt = tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: "DO_ACCEPTED",
            payload_json: &payload_json,
            status: "recorded",
        })?;
        Ok((transaction, accepted_receipt))
    })?;
    receipts.push(transaction.1.clone());
    let mut transaction = transaction.0;

    let task_request =
        lane.build_task_request(&dispatch, model.clone(), request.agent_command.clone())?;
    let task_id = forge.create_task(task_request)?;
    let task = forge
        .get_task(task_id)?
        .ok_or_else(|| anyhow!("stored Forge task disappeared after nota do dispatch"))?;
    let allocation_payload = NotaDoAllocationPayload {
        issue_id: dispatch.issue_id.clone(),
        issue_status: dispatch.issue_status.clone(),
        issue_status_source: dispatch.issue_status_source.clone(),
        issue_title: dispatch.issue_title.clone(),
        project_root: dispatch.project_root.clone(),
        worktree_path: dispatch.worktree_path.clone(),
        prompt_source: dispatch.prompt_source.clone(),
        model: model.clone(),
        agent_command: request.agent_command.clone(),
        repair_of_allocation_id: repair_origin.as_ref().map(|origin| origin.allocation_id),
        repair_of_transaction_id: repair_origin.as_ref().map(|origin| origin.transaction_id),
        repair_of_lineage_ref: repair_origin
            .as_ref()
            .map(|origin| origin.lineage_ref.clone()),
        execution_host: request.execution_host.as_str().to_string(),
        child_dispatch_role: actor_role_slug(dispatch.dispatch_role).to_string(),
        child_dispatch_tool_name: dispatch.dispatch_tool_name.clone(),
        terminal_outcome: None,
    };
    let allocation_payload_json = serde_json::to_string(&allocation_payload)
        .context("failed to serialize nota allocation payload")?;
    let child_execution_ref = task_id.to_string();
    let return_target_ref = transaction.id.to_string();
    let lineage_ref = lane.build_lineage_ref(transaction.id, task_id);
    let lowering_context =
        build_lowering_context(&transaction, task_id, &lane, &dispatch.project_root);
    let dispatch_packet = compile_nota_dispatch_packet();
    let lowered_dispatch = lower_dispatch(&dispatch_packet, &lowering_context)
        .expect("nota dispatch allocation lowering must succeed");

    debug_assert_eq!(lineage_ref, lowered_dispatch.lineage.lineage_ref);
    debug_assert_eq!(
        lane.transaction_kind(),
        lowered_dispatch.routing.allocation_kind
    );
    debug_assert_eq!("nota", lowered_dispatch.routing.allocator_role);
    debug_assert_eq!(
        "forge_task",
        lowered_dispatch.lineage.child_execution_kind.as_str()
    );
    debug_assert_eq!(
        child_execution_ref,
        lowered_dispatch.lineage.child_execution_ref
    );
    debug_assert_eq!(
        "nota_runtime_transaction",
        lowered_dispatch.lineage.return_target_kind.as_str()
    );
    debug_assert_eq!(
        return_target_ref,
        lowered_dispatch.lineage.return_target_ref
    );
    debug_assert_eq!(
        "nota_runtime_transaction",
        lowered_dispatch.lineage.escalation_target_kind.as_str()
    );
    debug_assert_eq!(
        return_target_ref,
        lowered_dispatch.lineage.escalation_target_ref
    );

    let spawn_error = launch_forge_task(forge, task_id, request.execution_host)
        .err()
        .map(|error| error.to_string());
    let task_after_spawn = forge
        .get_task(task_id)?
        .ok_or_else(|| anyhow!("stored Forge task disappeared after nota do spawn"))?;
    let transaction_status = if spawn_error.is_some() {
        "spawn_failed"
    } else {
        "dispatched"
    };
    let launch_receipt_kind = if spawn_error.is_some() {
        "FORGE_TASK_SPAWN_FAILED"
    } else {
        "FORGE_TASK_DISPATCHED"
    };
    let checkpoint_title = Some(lane.checkpoint_title(&dispatch.issue_id));
    let checkpoint_stable_level = lane.checkpoint_stable_level().to_string();
    let checkpoint_human_continuity_bus = if spawn_error.is_some() {
        "still required for operator recovery".to_string()
    } else {
        "reduced but not eliminated".to_string()
    };
    let checkpoint_selected_trunk = Some(lane.selected_trunk().to_string());
    let checkpoint_project_dir = Some(dispatch.project_root.clone());
    let forge_task_created_payload = serde_json::to_string(&json!({
        "task_id": task_id,
        "task_status": task.status,
        "task_command": task.command,
        "worktree_path": task.working_dir,
    }))
    .context("failed to serialize forge task receipt payload")?;
    let forge_launch_payload = serde_json::to_string(&json!({
        "task_id": task_id,
        "task_status": task_after_spawn.status.clone(),
        "status_message": task_after_spawn.status_message.clone(),
        "spawn_error": spawn_error.clone(),
    }))
    .context("failed to serialize forge launch receipt payload")?;

    let (updated_transaction, allocation, staged_receipts, checkpoint_report) = data_store
        .with_immediate_transaction(|tx| {
            let mut transaction = tx.update_nota_runtime_transaction(
                transaction.id,
                NotaRuntimeTransactionUpdate {
                    status: "task_created",
                    forge_task_id: Some(task_id),
                    cadence_checkpoint_id: None,
                },
            )?;
            let mut staged_receipts = Vec::new();
            let mut allocation = tx.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
                allocator_role: lowered_dispatch.routing.allocator_role.as_str(),
                allocator_surface: lane.allocator_surface(),
                allocation_kind: lowered_dispatch.routing.allocation_kind.as_str(),
                source_transaction_id: transaction.id,
                lineage_ref: lowered_dispatch.lineage.lineage_ref.as_str(),
                child_execution_kind: lowered_dispatch.lineage.child_execution_kind.as_str(),
                child_execution_ref: lowered_dispatch.lineage.child_execution_ref.as_str(),
                return_target_kind: lowered_dispatch.lineage.return_target_kind.as_str(),
                return_target_ref: lowered_dispatch.lineage.return_target_ref.as_str(),
                escalation_target_kind: lowered_dispatch.lineage.escalation_target_kind.as_str(),
                escalation_target_ref: lowered_dispatch.lineage.escalation_target_ref.as_str(),
                status: "task_created",
                payload_json: &allocation_payload_json,
            })?;
            debug_assert_eq!(allocation.lineage_ref, lowered_dispatch.lineage.lineage_ref);
            debug_assert_eq!(
                allocation.child_execution_ref,
                lowered_dispatch.lineage.child_execution_ref
            );
            debug_assert_eq!(
                allocation.return_target_ref,
                lowered_dispatch.lineage.return_target_ref
            );
            debug_assert_eq!(
                allocation.escalation_target_ref,
                lowered_dispatch.lineage.escalation_target_ref
            );
            debug_assert_eq!(
                allocation.allocation_kind,
                lowered_dispatch.routing.allocation_kind
            );
            debug_assert_eq!(
                allocation.allocator_role,
                lowered_dispatch.routing.allocator_role
            );
            staged_receipts.push(tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
                transaction_id: transaction.id,
                receipt_kind: "FORGE_TASK_CREATED",
                payload_json: &forge_task_created_payload,
                status: "recorded",
            })?);
            staged_receipts.push(
                tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
                    transaction_id: transaction.id,
                    receipt_kind: "ALLOCATION_RECORDED",
                    payload_json: &serde_json::to_string(&json!({
                        "allocation_id": allocation.id,
                        "allocator_role": allocation.allocator_role,
                        "allocator_surface": allocation.allocator_surface,
                        "allocation_kind": allocation.allocation_kind,
                        "source_transaction_id": allocation.source_transaction_id,
                        "lineage_ref": allocation.lineage_ref,
                        "child_execution_kind": allocation.child_execution_kind,
                        "child_execution_ref": allocation.child_execution_ref,
                        "return_target_kind": allocation.return_target_kind,
                        "return_target_ref": allocation.return_target_ref,
                        "escalation_target_kind": allocation.escalation_target_kind,
                        "escalation_target_ref": allocation.escalation_target_ref,
                        "status": allocation.status,
                    }))
                    .context("failed to serialize allocation receipt payload in transaction")?,
                    status: "recorded",
                })?,
            );
            if let Some(repair_origin) = repair_origin.as_ref() {
                staged_receipts.push(
                    tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
                        transaction_id: transaction.id,
                        receipt_kind: DEV_REPAIR_FOLLOWUP_RECORDED_RECEIPT_KIND,
                        payload_json: &serde_json::to_string(&json!({
                            "repair_of_allocation_id": repair_origin.allocation_id,
                            "repair_of_transaction_id": repair_origin.transaction_id,
                            "repair_of_lineage_ref": repair_origin.lineage_ref,
                            "new_transaction_id": transaction.id,
                        }))
                        .context("failed to serialize dev repair follow-up receipt payload")?,
                        status: "recorded",
                    })?,
                );
            }

            transaction = tx.update_nota_runtime_transaction(
                transaction.id,
                NotaRuntimeTransactionUpdate {
                    status: transaction_status,
                    forge_task_id: Some(task_id),
                    cadence_checkpoint_id: None,
                },
            )?;
            allocation = tx.update_nota_runtime_allocation(
                allocation.id,
                NotaRuntimeAllocationUpdate {
                    status: transaction_status,
                    payload_json: None,
                },
            )?;
            staged_receipts.push(tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
                transaction_id: transaction.id,
                receipt_kind: launch_receipt_kind,
                payload_json: &forge_launch_payload,
                status: "recorded",
            })?);

            let checkpoint_report = write_runtime_checkpoint_in_transaction(
                tx,
                NotaCheckpointRequest {
                    title: checkpoint_title.clone(),
                    stable_level: checkpoint_stable_level.clone(),
                    landed: lane.build_checkpoint_landed_items(
                        transaction.id,
                        &allocation,
                        task_id,
                        &dispatch,
                        &spawn_error,
                    ),
                    remaining: lane.build_checkpoint_remaining_items(
                        allocation.id,
                        task_id,
                        &spawn_error,
                    ),
                    human_continuity_bus: checkpoint_human_continuity_bus.clone(),
                    selected_trunk: checkpoint_selected_trunk.clone(),
                    next_start_hints: lane.build_checkpoint_hints(
                        transaction.id,
                        allocation.id,
                        task_id,
                        &spawn_error,
                    ),
                    project_dir: checkpoint_project_dir.clone(),
                },
            )?;
            transaction = tx.update_nota_runtime_transaction(
                transaction.id,
                NotaRuntimeTransactionUpdate {
                    status: if spawn_error.is_some() {
                        "checkpointed_with_spawn_failure"
                    } else {
                        "checkpointed"
                    },
                    forge_task_id: Some(task_id),
                    cadence_checkpoint_id: Some(checkpoint_report.checkpoint.cadence_object.id),
                },
            )?;
            staged_receipts.push(
                tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
                    transaction_id: transaction.id,
                    receipt_kind: "CADENCE_CHECKPOINT_WRITTEN",
                    payload_json: &serde_json::to_string(&json!({
                        "checkpoint_id": checkpoint_report.checkpoint.cadence_object.id,
                        "selected_trunk": checkpoint_report.checkpoint.payload.selected_trunk,
                    }))
                    .context("failed to serialize checkpoint receipt payload in transaction")?,
                    status: "recorded",
                })?,
            );

            Ok((transaction, allocation, staged_receipts, checkpoint_report))
        })?;

    transaction = updated_transaction;
    receipts.extend(staged_receipts);
    sync_runtime_truth(data_store, Some(transaction.id))?;

    Ok(NotaDoDispatchReport {
        transaction,
        allocation,
        receipts,
        dispatch,
        task_id,
        task_status: task_after_spawn.status.clone(),
        spawn_error,
        checkpoint: checkpoint_report.checkpoint,
    })
}

fn build_lowering_context(
    transaction: &StoredNotaRuntimeTransaction,
    task_id: i64,
    lane: &NotaDispatchLane,
    project_root: &str,
) -> LoweringContext {
    LoweringContext {
        transaction_id: transaction.id,
        task_id,
        dispatch_lane: lane.allocator_surface().to_string(),
        allocator_surface: lane.surface_action().to_string(),
        project_root: project_root.to_string(),
    }
}

fn compile_nota_dispatch_packet() -> TypedActionPacket {
    TypedActionPacket::compile(
        ActionRecord::new(
            ActorRole::Dev,
            ActionPrimitive::Dispatch,
            ActionRoom::Prep,
            KnowledgeLayer::Cold,
        )
        .expect("nota dispatch reconstruction should always compile a valid dispatch action"),
    )
}

fn launch_forge_task(
    forge: &ForgePlugin,
    task_id: i64,
    execution_host: NotaDispatchExecutionHost,
) -> Result<()> {
    match execution_host {
        NotaDispatchExecutionHost::InProcess => {
            forge.engine().spawn_task(task_id)?;
        }
        NotaDispatchExecutionHost::DetachedForgeCliSupervisor => {
            spawn_detached_forge_supervisor_process(task_id)?;
            wait_for_task_launch_transition(forge, task_id, Duration::from_millis(150))?;
        }
    }

    Ok(())
}

fn spawn_detached_forge_supervisor_process(task_id: i64) -> Result<()> {
    let current_exe =
        std::env::current_exe().context("failed to resolve current Entrance executable path")?;
    Command::new(current_exe)
        .args(["forge", "supervise-task", "--task-id", &task_id.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn detached forge supervisor for task {task_id}"))?;
    Ok(())
}

fn wait_for_task_launch_transition(
    forge: &ForgePlugin,
    task_id: i64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("stored Forge task {task_id} disappeared during launch"))?;
        if task.status != "Pending" {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

pub fn list_nota_runtime_transactions(
    data_store: &DataStore,
) -> Result<NotaRuntimeTransactionsReport> {
    let transactions = data_store.list_nota_runtime_transactions()?;
    Ok(NotaRuntimeTransactionsReport {
        transaction_count: transactions.len(),
        transactions,
    })
}

pub fn list_nota_runtime_allocations(
    data_store: &DataStore,
) -> Result<NotaRuntimeAllocationsReport> {
    let projected_allocations = project_terminal_allocation_outcomes(data_store)?;
    let allocations = projected_allocations
        .iter()
        .cloned()
        .map(|allocation| project_nota_runtime_allocation_read_record(data_store, allocation))
        .collect::<Result<Vec<_>>>()?;
    Ok(NotaRuntimeAllocationsReport {
        allocation_count: projected_allocations.len(),
        allocations,
        stored_allocations: projected_allocations,
    })
}

fn project_nota_runtime_allocation_read_record(
    data_store: &DataStore,
    allocation: StoredNotaRuntimeAllocation,
) -> Result<NotaRuntimeAllocationReadRecord> {
    let dispatch_truth =
        serde_json::from_str::<NotaDoAllocationPayload>(&allocation.payload_json).ok();
    let task = allocation
        .child_execution_ref
        .parse::<i64>()
        .ok()
        .filter(|_| allocation.child_execution_kind == "forge_task")
        .map(|task_id| data_store.get_forge_task(task_id))
        .transpose()?
        .flatten();
    let supervision =
        derive_runtime_supervision_projection_with_budget(&allocation, task.as_ref(), data_store);
    let budget_ledger = data_store.list_budget_ledger(allocation.id)?;
    let supervision_incident =
        build_runtime_supervision_incident_summary(&supervision, &budget_ledger);

    Ok(NotaRuntimeAllocationReadRecord {
        child_dispatch_role: dispatch_truth
            .as_ref()
            .map(|payload| payload.child_dispatch_role.clone()),
        child_dispatch_tool_name: dispatch_truth
            .as_ref()
            .map(|payload| payload.child_dispatch_tool_name.clone()),
        supervision,
        supervision_incident,
        allocation,
    })
}

pub fn list_nota_runtime_receipts(
    data_store: &DataStore,
    transaction_id: Option<i64>,
) -> Result<NotaRuntimeReceiptsReport> {
    let receipts = data_store.list_nota_runtime_receipts(transaction_id)?;
    Ok(NotaRuntimeReceiptsReport {
        receipt_count: receipts.len(),
        requested_transaction_id: transaction_id,
        receipts,
    })
}

fn sync_runtime_closure_truth(data_store: &DataStore, transaction_id: Option<i64>) -> Result<()> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let Some(current_checkpoint) = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
    else {
        return Ok(());
    };

    let allocations = project_terminal_allocation_outcomes(data_store)?;
    let Some(candidate) = latest_runtime_closure_checkpoint_candidate(
        data_store,
        Some(current_checkpoint),
        &allocations,
    )?
    else {
        return Ok(());
    };

    if transaction_id
        .map(|requested_id| candidate.source_transaction_id != requested_id)
        .unwrap_or(false)
    {
        return Ok(());
    }

    if !checkpoint_request_matches_current(Some(current_checkpoint), &candidate.request) {
        return Ok(());
    }

    data_store.with_immediate_transaction(|tx| {
        sync_runtime_closure_checkpoint_to_transaction(tx, &candidate, current_checkpoint)
    })?;
    Ok(())
}

fn project_terminal_allocation_outcomes(
    data_store: &DataStore,
) -> Result<Vec<StoredNotaRuntimeAllocation>> {
    data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .map(|allocation| project_terminal_allocation_outcome(data_store, allocation))
        .collect()
}

fn project_terminal_allocation_outcome(
    data_store: &DataStore,
    allocation: StoredNotaRuntimeAllocation,
) -> Result<StoredNotaRuntimeAllocation> {
    if !matches!(
        allocation.allocation_kind.as_str(),
        "forge_agent_dispatch" | "forge_dev_dispatch"
    ) || allocation.child_execution_kind != "forge_task"
    {
        return Ok(allocation);
    }

    let task_id = allocation
        .child_execution_ref
        .parse::<i64>()
        .with_context(|| {
            format!(
                "failed to parse forge task id `{}` for allocation {}",
                allocation.child_execution_ref, allocation.id
            )
        })?;
    let Some(task) = data_store.get_forge_task(task_id)? else {
        return Ok(allocation);
    };

    let Some((status, outcome)) = build_terminal_allocation_outcome(&allocation, &task) else {
        return Ok(allocation);
    };

    let mut projected = allocation.clone();
    let mut payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse nota allocation payload for allocation {}",
                allocation.id
            )
        })?;
    if allocation.status != status || payload.terminal_outcome.as_ref() != Some(&outcome) {
        payload.terminal_outcome = Some(outcome.clone());
        projected.status = status.to_string();
        projected.payload_json = serde_json::to_string(&payload).with_context(|| {
            format!(
                "failed to serialize allocation {} terminal outcome",
                allocation.id
            )
        })?;
    }

    Ok(projected)
}

fn reconcile_terminal_allocation_outcomes(
    data_store: &DataStore,
    transaction_id: Option<i64>,
) -> Result<()> {
    for stored_allocation in data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .filter(|allocation| {
            transaction_id
                .map(|requested_id| allocation.source_transaction_id == requested_id)
                .unwrap_or(true)
        })
    {
        let projected_allocation =
            project_terminal_allocation_outcome(data_store, stored_allocation.clone())?;
        let payload: NotaDoAllocationPayload =
            serde_json::from_str(&projected_allocation.payload_json).with_context(|| {
                format!(
                    "failed to parse projected nota allocation payload for allocation {}",
                    projected_allocation.id
                )
            })?;
        let Some(outcome) = payload.terminal_outcome.as_ref() else {
            continue;
        };
        if stored_allocation.status == projected_allocation.status
            && stored_allocation.payload_json == projected_allocation.payload_json
            && has_allocation_terminal_outcome_receipt(
                data_store,
                stored_allocation.source_transaction_id,
                &build_allocation_terminal_outcome_receipt_payload(
                    &projected_allocation,
                    &projected_allocation.status,
                    outcome,
                ),
            )?
        {
            continue;
        }

        data_store.with_immediate_transaction(|tx| {
            persist_terminal_allocation_projection(
                tx,
                &stored_allocation,
                &projected_allocation,
                outcome,
            )
        })?;

        // 1A: Collect gate evidence when a forge task reaches terminal state.
        if projected_allocation.child_execution_kind == "forge_task" {
            collect_gate_evidence_for_allocation(
                data_store,
                &projected_allocation,
            );
        }
    }

    Ok(())
}

/// 1A: Collect gate evidence for a terminal allocation.
///
/// When a Forge task completes, this builds a `StoredGateEvidence` record from
/// the task's terminal state, inserts it, derives the verdict, and updates it.
/// This is a best-effort operation — failures are logged but do not block
/// reconciliation.
fn collect_gate_evidence_for_allocation(
    data_store: &DataStore,
    allocation: &StoredNotaRuntimeAllocation,
) {
    let task_id = match allocation.child_execution_ref.parse::<i64>() {
        Ok(id) => id,
        Err(_) => return,
    };
    let task = match data_store.get_forge_task(task_id) {
        Ok(Some(task)) => task,
        _ => return,
    };

    // Check if evidence already exists for this allocation to avoid duplicates.
    match data_store.get_latest_gate_evidence(allocation.id) {
        Ok(Some(_)) => return,
        Ok(None) => {}
        Err(_) => return,
    }

    let evidence = collect_task_evidence(
        allocation.id,
        &task.name,
        &task.status,
        task.exit_code,
        task.status_message.as_deref(),
    );
    let verdict = derive_verdict(&task.status, task.exit_code);

    let stored = match data_store.insert_gate_evidence(
        evidence.allocation_id,
        evidence.evidence_kind,
        &evidence.summary,
        &evidence.payload_json,
    ) {
        Ok(stored) => stored,
        Err(error) => {
            tracing::warn!(
                allocation_id = allocation.id,
                task_id = task_id,
                %error,
                "failed to insert gate evidence for terminal allocation"
            );
            return;
        }
    };

    if let Err(error) = data_store.update_gate_evidence_verdict(stored.id, verdict) {
        tracing::warn!(
            evidence_id = stored.id,
            allocation_id = allocation.id,
            %error,
            "failed to update gate evidence verdict"
        );
        return;
    }

    // Record the attempt receipt
    let passed = verdict == crate::core::compiler::evidence::EvidenceVerdict::Accepted;
    if let Err(error) = data_store.insert_attempt_receipt(
        stored.id,
        1, // first attempt
        passed,
        &evidence.summary,
    ) {
        tracing::warn!(
            evidence_id = stored.id,
            allocation_id = allocation.id,
            %error,
            "failed to insert attempt receipt for gate evidence"
        );
    }
}

fn persist_terminal_allocation_projection(
    transaction: &DataStoreTransaction<'_>,
    stored_allocation: &StoredNotaRuntimeAllocation,
    projected_allocation: &StoredNotaRuntimeAllocation,
    outcome: &NotaDoAllocationTerminalOutcome,
) -> Result<()> {
    if stored_allocation.status != projected_allocation.status
        || stored_allocation.payload_json != projected_allocation.payload_json
    {
        transaction.update_nota_runtime_allocation(
            stored_allocation.id,
            NotaRuntimeAllocationUpdate {
                status: &projected_allocation.status,
                payload_json: Some(&projected_allocation.payload_json),
            },
        )?;
    }

    let receipt_payload = build_allocation_terminal_outcome_receipt_payload(
        projected_allocation,
        &projected_allocation.status,
        outcome,
    );
    let receipt_recorded = transaction
        .list_nota_runtime_receipts(Some(stored_allocation.source_transaction_id))?
        .into_iter()
        .filter(|receipt| receipt.receipt_kind == ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND)
        .filter_map(|receipt| {
            serde_json::from_str::<AllocationTerminalOutcomeReceiptPayload>(&receipt.payload_json)
                .ok()
        })
        .any(|payload| payload == receipt_payload);
    if !receipt_recorded {
        let receipt_payload_json = serde_json::to_string(&receipt_payload).with_context(|| {
            format!(
                "failed to serialize allocation {} terminal outcome receipt",
                stored_allocation.id
            )
        })?;
        transaction.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: stored_allocation.source_transaction_id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &receipt_payload_json,
            status: "recorded",
        })?;
    }

    Ok(())
}

fn build_allocation_terminal_outcome_receipt_payload(
    allocation: &StoredNotaRuntimeAllocation,
    allocation_status: &str,
    outcome: &NotaDoAllocationTerminalOutcome,
) -> AllocationTerminalOutcomeReceiptPayload {
    AllocationTerminalOutcomeReceiptPayload {
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        boundary_kind: outcome.boundary_kind.clone(),
        child_execution_status: outcome.child_execution_status.clone(),
        child_execution_status_message: outcome.child_execution_status_message.clone(),
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
        allocation_status: allocation_status.to_string(),
    }
}

fn has_allocation_terminal_outcome_receipt(
    data_store: &DataStore,
    transaction_id: i64,
    expected_payload: &AllocationTerminalOutcomeReceiptPayload,
) -> Result<bool> {
    for receipt in data_store.list_nota_runtime_receipts(Some(transaction_id))? {
        if receipt.receipt_kind != ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND {
            continue;
        }

        let payload: AllocationTerminalOutcomeReceiptPayload =
            serde_json::from_str(&receipt.payload_json).with_context(|| {
                format!(
                    "failed to parse allocation terminal outcome receipt {}",
                    receipt.id
                )
            })?;
        if &payload == expected_payload {
            return Ok(true);
        }
    }

    Ok(false)
}

fn sync_runtime_truth(data_store: &DataStore, transaction_id: Option<i64>) -> Result<()> {
    reconcile_terminal_allocation_outcomes(data_store, transaction_id)?;
    sync_runtime_closure_truth(data_store, transaction_id)?;
    materialize_current_runtime_human_round(data_store)?;
    materialize_current_runtime_bridge_objects(data_store)?;
    refresh_runtime_invariants(data_store).map(|_| ())
}

fn reconstruct_terminal_admitted_dispatch(
    allocation: &StoredNotaRuntimeAllocation,
) -> AdmittedDispatch {
    let packet = compile_nota_dispatch_packet();
    let sandbox_requirement = packet.semantics().sandbox_requirement;
    let routing_constraint = packet.semantics().routing_constraint;
    let lowered_dispatch = LoweredDispatch {
        packet,
        lineage: DispatchLineage {
            lineage_ref: allocation.lineage_ref.clone(),
            child_execution_kind: allocation.child_execution_kind.clone(),
            child_execution_ref: allocation.child_execution_ref.clone(),
            return_target_kind: allocation.return_target_kind.clone(),
            return_target_ref: allocation.return_target_ref.clone(),
            escalation_target_kind: allocation.escalation_target_kind.clone(),
            escalation_target_ref: allocation.escalation_target_ref.clone(),
        },
        sandbox: SandboxConfig {
            requirement: sandbox_requirement,
            working_dir: None,
        },
        routing: DispatchRouting {
            constraint: routing_constraint,
            allocator_role: allocation.allocator_role.clone(),
            allocation_kind: allocation.allocation_kind.clone(),
        },
    };

    admit_dispatch(lowered_dispatch, None)
        .expect("stored nota runtime allocation should always reconstruct an admitted dispatch")
}

fn boundary_kind(boundary: ReturnBoundary) -> &'static str {
    match boundary {
        ReturnBoundary::Return => "return",
        ReturnBoundary::Escalation => "escalation",
    }
}

fn build_terminal_allocation_outcome<'a>(
    allocation: &'a StoredNotaRuntimeAllocation,
    task: &'a StoredForgeTask,
) -> Option<(&'static str, NotaDoAllocationTerminalOutcome)> {
    let terminal_status = TerminalStatus::from_task_status(task.status.as_str())?;
    let admitted = reconstruct_terminal_admitted_dispatch(allocation);
    let route = resolve_return_route(&admitted, terminal_status)
        .expect("stored nota runtime allocation should always resolve a terminal return route");

    let allocation_status = match route.boundary {
        ReturnBoundary::Return => "return_ready",
        ReturnBoundary::Escalation => match terminal_status {
            TerminalStatus::Blocked => "escalated_blocked",
            TerminalStatus::Failed => "escalated_failed",
            TerminalStatus::Cancelled => "escalated_cancelled",
            TerminalStatus::Done => unreachable!("Done should never escalate"),
        },
    };

    Some((
        allocation_status,
        NotaDoAllocationTerminalOutcome {
            boundary_kind: boundary_kind(route.boundary).to_string(),
            child_execution_status: task.status.clone(),
            child_execution_status_message: task.status_message.clone(),
            target_kind: route.target_kind,
            target_ref: route.target_ref,
        },
    ))
}

// admission_policy_for_kind, projection_policy_for_kind moved to policy.rs

fn build_do_checkpoint_landed_items(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    task_id: i64,
    dispatch: &PreparedNotaDispatch,
    spawn_error: &Option<String>,
) -> Vec<String> {
    let mut landed = vec![
        format!("Created NOTA runtime transaction {transaction_id}."),
        format!(
            "Materialized NOTA allocation {} with lineage {}.",
            allocation.id, allocation.lineage_ref
        ),
        format!(
            "Bound allocation {} child execution target to Forge task {task_id} in {}.",
            allocation.id, dispatch.worktree_path
        ),
        format!(
            "Recorded return and escalation targets for allocation {} back to NOTA runtime transaction {transaction_id}.",
            allocation.id
        ),
    ];

    if let Some(error) = spawn_error {
        landed.push(format!(
            "Recorded spawn failure for allocation {} on Forge task {task_id}: {error}.",
            allocation.id
        ));
    } else {
        landed.push(format!(
            "Dispatched Forge task {task_id} for allocation {} from the NOTA `Do` ingress.",
            allocation.id
        ));
    }

    landed
}

fn build_dev_checkpoint_landed_items(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    task_id: i64,
    dispatch: &PreparedNotaDispatch,
    spawn_error: &Option<String>,
) -> Vec<String> {
    let mut landed = vec![
        format!("Created NOTA runtime transaction {transaction_id} for a dev child dispatch."),
        format!(
            "Materialized NOTA allocation {} with lineage {}.",
            allocation.id, allocation.lineage_ref
        ),
        format!(
            "Bound allocation {} child execution target to Forge task {task_id} in {}.",
            allocation.id, dispatch.worktree_path
        ),
        format!(
            "Recorded runtime-visible child dispatch role `dev` and tool `{}` for allocation {}.",
            dispatch.dispatch_tool_name, allocation.id
        ),
    ];

    if let Some(error) = spawn_error.as_ref() {
        landed.push(format!(
            "Recorded spawn failure for dev allocation {} on Forge task {task_id}: {error}.",
            allocation.id
        ));
    } else {
        landed.push(format!(
            "Dispatched Forge task {task_id} for dev allocation {} from the NOTA `Dev` ingress.",
            allocation.id
        ));
    }

    landed
}

fn build_do_checkpoint_remaining_items(
    allocation_id: i64,
    task_id: i64,
    spawn_error: &Option<String>,
) -> Vec<String> {
    if spawn_error.is_some() {
        vec![
            format!("Repair the execution environment for Forge task {task_id}."),
            format!("Re-dispatch allocation {allocation_id} after the runner boundary is healthy."),
        ]
    } else {
        vec![
            format!("Review Forge task {task_id} output and terminal status."),
            format!(
                "Read allocation {allocation_id} back through the persistent NOTA overview surface once the child reaches a terminal state."
            ),
            format!(
                "Prove allocation {allocation_id} terminal outcome against a live runtime task without relying on chat reconstruction."
            ),
        ]
    }
}

fn build_do_checkpoint_hints(
    transaction_id: i64,
    allocation_id: i64,
    task_id: i64,
    spawn_error: &Option<String>,
) -> Vec<String> {
    let mut hints = vec![
        format!("Resume from NOTA runtime transaction {transaction_id}."),
        format!("Inspect NOTA allocation {allocation_id} before replaying operator intent."),
        format!("Inspect Forge task {task_id} from runtime storage before re-entering chat."),
    ];

    if spawn_error.is_some() {
        hints.push("Check runner availability before retrying `nota do`.".to_string());
    }

    hints
}

fn build_dev_checkpoint_remaining_items(
    allocation_id: i64,
    task_id: i64,
    spawn_error: &Option<String>,
) -> Vec<String> {
    if spawn_error.is_some() {
        return vec![
            format!("Re-dispatch dev allocation {allocation_id} after the runner boundary is healthy."),
            format!(
                "Re-check Forge task {task_id} and the persisted NOTA runtime receipts before retrying the dev lane."
            ),
        ];
    }

    vec![
        format!(
            "Read dev allocation {allocation_id} back through `entrance nota allocations` or `nota_runtime_allocations` once the child reaches a terminal state."
        ),
        "Keep this cut scoped to the first NOTA-owned dev runtime lane; honest multi-role allocator and permission-finalization are still not landed.".to_string(),
    ]
}

fn build_dev_checkpoint_hints(
    transaction_id: i64,
    allocation_id: i64,
    task_id: i64,
    spawn_error: &Option<String>,
) -> Vec<String> {
    let mut hints = vec![
        format!("Resume from NOTA runtime transaction {transaction_id}."),
        format!("Inspect NOTA allocation {allocation_id} and confirm child_dispatch_role `dev`."),
    ];

    if spawn_error.is_some() {
        hints.push(format!(
            "Re-enter from Forge task {task_id} after the spawn failure is cleared."
        ));
    } else {
        hints.push(format!(
            "Start from `entrance nota status` or `nota_runtime_status`, then inspect Forge task {task_id} from storage-backed read surfaces."
        ));
    }

    hints
}

pub fn recommend_runtime_closure_checkpoint(
    data_store: &DataStore,
    allocations: &[StoredNotaRuntimeAllocation],
    current_checkpoint: Option<&NotaCheckpointRecord>,
) -> Result<Option<NotaCheckpointRequest>> {
    let Some(candidate) =
        latest_runtime_closure_checkpoint_candidate(data_store, current_checkpoint, allocations)?
    else {
        return Ok(None);
    };

    if checkpoint_request_matches_current(current_checkpoint, &candidate.request) {
        return Ok(None);
    }

    Ok(Some(candidate.request))
}

pub fn materialize_runtime_closure_checkpoint(
    data_store: &DataStore,
) -> Result<NotaRuntimeClosureCheckpointMaterializationReport> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .cloned();
    let allocations = list_nota_runtime_allocations(data_store)?;

    let Some(candidate) = latest_runtime_closure_checkpoint_candidate(
        data_store,
        current_checkpoint.as_ref(),
        allocations.stored_allocations(),
    )?
    else {
        return Ok(NotaRuntimeClosureCheckpointMaterializationReport {
            status: "unavailable".to_string(),
            checkpoint: current_checkpoint,
            source_recommendation: None,
            superseded_checkpoint_id: None,
            supersession_link: None,
        });
    };

    if checkpoint_request_matches_current(current_checkpoint.as_ref(), &candidate.request) {
        if let Some(current_checkpoint) = current_checkpoint.as_ref() {
            data_store.with_immediate_transaction(|tx| {
                sync_runtime_closure_checkpoint_to_transaction(tx, &candidate, current_checkpoint)
            })?;
            sync_runtime_truth(data_store, Some(candidate.source_transaction_id))?;
        }
        return Ok(NotaRuntimeClosureCheckpointMaterializationReport {
            status: "already_current".to_string(),
            checkpoint: current_checkpoint,
            source_recommendation: Some(candidate.request),
            superseded_checkpoint_id: None,
            supersession_link: None,
        });
    }

    let source_recommendation = candidate.request.clone();
    let source_transaction_id = candidate.source_transaction_id;
    let write_report = write_runtime_checkpoint(data_store, candidate.request)?;
    data_store.with_immediate_transaction(|tx| {
        sync_runtime_closure_checkpoint_to_transaction(
            tx,
            &RecommendedCheckpointCandidate {
                kind: candidate.kind,
                allocation_id: candidate.allocation_id,
                source_transaction_id,
                request: source_recommendation.clone(),
            },
            &write_report.checkpoint,
        )
    })?;
    sync_runtime_truth(data_store, Some(source_transaction_id))?;
    Ok(NotaRuntimeClosureCheckpointMaterializationReport {
        status: "applied".to_string(),
        checkpoint: Some(write_report.checkpoint),
        source_recommendation: Some(source_recommendation),
        superseded_checkpoint_id: write_report.superseded_checkpoint_id,
        supersession_link: write_report.supersession_link,
    })
}

fn latest_runtime_closure_checkpoint_candidate(
    data_store: &DataStore,
    current_checkpoint: Option<&NotaCheckpointRecord>,
    allocations: &[StoredNotaRuntimeAllocation],
) -> Result<Option<RecommendedCheckpointCandidate>> {
    let transactions = data_store.list_nota_runtime_transactions()?;
    let checkpoint_scope_ids = active_checkpoint_scope_ids(data_store, current_checkpoint)?;
    let mut candidates = Vec::new();
    if let Some(candidate) = recommend_single_lane_allocator_checkpoint_candidate(
        data_store,
        &checkpoint_scope_ids,
        &transactions,
        allocations,
    )? {
        candidates.push(candidate);
    }
    if let Some(candidate) = recommend_dev_return_checkpoint_candidate(
        data_store,
        &checkpoint_scope_ids,
        &transactions,
        allocations,
    )? {
        candidates.push(candidate);
    }

    let Some(candidate) = candidates
        .into_iter()
        .max_by_key(|candidate| candidate.allocation_id)
    else {
        return Ok(None);
    };

    Ok(Some(candidate))
}

fn sync_runtime_closure_checkpoint_to_transaction(
    transaction: &DataStoreTransaction<'_>,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(runtime_transaction) =
        transaction.get_nota_runtime_transaction(candidate.source_transaction_id)?
    else {
        return Ok(());
    };

    if runtime_transaction.cadence_checkpoint_id != Some(checkpoint.cadence_object.id) {
        transaction.update_nota_runtime_transaction(
            runtime_transaction.id,
            NotaRuntimeTransactionUpdate {
                status: &runtime_transaction.status,
                forge_task_id: runtime_transaction.forge_task_id,
                cadence_checkpoint_id: Some(checkpoint.cadence_object.id),
            },
        )?;
    }

    ensure_checkpoint_written_receipt(transaction, runtime_transaction.id, checkpoint)?;
    ensure_runtime_closure_acceptance_receipt(transaction, candidate, checkpoint)?;
    ensure_runtime_acceptance_bundle(transaction, candidate, checkpoint)
}

fn ensure_checkpoint_written_receipt(
    transaction: &DataStoreTransaction<'_>,
    transaction_id: i64,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let receipts = transaction.list_nota_runtime_receipts(Some(transaction_id))?;
    let has_receipt = receipts.into_iter().any(|receipt| {
        if receipt.receipt_kind != CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND {
            return false;
        }

        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&receipt.payload_json) else {
            return false;
        };
        payload
            .get("checkpoint_id")
            .and_then(|value| value.as_i64())
            == Some(checkpoint.cadence_object.id)
    });
    if has_receipt {
        return Ok(());
    }

    transaction.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id,
        receipt_kind: CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&json!({
            "checkpoint_id": checkpoint.cadence_object.id,
            "selected_trunk": checkpoint.payload.selected_trunk,
        }))
        .context("failed to serialize checkpoint receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

fn ensure_runtime_closure_acceptance_receipt(
    transaction: &DataStoreTransaction<'_>,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    match candidate.kind {
        RecommendedCheckpointCandidateKind::AgentEscalationContinuity => Ok(()),
        RecommendedCheckpointCandidateKind::AgentReturnAcceptance => {
            ensure_agent_return_accepted_receipt(transaction, candidate, checkpoint)
        }
        RecommendedCheckpointCandidateKind::AgentReturnClosure => Ok(()),
        RecommendedCheckpointCandidateKind::DevReturnAcceptance => {
            ensure_dev_return_accepted_receipt(transaction, candidate, checkpoint)?;
            ensure_dev_return_review_ready_receipt(transaction, candidate, checkpoint)
        }
        RecommendedCheckpointCandidateKind::DevReturnClosure => Ok(()),
    }
}

fn ensure_runtime_acceptance_bundle(
    transaction: &DataStoreTransaction<'_>,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(bundle) = build_runtime_acceptance_bundle(candidate, checkpoint, transaction)? else {
        return Ok(());
    };

    let existing_current = transaction
        .list_cadence_objects_by_kind(CADENCE_ACCEPTANCE_BUNDLE_KIND)?
        .into_iter()
        .find(|object| object.is_current)
        .map(parse_acceptance_bundle_record)
        .transpose()?;
    if existing_current
        .as_ref()
        .map(|existing| existing.payload == bundle)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let title = build_acceptance_bundle_title(&bundle);
    let summary = build_acceptance_bundle_summary(&bundle);
    let payload_json =
        serde_json::to_string(&bundle).context("failed to serialize acceptance bundle payload")?;
    let cadence_object = transaction.insert_cadence_object(NewCadenceObject {
        cadence_kind: CADENCE_ACCEPTANCE_BUNDLE_KIND,
        title: &title,
        summary: &summary,
        payload_json: &payload_json,
        scope_type: NOTA_RUNTIME_SCOPE_TYPE,
        scope_ref: NOTA_RUNTIME_SCOPE_REF,
        source_type: NOTA_RUNTIME_SOURCE_TYPE,
        source_ref: acceptance_bundle_source_ref(candidate.kind),
        admission_policy: admission_policy_for_kind(CADENCE_ACCEPTANCE_BUNDLE_KIND),
        projection_policy: projection_policy_for_kind(CADENCE_ACCEPTANCE_BUNDLE_KIND),
        status: if bundle.fully_settled {
            "fully_settled"
        } else {
            "accepted"
        },
        is_current: true,
    })?;
    transaction.insert_cadence_link(NewCadenceLink {
        src_cadence_object_id: checkpoint.cadence_object.id,
        dst_cadence_object_id: cadence_object.id,
        relation_type: "acceptance_bundle",
        status: "active",
    })?;
    transaction.insert_anti_zeno_event(crate::core::data_store::NewAntiZenoEvent {
        checkpoint_id: Some(checkpoint.cadence_object.id),
        acceptance_bundle_id: Some(cadence_object.id),
        event_kind: "acceptance_recorded",
        boundary_ref: &bundle.lineage_ref,
        budget_axis: "semantic",
        event_weight: 1,
        summary: &summary,
    })?;
    if bundle.fully_settled {
        let closure_summary = format!(
            "Accepted boundary {} is carried forward into fully settled closure on checkpoint {}.",
            bundle.lineage_ref, checkpoint.cadence_object.id
        );
        transaction.insert_anti_zeno_event(crate::core::data_store::NewAntiZenoEvent {
            checkpoint_id: Some(checkpoint.cadence_object.id),
            acceptance_bundle_id: Some(cadence_object.id),
            event_kind: "closure_recorded",
            boundary_ref: &bundle.lineage_ref,
            budget_axis: "semantic",
            event_weight: 1,
            summary: &closure_summary,
        })?;
    }

    Ok(())
}

fn build_runtime_acceptance_bundle(
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
    transaction: &DataStoreTransaction<'_>,
) -> Result<Option<CadenceAcceptanceBundlePayload>> {
    let Some(allocation) = transaction
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == candidate.allocation_id)
    else {
        return Ok(None);
    };
    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse acceptance bundle payload for allocation {}",
                allocation.id
            )
        })?;
    let Some(outcome) = payload.terminal_outcome.as_ref() else {
        return Ok(None);
    };
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        return Ok(None);
    }

    let receipts = transaction.list_nota_runtime_receipts(Some(candidate.source_transaction_id))?;
    let latest_review = latest_dev_return_review_recorded_for_boundary(&receipts, &allocation)?;
    let latest_integrate =
        latest_dev_return_integrate_recorded_for_boundary(&receipts, &allocation)?;
    let latest_finalize = latest_dev_return_finalize_recorded_for_boundary(&receipts, &allocation)?;
    let (acceptance_kind, round_state, fully_settled) = match candidate.kind {
        RecommendedCheckpointCandidateKind::AgentReturnAcceptance => {
            ("agent_return_acceptance", "accepted", false)
        }
        RecommendedCheckpointCandidateKind::AgentReturnClosure => {
            ("agent_return_acceptance", "fully_settled", true)
        }
        RecommendedCheckpointCandidateKind::DevReturnAcceptance => {
            ("dev_return_acceptance", "accepted", false)
        }
        RecommendedCheckpointCandidateKind::DevReturnClosure => {
            ("dev_return_acceptance", "fully_settled", true)
        }
        RecommendedCheckpointCandidateKind::AgentEscalationContinuity => return Ok(None),
    };
    let (review_verdict, integrate_outcome, finalize_state) = if fully_settled {
        (
            latest_review.and_then(|review| review.verdict),
            latest_integrate.and_then(|integrate| integrate.outcome),
            latest_finalize.map(|finalize| finalize.state),
        )
    } else {
        (None, None, None)
    };

    Ok(Some(CadenceAcceptanceBundlePayload {
        checkpoint_id: checkpoint.cadence_object.id,
        transaction_id: candidate.source_transaction_id,
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref,
        acceptance_kind: acceptance_kind.to_string(),
        round_state: round_state.to_string(),
        fully_settled,
        child_dispatch_role: payload.child_dispatch_role,
        execution_host: payload.execution_host,
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
        review_verdict,
        integrate_outcome,
        finalize_state,
    }))
}

fn acceptance_bundle_source_ref(kind: RecommendedCheckpointCandidateKind) -> &'static str {
    match kind {
        RecommendedCheckpointCandidateKind::AgentReturnAcceptance => {
            "nota_runtime:agent_return_acceptance_bundle"
        }
        RecommendedCheckpointCandidateKind::AgentReturnClosure => {
            "nota_runtime:agent_return_closure_bundle"
        }
        RecommendedCheckpointCandidateKind::DevReturnAcceptance => {
            "nota_runtime:dev_return_acceptance_bundle"
        }
        RecommendedCheckpointCandidateKind::DevReturnClosure => {
            "nota_runtime:dev_return_closure_bundle"
        }
        RecommendedCheckpointCandidateKind::AgentEscalationContinuity => {
            "nota_runtime:acceptance_bundle"
        }
    }
}

fn build_acceptance_bundle_title(bundle: &CadenceAcceptanceBundlePayload) -> String {
    if bundle.allocation_id == 0 {
        return format!(
            "Acceptance bundle: current round acceptance on checkpoint {}",
            bundle.checkpoint_id
        );
    }

    format!(
        "Acceptance bundle: {} {}",
        bundle.acceptance_kind.replace('_', " "),
        bundle.target_ref
    )
}

fn build_acceptance_bundle_summary(bundle: &CadenceAcceptanceBundlePayload) -> String {
    if bundle.allocation_id == 0 {
        return format!(
            "Acceptance is formalized for current human round target {} on checkpoint {}.",
            bundle.target_ref, bundle.checkpoint_id
        );
    }

    if bundle.fully_settled {
        format!(
            "Acceptance is fully settled for allocation {} on lineage {}.",
            bundle.allocation_id, bundle.lineage_ref
        )
    } else {
        format!(
            "Acceptance is recorded for allocation {} on lineage {}.",
            bundle.allocation_id, bundle.lineage_ref
        )
    }
}

fn ensure_agent_return_accepted_receipt(
    transaction: &DataStoreTransaction<'_>,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(allocation) = transaction
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == candidate.allocation_id)
    else {
        return Ok(());
    };
    if allocation.allocation_kind != "forge_agent_dispatch" {
        return Ok(());
    }

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse agent return acceptance payload for allocation {}",
                allocation.id
            )
        })?;
    let Some(outcome) = payload.terminal_outcome.as_ref() else {
        return Ok(());
    };
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        return Ok(());
    }

    let receipt_payload = AgentReturnAcceptedReceiptPayload {
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        checkpoint_id: checkpoint.cadence_object.id,
        child_dispatch_role: payload.child_dispatch_role,
        execution_host: payload.execution_host,
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
    };
    let has_receipt = transaction
        .list_nota_runtime_receipts(Some(candidate.source_transaction_id))?
        .into_iter()
        .any(|receipt| {
            if receipt.receipt_kind != AGENT_RETURN_ACCEPTED_RECEIPT_KIND {
                return false;
            }

            let Ok(payload) =
                serde_json::from_str::<AgentReturnAcceptedReceiptPayload>(&receipt.payload_json)
            else {
                return false;
            };
            payload == receipt_payload
        });
    if has_receipt {
        return Ok(());
    }

    transaction.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: candidate.source_transaction_id,
        receipt_kind: AGENT_RETURN_ACCEPTED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize agent return accepted receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

fn ensure_dev_return_accepted_receipt(
    transaction: &DataStoreTransaction<'_>,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(allocation) = transaction
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == candidate.allocation_id)
    else {
        return Ok(());
    };

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse dev return acceptance payload for allocation {}",
                allocation.id
            )
        })?;
    let Some(outcome) = payload.terminal_outcome.as_ref() else {
        return Ok(());
    };
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        return Ok(());
    }

    let receipt_payload = DevReturnAcceptedReceiptPayload {
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        checkpoint_id: checkpoint.cadence_object.id,
        child_dispatch_role: payload.child_dispatch_role,
        execution_host: payload.execution_host,
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
    };
    let has_receipt = transaction
        .list_nota_runtime_receipts(Some(candidate.source_transaction_id))?
        .into_iter()
        .any(|receipt| {
            if receipt.receipt_kind != DEV_RETURN_ACCEPTED_RECEIPT_KIND {
                return false;
            }

            let Ok(payload) =
                serde_json::from_str::<DevReturnAcceptedReceiptPayload>(&receipt.payload_json)
            else {
                return false;
            };
            payload == receipt_payload
        });
    if has_receipt {
        return Ok(());
    }

    transaction.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: candidate.source_transaction_id,
        receipt_kind: DEV_RETURN_ACCEPTED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev return accepted receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

fn ensure_dev_return_review_ready_receipt(
    transaction: &DataStoreTransaction<'_>,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(allocation) = transaction
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == candidate.allocation_id)
    else {
        return Ok(());
    };
    if allocation.allocation_kind != "forge_dev_dispatch" {
        return Ok(());
    }

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse dev review-ready payload for allocation {}",
                allocation.id
            )
        })?;
    let Some(outcome) = payload.terminal_outcome.as_ref() else {
        return Ok(());
    };
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        return Ok(());
    }

    let receipt_payload = DevReturnReviewReadyReceiptPayload {
        checkpoint_id: checkpoint.cadence_object.id,
        next_step: build_dev_return_review_next_step(
            candidate.source_transaction_id,
            &allocation,
            &payload,
            outcome,
        ),
    };
    let has_receipt = transaction
        .list_nota_runtime_receipts(Some(candidate.source_transaction_id))?
        .into_iter()
        .any(|receipt| {
            if receipt.receipt_kind != DEV_RETURN_REVIEW_READY_RECEIPT_KIND {
                return false;
            }

            let Ok(payload) =
                serde_json::from_str::<DevReturnReviewReadyReceiptPayload>(&receipt.payload_json)
            else {
                return false;
            };
            payload == receipt_payload
        });
    if has_receipt {
        return Ok(());
    }

    transaction.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: candidate.source_transaction_id,
        receipt_kind: DEV_RETURN_REVIEW_READY_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev review-ready receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

fn normalize_boundary_ask_code(raw: &str) -> Result<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "unblock" | "decide" | "replace" | "override" => Ok(normalized),
        _ => {
            bail!("unsupported ask code `{raw}`; use `unblock`, `decide`, `replace`, or `override`")
        }
    }
}

fn build_boundary_intake_lineage_ref(
    boundary_kind: &str,
    checkpoint_id: i64,
    transaction_id: i64,
) -> String {
    format!("nota/boundary/{boundary_kind}/checkpoint/{checkpoint_id}/transaction/{transaction_id}")
}

fn build_current_round_acceptance_lineage_ref(checkpoint_id: i64, human_round_id: i64) -> String {
    format!("nota/human-round/{human_round_id}/checkpoint/{checkpoint_id}/acceptance")
}

fn build_boundary_intake_next_step(
    step: &str,
    transaction_id: i64,
    checkpoint_id: i64,
) -> NotaRuntimeNextStep {
    NotaRuntimeNextStep {
        step: step.to_string(),
        transaction_id,
        allocation_id: 0,
        lineage_ref: build_boundary_intake_lineage_ref(step, checkpoint_id, transaction_id),
        child_dispatch_role: "nota".to_string(),
        execution_host: NOTA_BOUNDARY_EXECUTION_HOST.to_string(),
        target_kind: "cadence_checkpoint".to_string(),
        target_ref: checkpoint_id.to_string(),
    }
}

fn build_boundary_clarification_next_step(
    transaction_id: i64,
    checkpoint_id: i64,
) -> NotaRuntimeNextStep {
    build_boundary_intake_next_step("clarify", transaction_id, checkpoint_id)
}

fn build_boundary_ask_next_step(
    transaction_id: i64,
    checkpoint_id: i64,
    ask_code: &str,
) -> NotaRuntimeNextStep {
    build_boundary_intake_next_step(
        format!("ask_{ask_code}").as_str(),
        transaction_id,
        checkpoint_id,
    )
}

fn is_boundary_intake_transaction_open(transaction: &StoredNotaRuntimeTransaction) -> bool {
    matches!(
        (
            transaction.transaction_kind.as_str(),
            transaction.status.as_str(),
        ),
        (
            NOTA_DO_CLARIFICATION_TRANSACTION_KIND,
            CLARIFICATION_OPEN_TRANSACTION_STATUS
        ) | (NOTA_DO_ASK_TRANSACTION_KIND, ASK_OPEN_TRANSACTION_STATUS)
    )
}

fn update_boundary_intake_transactions_to_superseded(
    transaction: &DataStoreTransaction<'_>,
    transactions: &[StoredNotaRuntimeTransaction],
) -> Result<()> {
    for boundary_transaction in transactions {
        transaction.update_nota_runtime_transaction(
            boundary_transaction.id,
            NotaRuntimeTransactionUpdate {
                status: BOUNDARY_INTAKE_SUPERSEDED_TRANSACTION_STATUS,
                forge_task_id: boundary_transaction.forge_task_id,
                cadence_checkpoint_id: boundary_transaction.cadence_checkpoint_id,
            },
        )?;
    }

    Ok(())
}

fn current_runtime_checkpoint(
    data_store: &DataStore,
    context: &str,
) -> Result<NotaCheckpointRecord> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    checkpoints
        .checkpoints
        .into_iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .with_context(|| format!("{context} requires a current runtime checkpoint"))
}

fn current_runtime_checkpoint_scope(
    data_store: &DataStore,
    checkpoint: &NotaCheckpointRecord,
) -> Result<Vec<i64>> {
    active_checkpoint_scope_ids(data_store, Some(checkpoint))
}

fn boundary_intake_next_step_from_transaction(
    transaction: &StoredNotaRuntimeTransaction,
) -> Result<Option<NotaRuntimeNextStep>> {
    if transaction.cadence_checkpoint_id.is_none() {
        return Ok(None);
    }
    if !is_boundary_intake_transaction_open(transaction) {
        return Ok(None);
    }

    match transaction.transaction_kind.as_str() {
        NOTA_DO_CLARIFICATION_TRANSACTION_KIND => {
            let payload: NotaBoundaryClarificationPayload =
                serde_json::from_str(&transaction.payload_json).with_context(|| {
                    format!(
                        "failed to parse clarification payload for runtime transaction {}",
                        transaction.id
                    )
                })?;
            Ok(Some(build_boundary_clarification_next_step(
                transaction.id,
                payload.checkpoint_id,
            )))
        }
        NOTA_DO_ASK_TRANSACTION_KIND => {
            let payload: NotaBoundaryAskPayload = serde_json::from_str(&transaction.payload_json)
                .with_context(|| {
                format!(
                    "failed to parse ask payload for runtime transaction {}",
                    transaction.id
                )
            })?;
            Ok(Some(build_boundary_ask_next_step(
                transaction.id,
                payload.checkpoint_id,
                &payload.ask_code,
            )))
        }
        _ => Ok(None),
    }
}

fn derive_open_boundary_intake_next_step(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
) -> Result<Option<NotaRuntimeNextStep>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let checkpoint_rank = scoped_checkpoint_rank_map(checkpoint_scope_ids);
    let mut selected: Option<(usize, i64, &StoredNotaRuntimeTransaction)> = None;

    for transaction in transactions
        .iter()
        .filter(|transaction| is_boundary_intake_transaction_open(transaction))
    {
        let Some(checkpoint_id) = transaction.cadence_checkpoint_id else {
            continue;
        };
        let Some(scope_rank) = checkpoint_rank.get(&checkpoint_id).copied() else {
            continue;
        };

        match selected {
            Some((selected_rank, selected_id, _))
                if selected_rank < scope_rank
                    || (selected_rank == scope_rank && selected_id >= transaction.id) => {}
            _ => selected = Some((scope_rank, transaction.id, transaction)),
        }
    }

    let Some((_, _, transaction)) = selected else {
        return Ok(None);
    };
    boundary_intake_next_step_from_transaction(transaction)
}

pub fn record_nota_boundary_clarification(
    data_store: &DataStore,
    request: NotaBoundaryClarificationRequest,
) -> Result<NotaBoundaryClarificationReport> {
    let summary = normalize_optional(Some(request.summary.as_str()))
        .context("`entrance nota clarify --summary` must not be empty")?;
    sync_runtime_truth(data_store, None)?;
    let current_checkpoint = current_runtime_checkpoint(data_store, "runtime clarification")?;
    let superseded_transactions = list_nota_runtime_transactions(data_store)?
        .transactions
        .into_iter()
        .filter(|transaction| {
            transaction.cadence_checkpoint_id == Some(current_checkpoint.cadence_object.id)
                && is_boundary_intake_transaction_open(transaction)
        })
        .collect::<Vec<_>>();
    let superseded_transaction_ids = superseded_transactions
        .iter()
        .map(|transaction| transaction.id)
        .collect::<Vec<_>>();
    let clarification = NotaBoundaryClarificationPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        summary: summary.clone(),
    };
    let payload_json = serde_json::to_string(&clarification)
        .context("failed to serialize clarification payload")?;
    let title = format!(
        "Clarify checkpoint {}",
        current_checkpoint.cadence_object.id
    );

    let (transaction, next_step, receipt) = data_store.with_immediate_transaction(|tx| {
        update_boundary_intake_transactions_to_superseded(tx, &superseded_transactions)?;

        let transaction = tx.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "clarify",
            transaction_kind: NOTA_DO_CLARIFICATION_TRANSACTION_KIND,
            title: &title,
            payload_json: &payload_json,
            status: CLARIFICATION_OPEN_TRANSACTION_STATUS,
            forge_task_id: None,
            cadence_checkpoint_id: Some(current_checkpoint.cadence_object.id),
        })?;
        let next_step =
            build_boundary_clarification_next_step(transaction.id, clarification.checkpoint_id);
        let receipt_payload = DoClarificationRecordedReceiptPayload {
            checkpoint_id: clarification.checkpoint_id,
            clarification: clarification.clone(),
            next_step: next_step.clone(),
        };
        let receipt = tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: DO_CLARIFICATION_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&receipt_payload)
                .context("failed to serialize clarification receipt payload")?,
            status: "recorded",
        })?;

        Ok((transaction, next_step, receipt))
    })?;
    sync_runtime_truth(data_store, Some(transaction.id))?;

    Ok(NotaBoundaryClarificationReport {
        status: "recorded".to_string(),
        transaction,
        clarification,
        next_step,
        receipt,
        superseded_transaction_ids,
    })
}

pub fn record_nota_boundary_ask(
    data_store: &DataStore,
    request: NotaBoundaryAskRequest,
) -> Result<NotaBoundaryAskReport> {
    let ask_code = normalize_boundary_ask_code(&request.ask_code)?;
    let summary = normalize_optional(Some(request.summary.as_str()))
        .context("`entrance nota ask --summary` must not be empty")?;
    sync_runtime_truth(data_store, None)?;
    let current_checkpoint = current_runtime_checkpoint(data_store, "runtime ask")?;
    let superseded_transactions = list_nota_runtime_transactions(data_store)?
        .transactions
        .into_iter()
        .filter(|transaction| {
            transaction.cadence_checkpoint_id == Some(current_checkpoint.cadence_object.id)
                && is_boundary_intake_transaction_open(transaction)
        })
        .collect::<Vec<_>>();
    let superseded_transaction_ids = superseded_transactions
        .iter()
        .map(|transaction| transaction.id)
        .collect::<Vec<_>>();
    let ask = NotaBoundaryAskPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        ask_code,
        summary,
    };
    let payload_json = serde_json::to_string(&ask).context("failed to serialize ask payload")?;
    let title = format!(
        "Ask {} on checkpoint {}",
        ask.ask_code, current_checkpoint.cadence_object.id
    );

    let (transaction, next_step, receipt) = data_store.with_immediate_transaction(|tx| {
        update_boundary_intake_transactions_to_superseded(tx, &superseded_transactions)?;

        let transaction = tx.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "ask",
            transaction_kind: NOTA_DO_ASK_TRANSACTION_KIND,
            title: &title,
            payload_json: &payload_json,
            status: ASK_OPEN_TRANSACTION_STATUS,
            forge_task_id: None,
            cadence_checkpoint_id: Some(current_checkpoint.cadence_object.id),
        })?;
        let next_step =
            build_boundary_ask_next_step(transaction.id, ask.checkpoint_id, &ask.ask_code);
        let receipt_payload = DoAskRecordedReceiptPayload {
            checkpoint_id: ask.checkpoint_id,
            ask: ask.clone(),
            next_step: next_step.clone(),
        };
        let receipt = tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: DO_ASK_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&receipt_payload)
                .context("failed to serialize ask receipt payload")?,
            status: "recorded",
        })?;

        Ok((transaction, next_step, receipt))
    })?;
    sync_runtime_truth(data_store, Some(transaction.id))?;

    Ok(NotaBoundaryAskReport {
        status: "recorded".to_string(),
        transaction,
        ask,
        next_step,
        receipt,
        superseded_transaction_ids,
    })
}

pub fn accept_current_runtime_round(
    data_store: &DataStore,
    request: NotaCurrentRoundAcceptanceRequest,
) -> Result<NotaCurrentRoundAcceptanceReport> {
    sync_runtime_truth(data_store, None)?;
    let current_checkpoint = current_runtime_checkpoint(data_store, "current round acceptance")?;
    let checkpoint_scope_ids = current_runtime_checkpoint_scope(data_store, &current_checkpoint)?;
    if let Some(acceptance_bundle) =
        derive_current_runtime_acceptance_bundle(data_store, &checkpoint_scope_ids)?
    {
        return Ok(NotaCurrentRoundAcceptanceReport {
            status: "already_recorded".to_string(),
            acceptance_bundle,
            superseded_transaction_ids: Vec::new(),
        });
    }

    let current_human_round = derive_current_runtime_human_round(data_store)?
        .context("current round acceptance requires a materialized human round")?;
    let superseded_transactions = list_nota_runtime_transactions(data_store)?
        .transactions
        .into_iter()
        .filter(|transaction| {
            transaction.cadence_checkpoint_id == Some(current_checkpoint.cadence_object.id)
                && is_boundary_intake_transaction_open(transaction)
        })
        .collect::<Vec<_>>();
    let superseded_transaction_ids = superseded_transactions
        .iter()
        .map(|transaction| transaction.id)
        .collect::<Vec<_>>();
    let payload = CadenceAcceptanceBundlePayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        transaction_id: 0,
        allocation_id: 0,
        lineage_ref: build_current_round_acceptance_lineage_ref(
            current_checkpoint.cadence_object.id,
            current_human_round.cadence_object.id,
        ),
        acceptance_kind: HUMAN_ROUND_ACCEPTANCE_KIND.to_string(),
        round_state: "accepted".to_string(),
        fully_settled: false,
        child_dispatch_role: "human".to_string(),
        execution_host: NOTA_BOUNDARY_EXECUTION_HOST.to_string(),
        target_kind: "cadence_human_round".to_string(),
        target_ref: current_human_round.cadence_object.id.to_string(),
        review_verdict: None,
        integrate_outcome: None,
        finalize_state: None,
    };
    let title = "Acceptance bundle: current round acceptance".to_string();
    let summary = normalize_optional(request.summary.as_deref()).unwrap_or_else(|| {
        format!(
            "Acceptance is formalized for current human round {} on checkpoint {}.",
            current_human_round.cadence_object.id, current_checkpoint.cadence_object.id
        )
    });
    let payload_json = serde_json::to_string(&payload)
        .context("failed to serialize current round acceptance payload")?;

    let acceptance_bundle = data_store.with_immediate_transaction(|tx| {
        update_boundary_intake_transactions_to_superseded(tx, &superseded_transactions)?;

        let cadence_object = tx.insert_cadence_object(NewCadenceObject {
            cadence_kind: CADENCE_ACCEPTANCE_BUNDLE_KIND,
            title: &title,
            summary: &summary,
            payload_json: &payload_json,
            scope_type: NOTA_RUNTIME_SCOPE_TYPE,
            scope_ref: NOTA_RUNTIME_SCOPE_REF,
            source_type: NOTA_RUNTIME_SOURCE_TYPE,
            source_ref: "nota_runtime:current_round_acceptance_bundle",
            admission_policy: admission_policy_for_kind(CADENCE_ACCEPTANCE_BUNDLE_KIND),
            projection_policy: projection_policy_for_kind(CADENCE_ACCEPTANCE_BUNDLE_KIND),
            status: "accepted",
            is_current: true,
        })?;
        tx.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: current_checkpoint.cadence_object.id,
            dst_cadence_object_id: cadence_object.id,
            relation_type: "acceptance_bundle",
            status: "active",
        })?;
        tx.insert_anti_zeno_event(crate::core::data_store::NewAntiZenoEvent {
            checkpoint_id: Some(current_checkpoint.cadence_object.id),
            acceptance_bundle_id: Some(cadence_object.id),
            event_kind: "acceptance_recorded",
            boundary_ref: &payload.lineage_ref,
            budget_axis: "semantic",
            event_weight: 1,
            summary: &summary,
        })?;

        Ok(NotaAcceptanceBundleRecord {
            cadence_object,
            payload: payload.clone(),
        })
    })?;
    sync_runtime_truth(data_store, None)?;

    Ok(NotaCurrentRoundAcceptanceReport {
        status: "recorded".to_string(),
        acceptance_bundle,
        superseded_transaction_ids,
    })
}

pub fn record_dev_return_review(
    data_store: &DataStore,
    request: NotaDevReturnReviewRequest,
) -> Result<NotaDevReturnReviewReport> {
    let verdict = normalize_dev_return_review_verdict(&request.verdict)?;
    let summary = normalize_optional(request.summary.as_deref());
    sync_runtime_truth(data_store, Some(request.transaction_id))?;
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .cloned()
        .context("dev return review requires a current runtime checkpoint")?;
    let allocation = data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == request.allocation_id)
        .with_context(|| {
            format!(
                "runtime allocation `{}` was not found",
                request.allocation_id
            )
        })?;
    if allocation.source_transaction_id != request.transaction_id {
        bail!(
            "runtime allocation `{}` does not belong to transaction `{}`",
            request.allocation_id,
            request.transaction_id
        );
    }
    if allocation.allocation_kind != "forge_dev_dispatch" {
        bail!(
            "runtime allocation `{}` is not a dev dispatch boundary",
            allocation.id
        );
    }
    if allocation.status != "return_ready" {
        bail!(
            "runtime allocation `{}` is not reviewable because status is `{}`",
            allocation.id,
            allocation.status
        );
    }

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse dev review payload for allocation {}",
                allocation.id
            )
        })?;
    let outcome = payload
        .terminal_outcome
        .as_ref()
        .context("dev return review requires a terminal outcome")?;
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        bail!(
            "runtime allocation `{}` is not a returned Done dev boundary",
            allocation.id
        );
    }

    let receipts = data_store.list_nota_runtime_receipts(Some(request.transaction_id))?;
    let review_ready_exists = receipts.iter().any(|receipt| {
        if receipt.receipt_kind != DEV_RETURN_REVIEW_READY_RECEIPT_KIND {
            return false;
        }
        let Ok(payload) =
            serde_json::from_str::<DevReturnReviewReadyReceiptPayload>(&receipt.payload_json)
        else {
            return false;
        };
        payload.checkpoint_id == current_checkpoint.cadence_object.id
            && payload.next_step.transaction_id == request.transaction_id
            && payload.next_step.allocation_id == request.allocation_id
            && payload.next_step.lineage_ref == allocation.lineage_ref
    });
    if !review_ready_exists {
        bail!(
            "runtime transaction `{}` allocation `{}` is not review-ready on the current checkpoint",
            request.transaction_id,
            request.allocation_id
        );
    }

    let review = build_dev_return_review(
        request.transaction_id,
        &allocation,
        &payload,
        outcome,
        Some(verdict.as_str()),
        summary.as_deref(),
    );
    let next_step = build_dev_return_next_step(
        match verdict.as_str() {
            DEV_RETURN_REVIEW_APPROVED_VERDICT => "integrate",
            DEV_RETURN_REVIEW_CHANGES_REQUESTED_VERDICT => "repair",
            _ => unreachable!("verdict should be normalized"),
        },
        request.transaction_id,
        &allocation,
        &payload,
        outcome,
    );
    let receipt_payload = DevReturnReviewRecordedReceiptPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        review: review.clone(),
        next_step: next_step.clone(),
    };

    let matching_receipts = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == request.transaction_id
        })
        .filter_map(|receipt| {
            let payload = serde_json::from_str::<DevReturnReviewRecordedReceiptPayload>(
                &receipt.payload_json,
            )
            .ok()?;
            Some((receipt, payload))
        })
        .filter(|(_, payload)| {
            payload.checkpoint_id == current_checkpoint.cadence_object.id
                && payload.review.transaction_id == request.transaction_id
                && payload.review.allocation_id == request.allocation_id
                && payload.review.lineage_ref == allocation.lineage_ref
        })
        .collect::<Vec<_>>();
    if let Some((receipt, existing_payload)) = matching_receipts.last() {
        if existing_payload == &receipt_payload {
            return Ok(NotaDevReturnReviewReport {
                status: "already_recorded".to_string(),
                review,
                next_step,
                receipt: (*receipt).clone(),
            });
        }
        bail!(
            "a review outcome is already recorded for transaction `{}` allocation `{}` on checkpoint `{}`",
            request.transaction_id,
            request.allocation_id,
            current_checkpoint.cadence_object.id
        );
    }

    let receipt_payload_json = serde_json::to_string(&receipt_payload)
        .context("failed to serialize dev review recorded receipt payload")?;
    let repair_summary = (verdict == DEV_RETURN_REVIEW_CHANGES_REQUESTED_VERDICT).then(|| {
        summary.clone().unwrap_or_else(|| {
            format!(
                "Review requested repair for returned dev boundary {} on checkpoint {}.",
                allocation.lineage_ref, current_checkpoint.cadence_object.id
            )
        })
    });
    let receipt = data_store.with_immediate_transaction(|tx| {
        tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: request.transaction_id,
            receipt_kind: DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
            payload_json: &receipt_payload_json,
            status: "recorded",
        })?;

        if let Some(repair_summary) = repair_summary.as_deref() {
            tx.insert_anti_zeno_event(crate::core::data_store::NewAntiZenoEvent {
                checkpoint_id: Some(current_checkpoint.cadence_object.id),
                acceptance_bundle_id: None,
                event_kind: "repair_requested",
                boundary_ref: &allocation.lineage_ref,
                budget_axis: "repair",
                event_weight: 1,
                summary: repair_summary,
            })?;
        }

        tx.list_nota_runtime_receipts(Some(request.transaction_id))?
            .into_iter()
            .rev()
            .find(|receipt| {
                receipt.receipt_kind == DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
                    && serde_json::from_str::<DevReturnReviewRecordedReceiptPayload>(
                        &receipt.payload_json,
                    )
                    .map(|payload| payload == receipt_payload)
                    .unwrap_or(false)
            })
            .context("dev review recorded receipt should be readable after append")
    })?;
    sync_runtime_truth(data_store, Some(request.transaction_id))?;

    Ok(NotaDevReturnReviewReport {
        status: "recorded".to_string(),
        review,
        next_step,
        receipt,
    })
}

pub fn record_dev_return_integration(
    data_store: &DataStore,
    request: NotaDevReturnIntegrateRequest,
) -> Result<NotaDevReturnIntegrateReport> {
    let state = normalize_dev_return_integrate_state(&request.state)?;
    let summary = normalize_optional(request.summary.as_deref());
    sync_runtime_truth(data_store, Some(request.transaction_id))?;
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .cloned()
        .context("dev return integrate requires a current runtime checkpoint")?;
    let allocation = data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == request.allocation_id)
        .with_context(|| {
            format!(
                "runtime allocation `{}` was not found",
                request.allocation_id
            )
        })?;
    if allocation.source_transaction_id != request.transaction_id {
        bail!(
            "runtime allocation `{}` does not belong to transaction `{}`",
            request.allocation_id,
            request.transaction_id
        );
    }
    if allocation.allocation_kind != "forge_dev_dispatch" {
        bail!(
            "runtime allocation `{}` is not a dev dispatch boundary",
            allocation.id
        );
    }
    if allocation.status != "return_ready" {
        bail!(
            "runtime allocation `{}` is not integrate-ready because status is `{}`",
            allocation.id,
            allocation.status
        );
    }

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse dev integrate payload for allocation {}",
                allocation.id
            )
        })?;
    let outcome = payload
        .terminal_outcome
        .as_ref()
        .context("dev return integrate requires a terminal outcome")?;
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        bail!(
            "runtime allocation `{}` is not a returned Done dev boundary",
            allocation.id
        );
    }

    let receipts = data_store.list_nota_runtime_receipts(Some(request.transaction_id))?;
    let Some((_, approved_review)) = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == request.transaction_id
        })
        .filter_map(|receipt| {
            let payload = serde_json::from_str::<DevReturnReviewRecordedReceiptPayload>(
                &receipt.payload_json,
            )
            .ok()?;
            Some((receipt.id, payload))
        })
        .filter(|(_, payload)| {
            payload.checkpoint_id == current_checkpoint.cadence_object.id
                && payload.review.transaction_id == request.transaction_id
                && payload.review.allocation_id == request.allocation_id
                && payload.review.lineage_ref == allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
    else {
        bail!(
            "runtime transaction `{}` allocation `{}` is not integrate-ready on the current checkpoint",
            request.transaction_id,
            request.allocation_id
        );
    };
    if approved_review.review.verdict.as_deref() != Some(DEV_RETURN_REVIEW_APPROVED_VERDICT) {
        bail!(
            "runtime transaction `{}` allocation `{}` requires an approved review before integrate",
            request.transaction_id,
            request.allocation_id
        );
    }

    let integrate = build_dev_return_integrate(
        request.transaction_id,
        &allocation,
        &payload,
        outcome,
        state.as_str(),
        summary.as_deref(),
    );
    let next_step = build_dev_return_integrate_next_step(
        state.as_str(),
        request.transaction_id,
        &allocation,
        &payload,
        outcome,
    );
    let receipt_payload = DevReturnIntegrateRecordedReceiptPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        integrate: integrate.clone(),
        next_step: next_step.clone(),
    };

    let matching_receipts = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == request.transaction_id
        })
        .filter_map(|receipt| {
            let payload = serde_json::from_str::<DevReturnIntegrateRecordedReceiptPayload>(
                &receipt.payload_json,
            )
            .ok()?;
            Some((receipt, payload))
        })
        .filter(|(_, payload)| {
            payload.checkpoint_id == current_checkpoint.cadence_object.id
                && payload.integrate.transaction_id == request.transaction_id
                && payload.integrate.allocation_id == request.allocation_id
                && payload.integrate.lineage_ref == allocation.lineage_ref
        })
        .collect::<Vec<_>>();
    if let Some((receipt, existing_payload)) = matching_receipts.last() {
        if existing_payload == &receipt_payload {
            return Ok(NotaDevReturnIntegrateReport {
                status: "already_recorded".to_string(),
                integrate,
                next_step,
                receipt: (*receipt).clone(),
            });
        }
        if existing_payload.integrate.outcome.is_some() {
            bail!(
                "an integrate outcome is already recorded for transaction `{}` allocation `{}` on checkpoint `{}`",
                request.transaction_id,
                request.allocation_id,
                current_checkpoint.cadence_object.id
            );
        }
        if existing_payload.integrate.state == DEV_RETURN_INTEGRATE_STARTED_RUNTIME_STATE
            && integrate.outcome.is_none()
        {
            bail!(
                "integration is already started for transaction `{}` allocation `{}` on checkpoint `{}`",
                request.transaction_id,
                request.allocation_id,
                current_checkpoint.cadence_object.id
            );
        }
    }

    let receipt_payload_json = serde_json::to_string(&receipt_payload)
        .context("failed to serialize dev integrate recorded receipt payload")?;
    let repair_summary = (state == DEV_RETURN_INTEGRATE_REPAIR_REQUESTED_STATE).then(|| {
        summary.clone().unwrap_or_else(|| {
            format!(
                "Integration requested repair for returned dev boundary {} on checkpoint {}.",
                allocation.lineage_ref, current_checkpoint.cadence_object.id
            )
        })
    });
    let receipt = data_store.with_immediate_transaction(|tx| {
        tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: request.transaction_id,
            receipt_kind: DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND,
            payload_json: &receipt_payload_json,
            status: "recorded",
        })?;

        if let Some(repair_summary) = repair_summary.as_deref() {
            tx.insert_anti_zeno_event(crate::core::data_store::NewAntiZenoEvent {
                checkpoint_id: Some(current_checkpoint.cadence_object.id),
                acceptance_bundle_id: None,
                event_kind: "repair_requested",
                boundary_ref: &allocation.lineage_ref,
                budget_axis: "repair",
                event_weight: 1,
                summary: repair_summary,
            })?;
        }

        tx.list_nota_runtime_receipts(Some(request.transaction_id))?
            .into_iter()
            .rev()
            .find(|receipt| {
                receipt.receipt_kind == DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
                    && serde_json::from_str::<DevReturnIntegrateRecordedReceiptPayload>(
                        &receipt.payload_json,
                    )
                    .map(|payload| payload == receipt_payload)
                    .unwrap_or(false)
            })
            .context("dev integrate recorded receipt should be readable after append")
    })?;
    sync_runtime_truth(data_store, Some(request.transaction_id))?;

    Ok(NotaDevReturnIntegrateReport {
        status: "recorded".to_string(),
        integrate,
        next_step,
        receipt,
    })
}

pub fn record_dev_return_finalize(
    data_store: &DataStore,
    request: NotaDevReturnFinalizeRequest,
) -> Result<NotaDevReturnFinalizeReport> {
    let summary = normalize_optional(request.summary.as_deref());
    sync_runtime_truth(data_store, Some(request.transaction_id))?;
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
        .cloned()
        .context("dev return finalize requires a current runtime checkpoint")?;
    let allocation = data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .find(|allocation| allocation.id == request.allocation_id)
        .with_context(|| {
            format!(
                "runtime allocation `{}` was not found",
                request.allocation_id
            )
        })?;
    if allocation.source_transaction_id != request.transaction_id {
        bail!(
            "runtime allocation `{}` does not belong to transaction `{}`",
            request.allocation_id,
            request.transaction_id
        );
    }
    if allocation.allocation_kind != "forge_dev_dispatch" {
        bail!(
            "runtime allocation `{}` is not a dev dispatch boundary",
            allocation.id
        );
    }
    if allocation.status != "return_ready" {
        bail!(
            "runtime allocation `{}` is not finalize-ready because status is `{}`",
            allocation.id,
            allocation.status
        );
    }

    let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse dev finalize payload for allocation {}",
                allocation.id
            )
        })?;
    let outcome = payload
        .terminal_outcome
        .as_ref()
        .context("dev return finalize requires a terminal outcome")?;
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        bail!(
            "runtime allocation `{}` is not a returned Done dev boundary",
            allocation.id
        );
    }

    let receipts = data_store.list_nota_runtime_receipts(Some(request.transaction_id))?;
    let Some((_, integrated_receipt)) = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == request.transaction_id
        })
        .filter_map(|receipt| {
            let payload = serde_json::from_str::<DevReturnIntegrateRecordedReceiptPayload>(
                &receipt.payload_json,
            )
            .ok()?;
            Some((receipt.id, payload))
        })
        .filter(|(_, payload)| {
            payload.checkpoint_id == current_checkpoint.cadence_object.id
                && payload.integrate.transaction_id == request.transaction_id
                && payload.integrate.allocation_id == request.allocation_id
                && payload.integrate.lineage_ref == allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
    else {
        bail!(
            "runtime transaction `{}` allocation `{}` is not finalize-ready on the current checkpoint",
            request.transaction_id,
            request.allocation_id
        );
    };
    if integrated_receipt.integrate.outcome.as_deref()
        != Some(DEV_RETURN_INTEGRATE_INTEGRATED_STATE)
    {
        bail!(
            "runtime transaction `{}` allocation `{}` requires an integrated outcome before finalize",
            request.transaction_id,
            request.allocation_id
        );
    }

    let finalize = build_dev_return_finalize(
        request.transaction_id,
        &allocation,
        &payload,
        outcome,
        summary.as_deref(),
    );
    let receipt_payload = DevReturnFinalizeRecordedReceiptPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        finalize: finalize.clone(),
    };

    let matching_receipts = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == request.transaction_id
        })
        .filter_map(|receipt| {
            let payload = serde_json::from_str::<DevReturnFinalizeRecordedReceiptPayload>(
                &receipt.payload_json,
            )
            .ok()?;
            Some((receipt, payload))
        })
        .filter(|(_, payload)| {
            payload.checkpoint_id == current_checkpoint.cadence_object.id
                && payload.finalize.transaction_id == request.transaction_id
                && payload.finalize.allocation_id == request.allocation_id
                && payload.finalize.lineage_ref == allocation.lineage_ref
        })
        .collect::<Vec<_>>();
    if let Some((receipt, existing_payload)) = matching_receipts.last() {
        if existing_payload == &receipt_payload {
            return Ok(NotaDevReturnFinalizeReport {
                status: "already_recorded".to_string(),
                finalize,
                next_step: None,
                receipt: (*receipt).clone(),
            });
        }
        bail!(
            "a finalize outcome is already recorded for transaction `{}` allocation `{}` on checkpoint `{}`",
            request.transaction_id,
            request.allocation_id,
            current_checkpoint.cadence_object.id
        );
    }

    let receipt_payload_json = serde_json::to_string(&receipt_payload)
        .context("failed to serialize dev finalize recorded receipt payload")?;
    let receipt = data_store.with_immediate_transaction(|tx| {
        tx.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: request.transaction_id,
            receipt_kind: DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND,
            payload_json: &receipt_payload_json,
            status: "recorded",
        })?;

        tx.list_nota_runtime_receipts(Some(request.transaction_id))?
            .into_iter()
            .rev()
            .find(|receipt| {
                receipt.receipt_kind == DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
                    && serde_json::from_str::<DevReturnFinalizeRecordedReceiptPayload>(
                        &receipt.payload_json,
                    )
                    .map(|payload| payload == receipt_payload)
                    .unwrap_or(false)
            })
            .context("dev finalize recorded receipt should be readable after append")
    })?;
    sync_runtime_truth(data_store, Some(request.transaction_id))?;

    Ok(NotaDevReturnFinalizeReport {
        status: "recorded".to_string(),
        finalize,
        next_step: None,
        receipt,
    })
}

fn normalize_dev_return_review_verdict(raw: &str) -> Result<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        DEV_RETURN_REVIEW_APPROVED_VERDICT | DEV_RETURN_REVIEW_CHANGES_REQUESTED_VERDICT => {
            Ok(normalized)
        }
        _ => bail!(
            "unsupported dev return review verdict `{raw}`; use `approved` or `changes_requested`"
        ),
    }
}

fn normalize_dev_return_integrate_state(raw: &str) -> Result<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        DEV_RETURN_INTEGRATE_STARTED_STATE
        | DEV_RETURN_INTEGRATE_INTEGRATED_STATE
        | DEV_RETURN_INTEGRATE_REPAIR_REQUESTED_STATE => Ok(normalized),
        _ => bail!(
            "unsupported dev return integrate state `{raw}`; use `started`, `integrated`, or `repair_requested`"
        ),
    }
}

fn build_dev_return_review(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    payload: &NotaDoAllocationPayload,
    outcome: &NotaDoAllocationTerminalOutcome,
    verdict: Option<&str>,
    summary: Option<&str>,
) -> NotaRuntimeReview {
    NotaRuntimeReview {
        state: if verdict.is_some() {
            "review_recorded".to_string()
        } else {
            "review_ready".to_string()
        },
        transaction_id,
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        child_dispatch_role: payload.child_dispatch_role.clone(),
        execution_host: payload.execution_host.clone(),
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
        verdict: verdict.map(str::to_string),
        summary: normalize_optional(summary),
    }
}

fn build_dev_return_integrate(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    payload: &NotaDoAllocationPayload,
    outcome: &NotaDoAllocationTerminalOutcome,
    state: &str,
    summary: Option<&str>,
) -> NotaRuntimeIntegrate {
    let (runtime_state, integrate_outcome) = match state {
        DEV_RETURN_INTEGRATE_STARTED_STATE => (DEV_RETURN_INTEGRATE_STARTED_RUNTIME_STATE, None),
        DEV_RETURN_INTEGRATE_INTEGRATED_STATE => (
            DEV_RETURN_INTEGRATE_RECORDED_RUNTIME_STATE,
            Some(DEV_RETURN_INTEGRATE_INTEGRATED_STATE.to_string()),
        ),
        DEV_RETURN_INTEGRATE_REPAIR_REQUESTED_STATE => (
            DEV_RETURN_INTEGRATE_RECORDED_RUNTIME_STATE,
            Some(DEV_RETURN_INTEGRATE_REPAIR_REQUESTED_STATE.to_string()),
        ),
        _ => unreachable!("integrate state should be normalized"),
    };

    NotaRuntimeIntegrate {
        state: runtime_state.to_string(),
        transaction_id,
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        child_dispatch_role: payload.child_dispatch_role.clone(),
        execution_host: payload.execution_host.clone(),
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
        outcome: integrate_outcome,
        summary: normalize_optional(summary),
    }
}

fn build_dev_return_finalize(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    payload: &NotaDoAllocationPayload,
    outcome: &NotaDoAllocationTerminalOutcome,
    summary: Option<&str>,
) -> NotaRuntimeFinalize {
    NotaRuntimeFinalize {
        state: DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE.to_string(),
        transaction_id,
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        child_dispatch_role: payload.child_dispatch_role.clone(),
        execution_host: payload.execution_host.clone(),
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
        summary: normalize_optional(summary),
    }
}

fn build_dev_return_next_step(
    step: &str,
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    payload: &NotaDoAllocationPayload,
    outcome: &NotaDoAllocationTerminalOutcome,
) -> NotaRuntimeNextStep {
    NotaRuntimeNextStep {
        step: step.to_string(),
        transaction_id,
        allocation_id: allocation.id,
        lineage_ref: allocation.lineage_ref.clone(),
        child_dispatch_role: payload.child_dispatch_role.clone(),
        execution_host: payload.execution_host.clone(),
        target_kind: outcome.target_kind.clone(),
        target_ref: outcome.target_ref.clone(),
    }
}

fn build_dev_return_integrate_next_step(
    state: &str,
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    payload: &NotaDoAllocationPayload,
    outcome: &NotaDoAllocationTerminalOutcome,
) -> Option<NotaRuntimeNextStep> {
    match state {
        DEV_RETURN_INTEGRATE_STARTED_STATE => None,
        DEV_RETURN_INTEGRATE_INTEGRATED_STATE => Some(build_dev_return_next_step(
            "finalize",
            transaction_id,
            allocation,
            payload,
            outcome,
        )),
        DEV_RETURN_INTEGRATE_REPAIR_REQUESTED_STATE => Some(build_dev_return_next_step(
            "repair",
            transaction_id,
            allocation,
            payload,
            outcome,
        )),
        _ => unreachable!("integrate state should be normalized"),
    }
}

fn build_dev_return_review_next_step(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    payload: &NotaDoAllocationPayload,
    outcome: &NotaDoAllocationTerminalOutcome,
) -> NotaRuntimeNextStep {
    build_dev_return_next_step("review", transaction_id, allocation, payload, outcome)
}

fn recommend_single_lane_allocator_checkpoint_candidate(
    data_store: &DataStore,
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &[StoredNotaRuntimeAllocation],
) -> Result<Option<RecommendedCheckpointCandidate>> {
    let Some(latest_allocation) = active_lane_allocation(
        checkpoint_scope_ids,
        transactions,
        allocations,
        RuntimeBoundaryLane::Agent,
    ) else {
        return Ok(None);
    };

    let allocation_payload: NotaDoAllocationPayload =
        serde_json::from_str(&latest_allocation.payload_json).with_context(|| {
            format!(
                "failed to parse latest allocator continuity payload for allocation {}",
                latest_allocation.id
            )
        })?;
    let Some(outcome) = allocation_payload.terminal_outcome.as_ref() else {
        return Ok(None);
    };

    let transaction_id = latest_allocation.source_transaction_id;
    let receipts = data_store.list_nota_runtime_receipts(Some(transaction_id))?;
    let current_acceptance_bundle = if checkpoint_scope_ids.is_empty() {
        None
    } else {
        derive_current_runtime_acceptance_bundle(data_store, checkpoint_scope_ids)?
    };
    let terminal_receipt_fact = latest_terminal_receipt_for_allocation(&receipts, latest_allocation)?
        .map(|receipt| {
            format!(
                "Transaction {transaction_id} receipt history includes terminal receipt {ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND} capturing allocation {} back to {} {}.",
                latest_allocation.id,
                receipt.target_kind,
                receipt.target_ref
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Allocation {} already projects terminal outcome {} / {} back to {} {}, but the terminal outcome receipt still needs to be persisted by an explicit runtime write.",
                latest_allocation.id,
                outcome.boundary_kind,
                outcome.child_execution_status,
                outcome.target_kind,
                outcome.target_ref
            )
        });

    let (kind, recommendation) = if outcome.boundary_kind == "return"
        && outcome.child_execution_status == "Done"
    {
        let closure_ready = current_acceptance_bundle
            .as_ref()
            .map(|bundle| {
                bundle.payload.acceptance_kind == "agent_return_acceptance"
                    && bundle.payload.allocation_id == latest_allocation.id
                    && !bundle.payload.fully_settled
            })
            .unwrap_or(false);

        if closure_ready {
            (
                RecommendedCheckpointCandidateKind::AgentReturnClosure,
                NotaCheckpointRequest {
                    title: Some(format!(
                        "Checkpoint: agent return closure truth for {}",
                        allocation_payload.issue_id
                    )),
                    stable_level:
                        "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned closed agent-return boundary carried forward as storage-backed checkpoint truth"
                            .to_string(),
                    landed: vec![
                        format!(
                            "NOTA-owned agent allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                            latest_allocation.id,
                            latest_allocation.lineage_ref,
                            transaction_id,
                            latest_allocation.child_execution_ref
                        ),
                        format!(
                            "Agent allocation {} terminal outcome remains return / Done back to {} {}.",
                            latest_allocation.id,
                            outcome.target_kind,
                            outcome.target_ref
                        ),
                        format!(
                            "Checkpoint carry-forward closes the accepted agent boundary on lineage `{}` without reopening a review / integrate loop.",
                            latest_allocation.lineage_ref
                        ),
                        format!(
                            "Transaction {transaction_id} already preserves {AGENT_RETURN_ACCEPTED_RECEIPT_KIND} for allocation {}.",
                            latest_allocation.id
                        ),
                    ],
                    remaining: vec![
                        "This cut closes the current agent-return boundary, not fuller V0 closure or a general multi-role allocator."
                            .to_string(),
                        "Keep this checkpoint scoped to carry-forward for the already accepted boundary; do not infer a second truth plane or promote ARCH into a V0 runtime peer yet."
                            .to_string(),
                    ],
                    human_continuity_bus:
                        "further reduced for this boundary; a fresh window can resume from checkpointed closure truth"
                            .to_string(),
                    selected_trunk: Some("agent return closure truth".to_string()),
                    next_start_hints: vec![
                        "Start from `entrance nota status`, then `entrance nota overview`, then `entrance nota checkpoints`."
                            .to_string(),
                        format!(
                            "Treat lineage `{}` as a closed agent-return boundary; do not reopen it unless a new runtime transaction or allocation is created.",
                            latest_allocation.lineage_ref
                        ),
                        format!(
                            "Use `entrance nota receipts --transaction-id {transaction_id}` when you need the full receipt chain behind the active closure checkpoint."
                        ),
                    ],
                    project_dir: normalize_optional(Some(allocation_payload.project_root.as_str())),
                },
            )
        } else {
            (
                RecommendedCheckpointCandidateKind::AgentReturnAcceptance,
                NotaCheckpointRequest {
                    title: Some(format!(
                        "Checkpoint: agent return acceptance truth for {}",
                        allocation_payload.issue_id
                    )),
                    stable_level:
                        "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned agent return boundary surfaced as storage-backed acceptance truth"
                            .to_string(),
                    landed: vec![
                        format!(
                            "NOTA-owned agent allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                            latest_allocation.id,
                            latest_allocation.lineage_ref,
                            transaction_id,
                            latest_allocation.child_execution_ref
                        ),
                        format!(
                            "Agent allocation {} terminal outcome is return / Done back to {} {}.",
                            latest_allocation.id,
                            outcome.target_kind,
                            outcome.target_ref
                        ),
                        terminal_receipt_fact.clone(),
                        format!(
                            "Runtime payloads keep execution_host `{}` and child_dispatch_role `{}` visible for transaction {} / allocation {}.",
                            allocation_payload.execution_host,
                            allocation_payload.child_dispatch_role,
                            transaction_id,
                            latest_allocation.id
                        ),
                    ],
                    remaining: vec![
                        "This is a returned agent child boundary, not a completed review / integrate / repair loop; fuller allocator closure is still open."
                            .to_string(),
                        "Keep this cut scoped to agent return acceptance truth; dev lane, permission wiring, and a fuller multi-role allocator are still not landed."
                            .to_string(),
                    ],
                    human_continuity_bus:
                        "reduced but still required for acceptance and follow-on integration"
                            .to_string(),
                    selected_trunk: Some("agent return acceptance truth".to_string()),
                    next_start_hints: vec![
                        format!(
                            "Start from `entrance nota status`, then `entrance nota allocations`, then `entrance nota receipts --transaction-id {transaction_id}`."
                        ),
                        format!(
                            "Confirm allocation {} still carries child_dispatch_role `{}`, execution_host `{}`, and terminal_outcome return / Done before any acceptance write.",
                            latest_allocation.id,
                            allocation_payload.child_dispatch_role,
                            allocation_payload.execution_host
                        ),
                        format!(
                            "Treat lineage `{}` as a returned agent boundary only; do not collapse it into full allocator closure or a multi-role allocator.",
                            latest_allocation.lineage_ref
                        ),
                    ],
                    project_dir: normalize_optional(Some(allocation_payload.project_root.as_str())),
                },
            )
        }
    } else {
        let outcome_fact = match outcome.child_execution_status_message.as_deref() {
                Some(message) => format!(
                    "Allocation {} terminal outcome is {} / {} back to {} {} with status message `{message}`.",
                    latest_allocation.id,
                    outcome.boundary_kind,
                    outcome.child_execution_status,
                    outcome.target_kind,
                    outcome.target_ref
                ),
                None => format!(
                    "Allocation {} terminal outcome is {} / {} back to {} {}.",
                    latest_allocation.id,
                    outcome.boundary_kind,
                    outcome.child_execution_status,
                    outcome.target_kind,
                    outcome.target_ref
                ),
            };
        let current_gate = match outcome.child_execution_status_message.as_deref() {
            Some(message) => format!(
                "L3 remains open until the current {} gate is cleared: {message}.",
                outcome.child_execution_status
            ),
            None => format!(
                "L3 remains open until the current {} gate is cleared.",
                outcome.child_execution_status
            ),
        };

        (
                RecommendedCheckpointCandidateKind::AgentEscalationContinuity,
                NotaCheckpointRequest {
                    title: Some(format!(
                        "Checkpoint: agent escalation continuity for {}",
                        allocation_payload.issue_id
                    )),
                    stable_level:
                        "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned agent escalation boundary checkpointed into runtime continuity"
                            .to_string(),
                    landed: vec![
                        format!(
                            "NOTA-owned agent allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                            latest_allocation.id,
                            latest_allocation.lineage_ref,
                            transaction_id,
                            latest_allocation.child_execution_ref
                        ),
                        outcome_fact,
                        terminal_receipt_fact,
                        "Dedicated headless CLI and MCP read boundaries now expose the same runtime slice through `entrance nota overview` / `allocations` / `receipts` and `nota_runtime_overview` / `nota_runtime_allocations` / `nota_runtime_receipts`.".to_string(),
                    ],
                    remaining: vec![
                        current_gate,
                        "Keep this checkpoint scoped to agent escalation continuity; return acceptance, dev lane, permission wiring, and a fuller multi-role allocator are still not landed.".to_string(),
                    ],
                    human_continuity_bus: if outcome.boundary_kind == "escalation" {
                        "reduced but still required for escalation resolution".to_string()
                    } else {
                        "reduced but still required for return integration".to_string()
                    },
                    selected_trunk: Some("agent escalation continuity".to_string()),
                    next_start_hints: vec![
                        format!(
                            "Start from `entrance nota overview`, then `entrance nota allocations`, then `entrance nota receipts --transaction-id {transaction_id}`."
                        ),
                        format!(
                            "If you are on MCP, read `nota_runtime_overview`, `nota_runtime_allocations`, and `nota_runtime_receipts` for transaction {transaction_id} before any new write."
                        ),
                        format!(
                            "Treat lineage `{}` as the current agent escalation boundary until the {} gate is cleared.",
                            latest_allocation.lineage_ref,
                            outcome.child_execution_status
                        ),
                    ],
                    project_dir: normalize_optional(Some(allocation_payload.project_root.as_str())),
                },
            )
    };

    Ok(Some(RecommendedCheckpointCandidate {
        kind,
        allocation_id: latest_allocation.id,
        source_transaction_id: transaction_id,
        request: recommendation,
    }))
}

fn recommend_dev_return_checkpoint_candidate(
    data_store: &DataStore,
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &[StoredNotaRuntimeAllocation],
) -> Result<Option<RecommendedCheckpointCandidate>> {
    let Some(latest_allocation) = active_lane_allocation(
        checkpoint_scope_ids,
        transactions,
        allocations,
        RuntimeBoundaryLane::Dev,
    ) else {
        return Ok(None);
    };

    let allocation_payload: NotaDoAllocationPayload =
        serde_json::from_str(&latest_allocation.payload_json).with_context(|| {
            format!(
                "failed to parse latest dev closure payload for allocation {}",
                latest_allocation.id
            )
        })?;
    let Some(outcome) = allocation_payload.terminal_outcome.as_ref() else {
        return Ok(None);
    };
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        return Ok(None);
    }

    let transaction_id = latest_allocation.source_transaction_id;
    let receipts = data_store.list_nota_runtime_receipts(Some(transaction_id))?;
    let terminal_receipt_fact = latest_terminal_receipt_for_allocation(&receipts, latest_allocation)?
        .map(|receipt| {
            format!(
                "Transaction {transaction_id} receipt history includes terminal receipt {ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND} capturing allocation {} back to {} {}.",
                latest_allocation.id,
                receipt.target_kind,
                receipt.target_ref
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Allocation {} already projects terminal outcome return / Done back to {} {}, but the terminal outcome receipt still needs to be persisted by an explicit runtime write.",
                latest_allocation.id,
                outcome.target_kind,
                outcome.target_ref
            )
        });

    let latest_review =
        latest_dev_return_review_recorded_for_boundary(&receipts, latest_allocation)?;
    let latest_integrate =
        latest_dev_return_integrate_recorded_for_boundary(&receipts, latest_allocation)?;
    let latest_finalize =
        latest_dev_return_finalize_recorded_for_boundary(&receipts, latest_allocation)?;

    if latest_review
        .as_ref()
        .and_then(|review| review.verdict.as_deref())
        == Some(DEV_RETURN_REVIEW_APPROVED_VERDICT)
        && latest_integrate
            .as_ref()
            .and_then(|integrate| integrate.outcome.as_deref())
            == Some(DEV_RETURN_INTEGRATE_INTEGRATED_STATE)
        && latest_finalize
            .as_ref()
            .map(|finalize| finalize.state.as_str())
            == Some(DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE)
    {
        let recommendation = NotaCheckpointRequest {
            title: Some(format!(
                "Checkpoint: dev return closure truth for {}",
                allocation_payload.issue_id
            )),
            stable_level:
                "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned closed dev-return boundary carried forward as storage-backed checkpoint truth"
                    .to_string(),
            landed: vec![
                format!(
                    "NOTA-owned dev allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                    latest_allocation.id,
                    latest_allocation.lineage_ref,
                    transaction_id,
                    latest_allocation.child_execution_ref
                ),
                format!(
                    "Review truth is recorded as `{}` for transaction {} allocation {}.",
                    DEV_RETURN_REVIEW_APPROVED_VERDICT,
                    transaction_id,
                    latest_allocation.id
                ),
                format!(
                    "Integrate truth is recorded as `{}` for transaction {} allocation {}.",
                    DEV_RETURN_INTEGRATE_INTEGRATED_STATE,
                    transaction_id,
                    latest_allocation.id
                ),
                format!(
                    "Finalize truth is recorded as `{}` for transaction {} allocation {} on lineage `{}`.",
                    DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE,
                    transaction_id,
                    latest_allocation.id,
                    latest_allocation.lineage_ref
                ),
                format!(
                    "Transaction {transaction_id} receipt history preserves {DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND}, {DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND}, and {DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND} for allocation {}.",
                    latest_allocation.id
                ),
            ],
            remaining: vec![
                "This cut closes the current dev-return boundary, not full V0 closure or a general multi-role allocator."
                    .to_string(),
                "Keep this checkpoint scoped to checkpoint-side carry-forward for the already-closed boundary; do not infer a second truth plane or a new human round."
                    .to_string(),
            ],
            human_continuity_bus:
                "further reduced for this boundary; a fresh window can resume from checkpoint and receipt closure truth"
                    .to_string(),
            selected_trunk: Some("dev return closure truth".to_string()),
            next_start_hints: vec![
                "Start from `entrance nota status`, then `entrance nota overview`, then `entrance nota checkpoints`."
                    .to_string(),
                format!(
                    "Treat lineage `{}` as a closed dev-return boundary; do not reopen review / integrate / finalize unless a new runtime transaction or allocation is created.",
                    latest_allocation.lineage_ref
                ),
                format!(
                    "Use `entrance nota receipts --transaction-id {transaction_id}` when you need the full receipt chain behind the active closure checkpoint."
                ),
            ],
            project_dir: normalize_optional(Some(allocation_payload.project_root.as_str())),
        };

        return Ok(Some(RecommendedCheckpointCandidate {
            kind: RecommendedCheckpointCandidateKind::DevReturnClosure,
            allocation_id: latest_allocation.id,
            source_transaction_id: transaction_id,
            request: recommendation,
        }));
    }

    let recommendation = NotaCheckpointRequest {
        title: Some(format!(
            "Checkpoint: dev return acceptance truth for {}",
            allocation_payload.issue_id
        )),
        stable_level:
            "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned dev return boundary surfaced as storage-backed acceptance truth"
                .to_string(),
        landed: vec![
            format!(
                "NOTA-owned dev allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                latest_allocation.id,
                latest_allocation.lineage_ref,
                transaction_id,
                latest_allocation.child_execution_ref
            ),
            format!(
                "Dev allocation {} terminal outcome is return / Done back to {} {}.",
                latest_allocation.id,
                outcome.target_kind,
                outcome.target_ref
            ),
            terminal_receipt_fact,
            format!(
                "Runtime payloads keep execution_host `{}` and child_dispatch_role `{}` visible for transaction {} / allocation {}.",
                allocation_payload.execution_host,
                allocation_payload.child_dispatch_role,
                transaction_id,
                latest_allocation.id
            ),
        ],
        remaining: vec![
            "This is a returned dev child boundary, not a completed review / integrate / repair loop; M9 return closure is still open."
                .to_string(),
            "Keep this cut scoped to dev return acceptance truth; V0 closure, multi-role allocator, and fuller acceptance/finalization surfaces are still not landed."
                .to_string(),
        ],
        human_continuity_bus:
            "reduced but still required for acceptance and follow-on integration".to_string(),
        selected_trunk: Some("dev return acceptance truth".to_string()),
        next_start_hints: vec![
            format!(
                "Start from `entrance nota status`, then `entrance nota allocations`, then `entrance nota receipts --transaction-id {transaction_id}`."
            ),
            format!(
                "Confirm allocation {} still carries child_dispatch_role `{}`, execution_host `{}`, and terminal_outcome return / Done before any acceptance write.",
                latest_allocation.id,
                allocation_payload.child_dispatch_role,
                allocation_payload.execution_host
            ),
            format!(
                "Treat lineage `{}` as a returned dev boundary only; do not collapse it into full V0 closure or a complete allocator.",
                latest_allocation.lineage_ref
            ),
        ],
        project_dir: normalize_optional(Some(allocation_payload.project_root.as_str())),
    };

    Ok(Some(RecommendedCheckpointCandidate {
        kind: RecommendedCheckpointCandidateKind::DevReturnAcceptance,
        allocation_id: latest_allocation.id,
        source_transaction_id: transaction_id,
        request: recommendation,
    }))
}

fn latest_terminal_receipt_for_allocation(
    receipts: &[StoredNotaRuntimeReceipt],
    allocation: &StoredNotaRuntimeAllocation,
) -> Result<Option<AllocationTerminalOutcomeReceiptPayload>> {
    Ok(receipts
        .iter()
        .filter(|receipt| receipt.receipt_kind == ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND)
        .map(|receipt| {
            let payload: AllocationTerminalOutcomeReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!(
                        "failed to parse allocation terminal outcome receipt {}",
                        receipt.id
                    )
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            payload.allocation_id == allocation.id && payload.lineage_ref == allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, payload)| payload))
}

fn scoped_checkpoint_rank_map(checkpoint_scope_ids: &[i64]) -> HashMap<i64, usize> {
    checkpoint_scope_ids
        .iter()
        .enumerate()
        .map(|(index, checkpoint_id)| (*checkpoint_id, index))
        .collect()
}

fn transaction_checkpoint_map(transactions: &[StoredNotaRuntimeTransaction]) -> HashMap<i64, i64> {
    transactions
        .iter()
        .filter_map(|transaction| {
            transaction
                .cadence_checkpoint_id
                .map(|checkpoint_id| (transaction.id, checkpoint_id))
        })
        .collect()
}

fn active_scoped_lane_allocation<'a>(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &'a [StoredNotaRuntimeAllocation],
    lane: RuntimeBoundaryLane,
) -> Option<&'a StoredNotaRuntimeAllocation> {
    if checkpoint_scope_ids.is_empty() {
        return None;
    }

    let checkpoint_rank = scoped_checkpoint_rank_map(checkpoint_scope_ids);
    let transaction_checkpoint = transaction_checkpoint_map(transactions);
    let mut selected: Option<(usize, i64, &StoredNotaRuntimeAllocation)> = None;

    for allocation in allocations.iter().filter(|allocation| {
        allocation.allocator_role == "nota"
            && allocation.allocation_kind == lane.allocation_kind()
            && allocation.child_execution_kind == "forge_task"
    }) {
        let Some(checkpoint_id) = transaction_checkpoint
            .get(&allocation.source_transaction_id)
            .copied()
        else {
            continue;
        };
        let Some(scope_rank) = checkpoint_rank.get(&checkpoint_id).copied() else {
            continue;
        };

        match selected {
            Some((selected_rank, _, selected_allocation))
                if selected_rank < scope_rank
                    || (selected_rank == scope_rank && selected_allocation.id >= allocation.id) => {
            }
            _ => selected = Some((scope_rank, checkpoint_id, allocation)),
        }
    }

    selected.map(|(_, _, allocation)| allocation)
}

fn fallback_latest_lane_allocation<'a>(
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &'a [StoredNotaRuntimeAllocation],
    lane: RuntimeBoundaryLane,
) -> Option<&'a StoredNotaRuntimeAllocation> {
    let transaction_checkpoint = transaction_checkpoint_map(transactions);
    let mut selected: Option<(i64, i64, &StoredNotaRuntimeAllocation)> = None;

    for allocation in allocations.iter().filter(|allocation| {
        allocation.allocator_role == "nota"
            && allocation.allocation_kind == lane.allocation_kind()
            && allocation.child_execution_kind == "forge_task"
    }) {
        let checkpoint_id = transaction_checkpoint
            .get(&allocation.source_transaction_id)
            .copied()
            .unwrap_or(i64::MIN);

        match selected {
            Some((selected_checkpoint_id, selected_allocation_id, _))
                if selected_checkpoint_id > checkpoint_id
                    || (selected_checkpoint_id == checkpoint_id
                        && selected_allocation_id >= allocation.id) => {}
            _ => selected = Some((checkpoint_id, allocation.id, allocation)),
        }
    }

    selected.map(|(_, _, allocation)| allocation)
}

fn active_lane_allocation<'a>(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &'a [StoredNotaRuntimeAllocation],
    lane: RuntimeBoundaryLane,
) -> Option<&'a StoredNotaRuntimeAllocation> {
    active_scoped_lane_allocation(checkpoint_scope_ids, transactions, allocations, lane)
        .or_else(|| fallback_latest_lane_allocation(transactions, allocations, lane))
}

fn latest_dev_return_review_recorded_for_boundary(
    receipts: &[StoredNotaRuntimeReceipt],
    allocation: &StoredNotaRuntimeAllocation,
) -> Result<Option<NotaRuntimeReview>> {
    Ok(receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnReviewRecordedReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!("failed to parse dev review recorded receipt {}", receipt.id)
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            payload.review.allocation_id == allocation.id
                && payload.review.transaction_id == allocation.source_transaction_id
                && payload.review.lineage_ref == allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, payload)| payload.review))
}

fn latest_dev_return_integrate_recorded_for_boundary(
    receipts: &[StoredNotaRuntimeReceipt],
    allocation: &StoredNotaRuntimeAllocation,
) -> Result<Option<NotaRuntimeIntegrate>> {
    Ok(receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnIntegrateRecordedReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!(
                        "failed to parse dev integrate recorded receipt {}",
                        receipt.id
                    )
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            payload.integrate.allocation_id == allocation.id
                && payload.integrate.transaction_id == allocation.source_transaction_id
                && payload.integrate.lineage_ref == allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, payload)| payload.integrate))
}

fn latest_dev_return_finalize_recorded_for_boundary(
    receipts: &[StoredNotaRuntimeReceipt],
    allocation: &StoredNotaRuntimeAllocation,
) -> Result<Option<NotaRuntimeFinalize>> {
    Ok(receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnFinalizeRecordedReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!(
                        "failed to parse dev finalize recorded receipt {}",
                        receipt.id
                    )
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            payload.finalize.allocation_id == allocation.id
                && payload.finalize.transaction_id == allocation.source_transaction_id
                && payload.finalize.lineage_ref == allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, payload)| payload.finalize))
}

fn checkpoint_scope_contains(checkpoint_scope_ids: &[i64], checkpoint_id: i64) -> bool {
    checkpoint_scope_ids.contains(&checkpoint_id)
}

pub fn derive_nota_runtime_review(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeReview>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = active_scoped_lane_allocation(
        checkpoint_scope_ids,
        transactions,
        allocations,
        RuntimeBoundaryLane::Dev,
    ) else {
        return Ok(None);
    };
    if latest_dev_allocation.status != "return_ready" {
        return Ok(None);
    }

    let allocation_payload: NotaDoAllocationPayload =
        serde_json::from_str(&latest_dev_allocation.payload_json).with_context(|| {
            format!(
                "failed to parse latest dev review payload for allocation {}",
                latest_dev_allocation.id
            )
        })?;
    let Some(outcome) = allocation_payload.terminal_outcome.as_ref() else {
        return Ok(None);
    };
    if outcome.boundary_kind != "return" || outcome.child_execution_status != "Done" {
        return Ok(None);
    }

    if let Some((_, payload)) = receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == latest_dev_allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnReviewRecordedReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!("failed to parse dev review recorded receipt {}", receipt.id)
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                && payload.review.allocation_id == latest_dev_allocation.id
                && payload.review.transaction_id == latest_dev_allocation.source_transaction_id
                && payload.review.lineage_ref == latest_dev_allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
    {
        return Ok(Some(payload.review));
    }

    Ok(receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_REVIEW_READY_RECEIPT_KIND
                && receipt.transaction_id == latest_dev_allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnReviewReadyReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!("failed to parse dev review-ready receipt {}", receipt.id)
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                && payload.next_step.allocation_id == latest_dev_allocation.id
                && payload.next_step.transaction_id == latest_dev_allocation.source_transaction_id
                && payload.next_step.lineage_ref == latest_dev_allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, _)| {
            build_dev_return_review(
                latest_dev_allocation.source_transaction_id,
                latest_dev_allocation,
                &allocation_payload,
                outcome,
                None,
                None,
            )
        }))
}

pub fn derive_nota_runtime_integrate(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeIntegrate>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = active_scoped_lane_allocation(
        checkpoint_scope_ids,
        transactions,
        allocations,
        RuntimeBoundaryLane::Dev,
    ) else {
        return Ok(None);
    };
    if latest_dev_allocation.status != "return_ready" {
        return Ok(None);
    }

    Ok(receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == latest_dev_allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnIntegrateRecordedReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!(
                        "failed to parse dev integrate recorded receipt {}",
                        receipt.id
                    )
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                && payload.integrate.allocation_id == latest_dev_allocation.id
                && payload.integrate.transaction_id == latest_dev_allocation.source_transaction_id
                && payload.integrate.lineage_ref == latest_dev_allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, payload)| payload.integrate))
}

pub fn derive_nota_runtime_finalize(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeFinalize>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = active_scoped_lane_allocation(
        checkpoint_scope_ids,
        transactions,
        allocations,
        RuntimeBoundaryLane::Dev,
    ) else {
        return Ok(None);
    };
    if latest_dev_allocation.status != "return_ready" {
        return Ok(None);
    }

    Ok(receipts
        .iter()
        .filter(|receipt| {
            receipt.receipt_kind == DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
                && receipt.transaction_id == latest_dev_allocation.source_transaction_id
        })
        .map(|receipt| {
            let payload: DevReturnFinalizeRecordedReceiptPayload =
                serde_json::from_str(&receipt.payload_json).with_context(|| {
                    format!(
                        "failed to parse dev finalize recorded receipt {}",
                        receipt.id
                    )
                })?;
            Ok((receipt.id, payload))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, payload)| {
            checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                && payload.finalize.allocation_id == latest_dev_allocation.id
                && payload.finalize.transaction_id == latest_dev_allocation.source_transaction_id
                && payload.finalize.lineage_ref == latest_dev_allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
        .map(|(_, payload)| payload.finalize))
}

pub fn derive_nota_runtime_next_step(
    checkpoint_scope_ids: &[i64],
    transactions: &[StoredNotaRuntimeTransaction],
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeNextStep>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    if let Some(latest_dev_allocation) = active_scoped_lane_allocation(
        checkpoint_scope_ids,
        transactions,
        allocations,
        RuntimeBoundaryLane::Dev,
    ) {
        if latest_dev_allocation.status == "return_ready" {
            if receipts
                .iter()
                .filter(|receipt| {
                    receipt.receipt_kind == DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
                        && receipt.transaction_id == latest_dev_allocation.source_transaction_id
                })
                .map(|receipt| {
                    let payload: DevReturnFinalizeRecordedReceiptPayload =
                        serde_json::from_str(&receipt.payload_json).with_context(|| {
                            format!(
                                "failed to parse dev finalize recorded receipt {}",
                                receipt.id
                            )
                        })?;
                    Ok((receipt.id, payload))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .any(|(_, payload)| {
                    checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                        && payload.finalize.allocation_id == latest_dev_allocation.id
                        && payload.finalize.transaction_id
                            == latest_dev_allocation.source_transaction_id
                        && payload.finalize.lineage_ref == latest_dev_allocation.lineage_ref
                })
            {
                return derive_open_boundary_intake_next_step(checkpoint_scope_ids, transactions);
            }

            if let Some((_, payload)) = receipts
                .iter()
                .filter(|receipt| {
                    receipt.receipt_kind == DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
                        && receipt.transaction_id == latest_dev_allocation.source_transaction_id
                })
                .map(|receipt| {
                    let payload: DevReturnIntegrateRecordedReceiptPayload =
                        serde_json::from_str(&receipt.payload_json).with_context(|| {
                            format!(
                                "failed to parse dev integrate recorded receipt {}",
                                receipt.id
                            )
                        })?;
                    Ok((receipt.id, payload))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|(_, payload)| {
                    checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                        && payload.integrate.allocation_id == latest_dev_allocation.id
                        && payload.integrate.transaction_id
                            == latest_dev_allocation.source_transaction_id
                        && payload.integrate.lineage_ref == latest_dev_allocation.lineage_ref
                })
                .max_by_key(|(receipt_id, _)| *receipt_id)
            {
                return Ok(payload.next_step);
            }

            if let Some((_, payload)) = receipts
                .iter()
                .filter(|receipt| {
                    receipt.receipt_kind == DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
                        && receipt.transaction_id == latest_dev_allocation.source_transaction_id
                })
                .map(|receipt| {
                    let payload: DevReturnReviewRecordedReceiptPayload =
                        serde_json::from_str(&receipt.payload_json).with_context(|| {
                            format!("failed to parse dev review recorded receipt {}", receipt.id)
                        })?;
                    Ok((receipt.id, payload))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|(_, payload)| {
                    checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                        && payload.review.allocation_id == latest_dev_allocation.id
                        && payload.review.transaction_id
                            == latest_dev_allocation.source_transaction_id
                        && payload.review.lineage_ref == latest_dev_allocation.lineage_ref
                })
                .max_by_key(|(receipt_id, _)| *receipt_id)
            {
                return Ok(Some(payload.next_step));
            }

            if let Some(next_step) = receipts
                .iter()
                .filter(|receipt| {
                    receipt.receipt_kind == DEV_RETURN_REVIEW_READY_RECEIPT_KIND
                        && receipt.transaction_id == latest_dev_allocation.source_transaction_id
                })
                .map(|receipt| {
                    let payload: DevReturnReviewReadyReceiptPayload =
                        serde_json::from_str(&receipt.payload_json).with_context(|| {
                            format!("failed to parse dev review-ready receipt {}", receipt.id)
                        })?;
                    Ok((receipt.id, payload))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|(_, payload)| {
                    checkpoint_scope_contains(checkpoint_scope_ids, payload.checkpoint_id)
                        && payload.next_step.allocation_id == latest_dev_allocation.id
                        && payload.next_step.transaction_id
                            == latest_dev_allocation.source_transaction_id
                        && payload.next_step.lineage_ref == latest_dev_allocation.lineage_ref
                })
                .max_by_key(|(receipt_id, _)| *receipt_id)
                .map(|(_, payload)| payload.next_step)
            {
                return Ok(Some(next_step));
            }
        }
    }

    derive_open_boundary_intake_next_step(checkpoint_scope_ids, transactions)
}

fn checkpoint_request_matches_current(
    current_checkpoint: Option<&NotaCheckpointRecord>,
    request: &NotaCheckpointRequest,
) -> bool {
    let Some(current_checkpoint) = current_checkpoint else {
        return false;
    };

    current_checkpoint.payload.stable_level == request.stable_level.trim()
        && current_checkpoint.payload.landed == normalize_list(request.landed.clone())
        && current_checkpoint.payload.remaining == normalize_list(request.remaining.clone())
        && current_checkpoint.payload.human_continuity_bus == request.human_continuity_bus.trim()
        && current_checkpoint.payload.selected_trunk
            == normalize_optional(request.selected_trunk.as_deref())
        && current_checkpoint.payload.next_start_hints
            == normalize_list(request.next_start_hints.clone())
}

fn parse_checkpoint_record(object: StoredCadenceObject) -> Result<NotaCheckpointRecord> {
    let payload: NotaCheckpointPayload =
        serde_json::from_str(&object.payload_json).with_context(|| {
            format!(
                "failed to parse cadence checkpoint payload for row {}",
                object.id
            )
        })?;

    Ok(NotaCheckpointRecord {
        cadence_object: object,
        payload,
    })
}

fn parse_acceptance_bundle_record(
    object: StoredCadenceObject,
) -> Result<NotaAcceptanceBundleRecord> {
    let payload: CadenceAcceptanceBundlePayload = serde_json::from_str(&object.payload_json)
        .with_context(|| {
            format!(
                "failed to parse cadence acceptance bundle payload for row {}",
                object.id
            )
        })?;

    Ok(NotaAcceptanceBundleRecord {
        cadence_object: object,
        payload,
    })
}

fn parse_human_round_record(object: StoredCadenceObject) -> Result<NotaHumanRoundRecord> {
    let payload: CadenceHumanRoundPayload = serde_json::from_str(&object.payload_json)
        .with_context(|| {
            format!(
                "failed to parse cadence human-round payload for row {}",
                object.id
            )
        })?;

    Ok(NotaHumanRoundRecord {
        cadence_object: object,
        payload,
    })
}

fn parse_handout_record(object: StoredCadenceObject) -> Result<NotaHandoutRecord> {
    let payload: CadenceHandoutPayload =
        serde_json::from_str(&object.payload_json).with_context(|| {
            format!(
                "failed to parse cadence handout payload for row {}",
                object.id
            )
        })?;

    Ok(NotaHandoutRecord {
        cadence_object: object,
        payload,
    })
}

fn parse_wake_request_record(object: StoredCadenceObject) -> Result<NotaWakeRequestRecord> {
    let payload: CadenceWakeRequestPayload = serde_json::from_str(&object.payload_json)
        .with_context(|| {
            format!(
                "failed to parse cadence wake-request payload for row {}",
                object.id
            )
        })?;

    Ok(NotaWakeRequestRecord {
        cadence_object: object,
        payload,
    })
}

// build_checkpoint_summary moved to helpers.rs

pub fn derive_runtime_round_state_projection(
    current_checkpoint: Option<&NotaCheckpointRecord>,
    acceptance_bundle: Option<&NotaAcceptanceBundleRecord>,
    next_step: Option<&NotaRuntimeNextStep>,
) -> NotaRoundStateProjection {
    let checkpoint_id = current_checkpoint.map(|checkpoint| checkpoint.cadence_object.id);
    let acceptance_bundle_id = acceptance_bundle.map(|bundle| bundle.cadence_object.id);
    let acceptance_present = acceptance_bundle.is_some();
    let accepted = acceptance_present;
    let next_step_open = next_step.is_some();
    let carry_forward_checkpointed = match (current_checkpoint, acceptance_bundle) {
        (Some(checkpoint), Some(bundle)) if bundle.payload.fully_settled => {
            bundle.payload.checkpoint_id == checkpoint.cadence_object.id
        }
        _ => false,
    };
    let fully_settled = acceptance_bundle
        .map(|bundle| bundle.payload.fully_settled)
        .unwrap_or(false)
        && !next_step_open
        && carry_forward_checkpointed;
    let next_step_label = next_step.map(|step| step.step.clone());

    let (posture, state, detail_state, summary) =
        match (current_checkpoint, acceptance_bundle, next_step) {
            (None, _, _) => (
                "Opened round".to_string(),
                HumanRoundCanonicalState::Opened.as_str().to_string(),
                HumanRoundDetailState::Uncheckpointed.as_str().to_string(),
                "No checkpoint has anchored the current human round yet.".to_string(),
            ),
            (Some(checkpoint), None, _) => (
                "Checkpointed round without formal acceptance".to_string(),
                HumanRoundCanonicalState::Checkpointed.as_str().to_string(),
                HumanRoundDetailState::CheckpointedPendingAcceptance
                    .as_str()
                    .to_string(),
                format!(
                    "Checkpoint {} has anchored the current human round, but acceptance is not formalized yet.",
                    checkpoint.cadence_object.id
                ),
            ),
            (Some(checkpoint), Some(bundle), Some(step)) => (
                "Settling accepted round with bounded follow-up".to_string(),
                HumanRoundCanonicalState::Settling.as_str().to_string(),
                HumanRoundDetailState::AcceptedFollowupOpen
                    .as_str()
                    .to_string(),
                format!(
                    "Checkpoint {} carries accepted boundary {} with next step `{}` still open.",
                    checkpoint.cadence_object.id, bundle.cadence_object.id, step.step
                ),
            ),
            (Some(checkpoint), Some(bundle), None) if fully_settled => (
                "Fully settled round".to_string(),
                HumanRoundCanonicalState::FullySettled.as_str().to_string(),
                HumanRoundDetailState::FullySettled.as_str().to_string(),
                format!(
                    "Checkpoint {} carries accepted boundary {} with no open next-step or carry-forward debt.",
                    checkpoint.cadence_object.id, bundle.cadence_object.id
                ),
            ),
            (Some(checkpoint), Some(bundle), None) => (
                "Accepted round awaiting carry-forward".to_string(),
                HumanRoundCanonicalState::Accepted.as_str().to_string(),
                HumanRoundDetailState::AcceptedWaitingCarryForward
                    .as_str()
                    .to_string(),
                format!(
                    "Checkpoint {} carries accepted boundary {}, but a fresh carry-forward closure is still required.",
                    checkpoint.cadence_object.id, bundle.cadence_object.id
                ),
            ),
        };

    NotaRoundStateProjection {
        posture,
        state,
        detail_state,
        summary,
        accepted,
        acceptance_present,
        next_step_open,
        carry_forward_checkpointed,
        fully_settled,
        checkpoint_id,
        acceptance_bundle_id,
        next_step: next_step_label,
    }
}

fn anti_zeno_state_for_round_state(
    round_state: &NotaRoundStateProjection,
) -> (String, String, String, u8) {
    if round_state.checkpoint_id.is_none() {
        (
            "Opened round".to_string(),
            HumanRoundCanonicalState::Opened.as_str().to_string(),
            HumanRoundDetailState::Uncheckpointed.as_str().to_string(),
            18,
        )
    } else if round_state.fully_settled {
        (
            "Settled accepted round".to_string(),
            HumanRoundCanonicalState::FullySettled.as_str().to_string(),
            HumanRoundDetailState::FullySettled.as_str().to_string(),
            100,
        )
    } else if round_state.next_step_open {
        (
            "Accepted round with a bounded next cut".to_string(),
            HumanRoundCanonicalState::Settling.as_str().to_string(),
            HumanRoundDetailState::AcceptedFollowupOpen
                .as_str()
                .to_string(),
            78,
        )
    } else if round_state.acceptance_present {
        (
            "Accepted round awaiting carry-forward".to_string(),
            HumanRoundCanonicalState::Accepted.as_str().to_string(),
            HumanRoundDetailState::AcceptedWaitingCarryForward
                .as_str()
                .to_string(),
            88,
        )
    } else {
        (
            "Checkpointed round without formal acceptance".to_string(),
            HumanRoundCanonicalState::Checkpointed.as_str().to_string(),
            HumanRoundDetailState::CheckpointedPendingAcceptance
                .as_str()
                .to_string(),
            44,
        )
    }
}

fn build_human_round_summary(
    checkpoint: &NotaCheckpointRecord,
    round_state: &NotaRoundStateProjection,
) -> String {
    format!(
        "{} Stable level: {}.",
        round_state.summary, checkpoint.payload.stable_level
    )
}

fn materialize_current_runtime_human_round(
    data_store: &DataStore,
) -> Result<Option<NotaHumanRoundRecord>> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let Some(current_checkpoint) = checkpoints
        .checkpoints
        .into_iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
    else {
        return Ok(None);
    };
    let checkpoint_scope_ids = active_checkpoint_scope_ids(data_store, Some(&current_checkpoint))?;
    let transactions = list_nota_runtime_transactions(data_store)?;
    let allocations = list_nota_runtime_allocations(data_store)?;
    let receipts = list_nota_runtime_receipts(data_store, None)?;
    let next_step = derive_nota_runtime_next_step(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let acceptance_bundle =
        derive_current_runtime_acceptance_bundle(data_store, &checkpoint_scope_ids)?;
    let round_state = derive_runtime_round_state_projection(
        Some(&current_checkpoint),
        acceptance_bundle.as_ref(),
        next_step.as_ref(),
    );
    let summary = build_human_round_summary(&current_checkpoint, &round_state);
    let payload = CadenceHumanRoundPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        round_state: round_state.state.clone(),
        detail_round_state: Some(round_state.detail_state.clone()),
        accepted: round_state.accepted,
        acceptance_present: round_state.acceptance_present,
        carry_forward_checkpointed: round_state.carry_forward_checkpointed,
        fully_settled: round_state.fully_settled,
        next_step_open: round_state.next_step_open,
        stable_level: current_checkpoint.payload.stable_level.clone(),
        human_continuity_bus: current_checkpoint.payload.human_continuity_bus.clone(),
        selected_trunk: current_checkpoint.payload.selected_trunk.clone(),
        acceptance_bundle_id: round_state.acceptance_bundle_id,
        acceptance_kind: acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.payload.acceptance_kind.clone()),
        next_step: round_state.next_step.clone(),
    };

    let existing_current = data_store
        .list_cadence_objects_by_kind(CADENCE_HUMAN_ROUND_KIND)?
        .into_iter()
        .find(|object| object.is_current)
        .map(parse_human_round_record)
        .transpose()?;
    if existing_current
        .as_ref()
        .map(|existing| existing.payload == payload)
        .unwrap_or(false)
    {
        return Ok(existing_current);
    }

    let title = current_checkpoint
        .payload
        .selected_trunk
        .clone()
        .map(|trunk| format!("Human round: {trunk}"))
        .unwrap_or_else(|| {
            format!(
                "Human round: checkpoint {}",
                current_checkpoint.cadence_object.id
            )
        });
    let payload_json =
        serde_json::to_string(&payload).context("failed to serialize human round payload")?;
    let cadence_object = data_store.insert_cadence_object(NewCadenceObject {
        cadence_kind: CADENCE_HUMAN_ROUND_KIND,
        title: &title,
        summary: &summary,
        payload_json: &payload_json,
        scope_type: NOTA_RUNTIME_SCOPE_TYPE,
        scope_ref: NOTA_RUNTIME_SCOPE_REF,
        source_type: NOTA_RUNTIME_SOURCE_TYPE,
        source_ref: "nota_runtime:human_round",
        admission_policy: admission_policy_for_kind(CADENCE_HUMAN_ROUND_KIND),
        projection_policy: projection_policy_for_kind(CADENCE_HUMAN_ROUND_KIND),
        status: &round_state.state,
        is_current: true,
    })?;
    if let Some(previous) = existing_current.as_ref() {
        data_store.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: previous.cadence_object.id,
            dst_cadence_object_id: cadence_object.id,
            relation_type: "superseded_by",
            status: "active",
        })?;
    }
    data_store.insert_cadence_link(NewCadenceLink {
        src_cadence_object_id: current_checkpoint.cadence_object.id,
        dst_cadence_object_id: cadence_object.id,
        relation_type: "human_round",
        status: "active",
    })?;
    if let Some(bundle) = acceptance_bundle.as_ref() {
        data_store.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: cadence_object.id,
            dst_cadence_object_id: bundle.cadence_object.id,
            relation_type: "acceptance",
            status: "active",
        })?;
    }

    Ok(Some(NotaHumanRoundRecord {
        cadence_object,
        payload,
    }))
}

fn build_runtime_handout_summary(
    checkpoint: &NotaCheckpointRecord,
    round_state: &NotaRoundStateProjection,
) -> String {
    match round_state.next_step.as_deref() {
        Some(step) => format!(
            "Checkpoint {} carries round state `{}` / detail `{}` with next step `{step}` still open.",
            checkpoint.cadence_object.id, round_state.state, round_state.detail_state
        ),
        None if round_state.fully_settled => format!(
            "Checkpoint {} is fully settled and can resume from closure truth.",
            checkpoint.cadence_object.id
        ),
        None if round_state.acceptance_present => format!(
            "Checkpoint {} is accepted and awaiting carry-forward closure.",
            checkpoint.cadence_object.id
        ),
        None => format!(
            "Checkpoint {} is checkpointed, but acceptance is not formalized yet.",
            checkpoint.cadence_object.id
        ),
    }
}

fn next_step_requires_active_human_wake(step: &str) -> bool {
    matches!(
        step,
        "review" | "integrate" | "finalize" | "repair" | "ask_decide" | "ask_override"
    )
}

fn build_runtime_wake_request_summary(
    checkpoint: &NotaCheckpointRecord,
    round_state: &NotaRoundStateProjection,
) -> (String, String, Option<String>) {
    if let Some(step) = round_state.next_step.as_deref() {
        let status = if next_step_requires_active_human_wake(step) {
            "requested"
        } else {
            "resolved"
        };
        let title = if status == "requested" {
            format!("Wake request: {step}")
        } else {
            format!("Wake request resolved: {step}")
        };
        (title, status.to_string(), Some(step.to_string()))
    } else if !round_state.acceptance_present {
        (
            format!("Wake request: checkpoint {}", checkpoint.cadence_object.id),
            "requested".to_string(),
            None,
        )
    } else if !round_state.fully_settled {
        (
            format!(
                "Wake request: carry forward checkpoint {}",
                checkpoint.cadence_object.id
            ),
            "requested".to_string(),
            None,
        )
    } else {
        (
            format!(
                "Wake request resolved: checkpoint {}",
                checkpoint.cadence_object.id
            ),
            "resolved".to_string(),
            None,
        )
    }
}

fn materialize_current_runtime_bridge_objects(data_store: &DataStore) -> Result<()> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let Some(current_checkpoint) = checkpoints
        .checkpoints
        .into_iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
    else {
        return Ok(());
    };
    let checkpoint_scope_ids = active_checkpoint_scope_ids(data_store, Some(&current_checkpoint))?;
    let transactions = list_nota_runtime_transactions(data_store)?;
    let allocations = list_nota_runtime_allocations(data_store)?;
    let receipts = list_nota_runtime_receipts(data_store, None)?;
    let current_human_round = derive_current_runtime_human_round(data_store)?;
    let current_acceptance_bundle =
        derive_current_runtime_acceptance_bundle(data_store, &checkpoint_scope_ids)?;
    let next_step = derive_nota_runtime_next_step(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let round_state = derive_runtime_round_state_projection(
        Some(&current_checkpoint),
        current_acceptance_bundle.as_ref(),
        next_step.as_ref(),
    );

    let handout_payload = CadenceHandoutPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        round_state: round_state.state.clone(),
        detail_round_state: Some(round_state.detail_state.clone()),
        stable_level: current_checkpoint.payload.stable_level.clone(),
        human_continuity_bus: current_checkpoint.payload.human_continuity_bus.clone(),
        selected_trunk: current_checkpoint.payload.selected_trunk.clone(),
        human_round_id: current_human_round
            .as_ref()
            .map(|round| round.cadence_object.id),
        acceptance_bundle_id: current_acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.cadence_object.id),
        next_step: round_state.next_step.clone(),
        summary: build_runtime_handout_summary(&current_checkpoint, &round_state),
    };
    let wake_summary = match round_state.next_step.as_deref() {
        Some("clarify") => format!(
            "Clarification is recorded on checkpoint {}; no active human wake is required until NOTA re-opens a human-facing ask or accepts the round.",
            current_checkpoint.cadence_object.id
        ),
        Some("ask_unblock") => format!(
            "Unblock ask is recorded on checkpoint {}; it stays local to NOTA by default and does not open an active human wake request.",
            current_checkpoint.cadence_object.id
        ),
        Some("ask_replace") => format!(
            "Replace ask is recorded on checkpoint {}; it stays local to NOTA by default and does not open an active human wake request.",
            current_checkpoint.cadence_object.id
        ),
        Some("ask_decide") => format!(
            "Wake on checkpoint {} because the current ask requires an explicit human decision before the round can proceed.",
            current_checkpoint.cadence_object.id
        ),
        Some("ask_override") => format!(
            "Wake on checkpoint {} because the current ask requires an explicit human override before the round can proceed.",
            current_checkpoint.cadence_object.id
        ),
        Some(step) => format!(
            "Wake on checkpoint {} and complete `{step}` before treating the round as settled.",
            current_checkpoint.cadence_object.id
        ),
        None if !round_state.acceptance_present => format!(
            "Wake on checkpoint {} and formalize acceptance before closing the round.",
            current_checkpoint.cadence_object.id
        ),
        None if !round_state.fully_settled => format!(
            "Wake on checkpoint {} and carry the accepted boundary forward into closure truth.",
            current_checkpoint.cadence_object.id
        ),
        None => format!(
            "No active wake request remains for checkpoint {}; the round is fully settled.",
            current_checkpoint.cadence_object.id
        ),
    };
    let (wake_title, wake_status, requested_step) =
        build_runtime_wake_request_summary(&current_checkpoint, &round_state);
    let wake_payload = CadenceWakeRequestPayload {
        checkpoint_id: current_checkpoint.cadence_object.id,
        round_state: round_state.state.clone(),
        detail_round_state: Some(round_state.detail_state.clone()),
        human_round_id: current_human_round
            .as_ref()
            .map(|round| round.cadence_object.id),
        acceptance_bundle_id: current_acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.cadence_object.id),
        requested_step,
        summary: wake_summary,
    };

    let existing_handout = data_store
        .list_cadence_objects_by_kind(CADENCE_HANDOUT_KIND)?
        .into_iter()
        .find(|object| object.is_current)
        .map(parse_handout_record)
        .transpose()?;
    if !existing_handout
        .as_ref()
        .map(|record| record.payload == handout_payload)
        .unwrap_or(false)
    {
        let title = current_checkpoint
            .payload
            .selected_trunk
            .clone()
            .map(|trunk| format!("Handout: {trunk}"))
            .unwrap_or_else(|| {
                format!(
                    "Handout: checkpoint {}",
                    current_checkpoint.cadence_object.id
                )
            });
        let payload_json = serde_json::to_string(&handout_payload)
            .context("failed to serialize cadence handout payload")?;
        let cadence_object = data_store.insert_cadence_object(NewCadenceObject {
            cadence_kind: CADENCE_HANDOUT_KIND,
            title: &title,
            summary: &handout_payload.summary,
            payload_json: &payload_json,
            scope_type: NOTA_RUNTIME_SCOPE_TYPE,
            scope_ref: NOTA_RUNTIME_SCOPE_REF,
            source_type: NOTA_RUNTIME_SOURCE_TYPE,
            source_ref: "nota_runtime:handout",
            admission_policy: admission_policy_for_kind(CADENCE_HANDOUT_KIND),
            projection_policy: projection_policy_for_kind(CADENCE_HANDOUT_KIND),
            status: "active",
            is_current: true,
        })?;
        if let Some(previous) = existing_handout.as_ref() {
            data_store.insert_cadence_link(NewCadenceLink {
                src_cadence_object_id: previous.cadence_object.id,
                dst_cadence_object_id: cadence_object.id,
                relation_type: "superseded_by",
                status: "active",
            })?;
        }
        data_store.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: current_checkpoint.cadence_object.id,
            dst_cadence_object_id: cadence_object.id,
            relation_type: "handout",
            status: "active",
        })?;
    }

    let existing_wake_request = data_store
        .list_cadence_objects_by_kind(CADENCE_WAKE_REQUEST_KIND)?
        .into_iter()
        .find(|object| object.is_current)
        .map(parse_wake_request_record)
        .transpose()?;
    if !existing_wake_request
        .as_ref()
        .map(|record| record.payload == wake_payload && record.cadence_object.status == wake_status)
        .unwrap_or(false)
    {
        let payload_json = serde_json::to_string(&wake_payload)
            .context("failed to serialize cadence wake-request payload")?;
        let cadence_object = data_store.insert_cadence_object(NewCadenceObject {
            cadence_kind: CADENCE_WAKE_REQUEST_KIND,
            title: &wake_title,
            summary: &wake_payload.summary,
            payload_json: &payload_json,
            scope_type: NOTA_RUNTIME_SCOPE_TYPE,
            scope_ref: NOTA_RUNTIME_SCOPE_REF,
            source_type: NOTA_RUNTIME_SOURCE_TYPE,
            source_ref: "nota_runtime:wake_request",
            admission_policy: admission_policy_for_kind(CADENCE_WAKE_REQUEST_KIND),
            projection_policy: projection_policy_for_kind(CADENCE_WAKE_REQUEST_KIND),
            status: &wake_status,
            is_current: true,
        })?;
        if let Some(previous) = existing_wake_request.as_ref() {
            data_store.insert_cadence_link(NewCadenceLink {
                src_cadence_object_id: previous.cadence_object.id,
                dst_cadence_object_id: cadence_object.id,
                relation_type: "superseded_by",
                status: "active",
            })?;
        }
        data_store.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: current_checkpoint.cadence_object.id,
            dst_cadence_object_id: cadence_object.id,
            relation_type: "wake_request",
            status: "active",
        })?;
    }

    Ok(())
}

pub fn derive_anti_zeno_projection(
    current_checkpoint: Option<&NotaCheckpointRecord>,
    acceptance_bundle: Option<&NotaAcceptanceBundleRecord>,
    next_step: Option<&NotaRuntimeNextStep>,
    recommended_checkpoint: Option<&NotaCheckpointRequest>,
) -> NotaAntiZenoProjection {
    let round_state =
        derive_runtime_round_state_projection(current_checkpoint, acceptance_bundle, next_step);
    let checkpoint_id = round_state.checkpoint_id;
    let acceptance_bundle_id = round_state.acceptance_bundle_id;
    let acceptance_present = round_state.acceptance_present;
    let fully_settled = round_state.fully_settled;
    let next_step_open = round_state.next_step_open;

    let (posture, state, detail_state, default_value) =
        anti_zeno_state_for_round_state(&round_state);
    let value = if current_checkpoint.is_some()
        && acceptance_bundle.is_none()
        && recommended_checkpoint.is_some()
    {
        52
    } else {
        default_value
    };
    let summary = if current_checkpoint.is_none() {
        "No checkpoint has anchored the current human round yet, so the system can still fall back into replay.".to_string()
    } else if let Some(bundle) = acceptance_bundle {
        if bundle.payload.allocation_id == 0 {
            if round_state.fully_settled {
                format!(
                    "Current human round acceptance is fully settled and can resume from checkpointed closure truth."
                )
            } else if let Some(step) = next_step {
                format!(
                    "Current human round acceptance is landed, and the next semantic boundary is `{}` instead of open-ended recursion.",
                    step.step
                )
            } else {
                "Current human round acceptance is landed, but a fresh closure checkpoint still needs to carry the round into a fully settled state.".to_string()
            }
        } else if round_state.fully_settled {
            format!(
                "Acceptance for allocation {} is fully settled and can resume from checkpointed closure truth.",
                bundle.payload.allocation_id
            )
        } else if let Some(step) = next_step {
            format!(
                "Acceptance is landed for allocation {}, and the next semantic boundary is `{}` instead of open-ended recursion.",
                bundle.payload.allocation_id, step.step
            )
        } else {
            format!(
                "Acceptance is landed for allocation {}, but a fresh closure checkpoint still needs to carry the round into a fully settled state.",
                bundle.payload.allocation_id
            )
        }
    } else if let Some(recommendation) = recommended_checkpoint {
        recommendation
            .selected_trunk
            .clone()
            .map(|trunk| {
                format!(
                    "The round is checkpointed, but acceptance has not been formalized yet; current trunk is `{trunk}`."
                )
            })
            .unwrap_or_else(|| {
                "The round is checkpointed, but acceptance has not been formalized yet."
                    .to_string()
            })
    } else {
        "A checkpoint exists, but there is still no formal acceptance bundle to prove the human round passed.".to_string()
    };

    NotaAntiZenoProjection {
        posture,
        state,
        detail_state,
        value,
        summary,
        acceptance_present,
        fully_settled,
        next_step_open,
        checkpoint_id,
        acceptance_bundle_id,
    }
}

// build_*_lineage_ref, normalize_list, normalize_optional, capture_repo_context, run_git_command, actor_role_slug
// moved to helpers.rs

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::{OsStr, OsString},
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context, Result};
    use serde_json::Value;

    use crate::core::{
        action::{ActionPrimitive, ActionRecord, ActionRoom, ActorRole, KnowledgeLayer},
        bootstrap_for_paths,
        compiler::{
            admission::admit_dispatch,
            lowering::{
                lower_dispatch, DispatchLineage, DispatchRouting, LoweredDispatch, LoweringContext,
                SandboxConfig,
            },
            packet::TypedActionPacket,
            routing::{resolve_return_route, ReturnBoundary, TerminalStatus},
        },
        config_store::{render_config, EntranceConfig},
        data_store::{
            DataStore, MigrationPlan, NewNotaRuntimeAllocation, NewNotaRuntimeReceipt,
            NewNotaRuntimeTransaction, NotaRuntimeTransactionUpdate, StoredForgeTask,
            StoredNotaRuntimeAllocation,
        },
        event_bus::EventBus,
        AppPaths,
    };
    use crate::plugins::forge::ForgePlugin;

    use super::{
        accept_current_runtime_round, active_checkpoint_scope_ids, boundary_kind,
        build_lowering_context, build_terminal_allocation_outcome, compile_nota_dispatch_packet,
        default_nota_dispatch_execution_host, derive_anti_zeno_projection,
        derive_current_runtime_acceptance_bundle, derive_current_runtime_handout,
        derive_current_runtime_wake_request, derive_nota_runtime_finalize,
        derive_nota_runtime_integrate, derive_nota_runtime_next_step, derive_nota_runtime_review,
        derive_runtime_round_state_projection, list_nota_runtime_allocations,
        list_nota_runtime_receipts, list_nota_runtime_transactions,
        list_runtime_acceptance_bundles, list_runtime_checkpoints, list_runtime_human_rounds,
        materialize_runtime_closure_checkpoint, recommend_runtime_closure_checkpoint,
        record_dev_return_finalize, record_dev_return_integration, record_dev_return_review,
        record_nota_boundary_ask, record_nota_boundary_clarification, resolve_dev_repair_origin,
        run_nota_dev_dispatch, sync_runtime_truth, write_runtime_checkpoint,
        AllocationTerminalOutcomeReceiptPayload, NotaBoundaryAskRequest,
        NotaBoundaryClarificationRequest, NotaCheckpointRequest, NotaCurrentRoundAcceptanceRequest,
        NotaDevDispatchRequest, NotaDevReturnFinalizeRequest, NotaDevReturnIntegrateRequest,
        NotaDevReturnReviewRequest, NotaDispatchExecutionHost, NotaDispatchLane,
        NotaDoAllocationPayload, NotaDoAllocationTerminalOutcome,
        AGENT_RETURN_ACCEPTED_RECEIPT_KIND, ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
        ASK_OPEN_TRANSACTION_STATUS, BOUNDARY_INTAKE_SUPERSEDED_TRANSACTION_STATUS,
        CADENCE_ACCEPTANCE_BUNDLE_KIND, CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND,
        CLARIFICATION_OPEN_TRANSACTION_STATUS, DEV_RETURN_ACCEPTED_RECEIPT_KIND,
        DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE, DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND,
        DEV_RETURN_INTEGRATE_INTEGRATED_STATE, DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND,
        DEV_RETURN_INTEGRATE_RECORDED_RUNTIME_STATE, DEV_RETURN_REVIEW_APPROVED_VERDICT,
        DEV_RETURN_REVIEW_READY_RECEIPT_KIND, DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
        HUMAN_ROUND_ACCEPTANCE_KIND,
    };

    struct TempDbPath {
        root: PathBuf,
        db_path: PathBuf,
    }

    struct TestDir {
        path: PathBuf,
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl TempDbPath {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-nota-runtime-{label}-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            let db_path = root.join("data").join("entrance.db");
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent)?;
            }
            Ok(Self { root, db_path })
        }

        fn path(&self) -> &Path {
            &self.db_path
        }
    }

    impl Drop for TempDbPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "entrance-nota-dispatch-{label}-{}-{unique}",
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

    fn nota_dispatch_test_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::test_env_guard()
    }

    fn init_git_repo(path: &Path) {
        let output = Command::new("git")
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

    fn init_git_repo_with_commit(path: &Path) {
        init_git_repo(path);

        let add = Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .expect("git add should run");
        assert!(
            add.status.success(),
            "git add should succeed: {}",
            String::from_utf8_lossy(&add.stderr)
        );

        let commit = Command::new("git")
            .args([
                "-c",
                "user.name=Entrance Test",
                "-c",
                "user.email=entrance@example.com",
                "commit",
                "--quiet",
                "-m",
                "initial commit",
            ])
            .current_dir(path)
            .output()
            .expect("git commit should run");
        assert!(
            commit.status.success(),
            "git commit should succeed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }

    fn add_git_worktree(repo_root: &Path, worktree_path: &Path, branch: &str) {
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "--quiet",
                "-b",
                branch,
                worktree_path
                    .to_str()
                    .expect("worktree path should be valid UTF-8"),
            ])
            .current_dir(repo_root)
            .output()
            .expect("git worktree add should run");
        assert!(
            output.status.success(),
            "git worktree add should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn build_lowering_context_populates_all_fields() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;
        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Dispatch seed",
            payload_json: "{}",
            status: "accepted",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;

        let context = build_lowering_context(
            &transaction,
            42,
            &NotaDispatchLane::Dev,
            "A:/Publish/entrance",
        );

        assert_eq!(
            context,
            LoweringContext {
                transaction_id: transaction.id,
                task_id: 42,
                dispatch_lane: "nota_dev".to_string(),
                allocator_surface: "dev".to_string(),
                project_root: "A:/Publish/entrance".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn lowering_matches_hand_coded_allocation() -> Result<()> {
        let _guard = nota_dispatch_test_guard();

        let temp_dir = TestDir::new("lowering-enforcement");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let startup = bootstrap_for_paths(AppPaths::new(app_data_dir.clone()))?;
        let store = startup.data_store();
        let forge = ForgePlugin::new(store.clone(), EventBus::new());

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        let dev_role_dir = bootstrap_skill.join("roles");
        fs::create_dir_all(&dev_role_dir)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
        fs::write(dev_role_dir.join("dev.md"), "# test dev role\n")?;
        fs::write(
            project_root.join("README.md"),
            "dispatch lowering fixture\n",
        )?;
        init_git_repo_with_commit(&project_root);

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-53");
        fs::create_dir_all(
            managed_worktree
                .parent()
                .expect("managed worktree parent should exist"),
        )?;
        add_git_worktree(&project_root, &managed_worktree, "feat-MYT-53");

        let report = run_nota_dev_dispatch(
            &store,
            &forge,
            NotaDevDispatchRequest {
                project_dir: Some(project_root.to_string_lossy().replace('\\', "/")),
                model: "codex".to_string(),
                agent_command: Some("__entrance_missing_runner__".to_string()),
                title: None,
                repair_of_allocation_id: None,
                execution_host: NotaDispatchExecutionHost::InProcess,
            },
        )?;

        let packet = TypedActionPacket::compile(
            ActionRecord::new(
                ActorRole::Dev,
                ActionPrimitive::Dispatch,
                ActionRoom::Prep,
                KnowledgeLayer::Cold,
            )
            .expect("test dispatch record should satisfy lowering constraints"),
        );
        let context = build_lowering_context(
            &report.transaction,
            report.task_id,
            &NotaDispatchLane::Dev,
            &report.dispatch.project_root,
        );
        let lowered =
            lower_dispatch(&packet, &context).expect("nota dev dispatch should lower successfully");

        assert_eq!(report.allocation.lineage_ref, lowered.lineage.lineage_ref);
        assert_eq!(
            report.allocation.child_execution_kind,
            lowered.lineage.child_execution_kind
        );
        assert_eq!(
            report.allocation.child_execution_ref,
            lowered.lineage.child_execution_ref
        );
        assert_eq!(
            report.allocation.return_target_kind,
            lowered.lineage.return_target_kind
        );
        assert_eq!(
            report.allocation.return_target_ref,
            lowered.lineage.return_target_ref
        );
        assert_eq!(
            report.allocation.escalation_target_kind,
            lowered.lineage.escalation_target_kind
        );
        assert_eq!(
            report.allocation.escalation_target_ref,
            lowered.lineage.escalation_target_ref
        );
        assert_eq!(
            report.allocation.allocator_role,
            lowered.routing.allocator_role
        );
        assert_eq!(
            report.allocation.allocation_kind,
            lowered.routing.allocation_kind
        );

        Ok(())
    }

    #[test]
    fn runtime_checkpoint_persists_in_dedicated_cadence_storage() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let first = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: None,
                stable_level: "single-ingress, checkpointed, DB-first NOTA host".to_string(),
                landed: vec!["cadence object storage cut".to_string()],
                remaining: vec!["Do automatic checkpoint/receipt".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("cadence storage cut".to_string()),
                next_start_hints: vec!["wire Do receipts".to_string()],
                project_dir: None,
            },
        )?;
        assert!(first.checkpoint.cadence_object.is_current);
        assert!(first.superseded_checkpoint_id.is_none());
        assert_eq!(first.checkpoint.payload.landed.len(), 1);

        let second = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Second checkpoint".to_string()),
                stable_level: "single-ingress, checkpointed, DB-first NOTA host".to_string(),
                landed: vec!["cadence link supersession".to_string()],
                remaining: vec!["Do automatic checkpoint/receipt".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("Do automatic checkpoint/receipt".to_string()),
                next_start_hints: vec!["persist Do transaction".to_string()],
                project_dir: None,
            },
        )?;
        assert_eq!(
            second.superseded_checkpoint_id,
            Some(first.checkpoint.cadence_object.id)
        );
        assert!(second.supersession_link.is_some());

        let report = list_runtime_checkpoints(&store)?;
        assert_eq!(report.checkpoint_count, 2);
        assert_eq!(
            report.current_checkpoint_id,
            Some(second.checkpoint.cadence_object.id)
        );
        assert_eq!(
            report.checkpoints[0].cadence_object.id,
            second.checkpoint.cadence_object.id
        );
        assert!(!report.checkpoints[1].cadence_object.is_current);
        assert_eq!(store.list_memory_fragment_records()?.len(), 0);

        Ok(())
    }

    #[test]
    fn runtime_checkpoint_materializes_current_human_round() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Round seed".to_string()),
                stable_level: "single-ingress, checkpointed, DB-first NOTA host".to_string(),
                landed: vec!["opened current round".to_string()],
                remaining: vec!["formal acceptance still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("round seed".to_string()),
                next_start_hints: vec!["read nota status".to_string()],
                project_dir: None,
            },
        )?;

        let rounds = list_runtime_human_rounds(&store)?;
        assert_eq!(rounds.human_round_count, 1);
        let current_round = rounds
            .human_rounds
            .iter()
            .find(|round| round.cadence_object.is_current)
            .context("human round should be materialized")?;
        assert_eq!(
            current_round.payload.checkpoint_id,
            checkpoint.checkpoint.cadence_object.id
        );
        assert_eq!(current_round.payload.round_state, "checkpointed");
        assert_eq!(
            current_round.payload.detail_round_state.as_deref(),
            Some("checkpointed_pending_acceptance")
        );
        assert!(!current_round.payload.accepted);
        assert!(!current_round.payload.acceptance_present);
        assert!(!current_round.payload.carry_forward_checkpointed);
        assert!(!current_round.payload.fully_settled);
        assert!(!current_round.payload.next_step_open);
        assert_eq!(
            current_round.payload.selected_trunk.as_deref(),
            Some("round seed")
        );

        Ok(())
    }

    #[test]
    fn runtime_truth_materializes_handout_and_wake_request() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Bridge seed".to_string()),
                stable_level: "single-ingress, checkpointed, DB-first NOTA host".to_string(),
                landed: vec!["opened handout and wake bridge".to_string()],
                remaining: vec!["formal acceptance still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("bridge seed".to_string()),
                next_start_hints: vec!["read nota status".to_string()],
                project_dir: None,
            },
        )?;

        let handout =
            derive_current_runtime_handout(&store)?.context("runtime handout should exist")?;
        assert_eq!(
            handout.payload.checkpoint_id,
            checkpoint.checkpoint.cadence_object.id
        );
        assert_eq!(handout.payload.round_state, "checkpointed");
        assert_eq!(
            handout.payload.detail_round_state.as_deref(),
            Some("checkpointed_pending_acceptance")
        );
        assert_eq!(
            handout.payload.selected_trunk.as_deref(),
            Some("bridge seed")
        );
        assert!(handout.payload.summary.contains("acceptance"));

        let wake_request = derive_current_runtime_wake_request(&store)?
            .context("runtime wake request should exist before acceptance")?;
        assert_eq!(
            wake_request.payload.checkpoint_id,
            checkpoint.checkpoint.cadence_object.id
        );
        assert_eq!(wake_request.payload.round_state, "checkpointed");
        assert_eq!(
            wake_request.payload.detail_round_state.as_deref(),
            Some("checkpointed_pending_acceptance")
        );
        assert!(wake_request
            .payload
            .summary
            .contains("formalize acceptance"));

        Ok(())
    }

    #[test]
    fn accept_current_round_formalizes_generic_acceptance_bundle() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Acceptance seed".to_string()),
                stable_level: "single-ingress, checkpointed, DB-first NOTA host".to_string(),
                landed: vec!["anchored the current human round".to_string()],
                remaining: vec!["formal acceptance still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("acceptance seed".to_string()),
                next_start_hints: vec!["run accept current round".to_string()],
                project_dir: None,
            },
        )?;

        let report = accept_current_runtime_round(
            &store,
            NotaCurrentRoundAcceptanceRequest {
                summary: Some("current round is explicitly accepted".to_string()),
            },
        )?;

        assert_eq!(report.status, "recorded");
        assert_eq!(
            report.acceptance_bundle.payload.acceptance_kind,
            HUMAN_ROUND_ACCEPTANCE_KIND
        );
        assert_eq!(report.acceptance_bundle.payload.transaction_id, 0);
        assert_eq!(report.acceptance_bundle.payload.allocation_id, 0);
        assert_eq!(
            report.acceptance_bundle.payload.checkpoint_id,
            checkpoint.checkpoint.cadence_object.id
        );

        let checkpoint_scope_ids =
            active_checkpoint_scope_ids(&store, Some(&checkpoint.checkpoint))?;
        let current_acceptance =
            derive_current_runtime_acceptance_bundle(&store, &checkpoint_scope_ids)?
                .context("generic acceptance bundle should be current")?;
        assert_eq!(
            current_acceptance.cadence_object.id,
            report.acceptance_bundle.cadence_object.id
        );

        let current_round = list_runtime_human_rounds(&store)?
            .human_rounds
            .into_iter()
            .find(|round| round.cadence_object.is_current)
            .context("current human round should still be materialized")?;
        assert_eq!(current_round.payload.round_state, "accepted");
        assert_eq!(
            current_round.payload.detail_round_state.as_deref(),
            Some("accepted_waiting_carry_forward")
        );

        let wake_request = derive_current_runtime_wake_request(&store)?
            .context("accepted round should still request carry-forward wake")?;
        assert_eq!(wake_request.cadence_object.status, "requested");
        assert!(wake_request.payload.summary.contains("carry"));

        Ok(())
    }

    #[test]
    fn clarification_creates_local_next_step_without_active_wake_request() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Clarify seed".to_string()),
                stable_level: "clarify seed".to_string(),
                landed: vec!["opened clarification gate".to_string()],
                remaining: vec!["clarification still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("clarify seed".to_string()),
                next_start_hints: vec!["record clarify".to_string()],
                project_dir: None,
            },
        )?;

        let report = record_nota_boundary_clarification(
            &store,
            NotaBoundaryClarificationRequest {
                summary: "need to clarify the target ask graph".to_string(),
            },
        )?;

        assert_eq!(report.status, "recorded");
        assert_eq!(
            report.transaction.status,
            CLARIFICATION_OPEN_TRANSACTION_STATUS
        );
        assert_eq!(report.next_step.step, "clarify");
        assert_eq!(report.next_step.allocation_id, 0);
        assert_eq!(
            report.next_step.target_ref,
            checkpoint.checkpoint.cadence_object.id.to_string()
        );

        let checkpoint_scope_ids =
            active_checkpoint_scope_ids(&store, Some(&checkpoint.checkpoint))?;
        let transactions = list_nota_runtime_transactions(&store)?;
        let receipts = list_nota_runtime_receipts(&store, None)?;
        let next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &transactions.transactions,
            &[],
            &receipts.receipts,
        )?
        .context("clarification should be the active next step")?;
        assert_eq!(next_step.step, "clarify");
        assert!(derive_current_runtime_wake_request(&store)?.is_none());

        Ok(())
    }

    #[test]
    fn ask_decide_creates_active_human_wake_request() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Ask decide seed".to_string()),
                stable_level: "ask decide seed".to_string(),
                landed: vec!["opened ask decide gate".to_string()],
                remaining: vec!["decision still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("ask decide seed".to_string()),
                next_start_hints: vec!["record ask decide".to_string()],
                project_dir: None,
            },
        )?;

        let report = record_nota_boundary_ask(
            &store,
            NotaBoundaryAskRequest {
                ask_code: "decide".to_string(),
                summary: "human needs to decide whether ask graph becomes canonical".to_string(),
            },
        )?;

        assert_eq!(report.transaction.status, ASK_OPEN_TRANSACTION_STATUS);
        assert_eq!(report.next_step.step, "ask_decide");
        assert_eq!(
            report.next_step.target_ref,
            checkpoint.checkpoint.cadence_object.id.to_string()
        );

        let wake_request = derive_current_runtime_wake_request(&store)?
            .context("ask decide should open a wake request")?;
        assert_eq!(wake_request.cadence_object.status, "requested");
        assert_eq!(
            wake_request.payload.requested_step.as_deref(),
            Some("ask_decide")
        );
        assert!(wake_request
            .payload
            .summary
            .contains("explicit human decision"));

        Ok(())
    }

    #[test]
    fn ask_unblock_stays_local_and_supersedes_previous_open_intake() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Ask unblock seed".to_string()),
                stable_level: "ask unblock seed".to_string(),
                landed: vec!["opened local unblock ask".to_string()],
                remaining: vec!["unblock still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("ask unblock seed".to_string()),
                next_start_hints: vec!["record ask unblock".to_string()],
                project_dir: None,
            },
        )?;

        let first = record_nota_boundary_ask(
            &store,
            NotaBoundaryAskRequest {
                ask_code: "decide".to_string(),
                summary: "first ask opens a human-facing decision".to_string(),
            },
        )?;
        let second = record_nota_boundary_ask(
            &store,
            NotaBoundaryAskRequest {
                ask_code: "unblock".to_string(),
                summary: "second ask stays local to NOTA".to_string(),
            },
        )?;

        assert_eq!(second.next_step.step, "ask_unblock");
        assert_eq!(
            second.superseded_transaction_ids,
            vec![first.transaction.id]
        );

        let transactions = list_nota_runtime_transactions(&store)?.transactions;
        let first_transaction = transactions
            .iter()
            .find(|transaction| transaction.id == first.transaction.id)
            .context("first ask transaction should still exist")?;
        assert_eq!(
            first_transaction.status,
            BOUNDARY_INTAKE_SUPERSEDED_TRANSACTION_STATUS
        );

        let checkpoint_scope_ids =
            active_checkpoint_scope_ids(&store, Some(&checkpoint.checkpoint))?;
        let receipts = list_nota_runtime_receipts(&store, None)?;
        let next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &transactions,
            &[],
            &receipts.receipts,
        )?
        .context("second ask should become the active next step")?;
        assert_eq!(next_step.step, "ask_unblock");
        assert!(derive_current_runtime_wake_request(&store)?.is_none());

        Ok(())
    }

    #[test]
    fn accept_current_round_supersedes_open_boundary_intake() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Accept after ask".to_string()),
                stable_level: "accept after ask".to_string(),
                landed: vec!["checkpoint anchored before acceptance".to_string()],
                remaining: vec!["clarify closure still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("accept after ask".to_string()),
                next_start_hints: vec!["record ask then accept".to_string()],
                project_dir: None,
            },
        )?;

        let ask = record_nota_boundary_ask(
            &store,
            NotaBoundaryAskRequest {
                ask_code: "override".to_string(),
                summary: "human override is temporarily required".to_string(),
            },
        )?;

        let acceptance = accept_current_runtime_round(
            &store,
            NotaCurrentRoundAcceptanceRequest {
                summary: Some("override has been resolved and round can be accepted".to_string()),
            },
        )?;

        assert_eq!(acceptance.status, "recorded");
        assert_eq!(
            acceptance.superseded_transaction_ids,
            vec![ask.transaction.id]
        );

        let transactions = list_nota_runtime_transactions(&store)?.transactions;
        let ask_transaction = transactions
            .iter()
            .find(|transaction| transaction.id == ask.transaction.id)
            .context("ask transaction should still exist after acceptance")?;
        assert_eq!(
            ask_transaction.status,
            BOUNDARY_INTAKE_SUPERSEDED_TRANSACTION_STATUS
        );

        let checkpoint_scope_ids =
            active_checkpoint_scope_ids(&store, Some(&checkpoint.checkpoint))?;
        let receipts = list_nota_runtime_receipts(&store, None)?;
        assert!(derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &transactions,
            &[],
            &receipts.receipts,
        )?
        .is_none());

        let wake_request = derive_current_runtime_wake_request(&store)?
            .context("accepted round should still request carry-forward wake")?;
        assert!(wake_request.payload.summary.contains("carry"));

        Ok(())
    }

    #[test]
    fn dev_repair_origin_requires_active_repair_boundary() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Repair gate".to_string()),
                stable_level: "repair gate".to_string(),
                landed: vec!["repair boundary opened".to_string()],
                remaining: vec!["repair follow-up still pending".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("repair gate".to_string()),
                next_start_hints: vec!["read repair gate".to_string()],
                project_dir: None,
            },
        )?;
        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Repair source transaction",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: Some(checkpoint.checkpoint.cadence_object.id),
        })?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-REPAIR".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("Repair source boundary".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-REPAIR".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
            }),
        };
        let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/dev/transaction/77/forge-task/7",
            child_execution_kind: "forge_task",
            child_execution_ref: "7",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&serde_json::json!({
                "checkpoint_id": checkpoint.checkpoint.cadence_object.id,
                "review": {
                    "state": "review_recorded",
                    "transaction_id": transaction.id,
                    "allocation_id": allocation.id,
                    "lineage_ref": allocation.lineage_ref,
                    "child_dispatch_role": "dev",
                    "execution_host": "in_process",
                    "target_kind": "nota_runtime_transaction",
                    "target_ref": transaction.id.to_string(),
                    "verdict": "changes_requested",
                    "summary": "Repair is required"
                },
                "next_step": {
                    "step": "repair",
                    "transaction_id": transaction.id,
                    "allocation_id": allocation.id,
                    "lineage_ref": allocation.lineage_ref,
                    "child_dispatch_role": "dev",
                    "execution_host": "in_process",
                    "target_kind": "nota_runtime_transaction",
                    "target_ref": transaction.id.to_string()
                }
            }))?,
            status: "recorded",
        })?;

        let repair_origin = resolve_dev_repair_origin(&store, allocation.id)?;
        assert_eq!(repair_origin.allocation_id, allocation.id);
        assert_eq!(repair_origin.transaction_id, transaction.id);
        assert_eq!(repair_origin.lineage_ref, allocation.lineage_ref);
        assert_eq!(repair_origin.project_dir, "A:/Agent/Entrance");

        Ok(())
    }

    #[test]
    fn runtime_allocation_persists_separately_from_transactions_and_receipts() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: "Test transaction",
            payload_json: "{}",
            status: "accepted",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;

        let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/do/transaction/1/forge-task/9",
            child_execution_kind: "forge_task",
            child_execution_ref: "9",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "task_created",
            payload_json: "{}",
        })?;
        let receipt = store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: "ALLOCATION_RECORDED",
            payload_json: "{}",
            status: "recorded",
        })?;

        let transactions = store.list_nota_runtime_transactions()?;
        let allocations = store.list_nota_runtime_allocations()?;
        let receipts = store.list_nota_runtime_receipts(Some(transaction.id))?;

        assert_eq!(transactions.len(), 1);
        assert_eq!(allocations.len(), 1);
        assert_eq!(receipts.len(), 1);
        assert_eq!(allocation.source_transaction_id, transaction.id);
        assert_eq!(receipt.transaction_id, transaction.id);
        assert_eq!(allocations[0].id, allocation.id);
        assert_eq!(allocations[0].lineage_ref, allocation.lineage_ref);
        assert_eq!(allocations[0].return_target_ref, transaction.id.to_string());
        assert_eq!(
            allocations[0].escalation_target_ref,
            transaction.id.to_string()
        );

        Ok(())
    }

    #[test]
    fn allocation_surface_does_not_backfill_terminal_receipt_on_read() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: "Backfill terminal outcome receipt",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let task_id =
            store.insert_forge_task("Blocked child", "echo", "[]", None, None, "[]", "{}")?;
        store.update_forge_task_status(
            task_id,
            "Blocked",
            None,
            Some("add openai to Vault first"),
        )?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-48".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "linear".to_string(),
            issue_title: Some("Test issue".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-48".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "agent".to_string(),
            child_dispatch_tool_name: "forge_dispatch_agent".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Blocked".to_string(),
                child_execution_status_message: Some("add openai to Vault first".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
            }),
        };
        let allocation_payload_json = serde_json::to_string(&allocation_payload)?;
        store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/do/transaction/1/forge-task/1",
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "escalated_blocked",
            payload_json: &allocation_payload_json,
        })?;

        let report = list_nota_runtime_allocations(&store)?;
        assert_eq!(report.allocation_count, 1);
        assert_eq!(
            report.allocations[0].child_dispatch_role.as_deref(),
            Some("agent")
        );
        assert_eq!(
            report.allocations[0].child_dispatch_tool_name.as_deref(),
            Some("forge_dispatch_agent")
        );
        let projected_payload: NotaDoAllocationPayload =
            serde_json::from_str(&report.allocations[0].payload_json)?;
        let projected_outcome = projected_payload
            .terminal_outcome
            .expect("allocation read surface should expose the existing terminal outcome");
        assert_eq!(
            projected_outcome,
            NotaDoAllocationTerminalOutcome {
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Blocked".to_string(),
                child_execution_status_message: Some("add openai to Vault first".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
            }
        );
        assert!(store
            .list_nota_runtime_receipts(Some(transaction.id))?
            .is_empty());

        let second_report = list_nota_runtime_allocations(&store)?;
        assert_eq!(second_report.allocations[0].status, "escalated_blocked");
        assert!(store
            .list_nota_runtime_receipts(Some(transaction.id))?
            .is_empty());

        Ok(())
    }

    #[test]
    fn build_terminal_allocation_outcome_matches_resolved_return_route() -> Result<()> {
        let _guard = nota_dispatch_test_guard();

        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-visibility".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("Visibility reconstruction".to_string()),
            project_root: "A:/Publish/entrance".to_string(),
            worktree_path: "A:/Publish/entrance/worktrees/feat-MYT-visibility".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: None,
        };
        let allocation = StoredNotaRuntimeAllocation {
            id: 41,
            allocator_role: "nota".to_string(),
            allocator_surface: "nota_dev".to_string(),
            allocation_kind: "forge_dev_dispatch".to_string(),
            source_transaction_id: 11,
            lineage_ref: "nota/dev/transaction/11/forge-task/7".to_string(),
            child_execution_kind: "forge_task".to_string(),
            child_execution_ref: "7".to_string(),
            return_target_kind: "nota_runtime_transaction".to_string(),
            return_target_ref: "11".to_string(),
            escalation_target_kind: "human".to_string(),
            escalation_target_ref: "review".to_string(),
            status: "task_created".to_string(),
            payload_json: serde_json::to_string(&allocation_payload)?,
            created_at: "2026-04-03T00:00:00Z".to_string(),
            updated_at: "2026-04-03T00:00:00Z".to_string(),
        };
        let task_with_status = |status: &str, message: Option<&str>| StoredForgeTask {
            id: 7,
            name: "Synthetic terminal task".to_string(),
            command: "echo".to_string(),
            args: "[]".to_string(),
            working_dir: None,
            stdin_text: None,
            required_tokens: "[]".to_string(),
            metadata: "{}".to_string(),
            status: status.to_string(),
            status_message: message.map(str::to_string),
            exit_code: Some(0),
            created_at: "2026-04-03T00:00:00Z".to_string(),
            heartbeat_at: Some("2026-04-03T00:00:05Z".to_string()),
            finished_at: Some("2026-04-03T00:00:10Z".to_string()),
        };
        let expected_route = |terminal_status| {
            let packet = compile_nota_dispatch_packet();
            let sandbox_requirement = packet.semantics().sandbox_requirement;
            let routing_constraint = packet.semantics().routing_constraint;
            let lowered_dispatch = LoweredDispatch {
                packet,
                lineage: DispatchLineage {
                    lineage_ref: allocation.lineage_ref.clone(),
                    child_execution_kind: allocation.child_execution_kind.clone(),
                    child_execution_ref: allocation.child_execution_ref.clone(),
                    return_target_kind: allocation.return_target_kind.clone(),
                    return_target_ref: allocation.return_target_ref.clone(),
                    escalation_target_kind: allocation.escalation_target_kind.clone(),
                    escalation_target_ref: allocation.escalation_target_ref.clone(),
                },
                sandbox: SandboxConfig {
                    requirement: sandbox_requirement,
                    working_dir: None,
                },
                routing: DispatchRouting {
                    constraint: routing_constraint,
                    allocator_role: allocation.allocator_role.clone(),
                    allocation_kind: allocation.allocation_kind.clone(),
                },
            };
            let admitted = admit_dispatch(lowered_dispatch, None)
                .expect("synthetic allocation should reconstruct an admitted dispatch");

            resolve_return_route(&admitted, terminal_status)
                .expect("synthetic allocation should resolve a return route")
        };
        let assert_terminal_outcome =
            |terminal_status: TerminalStatus,
             task_status: &str,
             message: Option<&str>,
             expected_allocation_status: &str,
             expected_boundary: ReturnBoundary| {
                let (allocation_status, outcome) = build_terminal_allocation_outcome(
                    &allocation,
                    &task_with_status(task_status, message),
                )
                .expect("terminal task should produce a terminal outcome");
                let route = expected_route(terminal_status);

                assert_eq!(allocation_status, expected_allocation_status);
                assert_eq!(route.boundary, expected_boundary);
                assert_eq!(outcome.boundary_kind, boundary_kind(route.boundary));
                assert_eq!(outcome.child_execution_status, task_status);
                assert_eq!(outcome.child_execution_status_message.as_deref(), message);
                assert_eq!(outcome.target_kind, route.target_kind);
                assert_eq!(outcome.target_ref, route.target_ref);
            };

        assert_terminal_outcome(
            TerminalStatus::Done,
            "Done",
            Some("Merged cleanly"),
            "return_ready",
            ReturnBoundary::Return,
        );
        assert_terminal_outcome(
            TerminalStatus::Blocked,
            "Blocked",
            Some("Needs human review"),
            "escalated_blocked",
            ReturnBoundary::Escalation,
        );
        assert_terminal_outcome(
            TerminalStatus::Failed,
            "Failed",
            Some("Execution crashed"),
            "escalated_failed",
            ReturnBoundary::Escalation,
        );
        assert_terminal_outcome(
            TerminalStatus::Cancelled,
            "Cancelled",
            Some("Cancelled by human"),
            "escalated_cancelled",
            ReturnBoundary::Escalation,
        );

        Ok(())
    }

    #[test]
    fn receipt_surface_does_not_materialize_terminal_outcome_without_explicit_sync() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Receipt backflow",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let task_id =
            store.insert_forge_task("Heartbeat child", "echo", "[]", None, None, "[]", "{}")?;
        store.update_forge_task_status(task_id, "Failed", None, Some("Task heartbeat lost"))?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-1048".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "fallback".to_string(),
            issue_title: None,
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-1048".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: None,
        };
        store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/dev/transaction/1/forge-task/1",
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "task_created",
            payload_json: &serde_json::to_string(&allocation_payload)?,
        })?;

        assert!(store
            .list_nota_runtime_receipts(Some(transaction.id))?
            .is_empty());

        let report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(report.receipt_count, 0);

        let allocation_report = list_nota_runtime_allocations(&store)?;
        assert_eq!(allocation_report.allocation_count, 1);
        assert_eq!(allocation_report.allocations[0].status, "escalated_failed");
        let projected_payload: NotaDoAllocationPayload =
            serde_json::from_str(&allocation_report.allocations[0].payload_json)?;
        let projected_outcome = projected_payload
            .terminal_outcome
            .expect("allocation read surface should project a terminal outcome");
        assert_eq!(projected_outcome.boundary_kind, "escalation");
        assert_eq!(projected_outcome.child_execution_status, "Failed");
        assert_eq!(
            projected_outcome.child_execution_status_message.as_deref(),
            Some("Task heartbeat lost")
        );

        let stored_allocations = store.list_nota_runtime_allocations()?;
        assert_eq!(stored_allocations.len(), 1);
        assert_eq!(stored_allocations[0].status, "task_created");
        let stored_payload: NotaDoAllocationPayload =
            serde_json::from_str(&stored_allocations[0].payload_json)?;
        assert!(stored_payload.terminal_outcome.is_none());
        assert!(store
            .list_nota_runtime_receipts(Some(transaction.id))?
            .is_empty());

        Ok(())
    }

    #[test]
    fn explicit_runtime_sync_persists_terminal_outcome_and_receipt_truth() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Explicit runtime sync",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let task_id =
            store.insert_forge_task("Heartbeat child", "echo", "[]", None, None, "[]", "{}")?;
        store.update_forge_task_status(task_id, "Failed", None, Some("Task heartbeat lost"))?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-1048".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "fallback".to_string(),
            issue_title: None,
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-1048".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: None,
        };
        let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/dev/transaction/1/forge-task/1",
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "task_created",
            payload_json: &serde_json::to_string(&allocation_payload)?,
        })?;

        assert!(store
            .list_nota_runtime_receipts(Some(transaction.id))?
            .is_empty());
        assert_eq!(
            store.list_nota_runtime_allocations()?[0].status,
            "task_created"
        );

        sync_runtime_truth(&store, Some(transaction.id))?;

        let stored_allocations = store.list_nota_runtime_allocations()?;
        assert_eq!(stored_allocations.len(), 1);
        assert_eq!(stored_allocations[0].status, "escalated_failed");
        let stored_payload: NotaDoAllocationPayload =
            serde_json::from_str(&stored_allocations[0].payload_json)?;
        let stored_outcome = stored_payload
            .terminal_outcome
            .expect("explicit sync should persist the terminal outcome");
        assert_eq!(stored_outcome.boundary_kind, "escalation");
        assert_eq!(stored_outcome.child_execution_status, "Failed");
        assert_eq!(
            stored_outcome.child_execution_status_message.as_deref(),
            Some("Task heartbeat lost")
        );

        let receipts = store.list_nota_runtime_receipts(Some(transaction.id))?;
        assert_eq!(receipts.len(), 1);
        assert_eq!(
            receipts[0].receipt_kind,
            ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND
        );
        let receipt_payload: AllocationTerminalOutcomeReceiptPayload =
            serde_json::from_str(&receipts[0].payload_json)?;
        assert_eq!(
            receipt_payload,
            AllocationTerminalOutcomeReceiptPayload {
                allocation_id: allocation.id,
                lineage_ref: allocation.lineage_ref.clone(),
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Failed".to_string(),
                child_execution_status_message: Some("Task heartbeat lost".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
                allocation_status: "escalated_failed".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn checkpoint_write_backfills_dev_return_acceptance_for_current_checkpoint() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;
        let task_id = store.insert_forge_task("Dev child", "echo", "[]", None, None, "[]", "{}")?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Dev return acceptance backflow",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: Some(task_id),
            cadence_checkpoint_id: None,
        })?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-1048".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "fallback".to_string(),
            issue_title: None,
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "C:/Users/test/.entrance/worktrees/Entrance/feat-MYT-1048".to_string(),
            prompt_source: "Entrance-owned harness/bootstrap dev prompt".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: NotaDispatchExecutionHost::DetachedForgeCliSupervisor
                .as_str()
                .to_string(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
            }),
        };
        let lineage_ref = format!(
            "nota/dev/transaction/{}/forge-task/{task_id}",
            transaction.id
        );
        let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: &lineage_ref,
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&AllocationTerminalOutcomeReceiptPayload {
                allocation_id: allocation.id,
                lineage_ref: allocation.lineage_ref.clone(),
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
                allocation_status: "return_ready".to_string(),
            })?,
            status: "recorded",
        })?;

        let allocations = list_nota_runtime_allocations(&store)?;
        let recommendation =
            recommend_runtime_closure_checkpoint(&store, allocations.stored_allocations(), None)?
                .context("dev return checkpoint recommendation should exist")?;
        let checkpoint_report = write_runtime_checkpoint(&store, recommendation.clone())?;
        store.update_nota_runtime_transaction(
            transaction.id,
            NotaRuntimeTransactionUpdate {
                status: "checkpointed",
                forge_task_id: Some(task_id),
                cadence_checkpoint_id: Some(checkpoint_report.checkpoint.cadence_object.id),
            },
        )?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&serde_json::json!({
                "checkpoint_id": checkpoint_report.checkpoint.cadence_object.id,
                "selected_trunk": checkpoint_report.checkpoint.payload.selected_trunk,
            }))?,
            status: "recorded",
        })?;

        let seeded_receipts = store.list_nota_runtime_receipts(Some(transaction.id))?;
        let seeded_accepted = seeded_receipts
            .iter()
            .find(|receipt| receipt.receipt_kind == DEV_RETURN_ACCEPTED_RECEIPT_KIND)
            .context("checkpoint write should already backfill the dev acceptance receipt")?;
        let seeded_review_ready = seeded_receipts
            .iter()
            .find(|receipt| receipt.receipt_kind == DEV_RETURN_REVIEW_READY_RECEIPT_KIND)
            .context("checkpoint write should already backfill the review-ready receipt")?;

        let report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(report.receipt_count, seeded_receipts.len());
        let accepted_receipt = report
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_kind == DEV_RETURN_ACCEPTED_RECEIPT_KIND)
            .context("receipt surface should retain the backfilled dev acceptance receipt")?;
        let review_ready_receipt = report
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_kind == DEV_RETURN_REVIEW_READY_RECEIPT_KIND)
            .context("receipt surface should retain the backfilled review-ready receipt")?;
        assert_eq!(accepted_receipt.id, seeded_accepted.id);
        assert_eq!(review_ready_receipt.id, seeded_review_ready.id);

        let accepted_payload: Value = serde_json::from_str(&accepted_receipt.payload_json)?;
        assert_eq!(accepted_payload["allocation_id"], allocation.id);
        assert_eq!(accepted_payload["lineage_ref"], allocation.lineage_ref);
        assert_eq!(
            accepted_payload["checkpoint_id"],
            checkpoint_report.checkpoint.cadence_object.id
        );
        assert_eq!(accepted_payload["child_dispatch_role"], "dev");
        assert_eq!(
            accepted_payload["execution_host"],
            "detached_forge_cli_supervisor"
        );
        assert_eq!(accepted_payload["target_kind"], "nota_runtime_transaction");
        assert_eq!(accepted_payload["target_ref"], transaction.id.to_string());

        let review_ready_payload: Value = serde_json::from_str(&review_ready_receipt.payload_json)?;
        assert_eq!(
            review_ready_payload["checkpoint_id"],
            checkpoint_report.checkpoint.cadence_object.id
        );
        assert_eq!(review_ready_payload["step"], "review");
        assert_eq!(review_ready_payload["transaction_id"], transaction.id);
        assert_eq!(review_ready_payload["allocation_id"], allocation.id);
        assert_eq!(review_ready_payload["lineage_ref"], allocation.lineage_ref);
        assert_eq!(review_ready_payload["child_dispatch_role"], "dev");
        assert_eq!(
            review_ready_payload["execution_host"],
            "detached_forge_cli_supervisor"
        );
        assert_eq!(
            review_ready_payload["target_kind"],
            "nota_runtime_transaction"
        );
        assert_eq!(
            review_ready_payload["target_ref"],
            transaction.id.to_string()
        );

        let initial_acceptance_bundles = list_runtime_acceptance_bundles(&store)?;
        assert_eq!(initial_acceptance_bundles.acceptance_bundle_count, 1);
        let initial_acceptance = initial_acceptance_bundles
            .acceptance_bundles
            .iter()
            .find(|bundle| bundle.cadence_object.is_current)
            .context("dev return acceptance bundle should exist")?;
        assert_eq!(
            initial_acceptance.payload.acceptance_kind,
            "dev_return_acceptance"
        );
        assert_eq!(initial_acceptance.payload.round_state, "accepted");
        assert!(!initial_acceptance.payload.fully_settled);

        let initial_human_rounds = list_runtime_human_rounds(&store)?;
        let initial_human_round = initial_human_rounds
            .human_rounds
            .iter()
            .find(|round| round.cadence_object.is_current)
            .context("human round should exist after acceptance backflow")?;
        assert_eq!(
            initial_human_round.payload.checkpoint_id,
            checkpoint_report.checkpoint.cadence_object.id
        );
        assert_eq!(initial_human_round.payload.round_state, "settling");
        assert_eq!(
            initial_human_round.payload.detail_round_state.as_deref(),
            Some("accepted_followup_open")
        );
        assert!(initial_human_round.payload.accepted);
        assert!(initial_human_round.payload.acceptance_present);
        assert!(!initial_human_round.payload.fully_settled);
        assert!(!initial_human_round.payload.carry_forward_checkpointed);
        assert!(initial_human_round.payload.next_step_open);
        assert_eq!(
            initial_human_round.payload.acceptance_bundle_id,
            Some(initial_acceptance.cadence_object.id)
        );

        let checkpoints = list_runtime_checkpoints(&store)?;
        let current_checkpoint = checkpoints
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.cadence_object.is_current);
        let checkpoint_scope_ids = active_checkpoint_scope_ids(&store, current_checkpoint)?;
        let runtime_transactions = store.list_nota_runtime_transactions()?;
        let next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &report.receipts,
        )?
        .context("review-ready next step should be exposed")?;
        assert_eq!(next_step.step, "review");
        assert_eq!(next_step.transaction_id, transaction.id);
        assert_eq!(next_step.allocation_id, allocation.id);
        assert_eq!(next_step.lineage_ref, allocation.lineage_ref);
        assert_eq!(next_step.child_dispatch_role, "dev");
        assert_eq!(next_step.execution_host, "detached_forge_cli_supervisor");
        assert_eq!(next_step.target_kind, "nota_runtime_transaction");
        assert_eq!(next_step.target_ref, transaction.id.to_string());

        let review_recorded = record_dev_return_review(
            &store,
            NotaDevReturnReviewRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                verdict: "approved".to_string(),
                summary: Some("Review accepted for integration".to_string()),
            },
        )?;
        assert_eq!(review_recorded.status, "recorded");
        assert_eq!(review_recorded.review.state, "review_recorded");
        assert_eq!(review_recorded.review.verdict.as_deref(), Some("approved"));
        assert_eq!(
            review_recorded.review.summary.as_deref(),
            Some("Review accepted for integration")
        );
        assert_eq!(review_recorded.next_step.step, "integrate");
        assert_eq!(
            review_recorded.receipt.receipt_kind,
            DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
        );

        let recorded_report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert!(recorded_report.receipt_count >= seeded_receipts.len());
        assert!(recorded_report
            .receipts
            .iter()
            .any(|receipt| receipt.id == review_recorded.receipt.id));

        let review = derive_nota_runtime_review(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &recorded_report.receipts,
        )?
        .context("recorded dev review should be exposed")?;
        assert_eq!(review.state, "review_recorded");
        assert_eq!(review.verdict.as_deref(), Some("approved"));
        assert_eq!(
            review.summary.as_deref(),
            Some("Review accepted for integration")
        );
        assert_eq!(review.transaction_id, transaction.id);
        assert_eq!(review.allocation_id, allocation.id);
        assert_eq!(review.lineage_ref, allocation.lineage_ref);

        let integrated_next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &recorded_report.receipts,
        )?
        .context("recorded approved review should advance next step")?;
        assert_eq!(integrated_next_step.step, "integrate");
        assert_eq!(integrated_next_step.transaction_id, transaction.id);
        assert_eq!(integrated_next_step.allocation_id, allocation.id);
        assert_eq!(integrated_next_step.lineage_ref, allocation.lineage_ref);

        let duplicate_record = record_dev_return_review(
            &store,
            NotaDevReturnReviewRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                verdict: "approved".to_string(),
                summary: Some("Review accepted for integration".to_string()),
            },
        )?;
        assert_eq!(duplicate_record.status, "already_recorded");

        let conflicting_review_result = record_dev_return_review(
            &store,
            NotaDevReturnReviewRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                verdict: "changes_requested".to_string(),
                summary: Some("Needs repair".to_string()),
            },
        );
        assert!(conflicting_review_result.is_err());

        let integrate_started = record_dev_return_integration(
            &store,
            NotaDevReturnIntegrateRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                state: "started".to_string(),
                summary: Some("Integration is now in progress".to_string()),
            },
        )?;
        assert_eq!(integrate_started.status, "recorded");
        assert_eq!(integrate_started.integrate.state, "integrate_started");
        assert_eq!(integrate_started.integrate.outcome, None);
        assert_eq!(
            integrate_started.integrate.summary.as_deref(),
            Some("Integration is now in progress")
        );
        assert!(integrate_started.next_step.is_none());
        assert_eq!(
            integrate_started.receipt.receipt_kind,
            DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
        );

        let integrate_started_report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert!(integrate_started_report
            .receipts
            .iter()
            .any(|receipt| receipt.id == integrate_started.receipt.id));

        let started_integrate = derive_nota_runtime_integrate(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &integrate_started_report.receipts,
        )?
        .context("started integration should be exposed")?;
        assert_eq!(started_integrate.state, "integrate_started");
        assert_eq!(started_integrate.outcome, None);
        assert_eq!(
            started_integrate.summary.as_deref(),
            Some("Integration is now in progress")
        );
        assert!(derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &integrate_started_report.receipts,
        )?
        .is_none());

        let integrated = record_dev_return_integration(
            &store,
            NotaDevReturnIntegrateRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                state: "integrated".to_string(),
                summary: Some("Integration landed and is ready to finalize".to_string()),
            },
        )?;
        assert_eq!(integrated.status, "recorded");
        assert_eq!(integrated.integrate.state, "integrate_recorded");
        assert_eq!(integrated.integrate.outcome.as_deref(), Some("integrated"));
        assert_eq!(
            integrated.integrate.summary.as_deref(),
            Some("Integration landed and is ready to finalize")
        );
        assert_eq!(
            integrated
                .next_step
                .as_ref()
                .context("integrated next step should be present")?
                .step,
            "finalize"
        );

        let integrated_report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert!(integrated_report
            .receipts
            .iter()
            .any(|receipt| receipt.id == integrated.receipt.id));

        let recorded_integrate = derive_nota_runtime_integrate(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &integrated_report.receipts,
        )?
        .context("recorded integration should be exposed")?;
        assert_eq!(recorded_integrate.state, "integrate_recorded");
        assert_eq!(recorded_integrate.outcome.as_deref(), Some("integrated"));
        assert_eq!(
            recorded_integrate.summary.as_deref(),
            Some("Integration landed and is ready to finalize")
        );

        let finalize_next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &integrated_report.receipts,
        )?
        .context("integrated next step should advance to finalize")?;
        assert_eq!(finalize_next_step.step, "finalize");

        let finalized = record_dev_return_finalize(
            &store,
            NotaDevReturnFinalizeRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                summary: Some("Boundary closed after finalize".to_string()),
            },
        )?;
        assert_eq!(finalized.status, "recorded");
        assert_eq!(finalized.finalize.state, "closed");
        assert_eq!(
            finalized.finalize.summary.as_deref(),
            Some("Boundary closed after finalize")
        );
        assert!(finalized.next_step.is_none());
        assert_eq!(
            finalized.receipt.receipt_kind,
            DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
        );

        let finalized_report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert!(finalized_report
            .receipts
            .iter()
            .any(|receipt| receipt.id == finalized.receipt.id));

        let recorded_finalize = derive_nota_runtime_finalize(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &finalized_report.receipts,
        )?
        .context("recorded finalize should be exposed")?;
        assert_eq!(recorded_finalize.state, "closed");
        assert_eq!(
            recorded_finalize.summary.as_deref(),
            Some("Boundary closed after finalize")
        );
        assert!(derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &runtime_transactions,
            allocations.stored_allocations(),
            &finalized_report.receipts,
        )?
        .is_none());

        let duplicate_finalized = record_dev_return_finalize(
            &store,
            NotaDevReturnFinalizeRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                summary: Some("Boundary closed after finalize".to_string()),
            },
        )?;
        assert_eq!(duplicate_finalized.status, "already_recorded");

        let duplicate_integrated = record_dev_return_integration(
            &store,
            NotaDevReturnIntegrateRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                state: "integrated".to_string(),
                summary: Some("Integration landed and is ready to finalize".to_string()),
            },
        )?;
        assert_eq!(duplicate_integrated.status, "already_recorded");

        let conflicting_integrate_result = record_dev_return_integration(
            &store,
            NotaDevReturnIntegrateRequest {
                transaction_id: transaction.id,
                allocation_id: allocation.id,
                state: "repair_requested".to_string(),
                summary: Some("Integration found a regression".to_string()),
            },
        );
        assert!(conflicting_integrate_result.is_err());

        let closure_checkpoint = materialize_runtime_closure_checkpoint(&store)?;
        assert_eq!(closure_checkpoint.status, "applied");
        assert_eq!(
            closure_checkpoint
                .source_recommendation
                .as_ref()
                .and_then(|checkpoint| checkpoint.selected_trunk.as_deref()),
            Some("dev return closure truth")
        );
        assert_eq!(
            closure_checkpoint.superseded_checkpoint_id,
            current_checkpoint.map(|checkpoint| checkpoint.cadence_object.id)
        );

        let closure_checkpoints = list_runtime_checkpoints(&store)?;
        let closure_checkpoint_record = closure_checkpoints
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.cadence_object.is_current)
            .context("closure checkpoint should become current")?;
        assert_eq!(
            closure_checkpoint_record.payload.selected_trunk.as_deref(),
            Some("dev return closure truth")
        );

        let closure_scope_ids =
            active_checkpoint_scope_ids(&store, Some(closure_checkpoint_record))?;
        let closure_transactions = store.list_nota_runtime_transactions()?;
        assert_eq!(
            closure_scope_ids[0],
            closure_checkpoint_record.cadence_object.id
        );
        assert!(closure_scope_ids.contains(&current_checkpoint.unwrap().cadence_object.id));

        let closure_receipts = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert!(closure_receipts.receipts.iter().any(|receipt| {
            if receipt.receipt_kind != CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND {
                return false;
            }
            serde_json::from_str::<Value>(&receipt.payload_json)
                .ok()
                .and_then(|payload| payload["checkpoint_id"].as_i64())
                == Some(closure_checkpoint_record.cadence_object.id)
        }));

        let closure_acceptance_bundles = list_runtime_acceptance_bundles(&store)?;
        assert_eq!(closure_acceptance_bundles.acceptance_bundle_count, 2);
        let current_acceptance =
            derive_current_runtime_acceptance_bundle(&store, &closure_scope_ids)?
                .context("current acceptance bundle should follow the closure checkpoint")?;
        assert_eq!(
            current_acceptance.payload.acceptance_kind,
            "dev_return_acceptance"
        );
        assert_eq!(current_acceptance.payload.round_state, "fully_settled");
        assert!(current_acceptance.payload.fully_settled);
        assert_eq!(
            current_acceptance.payload.finalize_state.as_deref(),
            Some(DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE)
        );

        let closure_round_state = derive_runtime_round_state_projection(
            Some(closure_checkpoint_record),
            Some(&current_acceptance),
            None,
        );
        assert_eq!(closure_round_state.state, "fully_settled");
        assert!(closure_round_state.accepted);
        assert!(closure_round_state.acceptance_present);
        assert!(closure_round_state.carry_forward_checkpointed);
        assert!(closure_round_state.fully_settled);
        assert!(!closure_round_state.next_step_open);

        let closure_human_rounds = list_runtime_human_rounds(&store)?;
        let closure_human_round = closure_human_rounds
            .human_rounds
            .iter()
            .find(|round| round.cadence_object.is_current)
            .context("closure human round should be current")?;
        assert_eq!(closure_human_round.payload.round_state, "fully_settled");
        assert_eq!(
            closure_human_round.payload.detail_round_state.as_deref(),
            Some("fully_settled")
        );
        assert!(closure_human_round.payload.accepted);
        assert!(closure_human_round.payload.acceptance_present);
        assert!(closure_human_round.payload.carry_forward_checkpointed);
        assert!(closure_human_round.payload.fully_settled);
        assert!(!closure_human_round.payload.next_step_open);

        let anti_zeno = derive_anti_zeno_projection(
            Some(closure_checkpoint_record),
            Some(&current_acceptance),
            None,
            closure_checkpoint.source_recommendation.as_ref(),
        );
        assert_eq!(anti_zeno.state, "fully_settled");
        assert_eq!(anti_zeno.value, 100);

        let carried_review = derive_nota_runtime_review(
            &closure_scope_ids,
            &closure_transactions,
            allocations.stored_allocations(),
            &closure_receipts.receipts,
        )?
        .context("review truth should survive checkpoint supersession")?;
        assert_eq!(carried_review.state, "review_recorded");
        assert_eq!(
            carried_review.verdict.as_deref(),
            Some(DEV_RETURN_REVIEW_APPROVED_VERDICT)
        );

        let carried_integrate = derive_nota_runtime_integrate(
            &closure_scope_ids,
            &closure_transactions,
            allocations.stored_allocations(),
            &closure_receipts.receipts,
        )?
        .context("integrate truth should survive checkpoint supersession")?;
        assert_eq!(
            carried_integrate.state,
            DEV_RETURN_INTEGRATE_RECORDED_RUNTIME_STATE
        );
        assert_eq!(
            carried_integrate.outcome.as_deref(),
            Some(DEV_RETURN_INTEGRATE_INTEGRATED_STATE)
        );

        let carried_finalize = derive_nota_runtime_finalize(
            &closure_scope_ids,
            &closure_transactions,
            allocations.stored_allocations(),
            &closure_receipts.receipts,
        )?
        .context("finalize truth should survive checkpoint supersession")?;
        assert_eq!(
            carried_finalize.state,
            DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE
        );
        assert_eq!(
            carried_finalize.summary.as_deref(),
            Some("Boundary closed after finalize")
        );
        assert!(derive_nota_runtime_next_step(
            &closure_scope_ids,
            &closure_transactions,
            allocations.stored_allocations(),
            &closure_receipts.receipts,
        )?
        .is_none());

        Ok(())
    }

    #[test]
    fn checkpoint_write_backfills_agent_return_acceptance_for_current_checkpoint() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;
        let task_id =
            store.insert_forge_task("Agent child", "echo", "[]", None, None, "[]", "{}")?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: "Agent return acceptance backflow",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: Some(task_id),
            cadence_checkpoint_id: None,
        })?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-48".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "fallback".to_string(),
            issue_title: Some("Agent return acceptance".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-48".to_string(),
            prompt_source: "Entrance-owned harness/bootstrap agent prompt".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "agent".to_string(),
            child_dispatch_tool_name: "forge_dispatch_agent".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
            }),
        };
        let lineage_ref = format!(
            "nota/do/transaction/{}/forge-task/{task_id}",
            transaction.id
        );
        let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: &lineage_ref,
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&AllocationTerminalOutcomeReceiptPayload {
                allocation_id: allocation.id,
                lineage_ref: allocation.lineage_ref.clone(),
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
                allocation_status: "return_ready".to_string(),
            })?,
            status: "recorded",
        })?;

        let allocations = list_nota_runtime_allocations(&store)?;
        let recommendation =
            recommend_runtime_closure_checkpoint(&store, allocations.stored_allocations(), None)?
                .context("agent return checkpoint recommendation should exist")?;
        assert_eq!(
            recommendation.selected_trunk.as_deref(),
            Some("agent return acceptance truth")
        );
        assert_eq!(
            recommendation.title.as_deref(),
            Some("Checkpoint: agent return acceptance truth for MYT-48")
        );
        assert_eq!(
            recommendation.stable_level,
            "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned agent return boundary surfaced as storage-backed acceptance truth"
        );
        assert_eq!(
            recommendation.landed[3],
            format!(
                "Runtime payloads keep execution_host `in_process` and child_dispatch_role `agent` visible for transaction {} / allocation {}.",
                transaction.id,
                allocation.id
            )
        );
        assert_eq!(
            recommendation.remaining[0],
            "This is a returned agent child boundary, not a completed review / integrate / repair loop; fuller allocator closure is still open."
        );
        let checkpoint_report = write_runtime_checkpoint(&store, recommendation.clone())?;
        store.update_nota_runtime_transaction(
            transaction.id,
            NotaRuntimeTransactionUpdate {
                status: "checkpointed",
                forge_task_id: Some(task_id),
                cadence_checkpoint_id: Some(checkpoint_report.checkpoint.cadence_object.id),
            },
        )?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&serde_json::json!({
                "checkpoint_id": checkpoint_report.checkpoint.cadence_object.id,
                "selected_trunk": checkpoint_report.checkpoint.payload.selected_trunk,
            }))?,
            status: "recorded",
        })?;

        let seeded_receipts = store.list_nota_runtime_receipts(Some(transaction.id))?;
        let seeded_accepted = seeded_receipts
            .iter()
            .find(|receipt| receipt.receipt_kind == AGENT_RETURN_ACCEPTED_RECEIPT_KIND)
            .context("checkpoint write should already backfill the agent acceptance receipt")?;

        let report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(report.receipt_count, seeded_receipts.len());
        let accepted_receipt = report
            .receipts
            .iter()
            .find(|receipt| receipt.receipt_kind == AGENT_RETURN_ACCEPTED_RECEIPT_KIND)
            .context("receipt surface should retain the backfilled agent acceptance receipt")?;
        assert_eq!(accepted_receipt.id, seeded_accepted.id);

        let accepted_payload: Value = serde_json::from_str(&accepted_receipt.payload_json)?;
        assert_eq!(accepted_payload["allocation_id"], allocation.id);
        assert_eq!(accepted_payload["lineage_ref"], allocation.lineage_ref);
        assert_eq!(
            accepted_payload["checkpoint_id"],
            checkpoint_report.checkpoint.cadence_object.id
        );
        assert_eq!(accepted_payload["child_dispatch_role"], "agent");
        assert_eq!(accepted_payload["execution_host"], "in_process");
        assert_eq!(accepted_payload["target_kind"], "nota_runtime_transaction");
        assert_eq!(accepted_payload["target_ref"], transaction.id.to_string());

        let second_report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(second_report.receipt_count, report.receipt_count);

        let acceptance_bundles = list_runtime_acceptance_bundles(&store)?;
        assert_eq!(acceptance_bundles.acceptance_bundle_count, 1);
        let current_bundle = acceptance_bundles
            .acceptance_bundles
            .iter()
            .find(|bundle| bundle.cadence_object.is_current)
            .context("agent return acceptance bundle should be current")?;
        assert_eq!(
            current_bundle.cadence_object.cadence_kind,
            CADENCE_ACCEPTANCE_BUNDLE_KIND
        );
        assert_eq!(current_bundle.payload.transaction_id, transaction.id);
        assert_eq!(current_bundle.payload.allocation_id, allocation.id);
        assert_eq!(
            current_bundle.payload.acceptance_kind,
            "agent_return_acceptance"
        );
        assert!(!current_bundle.payload.fully_settled);

        let closure_recommendation = recommend_runtime_closure_checkpoint(
            &store,
            allocations.stored_allocations(),
            Some(&checkpoint_report.checkpoint),
        )?
        .context("agent return closure recommendation should exist after acceptance")?;
        assert_eq!(
            closure_recommendation.selected_trunk.as_deref(),
            Some("agent return closure truth")
        );
        assert_eq!(
            closure_recommendation.title.as_deref(),
            Some("Checkpoint: agent return closure truth for MYT-48")
        );

        let closure_checkpoint = materialize_runtime_closure_checkpoint(&store)?;
        assert_eq!(closure_checkpoint.status, "applied");
        assert_eq!(
            closure_checkpoint
                .source_recommendation
                .as_ref()
                .and_then(|checkpoint| checkpoint.selected_trunk.as_deref()),
            Some("agent return closure truth")
        );
        assert_eq!(
            closure_checkpoint.superseded_checkpoint_id,
            Some(checkpoint_report.checkpoint.cadence_object.id)
        );

        let closure_checkpoints = list_runtime_checkpoints(&store)?;
        let closure_checkpoint_record = closure_checkpoints
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.cadence_object.is_current)
            .context("agent closure checkpoint should become current")?;
        let closure_scope_ids =
            active_checkpoint_scope_ids(&store, Some(closure_checkpoint_record))?;
        let current_acceptance =
            derive_current_runtime_acceptance_bundle(&store, &closure_scope_ids)?
                .context("agent closure acceptance bundle should be readable")?;
        assert_eq!(
            current_acceptance.payload.acceptance_kind,
            "agent_return_acceptance"
        );
        assert_eq!(current_acceptance.payload.round_state, "fully_settled");
        assert!(current_acceptance.payload.fully_settled);

        let closure_round_state = derive_runtime_round_state_projection(
            Some(closure_checkpoint_record),
            Some(&current_acceptance),
            None,
        );
        assert_eq!(closure_round_state.state, "fully_settled");
        assert!(closure_round_state.accepted);
        assert!(closure_round_state.carry_forward_checkpointed);
        assert!(!closure_round_state.next_step_open);

        let settled_handout =
            derive_current_runtime_handout(&store)?.context("settled handout should exist")?;
        assert_eq!(
            settled_handout.payload.checkpoint_id,
            closure_checkpoint_record.cadence_object.id
        );
        assert_eq!(settled_handout.payload.round_state, "fully_settled");
        assert_eq!(
            settled_handout.payload.detail_round_state.as_deref(),
            Some("fully_settled")
        );
        assert!(derive_current_runtime_wake_request(&store)?.is_none());

        Ok(())
    }

    #[test]
    fn allocation_read_surface_projects_terminal_outcome_without_writing_on_readonly_database(
    ) -> Result<()> {
        let temp_db = TempDbPath::new("readonly-allocation-surface")?;
        let migration_plan = MigrationPlan::new(crate::plugins::forge::migrations());
        let writable_store = DataStore::open(temp_db.path(), migration_plan)?;

        let transaction =
            writable_store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
                actor_role: "nota",
                surface_action: "do",
                transaction_kind: "forge_agent_dispatch",
                title: "Readonly allocation surface",
                payload_json: "{}",
                status: "checkpointed",
                forge_task_id: None,
                cadence_checkpoint_id: None,
            })?;
        let task_id = writable_store.insert_forge_task(
            "Blocked child",
            "echo",
            "[]",
            None,
            None,
            "[]",
            "{}",
        )?;
        writable_store.update_forge_task_status(
            task_id,
            "Blocked",
            None,
            Some("add openai to Vault first"),
        )?;
        let allocation_payload = NotaDoAllocationPayload {
            issue_id: "MYT-48".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "linear".to_string(),
            issue_title: Some("Test issue".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-48".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "agent".to_string(),
            child_dispatch_tool_name: "forge_dispatch_agent".to_string(),
            terminal_outcome: None,
        };
        writable_store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/do/transaction/1/forge-task/1",
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "task_created",
            payload_json: &serde_json::to_string(&allocation_payload)?,
        })?;
        drop(writable_store);

        let readonly_store = DataStore::open_read_only(temp_db.path(), migration_plan)?;
        let readonly_report = list_nota_runtime_allocations(&readonly_store)?;
        assert_eq!(readonly_report.allocation_count, 1);
        assert_eq!(readonly_report.allocations[0].status, "escalated_blocked");
        assert_eq!(
            readonly_report.allocations[0]
                .child_dispatch_role
                .as_deref(),
            Some("agent")
        );
        assert_eq!(
            readonly_report.allocations[0]
                .child_dispatch_tool_name
                .as_deref(),
            Some("forge_dispatch_agent")
        );
        let readonly_payload: NotaDoAllocationPayload =
            serde_json::from_str(&readonly_report.allocations[0].payload_json)?;
        let readonly_outcome = readonly_payload
            .terminal_outcome
            .expect("read surface should project a terminal outcome");
        assert_eq!(readonly_outcome.boundary_kind, "escalation");
        assert_eq!(readonly_outcome.child_execution_status, "Blocked");
        assert_eq!(
            readonly_outcome.child_execution_status_message.as_deref(),
            Some("add openai to Vault first")
        );
        drop(readonly_store);

        let verify_store = DataStore::open(temp_db.path(), migration_plan)?;
        let stored_allocations = verify_store.list_nota_runtime_allocations()?;
        assert_eq!(stored_allocations.len(), 1);
        assert_eq!(stored_allocations[0].status, "task_created");
        let stored_payload: NotaDoAllocationPayload =
            serde_json::from_str(&stored_allocations[0].payload_json)?;
        assert!(stored_payload.terminal_outcome.is_none());
        assert!(verify_store
            .list_nota_runtime_receipts(Some(transaction.id))?
            .is_empty());

        Ok(())
    }

    #[test]
    fn runtime_closure_recommendation_prefers_newer_dev_return_boundary() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let do_transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: "Older agent continuity",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let do_payload = NotaDoAllocationPayload {
            issue_id: "MYT-48".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "fallback".to_string(),
            issue_title: None,
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-48".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "agent".to_string(),
            child_dispatch_tool_name: "forge_dispatch_agent".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Blocked".to_string(),
                child_execution_status_message: Some("add openai to Vault first".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: do_transaction.id.to_string(),
            }),
        };
        let do_allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: do_transaction.id,
            lineage_ref: "nota/do/transaction/1/forge-task/11",
            child_execution_kind: "forge_task",
            child_execution_ref: "11",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &do_transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &do_transaction.id.to_string(),
            status: "escalated_blocked",
            payload_json: &serde_json::to_string(&do_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: do_transaction.id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&AllocationTerminalOutcomeReceiptPayload {
                allocation_id: do_allocation.id,
                lineage_ref: do_allocation.lineage_ref.clone(),
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Blocked".to_string(),
                child_execution_status_message: Some("add openai to Vault first".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: do_transaction.id.to_string(),
                allocation_status: "escalated_blocked".to_string(),
            })?,
            status: "recorded",
        })?;

        let dev_transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Newer dev return",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let dev_payload = NotaDoAllocationPayload {
            issue_id: "MYT-1048".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "fallback".to_string(),
            issue_title: None,
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-1048".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: NotaDispatchExecutionHost::DetachedForgeCliSupervisor
                .as_str()
                .to_string(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: dev_transaction.id.to_string(),
            }),
        };
        let dev_allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: dev_transaction.id,
            lineage_ref: "nota/dev/transaction/2/forge-task/28",
            child_execution_kind: "forge_task",
            child_execution_ref: "28",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &dev_transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &dev_transaction.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&dev_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: dev_transaction.id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&AllocationTerminalOutcomeReceiptPayload {
                allocation_id: dev_allocation.id,
                lineage_ref: dev_allocation.lineage_ref.clone(),
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: dev_transaction.id.to_string(),
                allocation_status: "return_ready".to_string(),
            })?,
            status: "recorded",
        })?;

        let report = list_nota_runtime_allocations(&store)?;
        let recommendation =
            recommend_runtime_closure_checkpoint(&store, report.stored_allocations(), None)?
                .expect("newer dev return should become the recommended closure");

        assert_eq!(
            recommendation.selected_trunk.as_deref(),
            Some("dev return acceptance truth")
        );
        assert_eq!(
            recommendation.landed[0],
            format!(
                "NOTA-owned dev allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                dev_allocation.id,
                dev_allocation.lineage_ref,
                dev_transaction.id,
                dev_allocation.child_execution_ref
            )
        );
        assert_eq!(
            recommendation.landed[3],
            format!(
                "Runtime payloads keep execution_host `detached_forge_cli_supervisor` and child_dispatch_role `dev` visible for transaction {} / allocation {}.",
                dev_transaction.id,
                dev_allocation.id
            )
        );
        assert_eq!(
            recommendation.remaining[0],
            "This is a returned dev child boundary, not a completed review / integrate / repair loop; M9 return closure is still open."
        );

        Ok(())
    }

    #[test]
    fn dev_return_surfaces_stay_pinned_to_requested_checkpoint_scope() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint_one = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Scope one".to_string()),
                stable_level: "scope one".to_string(),
                landed: vec!["first scoped boundary".to_string()],
                remaining: vec!["review still open".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("scope one".to_string()),
                next_start_hints: vec!["read scope one".to_string()],
                project_dir: None,
            },
        )?;
        let checkpoint_two = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Scope two".to_string()),
                stable_level: "scope two".to_string(),
                landed: vec!["second scoped boundary".to_string()],
                remaining: vec!["review still open".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("scope two".to_string()),
                next_start_hints: vec!["read scope two".to_string()],
                project_dir: None,
            },
        )?;

        let transaction_one = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Scope one transaction",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: Some(checkpoint_one.checkpoint.cadence_object.id),
        })?;
        let transaction_two = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Scope two transaction",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: Some(checkpoint_two.checkpoint.cadence_object.id),
        })?;

        let allocation_one_payload = NotaDoAllocationPayload {
            issue_id: "MYT-SCOPE-1".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("Scoped boundary one".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-SCOPE-1".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction_one.id.to_string(),
            }),
        };
        let allocation_one = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction_one.id,
            lineage_ref: "nota/dev/transaction/1/forge-task/11",
            child_execution_kind: "forge_task",
            child_execution_ref: "11",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction_one.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction_one.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_one_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction_one.id,
            receipt_kind: DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&serde_json::json!({
                "checkpoint_id": checkpoint_one.checkpoint.cadence_object.id,
                "review": {
                    "state": "review_recorded",
                    "transaction_id": transaction_one.id,
                    "allocation_id": allocation_one.id,
                    "lineage_ref": allocation_one.lineage_ref,
                    "child_dispatch_role": "dev",
                    "execution_host": "in_process",
                    "target_kind": "nota_runtime_transaction",
                    "target_ref": transaction_one.id.to_string(),
                    "verdict": "approved",
                    "summary": "scope one review"
                },
                "next_step": {
                    "step": "integrate",
                    "transaction_id": transaction_one.id,
                    "allocation_id": allocation_one.id,
                    "lineage_ref": allocation_one.lineage_ref,
                    "child_dispatch_role": "dev",
                    "execution_host": "in_process",
                    "target_kind": "nota_runtime_transaction",
                    "target_ref": transaction_one.id.to_string()
                }
            }))?,
            status: "recorded",
        })?;

        let allocation_two_payload = NotaDoAllocationPayload {
            issue_id: "MYT-SCOPE-2".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("Scoped boundary two".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-SCOPE-2".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction_two.id.to_string(),
            }),
        };
        let allocation_two = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction_two.id,
            lineage_ref: "nota/dev/transaction/2/forge-task/22",
            child_execution_kind: "forge_task",
            child_execution_ref: "22",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction_two.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction_two.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_two_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction_two.id,
            receipt_kind: DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&serde_json::json!({
                "checkpoint_id": checkpoint_two.checkpoint.cadence_object.id,
                "review": {
                    "state": "review_recorded",
                    "transaction_id": transaction_two.id,
                    "allocation_id": allocation_two.id,
                    "lineage_ref": allocation_two.lineage_ref,
                    "child_dispatch_role": "dev",
                    "execution_host": "in_process",
                    "target_kind": "nota_runtime_transaction",
                    "target_ref": transaction_two.id.to_string(),
                    "verdict": "approved",
                    "summary": "scope two review"
                },
                "next_step": {
                    "step": "integrate",
                    "transaction_id": transaction_two.id,
                    "allocation_id": allocation_two.id,
                    "lineage_ref": allocation_two.lineage_ref,
                    "child_dispatch_role": "dev",
                    "execution_host": "in_process",
                    "target_kind": "nota_runtime_transaction",
                    "target_ref": transaction_two.id.to_string()
                }
            }))?,
            status: "recorded",
        })?;

        let checkpoint_scope_ids = vec![checkpoint_one.checkpoint.cadence_object.id];
        let transactions = store.list_nota_runtime_transactions()?;
        let allocations = store.list_nota_runtime_allocations()?;
        let receipts = store.list_nota_runtime_receipts(None)?;

        let review = derive_nota_runtime_review(
            &checkpoint_scope_ids,
            &transactions,
            &allocations,
            &receipts,
        )?
        .context("scope one review should stay visible")?;
        assert_eq!(review.allocation_id, allocation_one.id);
        assert_eq!(review.transaction_id, transaction_one.id);
        assert_eq!(review.summary.as_deref(), Some("scope one review"));

        let next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
            &transactions,
            &allocations,
            &receipts,
        )?
        .context("scope one next step should stay visible")?;
        assert_eq!(next_step.allocation_id, allocation_one.id);
        assert_eq!(next_step.transaction_id, transaction_one.id);
        assert_eq!(next_step.step, "integrate");

        Ok(())
    }

    #[test]
    fn runtime_closure_recommendation_respects_requested_checkpoint_scope() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))?;

        let checkpoint_one = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Requested scope".to_string()),
                stable_level: "checkpoint scope one".to_string(),
                landed: vec!["first recommendation scope".to_string()],
                remaining: vec!["acceptance not yet written".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("scope one".to_string()),
                next_start_hints: vec!["read scope one".to_string()],
                project_dir: None,
            },
        )?;
        let checkpoint_two = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Out of scope".to_string()),
                stable_level: "checkpoint scope two".to_string(),
                landed: vec!["second recommendation scope".to_string()],
                remaining: vec!["acceptance not yet written".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("scope two".to_string()),
                next_start_hints: vec!["read scope two".to_string()],
                project_dir: None,
            },
        )?;

        let transaction_one = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Requested scope transaction",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: Some(checkpoint_one.checkpoint.cadence_object.id),
        })?;
        let transaction_two = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "dev",
            transaction_kind: "forge_dev_dispatch",
            title: "Out of scope transaction",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: None,
            cadence_checkpoint_id: Some(checkpoint_two.checkpoint.cadence_object.id),
        })?;

        let allocation_one_payload = NotaDoAllocationPayload {
            issue_id: "MYT-SCOPE-OLD".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("Requested scope boundary".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-SCOPE-OLD".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction_one.id.to_string(),
            }),
        };
        let allocation_one = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction_one.id,
            lineage_ref: "nota/dev/transaction/10/forge-task/10",
            child_execution_kind: "forge_task",
            child_execution_ref: "10",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction_one.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction_one.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_one_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction_one.id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&AllocationTerminalOutcomeReceiptPayload {
                allocation_id: allocation_one.id,
                lineage_ref: allocation_one.lineage_ref.clone(),
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction_one.id.to_string(),
                allocation_status: "return_ready".to_string(),
            })?,
            status: "recorded",
        })?;

        let allocation_two_payload = NotaDoAllocationPayload {
            issue_id: "MYT-SCOPE-NEW".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("Out of scope boundary".to_string()),
            project_root: "A:/Agent/Entrance".to_string(),
            worktree_path: "A:/Agent/Entrance/worktrees/feat-MYT-SCOPE-NEW".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: default_nota_dispatch_execution_host(),
            child_dispatch_role: "dev".to_string(),
            child_dispatch_tool_name: "forge_dispatch_dev".to_string(),
            terminal_outcome: Some(NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction_two.id.to_string(),
            }),
        };
        let allocation_two = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_dev",
            allocation_kind: "forge_dev_dispatch",
            source_transaction_id: transaction_two.id,
            lineage_ref: "nota/dev/transaction/20/forge-task/20",
            child_execution_kind: "forge_task",
            child_execution_ref: "20",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction_two.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction_two.id.to_string(),
            status: "return_ready",
            payload_json: &serde_json::to_string(&allocation_two_payload)?,
        })?;
        store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction_two.id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &serde_json::to_string(&AllocationTerminalOutcomeReceiptPayload {
                allocation_id: allocation_two.id,
                lineage_ref: allocation_two.lineage_ref.clone(),
                boundary_kind: "return".to_string(),
                child_execution_status: "Done".to_string(),
                child_execution_status_message: None,
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction_two.id.to_string(),
                allocation_status: "return_ready".to_string(),
            })?,
            status: "recorded",
        })?;

        let allocations = list_nota_runtime_allocations(&store)?;
        let recommendation = recommend_runtime_closure_checkpoint(
            &store,
            allocations.stored_allocations(),
            Some(&checkpoint_one.checkpoint),
        )?
        .context("requested scope recommendation should exist")?;

        assert_eq!(
            recommendation.title.as_deref(),
            Some("Checkpoint: dev return acceptance truth for MYT-SCOPE-OLD")
        );
        assert_eq!(
            recommendation.selected_trunk.as_deref(),
            Some("dev return acceptance truth")
        );
        assert_eq!(
            recommendation.landed[0],
            format!(
                "NOTA-owned dev allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
                allocation_one.id,
                allocation_one.lineage_ref,
                transaction_one.id,
                allocation_one.child_execution_ref
            )
        );

        Ok(())
    }
}
