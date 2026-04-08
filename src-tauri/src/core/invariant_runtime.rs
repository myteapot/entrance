use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use crate::core::{
    anti_zeno_runtime::{build_anti_zeno_budget_report, AntiZenoBudgetReport},
    data_store::{
        DataStore, StoredRepairLaneItem, StoredRuntimeInvariant, UpsertRepairLaneItem,
        UpsertRuntimeInvariant,
    },
    environment_runtime::{
        current_runtime_host, list_owned_worktrees, OwnedWorktreeRegistryReport,
    },
    nota_runtime::{
        active_checkpoint_scope_ids, derive_current_runtime_acceptance_bundle,
        derive_current_runtime_handout, derive_current_runtime_human_round,
        derive_current_runtime_wake_request, derive_nota_runtime_next_step,
        derive_runtime_round_state_projection, list_nota_runtime_allocations,
        list_nota_runtime_receipts, list_nota_runtime_transactions, list_runtime_checkpoints,
        NotaAcceptanceBundleRecord, NotaCheckpointRecord, NotaHandoutRecord, NotaHumanRoundRecord,
        NotaRoundStateProjection, NotaWakeRequestRecord,
    },
    projection_runtime::{
        build_projection_status_report, ProjectionStatusReport, ProjectionTruthRevision,
    },
};

