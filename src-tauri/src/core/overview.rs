use anyhow::Result;
use serde::Serialize;

use crate::core::{
    anti_zeno_runtime::{build_anti_zeno_budget_report, AntiZenoBudgetReport},
    chat_archive::{
        get_chat_archive_policy, list_chat_captures, ChatArchivePolicyReport, ChatCaptureListReport,
    },
    cold_docs_runtime::{list_cold_documents, NotaColdDocListReport},
    data_store::{
        DataStore, StoredDecisionRecord, StoredNotaRuntimeReceipt, StoredNotaRuntimeTransaction,
        StoredTodoRecord, StoredVisionRecord,
    },
    design_governance::{list_design_decisions, DesignDecisionListReport},
    environment_runtime::{
        current_runtime_host, list_owned_worktrees, OwnedWorktreeRegistryReport,
    },
    front_door::{build_nota_front_door_projection, NotaFrontDoorProjection},
    invariant_runtime::{project_runtime_invariants, RepairLaneReport, RuntimeInvariantReport},
    nota_runtime::{
        active_checkpoint_scope_ids, derive_anti_zeno_projection,
        derive_current_runtime_acceptance_bundle, derive_current_runtime_handout,
        derive_current_runtime_human_round, derive_current_runtime_wake_request,
        derive_nota_runtime_finalize, derive_nota_runtime_integrate, derive_nota_runtime_next_step,
        derive_nota_runtime_review, derive_runtime_round_state_projection,
        list_nota_runtime_allocations, list_nota_runtime_receipts, list_nota_runtime_transactions,
        list_runtime_acceptance_bundles, list_runtime_checkpoints, list_runtime_human_rounds,
        recommend_runtime_closure_checkpoint, NotaAcceptanceBundleListReport,
        NotaAcceptanceBundleRecord, NotaAntiZenoProjection, NotaCheckpointListReport,
        NotaCheckpointRecord, NotaCheckpointRequest, NotaHandoutRecord, NotaHumanRoundListReport,
        NotaHumanRoundRecord, NotaRoundStateProjection, NotaRuntimeAllocationReadRecord,
        NotaRuntimeAllocationsReport, NotaRuntimeFinalize, NotaRuntimeIntegrate,
        NotaRuntimeNextStep, NotaRuntimeReview, NotaRuntimeTransactionsReport,
        NotaWakeRequestRecord,
    },
    projection_runtime::{
        build_projection_status_report, ProjectionStatusReport, ProjectionTruthRevision,
    },
    recovery::{build_recovery_status_report, RecoveryImportOnlyStatusReport},
    supervision::RuntimeSupervisionProjection,
};

