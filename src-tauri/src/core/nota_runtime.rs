use std::collections::HashSet;
use std::ops::Deref;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::core::data_store::{
    DataStore, NewCadenceLink, NewCadenceObject, NewNotaRuntimeAllocation, NewNotaRuntimeReceipt,
    NewNotaRuntimeTransaction, NotaRuntimeAllocationUpdate, NotaRuntimeTransactionUpdate,
    StoredCadenceLink, StoredCadenceObject, StoredForgeTask, StoredNotaRuntimeAllocation,
    StoredNotaRuntimeReceipt, StoredNotaRuntimeTransaction,
};
use crate::plugins::forge::{
    build_agent_task_request, build_dev_task_request, prepare_agent_dispatch_blocking,
    prepare_dev_dispatch_blocking, CreateTaskRequest, ForgePlugin, PreparedAgentDispatch,
    PreparedDevDispatch,
};

const CADENCE_CHECKPOINT_KIND: &str = "CADENCE_CHECKPOINT";
const CADENCE_HANDOUT_KIND: &str = "CADENCE_HANDOUT";
const CADENCE_WAKE_REQUEST_KIND: &str = "CADENCE_WAKE_REQUEST";
const CADENCE_POLICY_NOTE_KIND: &str = "CADENCE_POLICY_NOTE";
const NOTA_RUNTIME_SOURCE_TYPE: &str = "nota_runtime";
const NOTA_RUNTIME_SCOPE_TYPE: &str = "runtime";
const NOTA_RUNTIME_SCOPE_REF: &str = "Entrance";
const CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND: &str = "CADENCE_CHECKPOINT_WRITTEN";
const AGENT_RETURN_ACCEPTED_RECEIPT_KIND: &str = "AGENT_RETURN_ACCEPTED";
const DEV_RETURN_ACCEPTED_RECEIPT_KIND: &str = "DEV_RETURN_ACCEPTED";
const DEV_RETURN_REVIEW_READY_RECEIPT_KIND: &str = "DEV_RETURN_REVIEW_READY";
const DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND: &str = "DEV_RETURN_REVIEW_RECORDED";
const DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND: &str = "DEV_RETURN_INTEGRATE_RECORDED";
const DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND: &str = "DEV_RETURN_FINALIZE_RECORDED";
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
    fn as_str(self) -> &'static str {
        match self {
            Self::InProcess => "in_process",
            Self::DetachedForgeCliSupervisor => "detached_forge_cli_supervisor",
        }
    }
}

