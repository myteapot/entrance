use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::core::data_store::{
    DataStore, NewCadenceLink, NewCadenceObject, NewNotaRuntimeAllocation, NewNotaRuntimeReceipt,
    NewNotaRuntimeTransaction, NotaRuntimeAllocationUpdate, NotaRuntimeTransactionUpdate,
    StoredCadenceLink, StoredCadenceObject, StoredForgeTask, StoredNotaRuntimeAllocation,
    StoredNotaRuntimeReceipt, StoredNotaRuntimeTransaction,
};
use crate::plugins::forge::{
    build_agent_task_request, prepare_agent_dispatch_blocking, ForgePlugin, PreparedAgentDispatch,
};

const CADENCE_CHECKPOINT_KIND: &str = "CADENCE_CHECKPOINT";
const CADENCE_HANDOUT_KIND: &str = "CADENCE_HANDOUT";
const CADENCE_WAKE_REQUEST_KIND: &str = "CADENCE_WAKE_REQUEST";
const CADENCE_POLICY_NOTE_KIND: &str = "CADENCE_POLICY_NOTE";
const NOTA_RUNTIME_SOURCE_TYPE: &str = "nota_runtime";
const NOTA_RUNTIME_SCOPE_TYPE: &str = "runtime";
const NOTA_RUNTIME_SCOPE_REF: &str = "Entrance";
const ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND: &str =
    "ALLOCATION_TERMINAL_OUTCOME_RECORDED";