#[derive(Clone, Serialize)]
pub(crate) struct NotaRuntimeOverview {
    pub(crate) chat_policy: ChatArchivePolicyReport,
    pub(crate) checkpoints: NotaCheckpointListReport,
    pub(crate) human_rounds: NotaHumanRoundListReport,
    pub(crate) acceptance_bundles: NotaAcceptanceBundleListReport,
    pub(crate) transactions: NotaRuntimeTransactionsReport,
    pub(crate) allocations: NotaRuntimeAllocationsReport,
    pub(crate) visions: NotaVisionListReport,
    pub(crate) todos: NotaTodoListReport,
    pub(crate) cold_docs: NotaColdDocListReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<crate::core::data_store::StoredRuntimeHost>,
    pub(crate) worktrees: OwnedWorktreeRegistryReport,
    pub(crate) recovery: RecoveryImportOnlyStatusReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recommended_checkpoint: Option<NotaCheckpointRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) handout: Option<NotaHandoutRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) wake_request: Option<NotaWakeRequestRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) review: Option<NotaRuntimeReview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) integrate: Option<NotaRuntimeIntegrate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finalize: Option<NotaRuntimeFinalize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_step: Option<NotaRuntimeNextStep>,
    pub(crate) round_state: NotaRoundStateProjection,
    pub(crate) anti_zeno: NotaAntiZenoProjection,
    pub(crate) anti_zeno_budget: AntiZenoBudgetReport,
    pub(crate) front_door: NotaFrontDoorProjection,
    pub(crate) projections: ProjectionStatusReport,
    pub(crate) invariants: RuntimeInvariantReport,
    pub(crate) repair_lane: RepairLaneReport,
    pub(crate) decisions: DesignDecisionListReport,
    pub(crate) chat_captures: ChatCaptureListReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_supervision: Option<RuntimeSupervisionProjection>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaRuntimeStatus {
    pub(crate) chat_policy: ChatArchivePolicyReport,
    pub(crate) human_round_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_human_round: Option<NotaHumanRoundRecord>,
    pub(crate) checkpoint_count: usize,
    pub(crate) current_checkpoint_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_checkpoint: Option<NotaCheckpointRecord>,
    pub(crate) acceptance_bundle_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_acceptance_bundle: Option<NotaAcceptanceBundleRecord>,
    pub(crate) transaction_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_transaction: Option<StoredNotaRuntimeTransaction>,
    pub(crate) allocation_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_allocation: Option<NotaRuntimeAllocationReadRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_supervision: Option<RuntimeSupervisionProjection>,
    pub(crate) receipt_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_receipt: Option<StoredNotaRuntimeReceipt>,
    pub(crate) decision_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) latest_decision: Option<StoredDecisionRecord>,
    pub(crate) chat_capture_count: usize,
    pub(crate) vision_count: usize,
    pub(crate) todo_count: usize,
    pub(crate) cold_doc_count: usize,
    pub(crate) cold_docs: NotaColdDocListReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<crate::core::data_store::StoredRuntimeHost>,
    pub(crate) worktree_count: usize,
    pub(crate) worktrees: OwnedWorktreeRegistryReport,
    pub(crate) recovery: RecoveryImportOnlyStatusReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recommended_checkpoint: Option<NotaCheckpointRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) handout: Option<NotaHandoutRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) wake_request: Option<NotaWakeRequestRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) review: Option<NotaRuntimeReview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) integrate: Option<NotaRuntimeIntegrate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finalize: Option<NotaRuntimeFinalize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_step: Option<NotaRuntimeNextStep>,
    pub(crate) round_state: NotaRoundStateProjection,
    pub(crate) anti_zeno: NotaAntiZenoProjection,
    pub(crate) anti_zeno_budget: AntiZenoBudgetReport,
    pub(crate) front_door: NotaFrontDoorProjection,
    pub(crate) projections: ProjectionStatusReport,
    pub(crate) invariants: RuntimeInvariantReport,
    pub(crate) repair_lane: RepairLaneReport,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaTodoListReport {
    pub(crate) todo_count: usize,
    pub(crate) todos: Vec<StoredTodoRecord>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaVisionListReport {
    pub(crate) vision_count: usize,
    pub(crate) visions: Vec<StoredVisionRecord>,
}

pub(crate) fn build_projection_truth_revision(
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

pub(crate) fn build_nota_runtime_overview(data_store: &DataStore) -> Result<NotaRuntimeOverview> {
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
    let current_supervision = allocations
        .allocations
        .first()
        .map(|allocation| allocation.supervision.clone());

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
        current_supervision,
    })
}

pub(crate) fn build_nota_runtime_status(data_store: &DataStore) -> Result<NotaRuntimeStatus> {
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
    let current_supervision = allocations
        .allocations
        .first()
        .map(|allocation| allocation.supervision.clone());

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
        current_supervision,
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

pub(crate) fn list_nota_todos(data_store: &DataStore) -> Result<NotaTodoListReport> {
    let todos = data_store.list_todo_records()?;
    Ok(NotaTodoListReport {
        todo_count: todos.len(),
        todos,
    })
}

pub(crate) fn list_nota_visions(data_store: &DataStore) -> Result<NotaVisionListReport> {
    let visions = data_store.list_vision_records()?;
    Ok(NotaVisionListReport {
        vision_count: visions.len(),
        visions,
    })
}