const INVARIANT_PASSED: &str = "passed";
const INVARIANT_FAILED_REPAIRABLE: &str = "failed_repairable";
const INVARIANT_FAILED_BLOCKED: &str = "failed_blocked";
const INVARIANT_NOT_APPLICABLE: &str = "not_applicable";

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInvariantReport {
    pub invariant_count: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub repairable_count: usize,
    pub blocked_count: usize,
    pub not_applicable_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_checkpoint_id: Option<i64>,
    pub invariants: Vec<StoredRuntimeInvariant>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairLaneReport {
    pub item_count: usize,
    pub open_count: usize,
    pub blocked_count: usize,
    pub repairable_count: usize,
    pub resolved_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_checkpoint_id: Option<i64>,
    pub items: Vec<StoredRepairLaneItem>,
}

#[derive(Debug, Clone)]
struct InvariantEvaluation {
    invariant_key: &'static str,
    title: &'static str,
    status: &'static str,
    severity: &'static str,
    checkpoint_id: Option<i64>,
    acceptance_bundle_id: Option<i64>,
    human_round_id: Option<i64>,
    summary: String,
    repair_action: String,
    evidence_json: String,
}

#[derive(Debug, Clone)]
struct RepairLaneEvaluation {
    repair_key: String,
    source_invariant_key: &'static str,
    checkpoint_id: Option<i64>,
    acceptance_bundle_id: Option<i64>,
    urgency: &'static str,
    summary: String,
    repair_action: String,
    evidence_json: String,
}

pub fn refresh_runtime_invariants(
    data_store: &DataStore,
) -> Result<(RuntimeInvariantReport, RepairLaneReport)> {
    let (invariants, repair_items) = evaluate_runtime_invariant_state(data_store)?;

    for invariant in &invariants {
        data_store.upsert_runtime_invariant(UpsertRuntimeInvariant {
            invariant_key: invariant.invariant_key,
            title: invariant.title,
            status: invariant.status,
            severity: invariant.severity,
            checkpoint_id: invariant.checkpoint_id,
            acceptance_bundle_id: invariant.acceptance_bundle_id,
            human_round_id: invariant.human_round_id,
            summary: &invariant.summary,
            evidence_json: &invariant.evidence_json,
            repair_action: &invariant.repair_action,
        })?;
    }

    let active_repair_keys = repair_items
        .iter()
        .map(|item| item.repair_key.clone())
        .collect::<Vec<_>>();
    for repair_item in &repair_items {
        data_store.upsert_repair_lane_item(UpsertRepairLaneItem {
            repair_key: &repair_item.repair_key,
            source_invariant_key: Some(repair_item.source_invariant_key),
            checkpoint_id: repair_item.checkpoint_id,
            acceptance_bundle_id: repair_item.acceptance_bundle_id,
            item_kind: "runtime_invariant",
            urgency: repair_item.urgency,
            status: "open",
            summary: &repair_item.summary,
            repair_action: &repair_item.repair_action,
            evidence_json: &repair_item.evidence_json,
        })?;
    }
    data_store.mark_repair_lane_items_resolved(&active_repair_keys)?;

    let invariant_report = list_runtime_invariant_report(data_store)?;
    let repair_lane_report = list_repair_lane_report(data_store)?;
    Ok((invariant_report, repair_lane_report))
}

pub fn project_runtime_invariants(
    data_store: &DataStore,
) -> Result<(RuntimeInvariantReport, RepairLaneReport)> {
    let (invariants, repair_items) = evaluate_runtime_invariant_state(data_store)?;
    let stored_invariants = data_store
        .list_runtime_invariants()?
        .into_iter()
        .map(|invariant| (invariant.invariant_key.clone(), invariant))
        .collect::<HashMap<_, _>>();
    let mut projected_invariants = invariants
        .into_iter()
        .map(|invariant| {
            let invariant_key = invariant.invariant_key;
            project_runtime_invariant(invariant, stored_invariants.get(invariant_key))
        })
        .collect::<Vec<_>>();
    sort_runtime_invariants(&mut projected_invariants);

    let stored_repair_items = data_store
        .list_repair_lane_items()?
        .into_iter()
        .map(|item| (item.repair_key.clone(), item))
        .collect::<HashMap<_, _>>();
    let active_repair_keys = repair_items
        .iter()
        .map(|item| item.repair_key.clone())
        .collect::<HashSet<_>>();
    let mut projected_repair_items = repair_items
        .into_iter()
        .map(|repair_item| {
            let repair_key = repair_item.repair_key.clone();
            project_open_repair_lane_item(repair_item, stored_repair_items.get(&repair_key))
        })
        .collect::<Vec<_>>();
    projected_repair_items.extend(
        stored_repair_items
            .into_values()
            .filter(|item| !active_repair_keys.contains(&item.repair_key))
            .map(project_resolved_repair_lane_item),
    );
    sort_repair_lane_items(&mut projected_repair_items);

    Ok((
        build_runtime_invariant_report(projected_invariants),
        build_repair_lane_report(projected_repair_items),
    ))
}

pub fn list_runtime_invariant_report(data_store: &DataStore) -> Result<RuntimeInvariantReport> {
    Ok(build_runtime_invariant_report(
        data_store.list_runtime_invariants()?,
    ))
}

pub fn list_repair_lane_report(data_store: &DataStore) -> Result<RepairLaneReport> {
    Ok(build_repair_lane_report(
        data_store.list_repair_lane_items()?,
    ))
}

struct InvariantContext {
    current_checkpoint: Option<NotaCheckpointRecord>,
    current_human_round: Option<NotaHumanRoundRecord>,
    current_acceptance_bundle: Option<NotaAcceptanceBundleRecord>,
    current_handout: Option<NotaHandoutRecord>,
    current_wake_request: Option<NotaWakeRequestRecord>,
    round_state: NotaRoundStateProjection,
    projections: ProjectionStatusReport,
    anti_zeno_budget: AntiZenoBudgetReport,
    host: Option<crate::core::data_store::StoredRuntimeHost>,
    worktrees: OwnedWorktreeRegistryReport,
    next_step: Option<crate::core::nota_runtime::NotaRuntimeNextStep>,
}

fn gather_invariant_context(data_store: &DataStore) -> Result<InvariantContext> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let current_checkpoint = checkpoints
        .checkpoints
        .into_iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current);
    let checkpoint_scope_ids =
        active_checkpoint_scope_ids(data_store, current_checkpoint.as_ref())?;
    let allocations = list_nota_runtime_allocations(data_store)?;
    let transactions = list_nota_runtime_transactions(data_store)?;
    let receipts = list_nota_runtime_receipts(data_store, None)?;
    let next_step = derive_nota_runtime_next_step(
        &checkpoint_scope_ids,
        &transactions.transactions,
        allocations.stored_allocations(),
        &receipts.receipts,
    )?;
    let current_human_round = derive_current_runtime_human_round(data_store)?;
    let current_acceptance_bundle =
        derive_current_runtime_acceptance_bundle(data_store, &checkpoint_scope_ids)?;
    let current_handout = derive_current_runtime_handout(data_store)?;
    let current_wake_request = derive_current_runtime_wake_request(data_store)?;
    let round_state = derive_runtime_round_state_projection(
        current_checkpoint.as_ref(),
        current_acceptance_bundle.as_ref(),
        next_step.as_ref(),
    );
    let projections = build_projection_status_report(
        data_store,
        ProjectionTruthRevision {
            checkpoint_id: round_state.checkpoint_id,
            human_round_id: current_human_round
                .as_ref()
                .map(|round| round.cadence_object.id),
            acceptance_bundle_id: round_state.acceptance_bundle_id,
        },
    )?;
    let anti_zeno_budget = build_anti_zeno_budget_report(
        data_store,
        round_state.checkpoint_id,
        round_state.acceptance_bundle_id,
        round_state.acceptance_present,
        round_state.fully_settled,
        round_state.next_step_open,
        projections.dirty_required_target_count,
    )?;
    let host = current_runtime_host(data_store)?;
    let worktrees = list_owned_worktrees(
        data_store,
        host.as_ref().map(|value| value.host_key.as_str()),
    )?;

    Ok(InvariantContext {
        current_checkpoint,
        current_human_round,
        current_acceptance_bundle,
        current_handout,
        current_wake_request,
        round_state,
        projections,
        anti_zeno_budget,
        host,
        worktrees,
        next_step,
    })
}