#[derive(Debug, Clone)]
pub struct NotaCheckpointRequest {
    pub title: Option<String>,
    pub stable_level: String,
    pub landed: Vec<String>,
    pub remaining: Vec<String>,
    pub human_continuity_bus: String,
    pub selected_trunk: Option<String>,
    pub next_start_hints: Vec<String>,
    pub project_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoContext {
    pub project_dir: String,
    pub git_branch: Option<String>,
    pub git_head: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotaCheckpointPayload {
    pub stable_level: String,
    pub landed: Vec<String>,
    pub remaining: Vec<String>,
    pub human_continuity_bus: String,
    pub selected_trunk: Option<String>,
    pub next_start_hints: Vec<String>,
    pub repo_context: Option<RepoContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaCheckpointRecord {
    #[serde(flatten)]
    pub cadence_object: StoredCadenceObject,
    pub payload: NotaCheckpointPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaCheckpointWriteReport {
    pub checkpoint: NotaCheckpointRecord,
    pub superseded_checkpoint_id: Option<i64>,
    pub supersession_link: Option<StoredCadenceLink>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaCheckpointListReport {
    pub checkpoint_count: usize,
    pub current_checkpoint_id: Option<i64>,
    pub checkpoints: Vec<NotaCheckpointRecord>,
}

#[derive(Debug, Clone)]
pub struct NotaDoAgentDispatchRequest {
    pub project_dir: Option<String>,
    pub model: String,
    pub agent_command: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotaDoDispatchPayload {
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt_source: String,
    pub model: String,
    pub agent_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotaDoAllocationPayload {
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt_source: String,
    pub model: String,
    pub agent_command: Option<String>,
    pub child_dispatch_role: String,
    pub child_dispatch_tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_outcome: Option<NotaDoAllocationTerminalOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaDoAllocationTerminalOutcome {
    pub boundary_kind: String,
    pub child_execution_status: String,
    pub child_execution_status_message: Option<String>,
    pub target_kind: String,
    pub target_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AllocationTerminalOutcomeReceiptPayload {
    allocation_id: i64,
    lineage_ref: String,
    boundary_kind: String,
    child_execution_status: String,
    child_execution_status_message: Option<String>,
    target_kind: String,
    target_ref: String,
    allocation_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaDoDispatchReport {
    pub transaction: StoredNotaRuntimeTransaction,
    pub allocation: StoredNotaRuntimeAllocation,
    pub receipts: Vec<StoredNotaRuntimeReceipt>,
    pub dispatch: PreparedAgentDispatch,
    pub task_id: i64,
    pub task_status: String,
    pub spawn_error: Option<String>,
    pub checkpoint: NotaCheckpointRecord,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeTransactionsReport {
    pub transaction_count: usize,
    pub transactions: Vec<StoredNotaRuntimeTransaction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeAllocationsReport {
    pub allocation_count: usize,
    pub allocations: Vec<StoredNotaRuntimeAllocation>,
}

pub fn write_runtime_checkpoint(
    data_store: &DataStore,
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

    let superseded_checkpoint = data_store
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
    let cadence_object = data_store.insert_cadence_object(NewCadenceObject {
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
        Some(data_store.insert_cadence_link(NewCadenceLink {
            src_cadence_object_id: previous.id,
            dst_cadence_object_id: cadence_object.id,
            relation_type: "superseded_by",
            status: "active",
        })?)
    } else {
        None
    };

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

pub fn run_nota_do_agent_dispatch(
    data_store: &DataStore,
    forge: &ForgePlugin,
    request: NotaDoAgentDispatchRequest,
) -> Result<NotaDoDispatchReport> {
    let model = request.model.trim().to_string();
    if model.is_empty() {
        return Err(anyhow!("`model` must not be empty"));
    }

    let dispatch = prepare_agent_dispatch_blocking(data_store.clone(), request.project_dir.clone())
        .map_err(anyhow::Error::msg)?;
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
    };
    let payload_json =
        serde_json::to_string(&payload).context("failed to serialize nota do payload")?;

    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Do dispatch {}", dispatch.issue_id));

    let mut transaction =
        data_store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: &title,
            payload_json: &payload_json,
            status: "accepted",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
    let mut receipts = Vec::new();
    receipts.push(
        data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: "DO_ACCEPTED",
            payload_json: &payload_json,
            status: "recorded",
        })?,
    );

    let task_request = build_agent_task_request(
        dispatch.issue_id.clone(),
        dispatch.worktree_path.clone(),
        model.clone(),
        dispatch.prompt.clone(),
        Vec::new(),
        request.agent_command.clone(),
    )
    .map_err(anyhow::Error::msg)?;
    let task_id = forge.create_task(task_request)?;
    let task = forge
        .get_task(task_id)?
        .ok_or_else(|| anyhow!("stored Forge task disappeared after nota do dispatch"))?;
    transaction = data_store.update_nota_runtime_transaction(
        transaction.id,
        NotaRuntimeTransactionUpdate {
            status: "task_created",
            forge_task_id: Some(task_id),
            cadence_checkpoint_id: None,
        },
    )?;
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
        child_dispatch_role: actor_role_slug(dispatch.dispatch_role).to_string(),
        child_dispatch_tool_name: dispatch.dispatch_tool_name.clone(),
        terminal_outcome: None,
    };
    let allocation_payload_json = serde_json::to_string(&allocation_payload)
        .context("failed to serialize nota allocation payload")?;
    let child_execution_ref = task_id.to_string();
    let return_target_ref = transaction.id.to_string();
    let lineage_ref = build_do_allocation_lineage_ref(transaction.id, task_id);
    let mut allocation = data_store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
        allocator_role: "nota",
        allocator_surface: "nota_do",
        allocation_kind: "forge_agent_dispatch",
        source_transaction_id: transaction.id,
        lineage_ref: &lineage_ref,
        child_execution_kind: "forge_task",
        child_execution_ref: &child_execution_ref,
        return_target_kind: "nota_runtime_transaction",
        return_target_ref: &return_target_ref,
        escalation_target_kind: "nota_runtime_transaction",
        escalation_target_ref: &return_target_ref,
        status: "task_created",
        payload_json: &allocation_payload_json,
    })?;
    receipts.push(
        data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: "FORGE_TASK_CREATED",
            payload_json: &serde_json::to_string(&json!({
                "task_id": task_id,
                "task_status": task.status,
                "task_command": task.command,
                "worktree_path": task.working_dir,
            }))
            .context("failed to serialize forge task receipt payload")?,
            status: "recorded",
        })?,
    );
    receipts.push(
        data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
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
            .context("failed to serialize allocation receipt payload")?,
            status: "recorded",
        })?,
    );

    let spawn_error = forge
        .engine()
        .spawn_task(task_id)
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
    transaction = data_store.update_nota_runtime_transaction(
        transaction.id,
        NotaRuntimeTransactionUpdate {
            status: transaction_status,
            forge_task_id: Some(task_id),
            cadence_checkpoint_id: None,
        },
    )?;
    allocation = data_store.update_nota_runtime_allocation(
        allocation.id,
        NotaRuntimeAllocationUpdate {
            status: transaction_status,
            payload_json: None,
        },
    )?;
    let launch_receipt_kind = if spawn_error.is_some() {
        "FORGE_TASK_SPAWN_FAILED"
    } else {
        "FORGE_TASK_DISPATCHED"
    };
    receipts.push(
        data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: launch_receipt_kind,
            payload_json: &serde_json::to_string(&json!({
                "task_id": task_id,
                "task_status": task_after_spawn.status.clone(),
                "status_message": task_after_spawn.status_message.clone(),
                "spawn_error": spawn_error.clone(),
            }))
            .context("failed to serialize forge launch receipt payload")?,
            status: "recorded",
        })?,
    );

    let checkpoint_report = write_runtime_checkpoint(
        data_store,
        NotaCheckpointRequest {
            title: Some(format!("Do allocation: {}", dispatch.issue_id)),
            stable_level: "single-ingress, checkpointed, DB-first NOTA host with a minimal Do allocation object and allocation-owned terminal outcome boundary".to_string(),
            landed: build_do_checkpoint_landed_items(
                transaction.id,
                &allocation,
                task_id,
                &dispatch,
                &spawn_error,
            ),
            remaining: build_do_checkpoint_remaining_items(allocation.id, task_id, &spawn_error),
            human_continuity_bus: if spawn_error.is_some() {
                "still required for operator recovery".to_string()
            } else {
                "reduced but not eliminated".to_string()
            },
            selected_trunk: Some("Do allocation storage cut".to_string()),
            next_start_hints: build_do_checkpoint_hints(
                transaction.id,
                allocation.id,
                task_id,
                &spawn_error,
            ),
            project_dir: Some(dispatch.project_root.clone()),
        },
    )?;
    transaction = data_store.update_nota_runtime_transaction(
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
    receipts.push(
        data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: transaction.id,
            receipt_kind: "CADENCE_CHECKPOINT_WRITTEN",
            payload_json: &serde_json::to_string(&json!({
                "checkpoint_id": checkpoint_report.checkpoint.cadence_object.id,
                "selected_trunk": checkpoint_report.checkpoint.payload.selected_trunk,
            }))
            .context("failed to serialize checkpoint receipt payload")?,
            status: "recorded",
        })?,
    );

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
    materialize_terminal_allocation_outcomes(data_store)?;
    let allocations = data_store.list_nota_runtime_allocations()?;
    Ok(NotaRuntimeAllocationsReport {
        allocation_count: allocations.len(),
        allocations,
    })
}

fn materialize_terminal_allocation_outcomes(data_store: &DataStore) -> Result<()> {
    let allocations = data_store.list_nota_runtime_allocations()?;
    for allocation in allocations {
        materialize_terminal_allocation_outcome(data_store, &allocation)?;
    }

    Ok(())
}

fn materialize_terminal_allocation_outcome(
    data_store: &DataStore,
    allocation: &StoredNotaRuntimeAllocation,
) -> Result<()> {
    if allocation.allocation_kind != "forge_agent_dispatch"
        || allocation.child_execution_kind != "forge_task"
    {
        return Ok(());
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
        return Ok(());
    };

    let Some((status, outcome)) = build_terminal_allocation_outcome(allocation, &task) else {
        return Ok(());
    };
    let receipt_payload =
        build_allocation_terminal_outcome_receipt_payload(allocation, status, &outcome);
    let receipt_recorded = has_allocation_terminal_outcome_receipt(
        data_store,
        allocation.source_transaction_id,
        &receipt_payload,
    )?;

    let mut payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
        .with_context(|| {
            format!(
                "failed to parse nota allocation payload for allocation {}",
                allocation.id
            )
        })?;
    if allocation.status != status || payload.terminal_outcome.as_ref() != Some(&outcome) {
        payload.terminal_outcome = Some(outcome.clone());
        let payload_json = serde_json::to_string(&payload).with_context(|| {
            format!(
                "failed to serialize allocation {} terminal outcome",
                allocation.id
            )
        })?;
        data_store.update_nota_runtime_allocation(
            allocation.id,
            NotaRuntimeAllocationUpdate {
                status,
                payload_json: Some(&payload_json),
            },
        )?;
    }

    if !receipt_recorded {
        let receipt_payload_json = serde_json::to_string(&receipt_payload).with_context(|| {
            format!(
                "failed to serialize allocation {} terminal outcome receipt",
                allocation.id
            )
        })?;
        data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: allocation.source_transaction_id,
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

fn build_terminal_allocation_outcome<'a>(
    allocation: &'a StoredNotaRuntimeAllocation,
    task: &'a StoredForgeTask,
) -> Option<(&'static str, NotaDoAllocationTerminalOutcome)> {
    match task.status.as_str() {
        "Done" => Some((
            "return_ready",
            NotaDoAllocationTerminalOutcome {
                boundary_kind: "return".to_string(),
                child_execution_status: task.status.clone(),
                child_execution_status_message: task.status_message.clone(),
                target_kind: allocation.return_target_kind.clone(),
                target_ref: allocation.return_target_ref.clone(),
            },
        )),
        "Blocked" => Some((
            "escalated_blocked",
            NotaDoAllocationTerminalOutcome {
                boundary_kind: "escalation".to_string(),
                child_execution_status: task.status.clone(),
                child_execution_status_message: task.status_message.clone(),
                target_kind: allocation.escalation_target_kind.clone(),
                target_ref: allocation.escalation_target_ref.clone(),
            },
        )),
        "Failed" => Some((
            "escalated_failed",
            NotaDoAllocationTerminalOutcome {
                boundary_kind: "escalation".to_string(),
                child_execution_status: task.status.clone(),
                child_execution_status_message: task.status_message.clone(),
                target_kind: allocation.escalation_target_kind.clone(),
                target_ref: allocation.escalation_target_ref.clone(),
            },
        )),
        "Cancelled" => Some((
            "escalated_cancelled",
            NotaDoAllocationTerminalOutcome {
                boundary_kind: "escalation".to_string(),
                child_execution_status: task.status.clone(),
                child_execution_status_message: task.status_message.clone(),
                target_kind: allocation.escalation_target_kind.clone(),
                target_ref: allocation.escalation_target_ref.clone(),
            },
        )),
        _ => None,
    }
}

pub fn admission_policy_for_kind(cadence_kind: &str) -> &'static str {
    match cadence_kind {
        CADENCE_CHECKPOINT_KIND
        | CADENCE_HANDOUT_KIND
        | CADENCE_WAKE_REQUEST_KIND
        | CADENCE_POLICY_NOTE_KIND => "AP_STORAGE_AND_COLD_ALWAYS",
        _ => "AP_STORAGE_ALWAYS",
    }
}

pub fn projection_policy_for_kind(cadence_kind: &str) -> &'static str {
    match cadence_kind {
        CADENCE_CHECKPOINT_KIND | CADENCE_HANDOUT_KIND => "PP_HOT_ACTIVE_ONLY",
        CADENCE_WAKE_REQUEST_KIND => "PP_HOT_ON_ATTENTION_OR_REJECT",
        CADENCE_POLICY_NOTE_KIND => "PP_HOT_NEVER",
        _ => "PP_HOT_ACTIVE_ONLY",
    }
}

