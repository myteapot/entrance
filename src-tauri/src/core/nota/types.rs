use std::ops::Deref;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::data_store::{
    DataStore, StoredCadenceLink, StoredCadenceObject, StoredNotaRuntimeAllocation,
    StoredNotaRuntimeReceipt, StoredNotaRuntimeTransaction,
};
use crate::core::supervision::RuntimeSupervisionProjection;
use crate::plugins::forge::{
    build_agent_task_request, build_dev_task_request, prepare_agent_dispatch_blocking,
    prepare_dev_dispatch_blocking, CreateTaskRequest, PreparedAgentDispatch, PreparedDevDispatch,
};

use super::helpers::*;
use super::{
    build_dev_checkpoint_hints, build_dev_checkpoint_landed_items,
    build_dev_checkpoint_remaining_items, build_do_checkpoint_hints,
    build_do_checkpoint_landed_items, build_do_checkpoint_remaining_items,
};

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceHumanRoundPayload {
    pub checkpoint_id: i64,
    pub round_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_round_state: Option<String>,
    pub accepted: bool,
    pub acceptance_present: bool,
    pub carry_forward_checkpointed: bool,
    pub fully_settled: bool,
    pub next_step_open: bool,
    pub stable_level: String,
    pub human_continuity_bus: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_trunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_bundle_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaHumanRoundRecord {
    #[serde(flatten)]
    pub cadence_object: StoredCadenceObject,
    pub payload: CadenceHumanRoundPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaHumanRoundListReport {
    pub human_round_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_human_round_id: Option<i64>,
    pub human_rounds: Vec<NotaHumanRoundRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceHandoutPayload {
    pub checkpoint_id: i64,
    pub round_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_round_state: Option<String>,
    pub stable_level: String,
    pub human_continuity_bus: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_trunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_round_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_bundle_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaHandoutRecord {
    #[serde(flatten)]
    pub cadence_object: StoredCadenceObject,
    pub payload: CadenceHandoutPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceWakeRequestPayload {
    pub checkpoint_id: i64,
    pub round_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_round_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_round_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_bundle_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_step: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaWakeRequestRecord {
    #[serde(flatten)]
    pub cadence_object: StoredCadenceObject,
    pub payload: CadenceWakeRequestPayload,
}

#[derive(Debug, Clone)]
pub struct NotaBoundaryClarificationRequest {
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct NotaBoundaryAskRequest {
    pub ask_code: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct NotaCurrentRoundAcceptanceRequest {
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaBoundaryClarificationPayload {
    pub checkpoint_id: i64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaBoundaryAskPayload {
    pub checkpoint_id: i64,
    pub ask_code: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaBoundaryClarificationReport {
    pub status: String,
    pub transaction: StoredNotaRuntimeTransaction,
    pub clarification: NotaBoundaryClarificationPayload,
    pub next_step: NotaRuntimeNextStep,
    pub receipt: StoredNotaRuntimeReceipt,
    pub superseded_transaction_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaBoundaryAskReport {
    pub status: String,
    pub transaction: StoredNotaRuntimeTransaction,
    pub ask: NotaBoundaryAskPayload,
    pub next_step: NotaRuntimeNextStep,
    pub receipt: StoredNotaRuntimeReceipt,
    pub superseded_transaction_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaCurrentRoundAcceptanceReport {
    pub status: String,
    pub acceptance_bundle: NotaAcceptanceBundleRecord,
    pub superseded_transaction_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum HumanRoundCanonicalState {
    Opened,
    Checkpointed,
    Accepted,
    Settling,
    FullySettled,
}

impl HumanRoundCanonicalState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Opened => "opened",
            Self::Checkpointed => "checkpointed",
            Self::Accepted => "accepted",
            Self::Settling => "settling",
            Self::FullySettled => "fully_settled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum HumanRoundDetailState {
    Uncheckpointed,
    CheckpointedPendingAcceptance,
    AcceptedWaitingCarryForward,
    AcceptedFollowupOpen,
    FullySettled,
}

impl HumanRoundDetailState {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Uncheckpointed => "uncheckpointed",
            Self::CheckpointedPendingAcceptance => "checkpointed_pending_acceptance",
            Self::AcceptedWaitingCarryForward => "accepted_waiting_carry_forward",
            Self::AcceptedFollowupOpen => "accepted_followup_open",
            Self::FullySettled => "fully_settled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRoundStateProjection {
    pub posture: String,
    pub state: String,
    pub detail_state: String,
    pub summary: String,
    pub accepted: bool,
    pub acceptance_present: bool,
    pub next_step_open: bool,
    pub carry_forward_checkpointed: bool,
    pub fully_settled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_bundle_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceAcceptanceBundlePayload {
    pub checkpoint_id: i64,
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub lineage_ref: String,
    pub acceptance_kind: String,
    pub round_state: String,
    pub fully_settled: bool,
    pub child_dispatch_role: String,
    pub execution_host: String,
    pub target_kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrate_outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finalize_state: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaAcceptanceBundleRecord {
    #[serde(flatten)]
    pub cadence_object: StoredCadenceObject,
    pub payload: CadenceAcceptanceBundlePayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaAcceptanceBundleListReport {
    pub acceptance_bundle_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_acceptance_bundle_id: Option<i64>,
    pub acceptance_bundles: Vec<NotaAcceptanceBundleRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaAntiZenoProjection {
    pub posture: String,
    pub state: String,
    pub detail_state: String,
    pub value: u8,
    pub summary: String,
    pub acceptance_present: bool,
    pub fully_settled: bool,
    pub next_step_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_bundle_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeClosureCheckpointMaterializationReport {
    pub status: String,
    pub checkpoint: Option<NotaCheckpointRecord>,
    pub source_recommendation: Option<NotaCheckpointRequest>,
    pub superseded_checkpoint_id: Option<i64>,
    pub supersession_link: Option<StoredCadenceLink>,
}

#[derive(Debug, Clone)]
pub struct NotaDoAgentDispatchRequest {
    pub project_dir: Option<String>,
    pub model: String,
    pub agent_command: Option<String>,
    pub title: Option<String>,
    pub repair_of_allocation_id: Option<i64>,
    pub execution_host: NotaDispatchExecutionHost,
}

pub type NotaDevDispatchRequest = NotaDoAgentDispatchRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotaDispatchExecutionHost {
    InProcess,
    DetachedForgeCliSupervisor,
}

impl NotaDispatchExecutionHost {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::DetachedForgeCliSupervisor => "detached_forge_cli_supervisor",
        }
    }
}

// default_nota_dispatch_execution_host moved to helpers.rs

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_of_allocation_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_of_transaction_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_of_lineage_ref: Option<String>,
    #[serde(default = "default_nota_dispatch_execution_host")]
    pub execution_host: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_of_allocation_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_of_transaction_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_of_lineage_ref: Option<String>,
    #[serde(default = "default_nota_dispatch_execution_host")]
    pub execution_host: String,
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
pub(super) struct AllocationTerminalOutcomeReceiptPayload {
    pub(super) allocation_id: i64,
    pub(super) lineage_ref: String,
    pub(super) boundary_kind: String,
    pub(super) child_execution_status: String,
    pub(super) child_execution_status_message: Option<String>,
    pub(super) target_kind: String,
    pub(super) target_ref: String,
    pub(super) allocation_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct AgentReturnAcceptedReceiptPayload {
    pub(super) allocation_id: i64,
    pub(super) lineage_ref: String,
    pub(super) checkpoint_id: i64,
    pub(super) child_dispatch_role: String,
    pub(super) execution_host: String,
    pub(super) target_kind: String,
    pub(super) target_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DevReturnAcceptedReceiptPayload {
    pub(super) allocation_id: i64,
    pub(super) lineage_ref: String,
    pub(super) checkpoint_id: i64,
    pub(super) child_dispatch_role: String,
    pub(super) execution_host: String,
    pub(super) target_kind: String,
    pub(super) target_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaDoDispatchReport {
    pub transaction: StoredNotaRuntimeTransaction,
    pub allocation: StoredNotaRuntimeAllocation,
    pub receipts: Vec<StoredNotaRuntimeReceipt>,
    pub dispatch: PreparedNotaDispatch,
    pub task_id: i64,
    pub task_status: String,
    pub spawn_error: Option<String>,
    pub checkpoint: NotaCheckpointRecord,
}

pub type NotaDevDispatchReport = NotaDoDispatchReport;

#[derive(Debug, Clone, Serialize)]
pub struct PreparedNotaDispatch {
    pub dispatch_role: crate::core::action::ActorRole,
    pub dispatch_tool_name: String,
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt_source: String,
    pub prompt: String,
}

impl From<PreparedAgentDispatch> for PreparedNotaDispatch {
    fn from(dispatch: PreparedAgentDispatch) -> Self {
        Self {
            dispatch_role: dispatch.dispatch_role,
            dispatch_tool_name: dispatch.dispatch_tool_name,
            issue_id: dispatch.issue_id,
            issue_status: dispatch.issue_status,
            issue_status_source: dispatch.issue_status_source,
            issue_title: dispatch.issue_title,
            project_root: dispatch.project_root,
            worktree_path: dispatch.worktree_path,
            prompt_source: dispatch.prompt_source,
            prompt: dispatch.prompt,
        }
    }
}

impl From<PreparedDevDispatch> for PreparedNotaDispatch {
    fn from(dispatch: PreparedDevDispatch) -> Self {
        Self {
            dispatch_role: dispatch.dispatch_role,
            dispatch_tool_name: dispatch.dispatch_tool_name,
            issue_id: dispatch.issue_id,
            issue_status: dispatch.issue_status,
            issue_status_source: dispatch.issue_status_source,
            issue_title: dispatch.issue_title,
            project_root: dispatch.project_root,
            worktree_path: dispatch.worktree_path,
            prompt_source: dispatch.prompt_source,
            prompt: dispatch.prompt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NotaDispatchLane {
    Agent,
    Dev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeBoundaryLane {
    Agent,
    Dev,
}

impl RuntimeBoundaryLane {
    pub(super) fn allocation_kind(self) -> &'static str {
        match self {
            Self::Agent => "forge_agent_dispatch",
            Self::Dev => "forge_dev_dispatch",
        }
    }
}

impl NotaDispatchLane {
    pub(super) fn surface_action(self) -> &'static str {
        match self {
            Self::Agent => "do",
            Self::Dev => "dev",
        }
    }

    pub(super) fn allocator_surface(self) -> &'static str {
        match self {
            Self::Agent => "nota_do",
            Self::Dev => "nota_dev",
        }
    }

    pub(super) fn transaction_kind(self) -> &'static str {
        match self {
            Self::Agent => "forge_agent_dispatch",
            Self::Dev => "forge_dev_dispatch",
        }
    }

    pub(super) fn default_title(self, issue_id: &str) -> String {
        match self {
            Self::Agent => format!("Do dispatch {issue_id}"),
            Self::Dev => format!("Dev dispatch {issue_id}"),
        }
    }

    pub(super) fn checkpoint_title(self, issue_id: &str) -> String {
        match self {
            Self::Agent => format!("Do allocation: {issue_id}"),
            Self::Dev => format!("Dev allocation: {issue_id}"),
        }
    }

    pub(super) fn checkpoint_stable_level(self) -> &'static str {
        match self {
            Self::Agent => "single-ingress, checkpointed, DB-first NOTA host with a minimal Do allocation object and allocation-owned terminal outcome boundary",
            Self::Dev => "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned dev runtime lane",
        }
    }

    pub(super) fn selected_trunk(self) -> &'static str {
        match self {
            Self::Agent => "Do allocation storage cut",
            Self::Dev => "NOTA-owned dev runtime cut",
        }
    }

    pub(super) fn prepare_dispatch(
        self,
        data_store: &DataStore,
        project_dir: Option<String>,
    ) -> Result<PreparedNotaDispatch> {
        match self {
            Self::Agent => prepare_agent_dispatch_blocking(data_store.clone(), project_dir)
                .map(Into::into)
                .map_err(anyhow::Error::msg),
            Self::Dev => prepare_dev_dispatch_blocking(data_store.clone(), project_dir)
                .map(Into::into)
                .map_err(anyhow::Error::msg),
        }
    }

    pub(super) fn build_task_request(
        self,
        dispatch: &PreparedNotaDispatch,
        model: String,
        agent_command: Option<String>,
    ) -> Result<CreateTaskRequest> {
        match self {
            Self::Agent => build_agent_task_request(
                dispatch.issue_id.clone(),
                dispatch.worktree_path.clone(),
                model,
                dispatch.prompt.clone(),
                Vec::new(),
                agent_command,
            ),
            Self::Dev => build_dev_task_request(
                dispatch.issue_id.clone(),
                dispatch.worktree_path.clone(),
                model,
                dispatch.prompt.clone(),
                Vec::new(),
                agent_command,
            ),
        }
        .map_err(anyhow::Error::msg)
    }

    pub(super) fn build_lineage_ref(self, transaction_id: i64, task_id: i64) -> String {
        match self {
            Self::Agent => build_do_allocation_lineage_ref(transaction_id, task_id),
            Self::Dev => build_dev_allocation_lineage_ref(transaction_id, task_id),
        }
    }

    pub(super) fn build_checkpoint_landed_items(
        self,
        transaction_id: i64,
        allocation: &StoredNotaRuntimeAllocation,
        task_id: i64,
        dispatch: &PreparedNotaDispatch,
        spawn_error: &Option<String>,
    ) -> Vec<String> {
        match self {
            Self::Agent => build_do_checkpoint_landed_items(
                transaction_id,
                allocation,
                task_id,
                dispatch,
                spawn_error,
            ),
            Self::Dev => build_dev_checkpoint_landed_items(
                transaction_id,
                allocation,
                task_id,
                dispatch,
                spawn_error,
            ),
        }
    }

    pub(super) fn build_checkpoint_remaining_items(
        self,
        allocation_id: i64,
        task_id: i64,
        spawn_error: &Option<String>,
    ) -> Vec<String> {
        match self {
            Self::Agent => build_do_checkpoint_remaining_items(allocation_id, task_id, spawn_error),
            Self::Dev => build_dev_checkpoint_remaining_items(allocation_id, task_id, spawn_error),
        }
    }

    pub(super) fn build_checkpoint_hints(
        self,
        transaction_id: i64,
        allocation_id: i64,
        task_id: i64,
        spawn_error: &Option<String>,
    ) -> Vec<String> {
        match self {
            Self::Agent => {
                build_do_checkpoint_hints(transaction_id, allocation_id, task_id, spawn_error)
            }
            Self::Dev => {
                build_dev_checkpoint_hints(transaction_id, allocation_id, task_id, spawn_error)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeTransactionsReport {
    pub transaction_count: usize,
    pub transactions: Vec<StoredNotaRuntimeTransaction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeAllocationReadRecord {
    #[serde(flatten)]
    pub allocation: StoredNotaRuntimeAllocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_dispatch_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_dispatch_tool_name: Option<String>,
    pub supervision: RuntimeSupervisionProjection,
}

impl Deref for NotaRuntimeAllocationReadRecord {
    type Target = StoredNotaRuntimeAllocation;

    fn deref(&self) -> &Self::Target {
        &self.allocation
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeAllocationsReport {
    pub allocation_count: usize,
    pub allocations: Vec<NotaRuntimeAllocationReadRecord>,
    #[serde(skip)]
    pub(crate) stored_allocations: Vec<StoredNotaRuntimeAllocation>,
}

impl NotaRuntimeAllocationsReport {
    pub fn stored_allocations(&self) -> &[StoredNotaRuntimeAllocation] {
        &self.stored_allocations
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaRuntimeReceiptsReport {
    pub receipt_count: usize,
    pub requested_transaction_id: Option<i64>,
    pub receipts: Vec<StoredNotaRuntimeReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaRuntimeReview {
    pub state: String,
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub lineage_ref: String,
    pub child_dispatch_role: String,
    pub execution_host: String,
    pub target_kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaRuntimeNextStep {
    pub step: String,
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub lineage_ref: String,
    pub child_dispatch_role: String,
    pub execution_host: String,
    pub target_kind: String,
    pub target_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaRuntimeIntegrate {
    pub state: String,
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub lineage_ref: String,
    pub child_dispatch_role: String,
    pub execution_host: String,
    pub target_kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotaRuntimeFinalize {
    pub state: String,
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub lineage_ref: String,
    pub child_dispatch_role: String,
    pub execution_host: String,
    pub target_kind: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DevReturnReviewReadyReceiptPayload {
    pub(super) checkpoint_id: i64,
    #[serde(flatten)]
    pub(super) next_step: NotaRuntimeNextStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DoClarificationRecordedReceiptPayload {
    pub(super) checkpoint_id: i64,
    pub(super) clarification: NotaBoundaryClarificationPayload,
    pub(super) next_step: NotaRuntimeNextStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DoAskRecordedReceiptPayload {
    pub(super) checkpoint_id: i64,
    pub(super) ask: NotaBoundaryAskPayload,
    pub(super) next_step: NotaRuntimeNextStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DevReturnReviewRecordedReceiptPayload {
    pub(super) checkpoint_id: i64,
    pub(super) review: NotaRuntimeReview,
    pub(super) next_step: NotaRuntimeNextStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DevReturnIntegrateRecordedReceiptPayload {
    pub(super) checkpoint_id: i64,
    pub(super) integrate: NotaRuntimeIntegrate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) next_step: Option<NotaRuntimeNextStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct DevReturnFinalizeRecordedReceiptPayload {
    pub(super) checkpoint_id: i64,
    pub(super) finalize: NotaRuntimeFinalize,
}

#[derive(Debug, Clone)]
pub struct NotaDevReturnReviewRequest {
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub verdict: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaDevReturnReviewReport {
    pub status: String,
    pub review: NotaRuntimeReview,
    pub next_step: NotaRuntimeNextStep,
    pub receipt: StoredNotaRuntimeReceipt,
}

#[derive(Debug, Clone)]
pub struct NotaDevReturnIntegrateRequest {
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub state: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaDevReturnIntegrateReport {
    pub status: String,
    pub integrate: NotaRuntimeIntegrate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<NotaRuntimeNextStep>,
    pub receipt: StoredNotaRuntimeReceipt,
}

#[derive(Debug, Clone)]
pub struct NotaDevReturnFinalizeRequest {
    pub transaction_id: i64,
    pub allocation_id: i64,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaDevReturnFinalizeReport {
    pub status: String,
    pub finalize: NotaRuntimeFinalize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<NotaRuntimeNextStep>,
    pub receipt: StoredNotaRuntimeReceipt,
}

pub(super) struct RecommendedCheckpointCandidate {
    pub(super) kind: RecommendedCheckpointCandidateKind,
    pub(super) allocation_id: i64,
    pub(super) source_transaction_id: i64,
    pub(super) request: NotaCheckpointRequest,
}

#[derive(Debug, Clone)]
pub(super) struct DevRepairOrigin {
    pub(super) allocation_id: i64,
    pub(super) transaction_id: i64,
    pub(super) lineage_ref: String,
    pub(super) project_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecommendedCheckpointCandidateKind {
    AgentEscalationContinuity,
    AgentReturnAcceptance,
    AgentReturnClosure,
    DevReturnAcceptance,
    DevReturnClosure,
}