fn default_nota_dispatch_execution_host() -> String {
    NotaDispatchExecutionHost::InProcess.as_str().to_string()
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AgentReturnAcceptedReceiptPayload {
    allocation_id: i64,
    lineage_ref: String,
    checkpoint_id: i64,
    child_dispatch_role: String,
    execution_host: String,
    target_kind: String,
    target_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DevReturnAcceptedReceiptPayload {
    allocation_id: i64,
    lineage_ref: String,
    checkpoint_id: i64,
    child_dispatch_role: String,
    execution_host: String,
    target_kind: String,
    target_ref: String,
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
enum NotaDispatchLane {
    Agent,
    Dev,
}

impl NotaDispatchLane {
    fn surface_action(self) -> &'static str {
        match self {
            Self::Agent => "do",
            Self::Dev => "dev",
        }
    }

    fn allocator_surface(self) -> &'static str {
        match self {
            Self::Agent => "nota_do",
            Self::Dev => "nota_dev",
        }
    }

    fn transaction_kind(self) -> &'static str {
        match self {
            Self::Agent => "forge_agent_dispatch",
            Self::Dev => "forge_dev_dispatch",
        }
    }

    fn default_title(self, issue_id: &str) -> String {
        match self {
            Self::Agent => format!("Do dispatch {issue_id}"),
            Self::Dev => format!("Dev dispatch {issue_id}"),
        }
    }

    fn checkpoint_title(self, issue_id: &str) -> String {
        match self {
            Self::Agent => format!("Do allocation: {issue_id}"),
            Self::Dev => format!("Dev allocation: {issue_id}"),
        }
    }

    fn checkpoint_stable_level(self) -> &'static str {
        match self {
            Self::Agent => "single-ingress, checkpointed, DB-first NOTA host with a minimal Do allocation object and allocation-owned terminal outcome boundary",
            Self::Dev => "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned dev runtime lane",
        }
    }

    fn selected_trunk(self) -> &'static str {
        match self {
            Self::Agent => "Do allocation storage cut",
            Self::Dev => "NOTA-owned dev runtime cut",
        }
    }

    fn prepare_dispatch(
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

    fn build_task_request(
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

    fn build_lineage_ref(self, transaction_id: i64, task_id: i64) -> String {
        match self {
            Self::Agent => build_do_allocation_lineage_ref(transaction_id, task_id),
            Self::Dev => build_dev_allocation_lineage_ref(transaction_id, task_id),
        }
    }

    fn build_checkpoint_landed_items(
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

    fn build_checkpoint_remaining_items(
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

    fn build_checkpoint_hints(
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
    stored_allocations: Vec<StoredNotaRuntimeAllocation>,
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
struct DevReturnReviewReadyReceiptPayload {
    checkpoint_id: i64,
    #[serde(flatten)]
    next_step: NotaRuntimeNextStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DevReturnReviewRecordedReceiptPayload {
    checkpoint_id: i64,
    review: NotaRuntimeReview,
    next_step: NotaRuntimeNextStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DevReturnIntegrateRecordedReceiptPayload {
    checkpoint_id: i64,
    integrate: NotaRuntimeIntegrate,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_step: Option<NotaRuntimeNextStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DevReturnFinalizeRecordedReceiptPayload {
    checkpoint_id: i64,
    finalize: NotaRuntimeFinalize,
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

struct RecommendedCheckpointCandidate {
    kind: RecommendedCheckpointCandidateKind,
    allocation_id: i64,
    source_transaction_id: i64,
    request: NotaCheckpointRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecommendedCheckpointCandidateKind {
    AgentEscalationContinuity,
    AgentReturnAcceptance,
    DevReturnAcceptance,
    DevReturnClosure,
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

    let dispatch = lane.prepare_dispatch(data_store, request.project_dir.clone())?;
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
        .unwrap_or_else(|| lane.default_title(&dispatch.issue_id));

    let mut transaction =
        data_store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: lane.surface_action(),
            transaction_kind: lane.transaction_kind(),
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

    let task_request =
        lane.build_task_request(&dispatch, model.clone(), request.agent_command.clone())?;
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
    let mut allocation = data_store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
        allocator_role: "nota",
        allocator_surface: lane.allocator_surface(),
        allocation_kind: lane.transaction_kind(),
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
            title: Some(lane.checkpoint_title(&dispatch.issue_id)),
            stable_level: lane.checkpoint_stable_level().to_string(),
            landed: lane.build_checkpoint_landed_items(
                transaction.id,
                &allocation,
                task_id,
                &dispatch,
                &spawn_error,
            ),
            remaining: lane.build_checkpoint_remaining_items(allocation.id, task_id, &spawn_error),
            human_continuity_bus: if spawn_error.is_some() {
                "still required for operator recovery".to_string()
            } else {
                "reduced but not eliminated".to_string()
            },
            selected_trunk: Some(lane.selected_trunk().to_string()),
            next_start_hints: lane.build_checkpoint_hints(
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
    let stored_allocations = materialize_terminal_allocation_outcomes(data_store)?;
    let allocations = stored_allocations
        .iter()
        .cloned()
        .map(project_nota_runtime_allocation_read_record)
        .collect();
    Ok(NotaRuntimeAllocationsReport {
        allocation_count: stored_allocations.len(),
        allocations,
        stored_allocations,
    })
}

fn project_nota_runtime_allocation_read_record(
    allocation: StoredNotaRuntimeAllocation,
) -> NotaRuntimeAllocationReadRecord {
    let dispatch_truth =
        serde_json::from_str::<NotaDoAllocationPayload>(&allocation.payload_json).ok();

    NotaRuntimeAllocationReadRecord {
        child_dispatch_role: dispatch_truth
            .as_ref()
            .map(|payload| payload.child_dispatch_role.clone()),
        child_dispatch_tool_name: dispatch_truth
            .as_ref()
            .map(|payload| payload.child_dispatch_tool_name.clone()),
        allocation,
    }
}

pub fn list_nota_runtime_receipts(
    data_store: &DataStore,
    transaction_id: Option<i64>,
) -> Result<NotaRuntimeReceiptsReport> {
    materialize_terminal_receipt_backflow(data_store, transaction_id)?;
    materialize_runtime_closure_acceptance_receipt_backflow(data_store, transaction_id)?;
    let receipts = data_store.list_nota_runtime_receipts(transaction_id)?;
    Ok(NotaRuntimeReceiptsReport {
        receipt_count: receipts.len(),
        requested_transaction_id: transaction_id,
        receipts,
    })
}

fn materialize_terminal_receipt_backflow(
    data_store: &DataStore,
    transaction_id: Option<i64>,
) -> Result<()> {
    for allocation in data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .filter(|allocation| {
            transaction_id
                .map(|requested_id| allocation.source_transaction_id == requested_id)
                .unwrap_or(true)
        })
    {
        materialize_terminal_allocation_outcome(data_store, allocation)?;
    }

    Ok(())
}

fn materialize_runtime_closure_acceptance_receipt_backflow(
    data_store: &DataStore,
    transaction_id: Option<i64>,
) -> Result<()> {
    let checkpoints = list_runtime_checkpoints(data_store)?;
    let Some(current_checkpoint) = checkpoints
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.cadence_object.is_current)
    else {
        return Ok(());
    };

    let allocations = materialize_terminal_allocation_outcomes(data_store)?;
    let Some(candidate) = latest_runtime_closure_checkpoint_candidate(data_store, &allocations)?
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

    if let Err(error) =
        sync_runtime_closure_checkpoint_to_transaction(data_store, &candidate, current_checkpoint)
    {
        if !is_readonly_sqlite_error(&error) {
            return Err(error);
        }

        tracing::warn!(
            allocation_id = candidate.allocation_id,
            transaction_id = candidate.source_transaction_id,
            checkpoint_id = current_checkpoint.cadence_object.id,
            error = %error,
            "Skipping runtime closure acceptance receipt backflow on read-only database"
        );
    }

    Ok(())
}

fn materialize_terminal_allocation_outcomes(
    data_store: &DataStore,
) -> Result<Vec<StoredNotaRuntimeAllocation>> {
    data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .map(|allocation| materialize_terminal_allocation_outcome(data_store, allocation))
        .collect()
}

fn materialize_terminal_allocation_outcome(
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

    persist_terminal_allocation_projection(data_store, &allocation, &projected, &outcome)?;

    Ok(projected)
}

fn persist_terminal_allocation_projection(
    data_store: &DataStore,
    stored_allocation: &StoredNotaRuntimeAllocation,
    projected_allocation: &StoredNotaRuntimeAllocation,
    outcome: &NotaDoAllocationTerminalOutcome,
) -> Result<()> {
    if stored_allocation.status != projected_allocation.status
        || stored_allocation.payload_json != projected_allocation.payload_json
    {
        if let Err(error) = data_store.update_nota_runtime_allocation(
            stored_allocation.id,
            NotaRuntimeAllocationUpdate {
                status: &projected_allocation.status,
                payload_json: Some(&projected_allocation.payload_json),
            },
        ) {
            ignore_readonly_allocation_persistence_error(
                error,
                stored_allocation,
                "update_terminal_outcome",
            )?;
        }
    }

    let receipt_payload = build_allocation_terminal_outcome_receipt_payload(
        projected_allocation,
        &projected_allocation.status,
        outcome,
    );
    let receipt_recorded = has_allocation_terminal_outcome_receipt(
        data_store,
        stored_allocation.source_transaction_id,
        &receipt_payload,
    )?;
    if !receipt_recorded {
        let receipt_payload_json = serde_json::to_string(&receipt_payload).with_context(|| {
            format!(
                "failed to serialize allocation {} terminal outcome receipt",
                stored_allocation.id
            )
        })?;
        if let Err(error) = data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: stored_allocation.source_transaction_id,
            receipt_kind: ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
            payload_json: &receipt_payload_json,
            status: "recorded",
        }) {
            ignore_readonly_allocation_persistence_error(
                error,
                stored_allocation,
                "append_terminal_outcome_receipt",
            )?;
        }
    }

    Ok(())
}

fn ignore_readonly_allocation_persistence_error(
    error: anyhow::Error,
    allocation: &StoredNotaRuntimeAllocation,
    operation: &'static str,
) -> Result<()> {
    if !is_readonly_sqlite_error(&error) {
        return Err(error);
    }

    tracing::warn!(
        allocation_id = allocation.id,
        lineage_ref = %allocation.lineage_ref,
        operation,
        error = %error,
        "Skipping NOTA allocation read-surface persistence on read-only database"
    );
    Ok(())
}

fn is_readonly_sqlite_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .to_string()
            .to_ascii_lowercase()
            .contains("readonly database")
    })
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
    let Some(candidate) = latest_runtime_closure_checkpoint_candidate(data_store, allocations)?
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

    let Some(candidate) =
        latest_runtime_closure_checkpoint_candidate(data_store, allocations.stored_allocations())?
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
            sync_runtime_closure_checkpoint_to_transaction(
                data_store,
                &candidate,
                current_checkpoint,
            )?;
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
    sync_runtime_closure_checkpoint_to_transaction(
        data_store,
        &RecommendedCheckpointCandidate {
            kind: candidate.kind,
            allocation_id: candidate.allocation_id,
            source_transaction_id,
            request: source_recommendation.clone(),
        },
        &write_report.checkpoint,
    )?;
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
    allocations: &[StoredNotaRuntimeAllocation],
) -> Result<Option<RecommendedCheckpointCandidate>> {
    let mut candidates = Vec::new();
    if let Some(candidate) =
        recommend_single_lane_allocator_checkpoint_candidate(data_store, allocations)?
    {
        candidates.push(candidate);
    }
    if let Some(candidate) = recommend_dev_return_checkpoint_candidate(data_store, allocations)? {
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
    data_store: &DataStore,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(transaction) =
        data_store.get_nota_runtime_transaction(candidate.source_transaction_id)?
    else {
        return Ok(());
    };

    if transaction.cadence_checkpoint_id != Some(checkpoint.cadence_object.id) {
        data_store.update_nota_runtime_transaction(
            transaction.id,
            NotaRuntimeTransactionUpdate {
                status: &transaction.status,
                forge_task_id: transaction.forge_task_id,
                cadence_checkpoint_id: Some(checkpoint.cadence_object.id),
            },
        )?;
    }

    ensure_checkpoint_written_receipt(data_store, transaction.id, checkpoint)?;
    ensure_runtime_closure_acceptance_receipt(data_store, candidate, checkpoint)
}

fn ensure_checkpoint_written_receipt(
    data_store: &DataStore,
    transaction_id: i64,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let receipts = data_store.list_nota_runtime_receipts(Some(transaction_id))?;
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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
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
    data_store: &DataStore,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    match candidate.kind {
        RecommendedCheckpointCandidateKind::AgentEscalationContinuity => Ok(()),
        RecommendedCheckpointCandidateKind::AgentReturnAcceptance => {
            ensure_agent_return_accepted_receipt(data_store, candidate, checkpoint)
        }
        RecommendedCheckpointCandidateKind::DevReturnAcceptance => {
            ensure_dev_return_accepted_receipt(data_store, candidate, checkpoint)?;
            ensure_dev_return_review_ready_receipt(data_store, candidate, checkpoint)
        }
        RecommendedCheckpointCandidateKind::DevReturnClosure => Ok(()),
    }
}

fn ensure_agent_return_accepted_receipt(
    data_store: &DataStore,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(allocation) = data_store
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
    let has_receipt = data_store
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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: candidate.source_transaction_id,
        receipt_kind: AGENT_RETURN_ACCEPTED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize agent return accepted receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

fn ensure_dev_return_accepted_receipt(
    data_store: &DataStore,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(allocation) = data_store
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
    let has_receipt = data_store
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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: candidate.source_transaction_id,
        receipt_kind: DEV_RETURN_ACCEPTED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev return accepted receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

fn ensure_dev_return_review_ready_receipt(
    data_store: &DataStore,
    candidate: &RecommendedCheckpointCandidate,
    checkpoint: &NotaCheckpointRecord,
) -> Result<()> {
    let Some(allocation) = data_store
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
    let has_receipt = data_store
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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: candidate.source_transaction_id,
        receipt_kind: DEV_RETURN_REVIEW_READY_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev review-ready receipt payload")?,
        status: "recorded",
    })?;

    Ok(())
}

pub fn record_dev_return_review(
    data_store: &DataStore,
    request: NotaDevReturnReviewRequest,
) -> Result<NotaDevReturnReviewReport> {
    let verdict = normalize_dev_return_review_verdict(&request.verdict)?;
    let summary = normalize_optional(request.summary.as_deref());
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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: request.transaction_id,
        receipt_kind: DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev review recorded receipt payload")?,
        status: "recorded",
    })?;

    let receipt = data_store
        .list_nota_runtime_receipts(Some(request.transaction_id))?
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
        .context("dev review recorded receipt should be readable after append")?;

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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: request.transaction_id,
        receipt_kind: DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev integrate recorded receipt payload")?,
        status: "recorded",
    })?;

    let receipt = data_store
        .list_nota_runtime_receipts(Some(request.transaction_id))?
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
        .context("dev integrate recorded receipt should be readable after append")?;

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

    data_store.append_nota_runtime_receipt(NewNotaRuntimeReceipt {
        transaction_id: request.transaction_id,
        receipt_kind: DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND,
        payload_json: &serde_json::to_string(&receipt_payload)
            .context("failed to serialize dev finalize recorded receipt payload")?,
        status: "recorded",
    })?;

    let receipt = data_store
        .list_nota_runtime_receipts(Some(request.transaction_id))?
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
        .context("dev finalize recorded receipt should be readable after append")?;

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
    allocations: &[StoredNotaRuntimeAllocation],
) -> Result<Option<RecommendedCheckpointCandidate>> {
    let Some(latest_allocation) = allocations
        .iter()
        .filter(|allocation| {
            allocation.allocator_role == "nota"
                && allocation.allocation_kind == "forge_agent_dispatch"
                && allocation.child_execution_kind == "forge_task"
        })
        .max_by_key(|allocation| allocation.id)
    else {
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
    let Some(latest_terminal_receipt) =
        latest_terminal_receipt_for_allocation(&receipts, latest_allocation)?
    else {
        return Ok(None);
    };

    let (kind, recommendation) = if outcome.boundary_kind == "return"
        && outcome.child_execution_status == "Done"
    {
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
                        format!(
                            "Transaction {transaction_id} receipt history includes terminal receipt {ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND} capturing allocation {} back to {} {}.",
                            latest_allocation.id,
                            latest_terminal_receipt.target_kind,
                            latest_terminal_receipt.target_ref
                        ),
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
                        format!(
                            "Transaction {transaction_id} receipt history includes terminal receipt {ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND} capturing allocation {} back to {} {}.",
                            latest_allocation.id,
                            latest_terminal_receipt.target_kind,
                            latest_terminal_receipt.target_ref
                        ),
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
    allocations: &[StoredNotaRuntimeAllocation],
) -> Result<Option<RecommendedCheckpointCandidate>> {
    let Some(latest_allocation) = allocations
        .iter()
        .filter(|allocation| {
            allocation.allocator_role == "nota"
                && allocation.allocation_kind == "forge_dev_dispatch"
                && allocation.child_execution_kind == "forge_task"
        })
        .max_by_key(|allocation| allocation.id)
    else {
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
    let Some(latest_terminal_receipt) =
        latest_terminal_receipt_for_allocation(&receipts, latest_allocation)?
    else {
        return Ok(None);
    };

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
            format!(
                "Transaction {transaction_id} receipt history includes terminal receipt {ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND} capturing allocation {} back to {} {}.",
                latest_allocation.id,
                latest_terminal_receipt.target_kind,
                latest_terminal_receipt.target_ref
            ),
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
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeReview>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = allocations
        .iter()
        .filter(|allocation| {
            allocation.allocator_role == "nota"
                && allocation.allocation_kind == "forge_dev_dispatch"
                && allocation.child_execution_kind == "forge_task"
        })
        .max_by_key(|allocation| allocation.id)
    else {
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
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeIntegrate>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = allocations
        .iter()
        .filter(|allocation| {
            allocation.allocator_role == "nota"
                && allocation.allocation_kind == "forge_dev_dispatch"
                && allocation.child_execution_kind == "forge_task"
        })
        .max_by_key(|allocation| allocation.id)
    else {
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
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeFinalize>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = allocations
        .iter()
        .filter(|allocation| {
            allocation.allocator_role == "nota"
                && allocation.allocation_kind == "forge_dev_dispatch"
                && allocation.child_execution_kind == "forge_task"
        })
        .max_by_key(|allocation| allocation.id)
    else {
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
    allocations: &[StoredNotaRuntimeAllocation],
    receipts: &[StoredNotaRuntimeReceipt],
) -> Result<Option<NotaRuntimeNextStep>> {
    if checkpoint_scope_ids.is_empty() {
        return Ok(None);
    }

    let Some(latest_dev_allocation) = allocations
        .iter()
        .filter(|allocation| {
            allocation.allocator_role == "nota"
                && allocation.allocation_kind == "forge_dev_dispatch"
                && allocation.child_execution_kind == "forge_task"
        })
        .max_by_key(|allocation| allocation.id)
    else {
        return Ok(None);
    };
    if latest_dev_allocation.status != "return_ready" {
        return Ok(None);
    }

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
                && payload.finalize.transaction_id == latest_dev_allocation.source_transaction_id
                && payload.finalize.lineage_ref == latest_dev_allocation.lineage_ref
        })
    {
        return Ok(None);
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
                && payload.integrate.transaction_id == latest_dev_allocation.source_transaction_id
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
                && payload.review.transaction_id == latest_dev_allocation.source_transaction_id
                && payload.review.lineage_ref == latest_dev_allocation.lineage_ref
        })
        .max_by_key(|(receipt_id, _)| *receipt_id)
    {
        return Ok(Some(payload.next_step));
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
        .map(|(_, payload)| payload.next_step))
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

fn build_checkpoint_summary(stable_level: &str, landed: &[String]) -> String {
    match landed.first() {
        Some(first_landed) => format!("{stable_level}. Landed: {first_landed}"),
        None => stable_level.to_string(),
    }
}

fn build_do_allocation_lineage_ref(transaction_id: i64, task_id: i64) -> String {
    build_nota_allocation_lineage_ref("do", transaction_id, task_id)
}

fn build_dev_allocation_lineage_ref(transaction_id: i64, task_id: i64) -> String {
    build_nota_allocation_lineage_ref("dev", transaction_id, task_id)
}

fn build_nota_allocation_lineage_ref(
    surface_action: &str,
    transaction_id: i64,
    task_id: i64,
) -> String {
    format!("nota/{surface_action}/transaction/{transaction_id}/forge-task/{task_id}")
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
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context, Result};
    use serde_json::Value;

    use crate::core::data_store::{
        DataStore, MigrationPlan, NewNotaRuntimeAllocation, NewNotaRuntimeReceipt,
        NewNotaRuntimeTransaction, NotaRuntimeTransactionUpdate,
    };

    use super::{
        active_checkpoint_scope_ids, default_nota_dispatch_execution_host,
        derive_nota_runtime_finalize, derive_nota_runtime_integrate, derive_nota_runtime_next_step,
        derive_nota_runtime_review, list_nota_runtime_allocations, list_nota_runtime_receipts,
        list_runtime_checkpoints, materialize_runtime_closure_checkpoint,
        recommend_runtime_closure_checkpoint, record_dev_return_finalize,
        record_dev_return_integration, record_dev_return_review, write_runtime_checkpoint,
        AllocationTerminalOutcomeReceiptPayload, NotaCheckpointRequest,
        NotaDevReturnFinalizeRequest, NotaDevReturnIntegrateRequest, NotaDevReturnReviewRequest,
        NotaDispatchExecutionHost, NotaDoAllocationPayload, NotaDoAllocationTerminalOutcome,
        AGENT_RETURN_ACCEPTED_RECEIPT_KIND, ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND,
        CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND, DEV_RETURN_ACCEPTED_RECEIPT_KIND,
        DEV_RETURN_FINALIZE_CLOSED_RUNTIME_STATE, DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND,
        DEV_RETURN_INTEGRATE_INTEGRATED_STATE, DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND,
        DEV_RETURN_INTEGRATE_RECORDED_RUNTIME_STATE, DEV_RETURN_REVIEW_APPROVED_VERDICT,
        DEV_RETURN_REVIEW_READY_RECEIPT_KIND, DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND,
    };

    struct TempDbPath {
        root: PathBuf,
        db_path: PathBuf,
    }

    impl TempDbPath {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-nota-runtime-{label}-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            let db_path = root.join("entrance.db");
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

    #[test]
    fn receipt_surface_backfills_terminal_outcome_without_allocation_preread() -> Result<()> {
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
        assert_eq!(report.receipt_count, 1);
        assert_eq!(
            report.receipts[0].receipt_kind,
            ALLOCATION_TERMINAL_OUTCOME_RECORDED_RECEIPT_KIND
        );

        let receipt_payload: AllocationTerminalOutcomeReceiptPayload =
            serde_json::from_str(&report.receipts[0].payload_json)?;
        assert_eq!(
            receipt_payload,
            AllocationTerminalOutcomeReceiptPayload {
                allocation_id: 1,
                lineage_ref: "nota/dev/transaction/1/forge-task/1".to_string(),
                boundary_kind: "escalation".to_string(),
                child_execution_status: "Failed".to_string(),
                child_execution_status_message: Some("Task heartbeat lost".to_string()),
                target_kind: "nota_runtime_transaction".to_string(),
                target_ref: transaction.id.to_string(),
                allocation_status: "escalated_failed".to_string(),
            }
        );

        let stored_allocations = store.list_nota_runtime_allocations()?;
        assert_eq!(stored_allocations.len(), 1);
        assert_eq!(stored_allocations[0].status, "escalated_failed");
        let stored_payload: NotaDoAllocationPayload =
            serde_json::from_str(&stored_allocations[0].payload_json)?;
        let terminal_outcome = stored_payload
            .terminal_outcome
            .expect("receipt read should persist the allocation terminal outcome");
        assert_eq!(terminal_outcome.child_execution_status, "Failed");
        assert_eq!(
            terminal_outcome.child_execution_status_message.as_deref(),
            Some("Task heartbeat lost")
        );

        Ok(())
    }

    #[test]
    fn receipt_surface_backfills_dev_return_acceptance_for_current_checkpoint() -> Result<()> {
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
            worktree_path: "C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-1048"
                .to_string(),
            prompt_source: "Entrance-owned harness/bootstrap dev prompt".to_string(),
            model: "codex".to_string(),
            agent_command: None,
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
        assert_eq!(seeded_receipts.len(), 2);
        assert!(seeded_receipts.iter().all(|receipt| {
            receipt.receipt_kind != DEV_RETURN_ACCEPTED_RECEIPT_KIND
                && receipt.receipt_kind != DEV_RETURN_REVIEW_READY_RECEIPT_KIND
        }));

        let report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(report.receipt_count, 4);
        assert_eq!(
            report.receipts[2].receipt_kind,
            DEV_RETURN_ACCEPTED_RECEIPT_KIND
        );

        let accepted_payload: Value = serde_json::from_str(&report.receipts[2].payload_json)?;
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

        assert_eq!(
            report.receipts[3].receipt_kind,
            DEV_RETURN_REVIEW_READY_RECEIPT_KIND
        );
        let review_ready_payload: Value = serde_json::from_str(&report.receipts[3].payload_json)?;
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

        let checkpoints = list_runtime_checkpoints(&store)?;
        let current_checkpoint = checkpoints
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.cadence_object.is_current);
        let checkpoint_scope_ids = active_checkpoint_scope_ids(&store, current_checkpoint)?;
        let next_step = derive_nota_runtime_next_step(
            &checkpoint_scope_ids,
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
        assert_eq!(recorded_report.receipt_count, 5);
        assert_eq!(
            recorded_report.receipts[4].receipt_kind,
            DEV_RETURN_REVIEW_RECORDED_RECEIPT_KIND
        );

        let review = derive_nota_runtime_review(
            &checkpoint_scope_ids,
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
        assert_eq!(integrate_started_report.receipt_count, 6);
        assert_eq!(
            integrate_started_report.receipts[5].receipt_kind,
            DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
        );

        let started_integrate = derive_nota_runtime_integrate(
            &checkpoint_scope_ids,
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
        assert_eq!(integrated_report.receipt_count, 7);
        assert_eq!(
            integrated_report.receipts[6].receipt_kind,
            DEV_RETURN_INTEGRATE_RECORDED_RECEIPT_KIND
        );

        let recorded_integrate = derive_nota_runtime_integrate(
            &checkpoint_scope_ids,
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
        assert_eq!(finalized_report.receipt_count, 8);
        assert_eq!(
            finalized_report.receipts[7].receipt_kind,
            DEV_RETURN_FINALIZE_RECORDED_RECEIPT_KIND
        );

        let recorded_finalize = derive_nota_runtime_finalize(
            &checkpoint_scope_ids,
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
        assert_eq!(
            closure_scope_ids[0],
            closure_checkpoint_record.cadence_object.id
        );
        assert!(closure_scope_ids.contains(&current_checkpoint.unwrap().cadence_object.id));

        let closure_receipts = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(closure_receipts.receipt_count, 9);
        assert_eq!(
            closure_receipts.receipts[8].receipt_kind,
            CADENCE_CHECKPOINT_WRITTEN_RECEIPT_KIND
        );

        let carried_review = derive_nota_runtime_review(
            &closure_scope_ids,
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
            allocations.stored_allocations(),
            &closure_receipts.receipts,
        )?
        .is_none());

        Ok(())
    }

    #[test]
    fn receipt_surface_backfills_agent_return_acceptance_for_current_checkpoint() -> Result<()> {
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
        assert_eq!(seeded_receipts.len(), 2);
        assert!(seeded_receipts
            .iter()
            .all(|receipt| receipt.receipt_kind != AGENT_RETURN_ACCEPTED_RECEIPT_KIND));

        let report = list_nota_runtime_receipts(&store, Some(transaction.id))?;
        assert_eq!(report.receipt_count, 3);
        assert_eq!(
            report.receipts[2].receipt_kind,
            AGENT_RETURN_ACCEPTED_RECEIPT_KIND
        );

        let accepted_payload: Value = serde_json::from_str(&report.receipts[2].payload_json)?;
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
        assert_eq!(second_report.receipt_count, 3);

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
}