fn build_do_checkpoint_landed_items(
    transaction_id: i64,
    allocation: &StoredNotaRuntimeAllocation,
    task_id: i64,
    dispatch: &PreparedAgentDispatch,
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

fn build_checkpoint_summary(stable_level: &str, landed: &[String]) -> String {
    match landed.first() {
        Some(first_landed) => format!("{stable_level}. Landed: {first_landed}"),
        None => stable_level.to_string(),
    }
}

fn build_do_allocation_lineage_ref(transaction_id: i64, task_id: i64) -> String {
    format!("nota/do/transaction/{transaction_id}/forge-task/{task_id}")
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn capture_repo_context(project_dir: &str) -> Result<RepoContext> {
    let project_path = Path::new(project_dir);
    if !project_path.exists() {
        return Err(anyhow!(
            "nota checkpoint project directory `{}` does not exist",
            project_path.display()
        ));
    }

    Ok(RepoContext {
        project_dir: project_path.to_string_lossy().replace('\\', "/"),
        git_branch: run_git_command(project_path, &["rev-parse", "--abbrev-ref", "HEAD"]).ok(),
        git_head: run_git_command(project_path, &["rev-parse", "HEAD"]).ok(),
    })
}

fn run_git_command(project_path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .with_context(|| {
            format!(
                "failed to run git {} in {}",
                args.join(" "),
                project_path.display()
            )
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed in {}: {}",
            args.join(" "),
            project_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("git {} output was not valid UTF-8", args.join(" ")))?;
    Ok(value.trim().to_string())
}

fn actor_role_slug(role: crate::core::action::ActorRole) -> &'static str {
    match role {
        crate::core::action::ActorRole::Nota => "nota",
        crate::core::action::ActorRole::Arch => "arch",
        crate::core::action::ActorRole::Dev => "dev",
        crate::core::action::ActorRole::Agent => "agent",
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::core::data_store::{
        DataStore, MigrationPlan, NewNotaRuntimeAllocation, NewNotaRuntimeReceipt,
        NewNotaRuntimeTransaction,
    };

    use super::{
        list_nota_runtime_allocations, list_runtime_checkpoints, write_runtime_checkpoint,
        AllocationTerminalOutcomeReceiptPayload, NotaCheckpointRequest, NotaDoAllocationPayload,
        NotaDoAllocationTerminalOutcome, ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
    };

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
    fn allocation_terminal_outcome_receipt_backfills_existing_terminal_state() -> Result<()> {
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
        let first_receipts = store.list_nota_runtime_receipts(Some(transaction.id))?;
        assert_eq!(first_receipts.len(), 1);
        assert_eq!(
            first_receipts[0].receipt_kind,
            ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND
        );
        assert!(!first_receipts[0].created_at.is_empty());

        let receipt_payload: AllocationTerminalOutcomeReceiptPayload =
            serde_json::from_str(&first_receipts[0].payload_json)?;
        assert_eq!(
            receipt_payload,
            AllocationTerminalOutcomeReceiptPayload {
                allocation_id: report.allocations[0].id,
                lineage_ref: report.allocations[0].lineage_ref.clone(),
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Blocked".to_string(),
                child_execution_status_message: Some("add openai to Vault first".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
                allocation_status: "escalated_blocked".to_string(),
            }
        );

        let second_report = list_nota_runtime_allocations(&store)?;
        assert_eq!(second_report.allocations[0].status, "escalated_blocked");
        assert_eq!(
            store
                .list_nota_runtime_receipts(Some(transaction.id))?
                .len(),
            1
        );

        Ok(())
    }
}