fn evaluate_runtime_invariant_state(
    data_store: &DataStore,
) -> Result<(Vec<InvariantEvaluation>, Vec<RepairLaneEvaluation>)> {
    let context = gather_invariant_context(data_store)?;
    let invariants = evaluate_runtime_invariants(&context)?;
    let repair_items = invariants
        .iter()
        .filter_map(repair_lane_item_from_invariant)
        .collect::<Vec<_>>();
    Ok((invariants, repair_items))
}

fn evaluate_runtime_invariants(context: &InvariantContext) -> Result<Vec<InvariantEvaluation>> {
    let checkpoint_id = context
        .current_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.cadence_object.id);
    let human_round_id = context
        .current_human_round
        .as_ref()
        .map(|round| round.cadence_object.id);
    let acceptance_bundle_id = context
        .current_acceptance_bundle
        .as_ref()
        .map(|bundle| bundle.cadence_object.id);
    let dirty_required_targets = context
        .projections
        .targets
        .iter()
        .filter(|target| target.required && !target.fresh)
        .map(|target| target.target.target_key.clone())
        .collect::<Vec<_>>();
    let missing_worktrees = context
        .worktrees
        .worktrees
        .iter()
        .filter(|worktree| worktree.status == "missing")
        .map(|worktree| worktree.worktree_path.clone())
        .collect::<Vec<_>>();

    let mut invariants = Vec::new();
    invariants.push(InvariantEvaluation {
        invariant_key: "round_state_detail_alignment",
        title: "Round state detail alignment",
        status: if round_state_detail_matches(context.round_state.state.as_str(), context.round_state.detail_state.as_str()) {
            INVARIANT_PASSED
        } else {
            INVARIANT_FAILED_BLOCKED
        },
        severity: if round_state_detail_matches(context.round_state.state.as_str(), context.round_state.detail_state.as_str()) {
            "info"
        } else {
            "critical"
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: if round_state_detail_matches(context.round_state.state.as_str(), context.round_state.detail_state.as_str()) {
            format!(
                "Canonical state `{}` and detail state `{}` are aligned.",
                context.round_state.state, context.round_state.detail_state
            )
        } else {
            format!(
                "Canonical state `{}` does not accept detail state `{}`.",
                context.round_state.state, context.round_state.detail_state
            )
        },
        repair_action:
            "Recompute the round-state projection before trusting handout, wake, or settlement surfaces."
                .to_string(),
        evidence_json: serde_json::to_string(&json!({
            "round_state": context.round_state.state,
            "detail_state": context.round_state.detail_state,
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "checkpoint_presence_for_active_boundary",
        title: "Checkpoint presence for active boundary",
        status: match (
            context.current_checkpoint.is_some(),
            context.current_human_round.is_some()
                || context.current_acceptance_bundle.is_some()
                || context.next_step.is_some(),
        ) {
            (true, _) => INVARIANT_PASSED,
            (false, true) => INVARIANT_FAILED_BLOCKED,
            (false, false) => INVARIANT_NOT_APPLICABLE,
        },
        severity: match (
            context.current_checkpoint.is_some(),
            context.current_human_round.is_some()
                || context.current_acceptance_bundle.is_some()
                || context.next_step.is_some(),
        ) {
            (true, _) | (false, false) => "info",
            (false, true) => "critical",
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: if let Some(checkpoint) = context.current_checkpoint.as_ref() {
            format!(
                "Current runtime boundary is anchored by checkpoint {}.",
                checkpoint.cadence_object.id
            )
        } else if context.current_human_round.is_some()
            || context.current_acceptance_bundle.is_some()
            || context.next_step.is_some()
        {
            "A live runtime boundary exists without a current checkpoint, so settlement cannot be trusted."
                .to_string()
        } else {
            "No current checkpoint is recorded yet, but no active boundary truth depends on one."
                .to_string()
        },
        repair_action:
            "Write a runtime checkpoint before reopening semantic continuation.".to_string(),
        evidence_json: serde_json::to_string(&json!({
            "checkpoint_id": checkpoint_id,
            "human_round_id": human_round_id,
            "acceptance_bundle_id": acceptance_bundle_id,
            "next_step": context.next_step.as_ref().map(|step| step.step.as_str()),
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "runtime_host_snapshot",
        title: "Runtime host snapshot",
        status: if context.host.is_some() {
            INVARIANT_PASSED
        } else {
            INVARIANT_FAILED_REPAIRABLE
        },
        severity: if context.host.is_some() {
            "info"
        } else {
            "error"
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: context
            .host
            .as_ref()
            .map(|host| {
                format!(
                    "Runtime host snapshot is recorded for {} under {}.",
                    host.host_label, host.owner_root
                )
            })
            .unwrap_or_else(|| {
                "Runtime host snapshot is missing, so owner-root path truth is not explicit yet."
                    .to_string()
            }),
        repair_action:
            "Restart Entrance or refresh a NOTA surface to bootstrap host ownership truth."
                .to_string(),
        evidence_json: serde_json::to_string(&json!({
            "host_present": context.host.is_some(),
            "owner_root": context.host.as_ref().map(|host| host.owner_root.as_str()),
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "human_round_checkpoint_alignment",
        title: "Human round to checkpoint alignment",
        status: match (
            context.current_human_round.as_ref(),
            context.current_checkpoint.as_ref(),
        ) {
            (None, _) => INVARIANT_NOT_APPLICABLE,
            (Some(_), None) => INVARIANT_FAILED_BLOCKED,
            (Some(round), Some(checkpoint))
                if round.payload.checkpoint_id == checkpoint.cadence_object.id =>
            {
                INVARIANT_PASSED
            }
            (Some(_), Some(_)) => INVARIANT_FAILED_BLOCKED,
        },
        severity: match (
            context.current_human_round.as_ref(),
            context.current_checkpoint.as_ref(),
        ) {
            (None, _) => "info",
            (Some(round), Some(checkpoint))
                if round.payload.checkpoint_id == checkpoint.cadence_object.id =>
            {
                "info"
            }
            _ => "critical",
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: match (
            context.current_human_round.as_ref(),
            context.current_checkpoint.as_ref(),
        ) {
            (None, _) => "No current human round is materialized yet.".to_string(),
            (Some(round), Some(checkpoint))
                if round.payload.checkpoint_id == checkpoint.cadence_object.id =>
            {
                format!(
                    "Current human round {} is anchored to checkpoint {}.",
                    round.cadence_object.id, checkpoint.cadence_object.id
                )
            }
            (Some(round), Some(checkpoint)) => format!(
                "Current human round {} points to checkpoint {}, but the active checkpoint is {}.",
                round.cadence_object.id,
                round.payload.checkpoint_id,
                checkpoint.cadence_object.id
            ),
            (Some(round), None) => format!(
                "Current human round {} exists without a current checkpoint anchor.",
                round.cadence_object.id
            ),
        },
        repair_action:
            "Re-materialize the current human round from the active checkpoint before continuing."
                .to_string(),
        evidence_json: serde_json::to_string(&json!({
            "human_round_id": human_round_id,
            "human_round_checkpoint_id": context.current_human_round.as_ref().map(|round| round.payload.checkpoint_id),
            "current_checkpoint_id": checkpoint_id,
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "acceptance_checkpoint_alignment",
        title: "Acceptance bundle to checkpoint alignment",
        status: match (
            context.current_acceptance_bundle.as_ref(),
            context.current_checkpoint.as_ref(),
        ) {
            (None, _) => INVARIANT_NOT_APPLICABLE,
            (Some(_), None) => INVARIANT_FAILED_BLOCKED,
            (Some(bundle), Some(checkpoint))
                if bundle.payload.checkpoint_id == checkpoint.cadence_object.id =>
            {
                INVARIANT_PASSED
            }
            (Some(_), Some(_)) => INVARIANT_FAILED_BLOCKED,
        },
        severity: match (
            context.current_acceptance_bundle.as_ref(),
            context.current_checkpoint.as_ref(),
        ) {
            (None, _) => "info",
            (Some(bundle), Some(checkpoint))
                if bundle.payload.checkpoint_id == checkpoint.cadence_object.id =>
            {
                "info"
            }
            _ => "critical",
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: match (
            context.current_acceptance_bundle.as_ref(),
            context.current_checkpoint.as_ref(),
        ) {
            (None, _) => "No current acceptance bundle is materialized yet.".to_string(),
            (Some(bundle), Some(checkpoint))
                if bundle.payload.checkpoint_id == checkpoint.cadence_object.id =>
            {
                format!(
                    "Current acceptance bundle {} is anchored to checkpoint {}.",
                    bundle.cadence_object.id, checkpoint.cadence_object.id
                )
            }
            (Some(bundle), Some(checkpoint)) => format!(
                "Current acceptance bundle {} points to checkpoint {}, but the active checkpoint is {}.",
                bundle.cadence_object.id,
                bundle.payload.checkpoint_id,
                checkpoint.cadence_object.id
            ),
            (Some(bundle), None) => format!(
                "Current acceptance bundle {} exists without a current checkpoint anchor.",
                bundle.cadence_object.id
            ),
        },
        repair_action:
            "Rebuild acceptance materialization from the current checkpoint scope before settling the round."
                .to_string(),
        evidence_json: serde_json::to_string(&json!({
            "acceptance_bundle_id": acceptance_bundle_id,
            "acceptance_checkpoint_id": context.current_acceptance_bundle.as_ref().map(|bundle| bundle.payload.checkpoint_id),
            "current_checkpoint_id": checkpoint_id,
        }))?,
    });

    let required_projection_repair_action = context
        .projections
        .targets
        .iter()
        .find(|target| target.required && !target.repair_action.trim().is_empty())
        .map(|target| target.repair_action.clone())
        .unwrap_or_else(|| "entrance nota export-hot-root".to_string());
    invariants.push(InvariantEvaluation {
        invariant_key: "required_projection_freshness",
        title: "Required projection freshness",
        status: if context.projections.required_target_count == 0 {
            INVARIANT_NOT_APPLICABLE
        } else if context.projections.required_targets_fresh {
            INVARIANT_PASSED
        } else {
            INVARIANT_FAILED_REPAIRABLE
        },
        severity: if context.projections.required_target_count == 0 {
            "info"
        } else if context.projections.required_targets_fresh {
            "info"
        } else {
            "error"
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: if context.projections.required_target_count == 0 {
            "No required projection targets are registered yet for the current truth revision."
                .to_string()
        } else if context.projections.required_targets_fresh {
            format!(
                "All {} required projection targets are fresh for the current truth revision.",
                context.projections.required_target_count
            )
        } else {
            format!(
                "{} required projection targets are dirty or failed for the current truth revision.",
                context.projections.dirty_required_target_count
            )
        },
        repair_action: required_projection_repair_action.clone(),
        evidence_json: serde_json::to_string(&json!({
            "required_target_count": context.projections.required_target_count,
            "fresh_required_target_count": context.projections.fresh_required_target_count,
            "dirty_required_target_count": context.projections.dirty_required_target_count,
            "failed_required_target_count": context.projections.failed_required_target_count,
            "dirty_required_targets": dirty_required_targets,
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "anti_zeno_budget_headroom",
        title: "Anti-Zeno budget headroom",
        status: if checkpoint_id.is_none() && context.anti_zeno_budget.semantic_event_count == 0 {
            INVARIANT_NOT_APPLICABLE
        } else if context.anti_zeno_budget.budget_exhausted {
            INVARIANT_FAILED_BLOCKED
        } else {
            INVARIANT_PASSED
        },
        severity: if checkpoint_id.is_none() && context.anti_zeno_budget.semantic_event_count == 0 {
            "info"
        } else if context.anti_zeno_budget.budget_exhausted {
            "critical"
        } else {
            "info"
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: if checkpoint_id.is_none() && context.anti_zeno_budget.semantic_event_count == 0 {
            "Anti-Zeno budget is not active yet because no checkpoint anchors the current round."
                .to_string()
        } else {
            context.anti_zeno_budget.summary.clone()
        },
        repair_action: context
            .anti_zeno_budget
            .forced_action
            .clone()
            .unwrap_or_else(|| {
                "Force bounded closure, repair, or explicit human decision before opening another recursive cut."
                    .to_string()
            }),
        evidence_json: serde_json::to_string(&json!({
            "state": context.anti_zeno_budget.state,
            "semantic_event_count": context.anti_zeno_budget.semantic_event_count,
            "repair_event_count": context.anti_zeno_budget.repair_event_count,
            "projection_debt_count": context.anti_zeno_budget.projection_debt_count,
            "budget_exhausted": context.anti_zeno_budget.budget_exhausted,
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "owned_worktree_registry_consistency",
        title: "Owned worktree registry consistency",
        status: if context.worktrees.missing_count > 0 {
            INVARIANT_FAILED_REPAIRABLE
        } else {
            INVARIANT_PASSED
        },
        severity: if context.worktrees.missing_count > 0 {
            "warning"
        } else {
            "info"
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: if context.worktrees.missing_count > 0 {
            format!(
                "{} owned worktrees are recorded as missing and need cleanup or reattachment.",
                context.worktrees.missing_count
            )
        } else {
            format!(
                "Owned worktree registry is consistent with {} observed worktrees.",
                context.worktrees.observed_count
            )
        },
        repair_action: "Inspect or remove missing worktrees under ~/.entrance/worktrees before reopening that lane."
            .to_string(),
        evidence_json: serde_json::to_string(&json!({
            "worktree_count": context.worktrees.worktree_count,
            "observed_count": context.worktrees.observed_count,
            "missing_count": context.worktrees.missing_count,
            "missing_worktrees": missing_worktrees,
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "bridge_projection_alignment",
        title: "Bridge projection alignment",
        status: match (
            checkpoint_id,
            context.current_handout.as_ref(),
            context.current_wake_request.as_ref(),
            context.round_state.fully_settled,
        ) {
            (None, _, _, _) => INVARIANT_NOT_APPLICABLE,
            (Some(_), Some(handout), None, true)
                if bridge_payload_matches_round_state(
                    &context.round_state,
                    handout.payload.round_state.as_str(),
                    handout.payload.detail_round_state.as_deref(),
                ) =>
            {
                INVARIANT_PASSED
            }
            (Some(_), Some(handout), Some(wake_request), false)
                if bridge_payload_matches_round_state(
                    &context.round_state,
                    handout.payload.round_state.as_str(),
                    handout.payload.detail_round_state.as_deref(),
                ) && bridge_payload_matches_round_state(
                    &context.round_state,
                    wake_request.payload.round_state.as_str(),
                    wake_request.payload.detail_round_state.as_deref(),
                ) =>
            {
                INVARIANT_PASSED
            }
            _ => INVARIANT_FAILED_REPAIRABLE,
        },
        severity: match (
            checkpoint_id,
            context.current_handout.as_ref(),
            context.current_wake_request.as_ref(),
            context.round_state.fully_settled,
        ) {
            (None, _, _, _) => "info",
            (Some(_), Some(handout), None, true)
                if bridge_payload_matches_round_state(
                    &context.round_state,
                    handout.payload.round_state.as_str(),
                    handout.payload.detail_round_state.as_deref(),
                ) =>
            {
                "info"
            }
            (Some(_), Some(handout), Some(wake_request), false)
                if bridge_payload_matches_round_state(
                    &context.round_state,
                    handout.payload.round_state.as_str(),
                    handout.payload.detail_round_state.as_deref(),
                ) && bridge_payload_matches_round_state(
                    &context.round_state,
                    wake_request.payload.round_state.as_str(),
                    wake_request.payload.detail_round_state.as_deref(),
                ) =>
            {
                "info"
            }
            _ => "warning",
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: match (
            checkpoint_id,
            context.current_handout.as_ref(),
            context.current_wake_request.as_ref(),
            context.round_state.fully_settled,
        ) {
            (None, _, _, _) => {
                "No current checkpoint exists, so bridge alignment is not applicable.".to_string()
            }
            (Some(_), Some(_), None, true) => format!(
                "Current handout mirrors fully settled round state `{}` / `{}` and wake-request bridge is intentionally absent.",
                context.round_state.state, context.round_state.detail_state
            ),
            (Some(_), Some(_), Some(_), false) => format!(
                "Current handout and wake request mirror round state `{}` / `{}`.",
                context.round_state.state, context.round_state.detail_state
            ),
            _ => "Handout or wake-request bridge objects are missing or stale for the current round state."
                .to_string(),
        },
        repair_action:
            "Re-materialize runtime bridge objects before relying on handout or wake continuity."
                .to_string(),
        evidence_json: serde_json::to_string(&json!({
            "round_state": context.round_state.state,
            "detail_state": context.round_state.detail_state,
            "fully_settled": context.round_state.fully_settled,
            "handout_present": context.current_handout.is_some(),
            "handout_round_state": context.current_handout.as_ref().map(|handout| handout.payload.round_state.as_str()),
            "handout_detail_state": context.current_handout.as_ref().and_then(|handout| handout.payload.detail_round_state.as_deref()),
            "wake_request_present": context.current_wake_request.is_some(),
            "wake_request_round_state": context.current_wake_request.as_ref().map(|wake_request| wake_request.payload.round_state.as_str()),
            "wake_request_detail_state": context.current_wake_request.as_ref().and_then(|wake_request| wake_request.payload.detail_round_state.as_deref()),
        }))?,
    });

    invariants.push(InvariantEvaluation {
        invariant_key: "fully_settled_projection_boundary",
        title: "Fully settled projection boundary",
        status: if !context.round_state.fully_settled {
            INVARIANT_NOT_APPLICABLE
        } else if context.projections.required_targets_fresh {
            INVARIANT_PASSED
        } else {
            INVARIANT_FAILED_BLOCKED
        },
        severity: if !context.round_state.fully_settled {
            "info"
        } else if context.projections.required_targets_fresh {
            "info"
        } else {
            "critical"
        },
        checkpoint_id,
        acceptance_bundle_id,
        human_round_id,
        summary: if !context.round_state.fully_settled {
            "The current round is not yet fully settled, so projection-boundary settlement is not applicable."
                .to_string()
        } else if context.projections.required_targets_fresh {
            "Fully settled round also satisfies required projection freshness.".to_string()
        } else {
            "Round claims full settlement while required projections remain dirty, so settlement is blocked."
                .to_string()
        },
        repair_action: required_projection_repair_action,
        evidence_json: serde_json::to_string(&json!({
            "round_state": context.round_state.state,
            "fully_settled": context.round_state.fully_settled,
            "required_targets_fresh": context.projections.required_targets_fresh,
            "dirty_required_target_count": context.projections.dirty_required_target_count,
        }))?,
    });

    Ok(invariants)
}

fn repair_lane_item_from_invariant(
    invariant: &InvariantEvaluation,
) -> Option<RepairLaneEvaluation> {
    let urgency = match invariant.status {
        INVARIANT_FAILED_REPAIRABLE => "repairable",
        INVARIANT_FAILED_BLOCKED => "blocked",
        _ => return None,
    };

    Some(RepairLaneEvaluation {
        repair_key: format!("runtime_invariant:{}", invariant.invariant_key),
        source_invariant_key: invariant.invariant_key,
        checkpoint_id: invariant.checkpoint_id,
        acceptance_bundle_id: invariant.acceptance_bundle_id,
        urgency,
        summary: invariant.summary.clone(),
        repair_action: invariant.repair_action.clone(),
        evidence_json: invariant.evidence_json.clone(),
    })
}

fn round_state_detail_matches(state: &str, detail_state: &str) -> bool {
    matches!(
        (state, detail_state),
        ("opened", "uncheckpointed")
            | ("checkpointed", "checkpointed_pending_acceptance")
            | ("accepted", "accepted_waiting_carry_forward")
            | ("settling", "accepted_followup_open")
            | ("fully_settled", "fully_settled")
    )
}

fn bridge_payload_matches_round_state(
    round_state: &NotaRoundStateProjection,
    bridge_state: &str,
    bridge_detail_state: Option<&str>,
) -> bool {
    bridge_state == round_state.state
        && bridge_detail_state
            .map(|detail_state| detail_state == round_state.detail_state)
            .unwrap_or(false)
}

fn build_runtime_invariant_report(
    invariants: Vec<StoredRuntimeInvariant>,
) -> RuntimeInvariantReport {
    let passed_count = invariants
        .iter()
        .filter(|invariant| invariant.status == INVARIANT_PASSED)
        .count();
    let repairable_count = invariants
        .iter()
        .filter(|invariant| invariant.status == INVARIANT_FAILED_REPAIRABLE)
        .count();
    let blocked_count = invariants
        .iter()
        .filter(|invariant| invariant.status == INVARIANT_FAILED_BLOCKED)
        .count();
    let not_applicable_count = invariants
        .iter()
        .filter(|invariant| invariant.status == INVARIANT_NOT_APPLICABLE)
        .count();

    RuntimeInvariantReport {
        invariant_count: invariants.len(),
        passed_count,
        failed_count: repairable_count + blocked_count,
        repairable_count,
        blocked_count,
        not_applicable_count,
        current_checkpoint_id: invariants
            .iter()
            .find_map(|invariant| invariant.checkpoint_id),
        invariants,
    }
}

fn build_repair_lane_report(items: Vec<StoredRepairLaneItem>) -> RepairLaneReport {
    let open_count = items.iter().filter(|item| item.status == "open").count();
    let blocked_count = items
        .iter()
        .filter(|item| item.status == "open" && item.urgency == "blocked")
        .count();
    let repairable_count = items
        .iter()
        .filter(|item| item.status == "open" && item.urgency == "repairable")
        .count();
    let resolved_count = items
        .iter()
        .filter(|item| item.status == "resolved")
        .count();

    RepairLaneReport {
        item_count: items.len(),
        open_count,
        blocked_count,
        repairable_count,
        resolved_count,
        current_checkpoint_id: items.iter().find_map(|item| item.checkpoint_id),
        items,
    }
}

fn project_runtime_invariant(
    invariant: InvariantEvaluation,
    stored: Option<&StoredRuntimeInvariant>,
) -> StoredRuntimeInvariant {
    StoredRuntimeInvariant {
        id: stored.map(|record| record.id).unwrap_or(0),
        invariant_key: invariant.invariant_key.to_string(),
        title: invariant.title.to_string(),
        status: invariant.status.to_string(),
        severity: invariant.severity.to_string(),
        checkpoint_id: invariant.checkpoint_id,
        acceptance_bundle_id: invariant.acceptance_bundle_id,
        human_round_id: invariant.human_round_id,
        summary: invariant.summary,
        evidence_json: invariant.evidence_json,
        repair_action: invariant.repair_action,
        created_at: stored
            .map(|record| record.created_at.clone())
            .unwrap_or_default(),
        updated_at: stored
            .map(|record| record.updated_at.clone())
            .unwrap_or_default(),
    }
}

fn project_open_repair_lane_item(
    item: RepairLaneEvaluation,
    stored: Option<&StoredRepairLaneItem>,
) -> StoredRepairLaneItem {
    StoredRepairLaneItem {
        id: stored.map(|record| record.id).unwrap_or(0),
        repair_key: item.repair_key,
        source_invariant_key: Some(item.source_invariant_key.to_string()),
        checkpoint_id: item.checkpoint_id,
        acceptance_bundle_id: item.acceptance_bundle_id,
        item_kind: "runtime_invariant".to_string(),
        urgency: item.urgency.to_string(),
        status: "open".to_string(),
        summary: item.summary,
        repair_action: item.repair_action,
        evidence_json: item.evidence_json,
        created_at: stored
            .map(|record| record.created_at.clone())
            .unwrap_or_default(),
        updated_at: stored
            .map(|record| record.updated_at.clone())
            .unwrap_or_default(),
        resolved_at: None,
    }
}

fn project_resolved_repair_lane_item(mut item: StoredRepairLaneItem) -> StoredRepairLaneItem {
    item.status = "resolved".to_string();
    if item.resolved_at.is_none() && !item.updated_at.is_empty() {
        item.resolved_at = Some(item.updated_at.clone());
    }
    item
}

fn sort_runtime_invariants(invariants: &mut [StoredRuntimeInvariant]) {
    invariants.sort_by(|left, right| {
        invariant_status_sort_rank(&left.status)
            .cmp(&invariant_status_sort_rank(&right.status))
            .then_with(|| left.invariant_key.cmp(&right.invariant_key))
    });
}

fn sort_repair_lane_items(items: &mut [StoredRepairLaneItem]) {
    items.sort_by(|left, right| {
        repair_lane_status_sort_rank(&left.status)
            .cmp(&repair_lane_status_sort_rank(&right.status))
            .then_with(|| {
                repair_lane_urgency_sort_rank(&left.urgency)
                    .cmp(&repair_lane_urgency_sort_rank(&right.urgency))
            })
            .then_with(|| left.repair_key.cmp(&right.repair_key))
    });
}

fn invariant_status_sort_rank(status: &str) -> usize {
    match status {
        INVARIANT_FAILED_BLOCKED => 0,
        INVARIANT_FAILED_REPAIRABLE => 1,
        INVARIANT_PASSED => 2,
        _ => 3,
    }
}

fn repair_lane_status_sort_rank(status: &str) -> usize {
    match status {
        "open" => 0,
        _ => 1,
    }
}

fn repair_lane_urgency_sort_rank(urgency: &str) -> usize {
    match urgency {
        "blocked" => 0,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::core::{
        data_store::{DataStore, MigrationPlan, UpsertRuntimeHost},
        nota_runtime::{write_runtime_checkpoint, NotaCheckpointRequest},
        projection_runtime::{
            record_projection_failure, ProjectionTargetSpec, ProjectionTruthRevision,
            HOT_ROOT_PROJECTION_CLASS, REQUIRED_PROJECTION_POLICY,
        },
    };

    use super::refresh_runtime_invariants;

    #[test]
    fn runtime_invariants_surface_projection_debt_as_repair_lane_truth() -> Result<()> {
        let temp_db = TempDbPath::new("runtime-invariants")?;
        let migration_plan = MigrationPlan::new(crate::plugins::forge::migrations());
        let store = DataStore::open(temp_db.path(), migration_plan)?;

        store.upsert_runtime_host(UpsertRuntimeHost {
            host_key: "linux:test:/tmp/.entrance",
            os_family: "linux",
            host_label: "test-host",
            kernel_label: "unix",
            user_home: "/tmp",
            owner_root: "/tmp/.entrance",
            config_path: "/tmp/.entrance/entrance.toml",
            runtime_db_path: "/tmp/.entrance/data/entrance.db",
            exports_path: "/tmp/.entrance/exports",
            worktrees_root: "/tmp/.entrance/worktrees",
            wsl_distro_name: None,
            path_style: "posix",
            status: "active",
        })?;

        let checkpoint = write_runtime_checkpoint(
            &store,
            NotaCheckpointRequest {
                title: Some("Invariant checkpoint".to_string()),
                stable_level: "runtime invariant truth".to_string(),
                landed: vec!["checkpoint".to_string()],
                remaining: vec!["repair lane".to_string()],
                human_continuity_bus: "reduced".to_string(),
                selected_trunk: Some("runtime invariants".to_string()),
                next_start_hints: Vec::new(),
                project_dir: None,
            },
        )?;
        record_projection_failure(
            &store,
            ProjectionTargetSpec {
                projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                target_key: "exports/hot-root".into(),
                title: "Hot root export".into(),
                target_path: "/tmp/.entrance/exports/hot-root".into(),
                source_scope: "nota_runtime".into(),
                repair_action: "entrance nota export-hot-root".into(),
                projection_policy: REQUIRED_PROJECTION_POLICY.into(),
                is_required: true,
            },
            &ProjectionTruthRevision {
                checkpoint_id: Some(checkpoint.checkpoint.cadence_object.id),
                human_round_id: None,
                acceptance_bundle_id: None,
            },
            "test",
            "Hot root export failed.",
            "disk full",
        )?;

        let (invariants, repair_lane) = refresh_runtime_invariants(&store)?;
        assert_eq!(invariants.failed_count, 1);
        assert_eq!(invariants.repairable_count, 1);
        assert_eq!(
            invariants
                .invariants
                .iter()
                .find(|invariant| invariant.invariant_key == "required_projection_freshness")
                .map(|invariant| invariant.status.as_str()),
            Some("failed_repairable")
        );
        assert_eq!(repair_lane.open_count, 1);
        assert_eq!(repair_lane.repairable_count, 1);
        assert_eq!(
            repair_lane.items[0].source_invariant_key.as_deref(),
            Some("required_projection_freshness")
        );

        Ok(())
    }

    struct TempDbPath {
        root: PathBuf,
        db_path: PathBuf,
    }

    impl TempDbPath {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-runtime-invariants-{label}-{}-{suffix}",
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
}
