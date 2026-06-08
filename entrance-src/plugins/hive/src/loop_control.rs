use std::{
    collections::HashMap,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use entrance_core::{
    ConnectorProviderConfig, ConnectorsConfig, HiveComment, HiveCommentCreate, HiveIssue,
    HiveIssueCreate, HiveLoopAdmission, HiveLoopAdmissionCreate, HiveLoopContract,
    HiveLoopContractCreate, HiveLoopEvidence, HiveLoopEvidenceCreate, HiveLoopPacket,
    HiveLoopPacketCreate, HiveLoopPolicy, HiveLoopPolicyCreate, HiveLoopStage, HiveLoopStageCreate,
    HiveLoopVerdict, HiveLoopVerdictCreate, Store, StoreSchemaStatus,
};
use serde::{Deserialize, Serialize};

const PACKET_SCHEMA_VERSION: &str = "entrance.hive.packet.v1";
const POLICY_SCHEMA_VERSION: &str = "entrance.hive.policy.v1";
const ADMISSION_SCHEMA_VERSION: &str = "entrance.hive.admission.v1";
const VERDICT_SCHEMA_VERSION: &str = "entrance.hive.verdict.v1";
const WORKER_RECEIPT_SCHEMA_VERSION: &str = "entrance.hive.worker_receipt.v1";
const OPERATOR_DECISION_SCHEMA_VERSION: &str = "entrance.hive.operator_decision.v1";
const OPERATOR_COMMENT_SCHEMA_VERSION: &str = "entrance.hive.operator_comment.v1";
const ISSUE_ACTION_SCHEMA_VERSION: &str = "entrance.hive.issue_action.v1";
const ISSUE_MIRROR_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror.v1";
const CONNECTOR_REGISTRY_SCHEMA_VERSION: &str = "entrance.hive.connector_registry.v1";
const SYSTEM_COMMENT_SCHEMA_VERSION: &str = "entrance.hive.system_comment.v1";
const AUDIT_SCHEMA_VERSION: &str = "entrance.hive.audit.v1";
const DOCTOR_SCHEMA_VERSION: &str = "entrance.hive.doctor.v1";
pub const CONNECTOR_MIRROR_RECEIPT_GATE: &str = "connector_mirror_receipt_current";
pub const CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND: &str = "ISSUE_MIRROR_SYNC_RECEIPT";
const VERDICT_SCORE_METRICS: &[&str] = &[
    "stage_completeness",
    "runtime_readiness",
    "evidence_presence",
    "admission_integrity",
];
const DEFAULT_WORKER_TIMEOUT_SECS: u64 = 60;
const MAX_WORKER_TIMEOUT_SECS: u64 = 600;
const DEFAULT_WORKER_ATTEMPTS: u64 = 1;
const MAX_WORKER_ATTEMPTS: u64 = 3;
const CONNECTOR_RETRY_MAX_ATTEMPTS: u64 = 2;
const CONNECTOR_RETRY_BASE_BACKOFF_MS: u64 = 100;
const CONNECTOR_ADMISSION_REQUIRED_CHECKS: &[&str] = &[
    "provider_supported",
    "provider_admission_ready",
    "mirror_current",
    "readback_checks_passed",
    "remote_write_contract_ready",
    "remote_target_valid",
    "retry_policy_bound",
];

struct ConnectorAdmissionCheckSpecDef {
    name: &'static str,
    severity: &'static str,
    owner: &'static str,
    required_evidence: &'static [&'static str],
    summary: &'static str,
}

const CONNECTOR_ADMISSION_CHECK_REGISTRY: &[ConnectorAdmissionCheckSpecDef] = &[
    ConnectorAdmissionCheckSpecDef {
        name: "provider_supported",
        severity: "blocker",
        owner: "connector-registry",
        required_evidence: &["connector_registry.provider"],
        summary: "Connector provider must be registered for the issue surface.",
    },
    ConnectorAdmissionCheckSpecDef {
        name: "provider_admission_ready",
        severity: "blocker",
        owner: "provider-admission",
        required_evidence: &["connector_registry.provider_admission"],
        summary: "Provider admission policy must be ready and blocker-free.",
    },
    ConnectorAdmissionCheckSpecDef {
        name: "mirror_current",
        severity: "blocker",
        owner: "mirror-ledger",
        required_evidence: &["issue_mirror_status.current", "issue_mirror_sync_receipt"],
        summary: "Local issue/status/comment mirror must be current before admission.",
    },
    ConnectorAdmissionCheckSpecDef {
        name: "readback_checks_passed",
        severity: "blocker",
        owner: "readback-ledger",
        required_evidence: &["issue_mirror_status.checks"],
        summary: "Readback checks must pass for the connector mirror.",
    },
    ConnectorAdmissionCheckSpecDef {
        name: "remote_write_contract_ready",
        severity: "blocker",
        owner: "remote-contract",
        required_evidence: &["connector_writer_adapter", "connector_remote_contract"],
        summary: "Remote writer/readback contract must be ready when the provider needs a remote issue API.",
    },
    ConnectorAdmissionCheckSpecDef {
        name: "remote_target_valid",
        severity: "blocker",
        owner: "remote-target",
        required_evidence: &["connector_remote_target"],
        summary: "Review surface must parse as a provider-specific remote issue target when required.",
    },
    ConnectorAdmissionCheckSpecDef {
        name: "retry_policy_bound",
        severity: "blocker",
        owner: "retry-policy",
        required_evidence: &["connector_remote_contract.retry", "connector_remote_diagnostics"],
        summary: "Observed remote attempts must stay within the active retry policy budget.",
    },
];

#[derive(Debug, Clone, Copy)]
struct GateSpec {
    name: &'static str,
    description: &'static str,
    expected_object_kind: Option<&'static str>,
    required_receipts: &'static [&'static str],
    check: GateCheck,
}

#[derive(Debug, Clone, Copy)]
enum GateCheck {
    ReceiptRequirementsSatisfied,
    BodyFieldPresent(&'static str),
    DecisionPresent,
    ExternalReceiptCurrent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyGateSpec {
    pub schema_version: String,
    pub name: String,
    pub description: String,
    pub expected_object_kind: Option<String>,
    pub required_receipts: Vec<String>,
    pub check: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePolicyRegistry {
    pub schema_version: String,
    pub supported: Vec<RuntimePolicySpec>,
    pub worker: WorkerPolicySpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePolicySpec {
    pub name: String,
    pub mode: String,
    pub description: String,
    pub command: Option<String>,
    pub required_worker_context: Vec<String>,
    pub sandbox: RuntimeSandboxSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSandboxSpec {
    pub filesystem: String,
    pub network: String,
    pub writes_artifacts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPolicySpec {
    pub default_timeout_secs: u64,
    pub max_timeout_secs: u64,
    pub timeout_env: String,
    pub default_attempts: u64,
    pub max_attempts: u64,
    pub attempts_env: String,
    pub required_receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorPolicyRegistry {
    pub schema_version: String,
    pub admission: ConnectorAdmissionPolicySpec,
    pub retry: Vec<ConnectorRetryPolicySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorRetryPolicySpec {
    pub schema_version: String,
    pub provider: String,
    pub transport: String,
    pub applies_to: Vec<String>,
    pub max_attempts: u64,
    pub base_backoff_ms: u64,
    pub backoff_strategy: String,
    pub retryable_http_statuses: Vec<u16>,
    pub rate_limit_http_statuses: Vec<u16>,
    pub rate_limit_headers: Vec<String>,
    pub no_immediate_retry_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorRegistryReport {
    pub schema_version: String,
    pub providers: Vec<ConnectorProviderSpec>,
    pub admission: ConnectorAdmissionPolicySpec,
    pub provider_admissions: Vec<ConnectorProviderAdmissionSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorProviderSpec {
    pub name: String,
    pub display_name: String,
    pub status: String,
    pub mode: String,
    pub review_surface_prefixes: Vec<String>,
    pub auth_required: bool,
    pub auth_env: Vec<String>,
    pub configured: bool,
    pub supports_status: bool,
    pub supports_publish: bool,
    pub supports_readback: bool,
    pub supports_admission: bool,
    pub storage: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorAdmissionPolicySpec {
    pub schema_version: String,
    pub gate: String,
    pub route_to: String,
    pub expected_object_kind: String,
    pub check: String,
    pub required_receipts: Vec<String>,
    pub required_checks: Vec<String>,
    pub check_registry: Vec<ConnectorAdmissionCheckSpec>,
    pub dry_run_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorAdmissionCheckSpec {
    pub name: String,
    pub severity: String,
    pub owner: String,
    pub required_evidence: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorProviderAdmissionSpec {
    pub schema_version: String,
    pub provider: String,
    pub status: String,
    pub gate: String,
    pub route_to: Option<String>,
    pub expected_object_kind: String,
    pub check: String,
    pub required_receipts: Vec<String>,
    pub required_checks: Vec<String>,
    pub check_registry: Vec<ConnectorAdmissionCheckSpec>,
    pub blockers: Vec<String>,
    pub dry_run_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopCreateRequest {
    pub title: String,
    pub goal: String,
    pub boundary: String,
    pub approach_space: Vec<String>,
    pub eval_space: Vec<String>,
    pub review_surface: String,
    pub autonomy_level: String,
    pub runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRunRequest {
    pub loop_id: i64,
    pub runtime: Option<String>,
    pub decision: Option<String>,
    pub worker_timeout_secs: Option<u64>,
    pub worker_attempts: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerdictDecision {
    Keep,
    Reject,
    NeedsReview,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeFailure {
    Probe,
    Worker,
}

#[derive(Debug, Clone, Copy)]
struct LoopPolicySpec {
    object_kind: &'static str,
    writer_role: &'static str,
    route_from: &'static str,
    route_to: &'static str,
    gate: &'static str,
}

const CURRENT_LOOP_ROLES: &[&str] = &["explorer", "developer", "reviewer"];
const LEGACY_LOOP_ROLES: &[&str] = &["explorer", "doer", "evaluator"];
const REVIEWER_INVALID_ROUND_BUDGET: i64 = 3;

const DEFAULT_LOOP_POLICIES: &[LoopPolicySpec] = &[
    LoopPolicySpec {
        object_kind: "EXPLORATION_PACKET",
        writer_role: "explorer",
        route_from: "explorer",
        route_to: "developer",
        gate: "candidate_receipts_present",
    },
    LoopPolicySpec {
        object_kind: "EXECUTION_PACKET",
        writer_role: "developer",
        route_from: "developer",
        route_to: "reviewer",
        gate: "runtime_receipts_present",
    },
    LoopPolicySpec {
        object_kind: "VERDICT_PACKET",
        writer_role: "reviewer",
        route_from: "reviewer",
        route_to: "complete",
        gate: "verdict_receipts_present",
    },
];

const LEGACY_LOOP_POLICIES: &[LoopPolicySpec] = &[
    LoopPolicySpec {
        object_kind: "EXPLORATION_PACKET",
        writer_role: "explorer",
        route_from: "explorer",
        route_to: "doer",
        gate: "candidate_receipts_present",
    },
    LoopPolicySpec {
        object_kind: "EXECUTION_PACKET",
        writer_role: "doer",
        route_from: "doer",
        route_to: "evaluator",
        gate: "runtime_receipts_present",
    },
    LoopPolicySpec {
        object_kind: "VERDICT_PACKET",
        writer_role: "evaluator",
        route_from: "evaluator",
        route_to: "complete",
        gate: "verdict_receipts_present",
    },
];

struct TypedVerdict {
    decision: VerdictDecision,
    reason_code: &'static str,
    summary: String,
    runtime_ready: bool,
    evidence_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopReport {
    pub contract: HiveLoopContract,
    pub policies: Vec<HiveLoopPolicy>,
    pub packets: Vec<HiveLoopPacket>,
    pub admissions: Vec<HiveLoopAdmission>,
    pub stages: Vec<HiveLoopStage>,
    pub evidence: Vec<HiveLoopEvidence>,
    pub verdicts: Vec<HiveLoopVerdict>,
    pub issues: Vec<IssueCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRegistryReport {
    pub schema_version: String,
    pub gates: Vec<PolicyGateSpec>,
    pub runtime: RuntimePolicyRegistry,
    pub connector: ConnectorPolicyRegistry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopPolicyReport {
    pub loop_id: i64,
    pub policies: Vec<HiveLoopPolicyCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopPolicyCard {
    pub policy: HiveLoopPolicy,
    pub gate_spec: Option<PolicyGateSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopTraceReport {
    pub contract: HiveLoopContract,
    pub issue: Option<HiveIssue>,
    pub trace: IssueTraceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceReport {
    pub contract: HiveLoopContract,
    pub current_round: i64,
    pub evidence: Vec<IssueEvidenceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopAuditReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub passed: bool,
    pub failed_count: usize,
    pub checks: Vec<HiveLoopAuditCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopAuditCheck {
    pub name: String,
    pub passed: bool,
    pub summary: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDoctorReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub health: String,
    pub summary: String,
    pub next_actions: Vec<String>,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub runtime: String,
    pub issue_id: Option<i64>,
    pub issue_status: Option<String>,
    pub decision: Option<String>,
    pub reason_code: Option<String>,
    pub counts: HiveLoopDoctorCounts,
    pub failed_checks: Vec<String>,
    pub audit_failure_details: Vec<String>,
    pub missing_receipts: Vec<String>,
    pub worker_failures: Vec<String>,
    pub checks: Vec<HiveLoopDoctorCheck>,
    pub trace: IssueTraceSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDoctorCounts {
    pub packet_count: usize,
    pub admission_count: usize,
    pub evidence_count: usize,
    pub verdict_count: usize,
    pub round_packet_count: usize,
    pub round_admission_count: usize,
    pub round_evidence_count: usize,
    pub round_verdict_count: usize,
    pub receipt_required_count: usize,
    pub receipt_missing_count: usize,
    pub round_receipt_required_count: usize,
    pub round_receipt_missing_count: usize,
    pub role_worker_count: usize,
    pub role_worker_ok_count: usize,
    pub round_role_worker_count: usize,
    pub round_role_worker_ok_count: usize,
    pub round_worker_duration_ms: u64,
    pub round_worker_timeout_count: usize,
    pub round_worker_retry_exhausted_count: usize,
    pub audit_failed_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDoctorCheck {
    pub name: String,
    pub passed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCard {
    pub issue: HiveIssue,
    pub comments: Vec<HiveComment>,
    pub actions: Vec<IssueAction>,
    pub trace: Option<IssueTraceSummary>,
    pub doctor: Option<IssueDoctorSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueMirrorReport {
    pub schema_version: String,
    pub provider: String,
    pub review_surface: String,
    pub external_key: String,
    pub issue: HiveIssue,
    pub loop_contract: Option<HiveLoopContract>,
    pub comments: Vec<HiveComment>,
    pub actions: Vec<IssueAction>,
    pub trace: Option<IssueTraceSummary>,
    pub doctor: Option<IssueDoctorSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAction {
    pub schema_version: String,
    pub action: String,
    pub label: String,
    pub command: String,
    pub source: String,
    pub input: String,
    pub destructive: bool,
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTraceSummary {
    pub current_round: i64,
    pub rounds: Vec<IssueRoundSummary>,
    pub packet_count: usize,
    pub admission_count: usize,
    pub evidence_count: usize,
    pub verdict_count: usize,
    pub round_packet_count: usize,
    pub round_admission_count: usize,
    pub round_evidence_count: usize,
    pub round_verdict_count: usize,
    pub receipt_required_count: usize,
    pub receipt_missing_count: usize,
    pub round_receipt_required_count: usize,
    pub round_receipt_missing_count: usize,
    pub role_worker_count: usize,
    pub role_worker_ok_count: usize,
    pub round_role_worker_count: usize,
    pub round_role_worker_ok_count: usize,
    pub round_worker_duration_ms: u64,
    pub round_worker_timeout_count: usize,
    pub round_worker_retry_exhausted_count: usize,
    pub packet_schema: Option<String>,
    pub policy_schema: Option<String>,
    pub admission_schema: Option<String>,
    pub verdict_schema: Option<String>,
    pub last_admission_gate: Option<String>,
    pub last_gate_description: Option<String>,
    pub last_gate_expected_object_kind: Option<String>,
    pub last_admission_passed: Option<bool>,
    pub last_decision: Option<String>,
    pub reason_code: Option<String>,
    pub score_vector: Vec<ScoreVectorMetric>,
    pub human_options: Vec<String>,
    pub operator_event_count: usize,
    pub round_operator_event_count: usize,
    pub last_operator_event: Option<IssueOperatorSummary>,
    pub operator_events: Vec<IssueOperatorSummary>,
    pub worker_kind: Option<String>,
    pub worker_mode: Option<String>,
    pub worker_ok: Option<bool>,
    pub audit_schema: Option<String>,
    pub audit_passed: Option<bool>,
    pub audit_failed_count: usize,
    pub audit_failed_checks: Vec<String>,
    pub audit_failure_details: Vec<String>,
    pub evidence: Vec<IssueEvidenceSummary>,
    pub stages: Vec<IssueStageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueRoundSummary {
    pub round: i64,
    pub status: String,
    pub decision: Option<String>,
    pub evidence_count: usize,
    pub rejected_count: usize,
    pub receipt_required_count: usize,
    pub receipt_missing_count: usize,
    pub worker_count: usize,
    pub worker_ok_count: usize,
    pub worker_timeout_count: usize,
    pub worker_retry_exhausted_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDoctorSummary {
    pub schema_version: String,
    pub health: String,
    pub summary: String,
    pub next_actions: Vec<String>,
    pub runtime: String,
    pub current_round: i64,
    pub counts: HiveLoopDoctorCounts,
    pub failed_checks: Vec<String>,
    pub audit_failure_details: Vec<String>,
    pub missing_receipts: Vec<String>,
    pub worker_failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreVectorMetric {
    pub name: String,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueOperatorSummary {
    pub id: i64,
    pub round: i64,
    pub kind: String,
    pub author: Option<String>,
    pub action: Option<String>,
    pub issue_status: Option<String>,
    pub loop_status: Option<String>,
    pub note: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueEvidenceSummary {
    pub id: i64,
    pub round: i64,
    pub stage_role: Option<String>,
    pub kind: String,
    pub summary: String,
    pub schema_version: Option<String>,
    pub admission_result: Option<String>,
    pub blocked_phase: Option<String>,
    pub missing_receipts: Vec<String>,
    pub packet_envelope_errors: Vec<String>,
    pub operator_options: Vec<String>,
    pub operator_author: Option<String>,
    pub operator_action: Option<String>,
    pub worker_kind: Option<String>,
    pub worker_mode: Option<String>,
    pub worker_ok: Option<bool>,
    pub worker_receipt_ok: Option<bool>,
    pub worker_timed_out: Option<bool>,
    pub worker_status: Option<i64>,
    pub worker_duration_ms: Option<u64>,
    pub worker_timeout_secs: Option<u64>,
    pub worker_attempt_count: Option<u64>,
    pub worker_max_attempts: Option<u64>,
    pub worker_retry_exhausted: Option<bool>,
    pub worker_command: Option<String>,
    pub worker_cwd: Option<String>,
    pub worker_action: Option<String>,
    pub worker_evidence_summary: Option<String>,
    pub worker_gate_count: Option<usize>,
    pub worker_receipt_errors: Vec<String>,
    pub transcript_excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueStageSummary {
    pub role: String,
    pub status: String,
    pub summary: Option<String>,
    pub evidence_kind: Option<String>,
    pub evidence_summary: Option<String>,
    pub admission_result: Option<String>,
    pub worker_kind: Option<String>,
    pub worker_mode: Option<String>,
    pub worker_ok: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCommentRequest {
    pub issue_id: i64,
    pub author: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDecisionRequest {
    pub issue_id: i64,
    pub action: String,
    pub author: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueRunRequest {
    pub issue_id: i64,
    pub runtime: Option<String>,
    pub decision: Option<String>,
    pub worker_timeout_secs: Option<u64>,
    pub worker_attempts: Option<u64>,
    pub retry: bool,
    pub author: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IssueDecisionAction {
    Retry,
    RequestReview,
    Cancel,
}

pub fn create(store: &Store, request: HiveLoopCreateRequest) -> Result<HiveLoopReport> {
    let loop_id = store.insert_hive_loop_contract(HiveLoopContractCreate {
        title: request.title.clone(),
        goal: request.goal.clone(),
        boundary: default_text(request.boundary, "No explicit boundary supplied."),
        approach_space: default_vec(request.approach_space, "Explore the smallest runnable MVP"),
        eval_space: default_vec(
            request.eval_space,
            "CLI loop run produces a keep/reject/block verdict",
        ),
        review_surface: default_text(request.review_surface, "local-hive-panel"),
        autonomy_level: default_text(request.autonomy_level, "run-approved-candidates"),
        runtime: default_text(request.runtime, "local"),
    })?;
    seed_default_policies(store, loop_id)?;

    let issue_id = store.insert_hive_issue(HiveIssueCreate {
        loop_id: Some(loop_id),
        title: format!("Loop #{loop_id}: {}", request.title),
        status: "Todo".to_string(),
        summary: Some("Loop contract created; waiting for Explorer.".to_string()),
    })?;

    store.insert_hive_comment(HiveCommentCreate {
        issue_id,
        author: "compiler".to_string(),
        body: "Loop contract admitted into Hive with 3 active policies.".to_string(),
        payload: system_comment_payload(
            "compiler",
            serde_json::json!({
                "loop_id": loop_id,
                "goal": request.goal,
                "next_phase": "explorer",
                "policy_count": 3
            }),
        ),
    })?;

    report(store, loop_id)
}

pub fn run(store: &Store, request: HiveLoopRunRequest) -> Result<HiveLoopReport> {
    let mut contract = store
        .get_hive_loop_contract(request.loop_id)?
        .with_context(|| format!("unknown hive loop `{}`", request.loop_id))?;
    let runtime = request.runtime.unwrap_or_else(|| contract.runtime.clone());
    let worker_timeout_secs = worker_timeout_secs(request.worker_timeout_secs)?;
    let worker_attempts = worker_attempts(request.worker_attempts)?;
    let issues = store.list_hive_issues_for_loop(contract.id)?;
    let issue_id = issues.first().map(|issue| issue.id);

    if contract.status != "todo" {
        return report(store, contract.id);
    }

    if runtime != contract.runtime {
        store.update_hive_loop_contract_runtime(contract.id, &runtime)?;
        contract.runtime = runtime.clone();
    }

    let runtime_probe = probe_runtime(&runtime);

    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(
            issue_id,
            "Doing",
            Some("Explorer, Developer, and Reviewer are running."),
        )?;
        add_system_comment(
            store,
            issue_id,
            "Loop run started.",
            serde_json::json!({ "loop_id": contract.id, "runtime": runtime }),
        )?;
    }

    store.update_hive_loop_contract_state(
        contract.id,
        "running",
        "explorer",
        contract.current_round,
    )?;
    let explorer_worker = run_role_worker(
        &runtime,
        "explorer",
        &contract,
        &runtime_probe,
        worker_timeout_secs,
        worker_attempts,
    );
    let explorer_stage = insert_stage(
        store,
        &contract,
        "explorer",
        "Explorer compiled the goal into a runnable candidate.",
        serde_json::json!({
            "goal": contract.goal,
            "boundary": contract.boundary,
            "approach_space": contract.approach_space
        }),
        serde_json::json!({
            "candidate": "Run a local MVP loop through Hive",
            "role_worker": explorer_worker,
            "constraints": [
                "keep work in SQLite/Hive",
                "separate explorer, developer, reviewer stages",
                "record issue/status/comment evidence"
            ]
        }),
    )?;
    let explorer_admission = emit_and_admit(
        store,
        &contract,
        "EXPLORATION_PACKET",
        "explorer",
        "explorer",
        "developer",
        serde_json::json!({
            "candidate": "Run a local MVP loop through Hive",
            "role_worker": explorer_worker,
            "constraints": [
                "keep work in SQLite/Hive",
                "separate explorer, developer, reviewer stages",
                "record issue/status/comment evidence"
            ]
        }),
    )?;
    if explorer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "explorer",
            Some(explorer_stage),
            &explorer_admission,
        );
    }
    let explorer_evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(explorer_stage),
        round: contract.current_round,
        kind: "exploration_packet".to_string(),
        summary: "Explorer produced a concrete local-loop candidate.".to_string(),
        path: None,
        payload: serde_json::json!({
            "candidate": "local-loop-mvp",
            "approach_count": contract.approach_space.len(),
            "worker": explorer_worker,
            "admission": explorer_admission.result
        }),
    })?;
    if let Some(issue_id) = issue_id {
        add_stage_system_comment(
            store,
            issue_id,
            contract.id,
            contract.current_round,
            "explorer",
            "exploration_packet",
            explorer_evidence_id,
            "Explorer admitted a candidate for this round.",
            &explorer_admission.result,
            &explorer_worker,
        )?;
    }

    store.update_hive_loop_contract_state(
        contract.id,
        "running",
        "developer",
        contract.current_round,
    )?;
    let runtime_worker = run_role_worker(
        &runtime,
        "developer",
        &contract,
        &runtime_probe,
        worker_timeout_secs,
        worker_attempts,
    );
    let developer_stage = insert_stage(
        store,
        &contract,
        "developer",
        "Developer executed the accepted MVP action and captured runtime evidence.",
        serde_json::json!({
            "candidate": "local-loop-mvp",
            "runtime": runtime
        }),
        serde_json::json!({
            "runtime_probe": runtime_probe,
            "runtime_worker": runtime_worker,
            "role_worker": runtime_worker,
            "artifact": "hive-loop-ledger"
        }),
    )?;
    let developer_admission = emit_and_admit(
        store,
        &contract,
        "EXECUTION_PACKET",
        "developer",
        "developer",
        "reviewer",
        serde_json::json!({
            "runtime": runtime,
            "runtime_probe": runtime_probe,
            "runtime_worker": runtime_worker,
            "role_worker": runtime_worker,
            "artifact": "hive-loop-ledger"
        }),
    )?;
    if developer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "developer",
            Some(developer_stage),
            &developer_admission,
        );
    }
    let developer_evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(developer_stage),
        round: contract.current_round,
        kind: "execution_packet".to_string(),
        summary: format!("Developer ran `{runtime}` runtime worker."),
        path: None,
        payload: serde_json::json!({
            "runtime": runtime,
            "probe": runtime_probe,
            "worker": runtime_worker,
            "admission": developer_admission.result
        }),
    })?;
    if let Some(issue_id) = issue_id {
        add_stage_system_comment(
            store,
            issue_id,
            contract.id,
            contract.current_round,
            "developer",
            "execution_packet",
            developer_evidence_id,
            "Developer admitted the execution packet.",
            &developer_admission.result,
            &runtime_worker,
        )?;
    }

    store.update_hive_loop_contract_state(
        contract.id,
        "evaluating",
        "reviewer",
        contract.current_round,
    )?;
    let evidence = store.list_hive_loop_evidence(contract.id)?;
    let reviewer_worker = run_role_worker(
        &runtime,
        "reviewer",
        &contract,
        &runtime_probe,
        worker_timeout_secs,
        worker_attempts,
    );
    let runtime_ready = runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        && worker_ok(&explorer_worker)
        && worker_ok(&runtime_worker)
        && worker_ok(&reviewer_worker);
    let runtime_failure = runtime_failure(&runtime_probe, &runtime_worker);
    let decision_override = parse_decision_override(request.decision.as_deref())?;
    let round_stage_evidence_count = evidence
        .iter()
        .filter(|row| row.round == contract.current_round && stage_bound_evidence_kind(&row.kind))
        .count();
    let typed_verdict = build_verdict(
        decision_override,
        runtime_ready,
        runtime_failure,
        &runtime,
        round_stage_evidence_count,
        contract.current_round,
    );
    let reviewer_stage = insert_stage(
        store,
        &contract,
        "reviewer",
        &typed_verdict.summary,
        serde_json::json!({
            "evidence_count": round_stage_evidence_count,
            "eval_space": contract.eval_space
        }),
        serde_json::json!({
            "decision": typed_verdict.decision.as_str(),
            "role_worker": reviewer_worker,
            "gates": {
                "three_stages_recorded": true,
                "evidence_recorded": round_stage_evidence_count > 0,
                "runtime_ready": typed_verdict.runtime_ready
            }
        }),
    )?;
    let reviewer_admission = emit_and_admit(
        store,
        &contract,
        "VERDICT_PACKET",
        "reviewer",
        "reviewer",
        "complete",
        typed_verdict.packet_payload(&reviewer_worker),
    )?;
    if reviewer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "reviewer",
            Some(reviewer_stage),
            &reviewer_admission,
        );
    }
    let reviewer_evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(reviewer_stage),
        round: contract.current_round,
        kind: "verdict_packet".to_string(),
        summary: typed_verdict.summary.clone(),
        path: None,
        payload: serde_json::json!({
            "decision": typed_verdict.decision.as_str(),
            "reason_code": typed_verdict.reason_code,
            "runtime_ready": typed_verdict.runtime_ready,
            "worker": reviewer_worker,
            "admission": reviewer_admission.result
        }),
    })?;
    if let Some(issue_id) = issue_id {
        add_stage_system_comment(
            store,
            issue_id,
            contract.id,
            contract.current_round,
            "reviewer",
            "verdict_packet",
            reviewer_evidence_id,
            "Reviewer admitted the verdict packet.",
            &reviewer_admission.result,
            &reviewer_worker,
        )?;
    }
    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: typed_verdict.decision.as_str().to_string(),
        summary: typed_verdict.summary.clone(),
        score: typed_verdict.score_payload(),
        evidence: typed_verdict.evidence_payload(&runtime, &reviewer_worker),
    })?;

    let final_status = typed_verdict.decision.contract_status();
    let issue_status = typed_verdict.decision.issue_status();
    store.update_hive_loop_contract_state(
        contract.id,
        final_status,
        "complete",
        contract.current_round,
    )?;
    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(issue_id, issue_status, Some(&typed_verdict.summary))?;
        let admission_summary = format!(
            "{} Workers: explorer={}, developer={}, reviewer={}. Admissions: explorer={}, developer={}, reviewer={}.",
            typed_verdict.summary,
            runtime_worker_summary(&explorer_worker),
            runtime_worker_summary(&runtime_worker),
            runtime_worker_summary(&reviewer_worker),
            explorer_admission.result,
            developer_admission.result,
            reviewer_admission.result
        );
        add_system_comment(
            store,
            issue_id,
            &admission_summary,
            serde_json::json!({
                "loop_id": contract.id,
                "decision": typed_verdict.decision.as_str(),
                "reason_code": typed_verdict.reason_code,
                "phase": "reviewer",
                "runtime_worker": runtime_worker,
                "role_workers": {
                    "explorer": explorer_worker,
                    "developer": runtime_worker,
                    "reviewer": reviewer_worker
                },
                "admissions": {
                    "explorer": explorer_admission.result,
                    "developer": developer_admission.result,
                    "reviewer": reviewer_admission.result
                }
            }),
        )?;
    }

    contract = store
        .get_hive_loop_contract(contract.id)?
        .expect("loop contract should exist after run");
    let mut output = report(store, contract.id)?;
    output.contract = contract;
    Ok(output)
}

pub fn report(store: &Store, loop_id: i64) -> Result<HiveLoopReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issues = store
        .list_hive_issues_for_loop(loop_id)?
        .into_iter()
        .map(|issue| issue_card_from_issue(store, issue))
        .collect::<Result<Vec<_>>>()?;

    Ok(HiveLoopReport {
        policies: store.list_hive_loop_policies(loop_id)?,
        packets: store.list_hive_loop_packets(loop_id)?,
        admissions: store.list_hive_loop_admissions(loop_id)?,
        stages: store.list_hive_loop_stages(loop_id)?,
        evidence: store.list_hive_loop_evidence(loop_id)?,
        verdicts: store.list_hive_loop_verdicts(loop_id)?,
        contract,
        issues,
    })
}

pub fn list(store: &Store) -> Result<Vec<HiveLoopContract>> {
    store.list_hive_loop_contracts()
}

pub fn policy_registry() -> PolicyRegistryReport {
    PolicyRegistryReport {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        gates: all_gate_specs()
            .into_iter()
            .map(PolicyGateSpec::from)
            .collect(),
        runtime: runtime_policy_registry(),
        connector: connector_policy_registry(),
    }
}

#[cfg(test)]
pub fn connector_registry() -> ConnectorRegistryReport {
    connector_registry_with_config(&ConnectorsConfig::default())
}

pub fn connector_registry_with_config(config: &ConnectorsConfig) -> ConnectorRegistryReport {
    let mut file = ConnectorProviderSpec {
        name: "file".to_string(),
        display_name: "File Mirror".to_string(),
        status: "active".to_string(),
        mode: "local-json-mirror".to_string(),
        review_surface_prefixes: vec!["file:".to_string()],
        auth_required: false,
        auth_env: Vec::new(),
        configured: true,
        supports_status: true,
        supports_publish: true,
        supports_readback: true,
        supports_admission: true,
        storage: "connectors/issue-mirrors/*.json".to_string(),
        notes: "Local connector mirror used as the external issue surface dry-run.".to_string(),
    };
    apply_connector_provider_config(&mut file, &config.file);
    let remote_fixture = ConnectorProviderSpec {
        name: "remote-fixture".to_string(),
        display_name: "Remote Fixture".to_string(),
        status: "active".to_string(),
        mode: "remote-issue-api-fixture".to_string(),
        review_surface_prefixes: vec!["remote-fixture:".to_string(), "fixture:".to_string()],
        auth_required: false,
        auth_env: Vec::new(),
        configured: true,
        supports_status: true,
        supports_publish: true,
        supports_readback: true,
        supports_admission: true,
        storage: "connectors/remote-fixture/{external_key}.json".to_string(),
        notes: "File-backed remote issue API fixture for validating remote write/readback contracts; not a third-party connector.".to_string(),
    };
    let mut linear = ConnectorProviderSpec {
        name: "linear".to_string(),
        display_name: "Linear".to_string(),
        status: "planned".to_string(),
        mode: "remote-issue-api".to_string(),
        review_surface_prefixes: vec!["linear:".to_string()],
        auth_required: true,
        auth_env: vec!["LINEAR_API_KEY".to_string()],
        configured: false,
        supports_status: false,
        supports_publish: false,
        supports_readback: false,
        supports_admission: false,
        storage: "not-configured".to_string(),
        notes: "Target provider for real issue/status/comment sync; connector is not active yet."
            .to_string(),
    };
    apply_connector_provider_config(&mut linear, &config.linear);
    let mut github = ConnectorProviderSpec {
        name: "github".to_string(),
        display_name: "GitHub Issues".to_string(),
        status: "planned".to_string(),
        mode: "remote-issue-api".to_string(),
        review_surface_prefixes: vec!["github:".to_string(), "gh:".to_string()],
        auth_required: true,
        auth_env: vec!["GITHUB_TOKEN".to_string(), "GH_TOKEN".to_string()],
        configured: false,
        supports_status: false,
        supports_publish: false,
        supports_readback: false,
        supports_admission: false,
        storage: "not-configured".to_string(),
        notes: "Target provider for GitHub issue mirrors; connector is not active yet.".to_string(),
    };
    apply_connector_provider_config(&mut github, &config.github);

    let providers = vec![
        ConnectorProviderSpec {
            name: "local-hive-panel".to_string(),
            display_name: "Local Hive Panel".to_string(),
            status: "active".to_string(),
            mode: "in-process-issue-board".to_string(),
            review_surface_prefixes: vec!["local-hive-panel".to_string()],
            auth_required: false,
            auth_env: Vec::new(),
            configured: true,
            supports_status: true,
            supports_publish: true,
            supports_readback: true,
            supports_admission: true,
            storage: "sqlite".to_string(),
            notes: "Built-in local issue/status/comment surface; publish/readback are in-process checks."
                .to_string(),
        },
        file,
        remote_fixture,
        linear,
        github,
    ];
    let admission = connector_admission_policy_spec();
    let provider_admissions = providers
        .iter()
        .map(|provider| connector_provider_admission_spec(provider, &admission))
        .collect();

    ConnectorRegistryReport {
        schema_version: CONNECTOR_REGISTRY_SCHEMA_VERSION.to_string(),
        providers,
        admission,
        provider_admissions,
    }
}

fn connector_policy_registry() -> ConnectorPolicyRegistry {
    ConnectorPolicyRegistry {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        admission: connector_admission_policy_spec(),
        retry: connector_retry_policies(),
    }
}

fn connector_admission_policy_spec() -> ConnectorAdmissionPolicySpec {
    let connector_gate = gate_spec(CONNECTOR_MIRROR_RECEIPT_GATE)
        .expect("connector mirror receipt gate should be registered");
    ConnectorAdmissionPolicySpec {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        gate: connector_gate.name.to_string(),
        route_to: "external_issue_surface".to_string(),
        expected_object_kind: connector_gate
            .expected_object_kind
            .unwrap_or(CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND)
            .to_string(),
        check: connector_gate.check.as_str().to_string(),
        required_receipts: connector_gate
            .required_receipts
            .iter()
            .map(|receipt| (*receipt).to_string())
            .collect(),
        required_checks: CONNECTOR_ADMISSION_REQUIRED_CHECKS
            .iter()
            .map(|check| (*check).to_string())
            .collect(),
        check_registry: connector_admission_check_registry(),
        dry_run_command: "entrance hive issue connector-admission <id> --compact".to_string(),
    }
}

fn connector_admission_check_registry() -> Vec<ConnectorAdmissionCheckSpec> {
    CONNECTOR_ADMISSION_CHECK_REGISTRY
        .iter()
        .map(|check| ConnectorAdmissionCheckSpec {
            name: check.name.to_string(),
            severity: check.severity.to_string(),
            owner: check.owner.to_string(),
            required_evidence: check
                .required_evidence
                .iter()
                .map(|evidence| (*evidence).to_string())
                .collect(),
            summary: check.summary.to_string(),
        })
        .collect()
}

pub fn connector_retry_policy_for_provider(provider: &str) -> Option<ConnectorRetryPolicySpec> {
    connector_retry_policies()
        .into_iter()
        .find(|policy| policy.provider == provider)
}

fn connector_retry_policies() -> Vec<ConnectorRetryPolicySpec> {
    vec![
        connector_retry_policy_spec(
            "github",
            "rest",
            &[
                "remote_issue_read",
                "remote_issue_write",
                "remote_comment_read",
                "remote_comment_write",
            ],
            &[
                "retry-after",
                "x-ratelimit-remaining",
                "x-ratelimit-reset",
                "x-ratelimit-resource",
            ],
        ),
        connector_retry_policy_spec(
            "linear",
            "graphql",
            &[
                "remote_issue_read",
                "remote_issue_write",
                "remote_comment_read",
                "remote_comment_write",
            ],
            &[
                "retry-after",
                "x-ratelimit-remaining",
                "x-ratelimit-reset",
                "x-ratelimit-limit",
                "x-rate-limit-remaining",
                "x-rate-limit-reset",
                "x-rate-limit-limit",
            ],
        ),
    ]
}

fn connector_retry_policy_spec(
    provider: &str,
    transport: &str,
    applies_to: &[&str],
    rate_limit_headers: &[&str],
) -> ConnectorRetryPolicySpec {
    ConnectorRetryPolicySpec {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        provider: provider.to_string(),
        transport: transport.to_string(),
        applies_to: applies_to
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        max_attempts: CONNECTOR_RETRY_MAX_ATTEMPTS,
        base_backoff_ms: CONNECTOR_RETRY_BASE_BACKOFF_MS,
        backoff_strategy: "linear".to_string(),
        retryable_http_statuses: vec![500, 502, 503, 504],
        rate_limit_http_statuses: vec![403, 429],
        rate_limit_headers: rate_limit_headers
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        no_immediate_retry_checks: vec!["remote_rate_limited".to_string()],
    }
}

fn apply_connector_provider_config(
    provider: &mut ConnectorProviderSpec,
    config: &ConnectorProviderConfig,
) {
    if !config.auth_env.is_empty() {
        provider.auth_env = config.auth_env.clone();
    }
    if !config.review_surface_prefixes.is_empty() {
        provider.review_surface_prefixes = config.review_surface_prefixes.clone();
    }
    if let Some(storage) = config
        .storage
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        provider.storage = storage.trim().to_string();
    }
    if let Some(mode) = config
        .mode
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        provider.mode = mode.trim().to_string();
    }

    if let Some(enabled) = config.enabled {
        if !enabled {
            provider.status = "disabled".to_string();
            provider.configured = false;
            provider.supports_status = false;
            provider.supports_publish = false;
            provider.supports_readback = false;
            provider.supports_admission = false;
            provider.notes = format!("{} Config disabled in entrance.toml.", provider.notes);
            return;
        }

        provider.configured = connector_provider_auth_configured(provider);
        if provider.configured
            && matches!(provider.name.as_str(), "github" | "linear")
            && provider.mode == "remote-issue-api"
        {
            provider.status = "active".to_string();
            provider.supports_status = true;
            provider.supports_publish = true;
            provider.supports_readback = true;
            provider.supports_admission = true;
            if provider.storage == "not-configured" {
                provider.storage = if provider.name == "github" {
                    "github-rest-api".to_string()
                } else {
                    "linear-graphql-api".to_string()
                };
            }
            provider.notes = if provider.name == "github" {
                "GitHub REST publish/readback connector is active for configured issue targets."
                    .to_string()
            } else {
                "Linear GraphQL publish/readback connector is active for configured issue targets."
                    .to_string()
            };
        }
        provider.notes = format!("{} Configured from entrance.toml.", provider.notes);
    }
}

fn connector_provider_auth_configured(provider: &ConnectorProviderSpec) -> bool {
    if !provider.auth_required {
        return true;
    }
    provider
        .auth_env
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

fn connector_provider_admission_spec(
    provider: &ConnectorProviderSpec,
    admission: &ConnectorAdmissionPolicySpec,
) -> ConnectorProviderAdmissionSpec {
    let mut blockers = Vec::new();
    if provider.status != "active" {
        blockers.push("provider_not_active".to_string());
    }
    if !provider.configured {
        blockers.push("connector_not_configured".to_string());
    }
    if !provider.supports_admission {
        blockers.push("admission_not_supported".to_string());
    }
    let status = if blockers.is_empty() {
        "ready"
    } else {
        "blocked"
    };
    ConnectorProviderAdmissionSpec {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        provider: provider.name.clone(),
        status: status.to_string(),
        gate: admission.gate.clone(),
        route_to: blockers.is_empty().then(|| {
            if provider.name == "local-hive-panel" {
                "local_issue_surface".to_string()
            } else {
                admission.route_to.clone()
            }
        }),
        expected_object_kind: admission.expected_object_kind.clone(),
        check: admission.check.clone(),
        required_receipts: admission.required_receipts.clone(),
        required_checks: admission.required_checks.clone(),
        check_registry: admission.check_registry.clone(),
        blockers,
        dry_run_command: "entrance hive issue connector-admission <id> --compact".to_string(),
    }
}

fn runtime_policy_registry() -> RuntimePolicyRegistry {
    RuntimePolicyRegistry {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        supported: vec![
            RuntimePolicySpec {
                name: "local".to_string(),
                mode: "deterministic-worker".to_string(),
                description: "In-process deterministic worker for local loop smoke tests."
                    .to_string(),
                command: None,
                required_worker_context: Vec::new(),
                sandbox: RuntimeSandboxSpec {
                    filesystem: "in-process".to_string(),
                    network: "none".to_string(),
                    writes_artifacts: false,
                },
            },
            RuntimePolicySpec {
                name: "codex".to_string(),
                mode: "codex-exec".to_string(),
                description: "External codex exec role worker with read-only filesystem sandbox."
                    .to_string(),
                command: Some("codex exec --sandbox read-only -".to_string()),
                required_worker_context: vec![
                    "command".to_string(),
                    "cwd".to_string(),
                    "output_last_message_path".to_string(),
                    "prompt_chars".to_string(),
                ],
                sandbox: RuntimeSandboxSpec {
                    filesystem: "read-only".to_string(),
                    network: "codex-runtime-default".to_string(),
                    writes_artifacts: true,
                },
            },
        ],
        worker: WorkerPolicySpec {
            default_timeout_secs: DEFAULT_WORKER_TIMEOUT_SECS,
            max_timeout_secs: MAX_WORKER_TIMEOUT_SECS,
            timeout_env: "ENTRANCE_HIVE_WORKER_TIMEOUT_SECS".to_string(),
            default_attempts: DEFAULT_WORKER_ATTEMPTS,
            max_attempts: MAX_WORKER_ATTEMPTS,
            attempts_env: "ENTRANCE_HIVE_WORKER_ATTEMPTS".to_string(),
            required_receipt_fields: vec![
                "kind".to_string(),
                "mode".to_string(),
                "role".to_string(),
                "ok".to_string(),
                "timeout_secs".to_string(),
                "attempt_count".to_string(),
                "max_attempts".to_string(),
                "receipt.ok".to_string(),
                "receipt.role".to_string(),
                "receipt.action".to_string(),
                "receipt.evidence_summary".to_string(),
                "receipt.gates".to_string(),
            ],
        },
    }
}

pub fn policies(store: &Store, loop_id: i64) -> Result<HiveLoopPolicyReport> {
    store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let policies = store
        .list_hive_loop_policies(loop_id)?
        .into_iter()
        .map(|policy| HiveLoopPolicyCard {
            gate_spec: gate_spec(&policy.gate).map(PolicyGateSpec::from),
            policy,
        })
        .collect();
    Ok(HiveLoopPolicyReport { loop_id, policies })
}

pub fn trace(store: &Store, loop_id: i64) -> Result<HiveLoopTraceReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issue = store.list_hive_issues_for_loop(loop_id)?.into_iter().next();
    Ok(HiveLoopTraceReport {
        trace: issue_trace_summary(store, loop_id, issue.as_ref())?,
        contract,
        issue,
    })
}

pub fn evidence_report(store: &Store, loop_id: i64) -> Result<HiveLoopEvidenceReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let evidence = store
        .list_hive_loop_evidence(loop_id)?
        .iter()
        .map(|row| issue_evidence_summary(row, &stage_roles))
        .collect();
    Ok(HiveLoopEvidenceReport {
        current_round: contract.current_round,
        contract,
        evidence,
    })
}

pub fn audit(store: &Store, loop_id: i64) -> Result<HiveLoopAuditReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let policies = store.list_hive_loop_policies(loop_id)?;
    let packets = store.list_hive_loop_packets(loop_id)?;
    let admissions = store.list_hive_loop_admissions(loop_id)?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let verdicts = store.list_hive_loop_verdicts(loop_id)?;
    let issues = store.list_hive_issues_for_loop(loop_id)?;
    let evidence = store.list_hive_loop_evidence(loop_id)?;
    let schema_status = store.schema_status()?;
    let packet_by_id = packets
        .iter()
        .map(|packet| (packet.id, packet))
        .collect::<HashMap<_, _>>();

    let active_policies = policies
        .iter()
        .filter(|policy| policy.status == "active")
        .collect::<Vec<_>>();
    let policy_errors = active_policy_audit_errors(&active_policies);
    let stage_sequence_errors = stage_sequence_audit_errors(&contract, &stages, &evidence);
    let packet_sequence_errors = packet_sequence_audit_errors(&packets);
    let packet_errors = packets
        .iter()
        .filter_map(|packet| {
            let mut errors = typed_packet_envelope_errors(&packet.payload);
            errors.extend(packet_row_binding_errors(packet));
            if errors.is_empty() {
                None
            } else {
                Some(serde_json::json!({
                    "packet_id": packet.id,
                    "object_kind": packet.object_kind,
                    "errors": errors
                }))
            }
        })
        .collect::<Vec<_>>();
    let mut admission_errors = admissions
        .iter()
        .filter_map(|admission| admission_audit_errors(admission, &packet_by_id))
        .collect::<Vec<_>>();
    admission_errors.extend(packet_admission_audit_errors(&packets, &admissions));
    let worker_errors = packets
        .iter()
        .filter_map(worker_receipt_audit_errors)
        .collect::<Vec<_>>();
    let runtime_policy_errors = runtime_policy_audit_errors(&contract, &packets);
    let mut verdict_errors = verdicts
        .iter()
        .filter_map(verdict_audit_errors)
        .collect::<Vec<_>>();
    verdict_errors.extend(verdict_sequence_audit_errors(&contract, &verdicts));
    verdict_errors.extend(verdict_evidence_binding_audit_errors(
        &contract,
        &verdicts,
        &packets,
        &admissions,
        &evidence,
    ));
    let issue_surface = issue_surface_audit(store, &contract, &issues, &evidence)?;

    let mut stage_evidence_errors = stage_evidence_audit_errors(&contract, &stages, &evidence);
    stage_evidence_errors.extend(evidence_worker_policy_audit_errors(&stages, &evidence));
    let checks = vec![
        audit_check(
            "contract_loaded",
            true,
            format!("Loop #{} `{}` loaded.", contract.id, contract.title),
            serde_json::json!({
                "status": contract.status,
                "active_phase": contract.active_phase,
                "current_round": contract.current_round
            }),
        ),
        store_schema_audit_check(&schema_status),
        audit_check(
            "active_policy_registry",
            policy_errors.is_empty(),
            format!(
                "{} active policies inspected; {} policy contract issues.",
                active_policies.len(),
                policy_errors.len()
            ),
            serde_json::json!({
                "active_policy_count": active_policies.len(),
                "expected_policy_count": DEFAULT_LOOP_POLICIES.len(),
                "policy_errors": policy_errors
            }),
        ),
        audit_check(
            "stage_sequence",
            stage_sequence_errors.is_empty(),
            format!(
                "{} stages inspected; {} stage sequence issues.",
                stages.len(),
                stage_sequence_errors.len()
            ),
            serde_json::json!({ "stage_sequence_errors": stage_sequence_errors }),
        ),
        audit_check(
            "stage_evidence",
            stage_evidence_errors.is_empty(),
            format!(
                "{} evidence rows inspected; {} stage evidence issues.",
                evidence.len(),
                stage_evidence_errors.len()
            ),
            serde_json::json!({ "stage_evidence_errors": stage_evidence_errors }),
        ),
        audit_check(
            "packet_sequence",
            packet_sequence_errors.is_empty(),
            format!(
                "{} packets inspected; {} route cardinality issues.",
                packets.len(),
                packet_sequence_errors.len()
            ),
            serde_json::json!({ "packet_sequence_errors": packet_sequence_errors }),
        ),
        audit_check(
            "packet_envelopes",
            packet_errors.is_empty(),
            format!(
                "{} packets inspected; {} envelope or row-binding issues.",
                packets.len(),
                packet_errors.len()
            ),
            serde_json::json!({ "packet_errors": packet_errors }),
        ),
        audit_check(
            "admission_receipts",
            admission_errors.is_empty(),
            format!(
                "{} admissions inspected; {} receipt issues.",
                admissions.len(),
                admission_errors.len()
            ),
            serde_json::json!({ "admission_errors": admission_errors }),
        ),
        audit_check(
            "worker_receipts",
            worker_errors.is_empty(),
            format!(
                "{} packets inspected; {} worker receipt issues.",
                packets.len(),
                worker_errors.len()
            ),
            serde_json::json!({ "worker_errors": worker_errors }),
        ),
        audit_check(
            "runtime_policy",
            runtime_policy_errors.is_empty(),
            format!(
                "Runtime `{}` and current-round worker receipts inspected; {} runtime policy issues.",
                contract.runtime,
                runtime_policy_errors.len()
            ),
            serde_json::json!({
                "current_round": contract.current_round,
                "supported_runtimes": runtime_policy_registry()
                    .supported
                    .iter()
                    .map(|runtime| runtime.name.clone())
                    .collect::<Vec<_>>(),
                "runtime_policy_errors": runtime_policy_errors
            }),
        ),
        audit_check(
            "verdict_packets",
            verdict_errors.is_empty(),
            format!(
                "{} verdicts inspected; {} verdict issues.",
                verdicts.len(),
                verdict_errors.len()
            ),
            serde_json::json!({ "verdict_errors": verdict_errors }),
        ),
        audit_check(
            "issue_surface",
            issue_surface.errors.is_empty(),
            format!(
                "{} linked issues, {} comments, {} actions, and {} operator evidence rows inspected; {} issue surface issues.",
                issues.len(),
                issue_surface.comment_count,
                issue_surface.action_count,
                issue_surface.operator_evidence_count,
                issue_surface.errors.len()
            ),
            serde_json::json!({
                "issue_ids": issues.iter().map(|issue| issue.id).collect::<Vec<_>>(),
                "action_count": issue_surface.action_count,
                "comment_count": issue_surface.comment_count,
                "operator_evidence_count": issue_surface.operator_evidence_count,
                "issue_surface_errors": issue_surface.errors
            }),
        ),
    ];
    let failed_count = checks.iter().filter(|check| !check.passed).count();
    Ok(HiveLoopAuditReport {
        schema_version: AUDIT_SCHEMA_VERSION.to_string(),
        loop_id,
        passed: failed_count == 0,
        failed_count,
        checks,
    })
}

pub fn doctor(store: &Store, loop_id: i64) -> Result<HiveLoopDoctorReport> {
    let trace_report = trace(store, loop_id)?;
    let audit_report = audit(store, loop_id)?;
    let contract = trace_report.contract;
    let issue_id = trace_report.issue.as_ref().map(|issue| issue.id);
    let issue_status = trace_report
        .issue
        .as_ref()
        .map(|issue| issue.status.clone());
    let trace_summary = trace_report.trace;
    let counts = doctor_counts(&trace_summary);
    let failed_checks = audit_report
        .checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.name.clone())
        .collect::<Vec<_>>();
    let audit_failure_details = audit_failure_details(&audit_report);
    let missing_receipts = doctor_missing_receipts(&trace_summary);
    let worker_failures = doctor_worker_failures(&trace_summary);
    let health = doctor_health(
        &contract.status,
        issue_status.as_deref(),
        trace_summary.last_decision.as_deref(),
        audit_report.passed,
        !worker_failures.is_empty(),
    )
    .to_string();
    let summary = doctor_summary(
        &contract,
        issue_status.as_deref(),
        &trace_summary,
        audit_report.passed,
        audit_report.failed_count,
        &health,
    );
    let next_actions = doctor_next_actions(
        &health,
        contract.id,
        issue_id,
        &contract.runtime,
        audit_report.passed,
    );
    let checks = audit_report
        .checks
        .into_iter()
        .map(|check| HiveLoopDoctorCheck {
            name: check.name,
            passed: check.passed,
            summary: check.summary,
        })
        .collect();

    Ok(HiveLoopDoctorReport {
        schema_version: DOCTOR_SCHEMA_VERSION.to_string(),
        loop_id: contract.id,
        health,
        summary,
        next_actions,
        status: contract.status,
        active_phase: contract.active_phase,
        current_round: contract.current_round,
        runtime: contract.runtime,
        issue_id,
        issue_status,
        decision: trace_summary.last_decision.clone(),
        reason_code: trace_summary.reason_code.clone(),
        counts,
        failed_checks,
        audit_failure_details,
        missing_receipts,
        worker_failures,
        checks,
        trace: trace_summary,
    })
}

fn store_schema_audit_check(status: &StoreSchemaStatus) -> HiveLoopAuditCheck {
    let present_table_count = status.tables.iter().filter(|table| table.present).count();
    let present_index_count = status.indexes.iter().filter(|index| index.present).count();
    let errors = store_schema_audit_errors(status);
    let health = if status.healthy {
        "healthy"
    } else {
        "unhealthy"
    };
    audit_check(
        "store_schema",
        status.healthy,
        format!(
            "SQLite ledger schema is {health}: user_version {}/{}; tables {}/{}; indexes {}/{}.",
            status.user_version,
            status.expected_user_version,
            present_table_count,
            status.tables.len(),
            present_index_count,
            status.indexes.len()
        ),
        serde_json::json!({
            "schema_version": status.schema_version,
            "db_path": status.db_path,
            "user_version": status.user_version,
            "expected_user_version": status.expected_user_version,
            "present_table_count": present_table_count,
            "expected_table_count": status.tables.len(),
            "present_index_count": present_index_count,
            "expected_index_count": status.indexes.len(),
            "missing_tables": &status.missing_tables,
            "missing_columns": &status.missing_columns,
            "missing_indexes": &status.missing_indexes,
            "errors": errors
        }),
    )
}

fn store_schema_audit_errors(status: &StoreSchemaStatus) -> Vec<&'static str> {
    let mut errors = Vec::new();
    if status.user_version < status.expected_user_version {
        errors.push("schema.user_version");
    }
    if !status.missing_tables.is_empty() {
        errors.push("schema.missing_tables");
    }
    if !status.missing_columns.is_empty() {
        errors.push("schema.missing_columns");
    }
    if !status.missing_indexes.is_empty() {
        errors.push("schema.missing_indexes");
    }
    if !status.healthy && errors.is_empty() {
        errors.push("schema.healthy");
    }
    errors
}

fn audit_failure_details(report: &HiveLoopAuditReport) -> Vec<String> {
    let mut details = Vec::new();
    for check in report.checks.iter().filter(|check| !check.passed) {
        let before = details.len();
        collect_audit_failure_details(&check.name, &check.details, &mut details);
        if details.len() == before {
            details.push(check.name.clone());
        }
    }
    details.sort();
    details.dedup();
    details
}

fn collect_audit_failure_details(
    prefix: &str,
    value: &serde_json::Value,
    details: &mut Vec<String>,
) {
    if let Some(values) = value.as_array() {
        for value in values {
            collect_audit_failure_details(prefix, value, details);
        }
        return;
    }

    let Some(object) = value.as_object() else {
        return;
    };
    let scoped_prefix = object
        .get("scope")
        .and_then(|value| value.as_str())
        .map(|scope| format!("{prefix}:{scope}"))
        .unwrap_or_else(|| prefix.to_string());
    if let Some(errors) = object.get("errors").and_then(|value| value.as_array()) {
        for error in errors.iter().filter_map(|value| value.as_str()) {
            details.push(format!("{scoped_prefix}:{error}"));
        }
    }
    for (key, value) in object {
        if key != "errors" {
            collect_audit_failure_details(&scoped_prefix, value, details);
        }
    }
}

fn doctor_counts(trace: &IssueTraceSummary) -> HiveLoopDoctorCounts {
    HiveLoopDoctorCounts {
        packet_count: trace.packet_count,
        admission_count: trace.admission_count,
        evidence_count: trace.evidence_count,
        verdict_count: trace.verdict_count,
        round_packet_count: trace.round_packet_count,
        round_admission_count: trace.round_admission_count,
        round_evidence_count: trace.round_evidence_count,
        round_verdict_count: trace.round_verdict_count,
        receipt_required_count: trace.receipt_required_count,
        receipt_missing_count: trace.receipt_missing_count,
        round_receipt_required_count: trace.round_receipt_required_count,
        round_receipt_missing_count: trace.round_receipt_missing_count,
        role_worker_count: trace.role_worker_count,
        role_worker_ok_count: trace.role_worker_ok_count,
        round_role_worker_count: trace.round_role_worker_count,
        round_role_worker_ok_count: trace.round_role_worker_ok_count,
        round_worker_duration_ms: trace.round_worker_duration_ms,
        round_worker_timeout_count: trace.round_worker_timeout_count,
        round_worker_retry_exhausted_count: trace.round_worker_retry_exhausted_count,
        audit_failed_count: trace.audit_failed_count,
    }
}

fn doctor_missing_receipts(trace: &IssueTraceSummary) -> Vec<String> {
    let mut receipts = trace
        .evidence
        .iter()
        .flat_map(|evidence| evidence.missing_receipts.iter().cloned())
        .collect::<Vec<_>>();
    receipts.sort();
    receipts.dedup();
    receipts
}

fn doctor_worker_failures(trace: &IssueTraceSummary) -> Vec<String> {
    trace
        .evidence
        .iter()
        .filter(|evidence| {
            evidence.worker_ok == Some(false)
                || evidence.worker_receipt_ok == Some(false)
                || evidence.worker_timed_out == Some(true)
                || evidence.worker_retry_exhausted == Some(true)
                || !evidence.worker_receipt_errors.is_empty()
        })
        .map(|evidence| {
            let receipt_suffix = if evidence.worker_receipt_errors.is_empty() {
                String::new()
            } else {
                format!(
                    " receipt_errors={}",
                    evidence.worker_receipt_errors.join("|")
                )
            };
            format!(
                "{}:{} worker={} ok={} receipt={}{}{}",
                evidence.stage_role.as_deref().unwrap_or("loop"),
                evidence.kind,
                evidence.worker_kind.as_deref().unwrap_or("unknown"),
                doctor_bool_label(evidence.worker_ok),
                doctor_bool_label(evidence.worker_receipt_ok),
                if evidence.worker_retry_exhausted == Some(true) {
                    " retry_exhausted"
                } else if evidence.worker_timed_out == Some(true) {
                    " timeout"
                } else {
                    ""
                },
                receipt_suffix
            )
        })
        .collect()
}

fn doctor_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

fn doctor_health(
    contract_status: &str,
    issue_status: Option<&str>,
    decision: Option<&str>,
    audit_passed: bool,
    has_worker_failures: bool,
) -> &'static str {
    if contract_status == "needs-review"
        || issue_status == Some("Needs Review")
        || decision == Some("needs-review")
    {
        return "needs_review";
    }
    if contract_status == "blocked"
        || issue_status == Some("Blocked")
        || decision == Some("blocked")
    {
        return "blocked";
    }
    if contract_status == "rejected"
        || issue_status == Some("Canceled")
        || decision == Some("reject")
    {
        return "rejected";
    }
    if !audit_passed {
        return "audit_failed";
    }
    if has_worker_failures {
        return "worker_failed";
    }
    if contract_status == "kept" && decision == Some("keep") {
        return "ok";
    }
    if contract_status == "todo" || decision.is_none() {
        return "pending";
    }
    "unknown"
}

fn doctor_summary(
    contract: &HiveLoopContract,
    issue_status: Option<&str>,
    trace: &IssueTraceSummary,
    audit_passed: bool,
    audit_failed_count: usize,
    health: &str,
) -> String {
    let health_label = doctor_health_label(health);
    let audit_state = if audit_passed {
        "audit ok".to_string()
    } else {
        format!("audit failed {audit_failed_count} checks")
    };
    format!(
        "Loop #{} is {health_label} at {} round {}; issue {}; decision {}; {}; workers {}/{} current round; receipts missing {}/{} current round; worker time {} current round.",
        contract.id,
        contract.active_phase,
        contract.current_round,
        issue_status.unwrap_or("none"),
        trace.last_decision.as_deref().unwrap_or("pending"),
        audit_state,
        trace.round_role_worker_ok_count,
        trace.round_role_worker_count,
        trace.round_receipt_missing_count,
        trace.round_receipt_required_count,
        worker_duration_summary(trace.round_worker_duration_ms)
    )
}

fn doctor_health_label(health: &str) -> &str {
    match health {
        "needs_review" => "needs review",
        "audit_failed" => "audit failed",
        "worker_failed" => "worker failed",
        other => other,
    }
}

fn worker_duration_summary(duration_ms: u64) -> String {
    if duration_ms >= 1000 {
        format!("{:.1}s", duration_ms as f64 / 1000.0)
    } else {
        format!("{duration_ms}ms")
    }
}

fn doctor_next_actions(
    health: &str,
    loop_id: i64,
    issue_id: Option<i64>,
    runtime: &str,
    audit_passed: bool,
) -> Vec<String> {
    let mut actions = Vec::new();
    if !audit_passed {
        actions.push(compact_audit_command(loop_id));
        actions.push(format!("entrance hive loop evidence {loop_id}"));
    }
    match health {
        "ok" => {
            actions.push(compact_audit_command(loop_id));
            actions.push(format!("entrance hive loop trace {loop_id}"));
            actions.push(format!("entrance hive loop evidence {loop_id}"));
        }
        "pending" => {
            actions.push(pending_run_command(loop_id, issue_id, runtime));
        }
        "blocked" => {
            actions.push(format!("entrance hive loop evidence {loop_id}"));
            if let Some(issue_id) = issue_id {
                actions.push(retry_run_command(issue_id, runtime));
                actions.push(format!(
                    "entrance hive issue decide {issue_id} request-review --body <note> --compact"
                ));
            }
        }
        "needs_review" => {
            if let Some(issue_id) = issue_id {
                actions.push(format!("entrance hive issue show {issue_id} --compact"));
                actions.push(retry_run_command(issue_id, runtime));
            }
        }
        "rejected" => {
            if let Some(issue_id) = issue_id {
                actions.push(format!("entrance hive issue show {issue_id} --compact"));
            }
        }
        "audit_failed" => {
            actions.push(compact_audit_command(loop_id));
            actions.push(format!("entrance hive loop evidence {loop_id}"));
        }
        "worker_failed" => {
            actions.push(format!("entrance hive loop evidence {loop_id}"));
            actions.push(format!("entrance hive loop doctor {loop_id}"));
            if let Some(issue_id) = issue_id {
                actions.push(retry_run_command(issue_id, runtime));
            }
        }
        _ => {
            actions.push(format!("entrance hive loop show {loop_id}"));
            actions.push(format!("entrance hive loop trace {loop_id}"));
        }
    }
    let mut deduped = Vec::new();
    for action in actions {
        if !deduped.contains(&action) {
            deduped.push(action);
        }
    }
    deduped
}

fn compact_audit_command(loop_id: i64) -> String {
    format!("entrance hive loop audit {loop_id} --compact")
}

fn pending_run_command(loop_id: i64, issue_id: Option<i64>, runtime: &str) -> String {
    match issue_id {
        Some(issue_id) => {
            format!("entrance hive issue run {issue_id} --runtime {runtime} --compact")
        }
        None => format!("entrance hive loop run {loop_id} --runtime {runtime} --compact"),
    }
}

fn audit_check(
    name: &str,
    passed: bool,
    summary: String,
    details: serde_json::Value,
) -> HiveLoopAuditCheck {
    HiveLoopAuditCheck {
        name: name.to_string(),
        passed,
        summary,
        details,
    }
}

fn retry_run_command(issue_id: i64, runtime: &str) -> String {
    if runtime == "codex" {
        return format!(
            "entrance hive issue retry-run {issue_id} --body <note> --runtime codex --worker-attempts 2 --compact"
        );
    }
    format!("entrance hive issue retry-run {issue_id} --body <note> --compact")
}

fn stage_sequence_audit_errors(
    contract: &HiveLoopContract,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let mut groups: HashMap<(i64, &str), Vec<&HiveLoopStage>> = HashMap::new();
    for stage in stages {
        let mut row_errors = Vec::new();
        if stage.round < 1 || stage.round > contract.current_round {
            row_errors.push("stage.round");
        }
        if !known_stage_roles().contains(&stage.role.as_str()) {
            row_errors.push("stage.role");
        }
        if stage.status != "done" {
            row_errors.push("stage.status");
        }
        if !row_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "stage_row",
                "stage_id": stage.id,
                "round": stage.round,
                "role": stage.role,
                "status": stage.status,
                "current_round": contract.current_round,
                "errors": row_errors
            }));
        }
        groups
            .entry((stage.round, stage.role.as_str()))
            .or_default()
            .push(stage);
    }

    for ((round, role), role_stages) in groups {
        if role_stages.len() > 1 {
            errors.push(serde_json::json!({
                "scope": "stage_role",
                "round": round,
                "role": role,
                "stage_ids": role_stages.iter().map(|stage| stage.id).collect::<Vec<_>>(),
                "errors": ["stage.role_duplicate"]
            }));
        }
    }

    let admission_rejection_role =
        current_round_admission_rejection_role(contract, stages, evidence);
    let expected_roles =
        expected_stage_roles_for_contract(contract, admission_rejection_role.as_deref(), stages);
    if !expected_roles.is_empty() {
        let missing_roles = expected_roles
            .iter()
            .copied()
            .filter(|role| {
                !stages.iter().any(|stage| {
                    stage.round == contract.current_round && stage.role.as_str() == *role
                })
            })
            .collect::<Vec<_>>();
        if !missing_roles.is_empty() {
            errors.push(serde_json::json!({
                "scope": "stage_round",
                "round": contract.current_round,
                "status": contract.status,
                "active_phase": contract.active_phase,
                "expected_roles": expected_roles,
                "missing_roles": missing_roles,
                "errors": ["stage.role_missing"]
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            error
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn canonical_stage_roles() -> &'static [&'static str] {
    CURRENT_LOOP_ROLES
}

fn legacy_stage_roles() -> &'static [&'static str] {
    LEGACY_LOOP_ROLES
}

fn known_stage_roles() -> &'static [&'static str] {
    &["explorer", "developer", "reviewer", "doer", "evaluator"]
}

fn expected_stage_roles_for_contract(
    contract: &HiveLoopContract,
    admission_rejection_role: Option<&str>,
    stages: &[HiveLoopStage],
) -> Vec<&'static str> {
    let roles = stage_role_family_for_contract(contract, admission_rejection_role, stages);
    match contract.status.as_str() {
        "kept" | "rejected" => roles.to_vec(),
        "needs-review"
            if contract.active_phase == "human-review" && admission_rejection_role.is_some() =>
        {
            expected_stage_roles_through(admission_rejection_role.unwrap_or_default())
        }
        "needs-review" => roles.to_vec(),
        "blocked" => match contract.active_phase.as_str() {
            _ if admission_rejection_role.is_some() => {
                expected_stage_roles_through(admission_rejection_role.unwrap_or_default())
            }
            "explorer" => vec!["explorer"],
            "developer" => vec!["explorer", "developer"],
            "reviewer" => canonical_stage_roles().to_vec(),
            "doer" => vec!["explorer", "doer"],
            "evaluator" => legacy_stage_roles().to_vec(),
            "complete" | "human-review" => roles.to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn stage_role_family_for_contract(
    contract: &HiveLoopContract,
    admission_rejection_role: Option<&str>,
    stages: &[HiveLoopStage],
) -> &'static [&'static str] {
    if matches!(
        admission_rejection_role,
        Some("developer") | Some("reviewer")
    ) || matches!(contract.active_phase.as_str(), "developer" | "reviewer")
        || stages
            .iter()
            .any(|stage| matches!(stage.role.as_str(), "developer" | "reviewer"))
    {
        return canonical_stage_roles();
    }
    if matches!(admission_rejection_role, Some("doer") | Some("evaluator"))
        || matches!(contract.active_phase.as_str(), "doer" | "evaluator")
        || stages
            .iter()
            .any(|stage| matches!(stage.role.as_str(), "doer" | "evaluator"))
    {
        return legacy_stage_roles();
    }
    canonical_stage_roles()
}

fn expected_stage_roles_through(role: &str) -> Vec<&'static str> {
    match role {
        "explorer" => vec!["explorer"],
        "developer" => vec!["explorer", "developer"],
        "reviewer" => canonical_stage_roles().to_vec(),
        "doer" => vec!["explorer", "doer"],
        "evaluator" => legacy_stage_roles().to_vec(),
        _ => Vec::new(),
    }
}

fn current_round_admission_rejection_role(
    contract: &HiveLoopContract,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Option<String> {
    let stages_by_id = stages
        .iter()
        .map(|stage| (stage.id, stage))
        .collect::<HashMap<_, _>>();
    evidence.iter().find_map(|row| {
        if row.round != contract.current_round || row.kind != "admission_rejection" {
            return None;
        }
        row.stage_id
            .and_then(|stage_id| stages_by_id.get(&stage_id))
            .map(|stage| stage.role.clone())
            .or_else(|| {
                row.payload
                    .get("phase")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned)
            })
    })
}

fn stage_evidence_audit_errors(
    contract: &HiveLoopContract,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let stages_by_id = stages
        .iter()
        .map(|stage| (stage.id, stage))
        .collect::<HashMap<_, _>>();
    let mut stage_evidence_groups: HashMap<(i64, &str), Vec<&HiveLoopEvidence>> = HashMap::new();

    let admission_rejection_role =
        current_round_admission_rejection_role(contract, stages, evidence);

    for row in evidence {
        match row.stage_id {
            Some(stage_id) => {
                let mut row_errors = Vec::new();
                if row.round < 1 || row.round > contract.current_round {
                    row_errors.push("evidence.round");
                }
                if let Some(stage) = stages_by_id.get(&stage_id) {
                    if row.round != stage.round {
                        row_errors.push("evidence.stage_round");
                    }
                    let expected_kind = expected_stage_evidence_kind(
                        contract,
                        stage,
                        admission_rejection_role.as_deref(),
                    );
                    if stage.round == contract.current_round
                        && expected_kind.is_some_and(|expected| expected != row.kind)
                    {
                        row_errors.push("evidence.kind");
                    } else if !stage_evidence_kind_allowed_for_role(&stage.role, &row.kind) {
                        row_errors.push("evidence.kind");
                    }
                } else {
                    row_errors.push("evidence.stage_link");
                }
                if !row_errors.is_empty() {
                    errors.push(serde_json::json!({
                        "scope": "evidence_row",
                        "evidence_id": row.id,
                        "stage_id": stage_id,
                        "round": row.round,
                        "kind": row.kind,
                        "errors": row_errors
                    }));
                }
                stage_evidence_groups
                    .entry((stage_id, row.kind.as_str()))
                    .or_default()
                    .push(row);
            }
            None if stage_bound_evidence_kind(&row.kind) => {
                errors.push(serde_json::json!({
                    "scope": "evidence_row",
                    "evidence_id": row.id,
                    "round": row.round,
                    "kind": row.kind,
                    "errors": ["evidence.stage_id"]
                }));
            }
            None => {}
        }
    }

    for ((stage_id, kind), rows) in stage_evidence_groups {
        if rows.len() > 1 {
            let stage = stages_by_id.get(&stage_id);
            errors.push(serde_json::json!({
                "scope": "evidence_stage",
                "stage_id": stage_id,
                "round": stage.map(|stage| stage.round),
                "role": stage.map(|stage| stage.role.as_str()),
                "kind": kind,
                "evidence_ids": rows.iter().map(|row| row.id).collect::<Vec<_>>(),
                "errors": ["evidence.stage_duplicate"]
            }));
        }
    }

    let expected_roles =
        expected_stage_roles_for_contract(contract, admission_rejection_role.as_deref(), stages);
    for stage in stages.iter().filter(|stage| {
        stage.round == contract.current_round
            && expected_roles
                .iter()
                .any(|role| *role == stage.role.as_str())
    }) {
        let Some(expected_kind) =
            expected_stage_evidence_kind(contract, stage, admission_rejection_role.as_deref())
        else {
            continue;
        };
        if !evidence
            .iter()
            .any(|row| row.stage_id == Some(stage.id) && row.kind == expected_kind)
        {
            errors.push(serde_json::json!({
                "scope": "stage",
                "stage_id": stage.id,
                "round": stage.round,
                "role": stage.role,
                "expected_kind": expected_kind,
                "errors": ["evidence.stage_missing"]
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            error
                .get("stage_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("kind")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn evidence_worker_policy_audit_errors(
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let registry = runtime_policy_registry();
    let stages_by_id = stages
        .iter()
        .map(|stage| (stage.id, stage))
        .collect::<HashMap<_, _>>();
    let mut errors = Vec::new();

    for row in evidence.iter().filter(|row| {
        matches!(
            row.kind.as_str(),
            "exploration_packet" | "execution_packet" | "verdict_packet"
        )
    }) {
        let Some(stage) = row
            .stage_id
            .and_then(|stage_id| stages_by_id.get(&stage_id))
        else {
            continue;
        };
        let row_errors = match row.payload.get("worker") {
            Some(worker) => runtime_worker_policy_errors(&registry, worker, &stage.role),
            None => vec!["worker".to_string()],
        };
        if !row_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "evidence_worker",
                "evidence_id": row.id,
                "stage_id": stage.id,
                "round": row.round,
                "role": stage.role,
                "kind": row.kind,
                "errors": row_errors
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("stage_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("evidence_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
        )
    });
    errors
}

fn expected_stage_evidence_kind(
    contract: &HiveLoopContract,
    stage: &HiveLoopStage,
    admission_rejection_role: Option<&str>,
) -> Option<&'static str> {
    if stage.round == contract.current_round
        && admission_rejection_role.is_some_and(|role| role == stage.role.as_str())
    {
        return Some("admission_rejection");
    }
    if contract.status == "blocked"
        && stage.round == contract.current_round
        && contract.active_phase == stage.role
        && matches!(
            contract.active_phase.as_str(),
            "explorer" | "developer" | "reviewer" | "doer" | "evaluator"
        )
    {
        return Some("admission_rejection");
    }
    canonical_stage_evidence_kind(&stage.role)
}

fn canonical_stage_evidence_kind(role: &str) -> Option<&'static str> {
    match role {
        "explorer" => Some("exploration_packet"),
        "developer" => Some("execution_packet"),
        "reviewer" => Some("verdict_packet"),
        "doer" => Some("execution_packet"),
        "evaluator" => Some("verdict_packet"),
        _ => None,
    }
}

fn stage_evidence_kind_allowed_for_role(role: &str, kind: &str) -> bool {
    canonical_stage_evidence_kind(role) == Some(kind) || kind == "admission_rejection"
}

fn stage_bound_evidence_kind(kind: &str) -> bool {
    matches!(
        kind,
        "exploration_packet" | "execution_packet" | "verdict_packet" | "admission_rejection"
    )
}

fn packet_sequence_audit_errors(packets: &[HiveLoopPacket]) -> Vec<serde_json::Value> {
    let mut groups: HashMap<(i64, &str, &str, &str, &str), Vec<&HiveLoopPacket>> = HashMap::new();
    for packet in packets {
        groups
            .entry((
                packet.round,
                packet.object_kind.as_str(),
                packet.writer_role.as_str(),
                packet.route_from.as_str(),
                packet.route_to.as_str(),
            ))
            .or_default()
            .push(packet);
    }

    let mut errors = groups
        .into_iter()
        .filter_map(
            |((round, object_kind, writer_role, route_from, route_to), packets)| {
                (packets.len() > 1).then(|| {
                    serde_json::json!({
                        "scope": "packet_route",
                        "round": round,
                        "object_kind": object_kind,
                        "writer_role": writer_role,
                        "route_from": route_from,
                        "route_to": route_to,
                        "packet_ids": packets.iter().map(|packet| packet.id).collect::<Vec<_>>(),
                        "errors": ["packet.route_duplicate"]
                    })
                })
            },
        )
        .collect::<Vec<_>>();
    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("object_kind")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn packet_row_binding_errors(packet: &HiveLoopPacket) -> Vec<String> {
    let payload = &packet.payload;
    let mut errors = Vec::new();
    if payload
        .get("loop_id")
        .and_then(|value| value.as_i64())
        .is_some_and(|value| value != packet.loop_id)
    {
        errors.push("row.loop_id".to_string());
    }
    if payload
        .get("round")
        .and_then(|value| value.as_i64())
        .is_some_and(|value| value != packet.round)
    {
        errors.push("row.round".to_string());
    }
    if packet_object_kind(payload).is_some_and(|value| value != packet.object_kind.as_str()) {
        errors.push("row.object_kind".to_string());
    }
    if payload
        .pointer("/writer/role")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.writer_role.as_str())
    {
        errors.push("row.writer_role".to_string());
    }
    if payload
        .pointer("/route/from")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.route_from.as_str())
    {
        errors.push("row.route_from".to_string());
    }
    if payload
        .pointer("/route/to")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.route_to.as_str())
    {
        errors.push("row.route_to".to_string());
    }
    if payload
        .get("state_code")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.state_code.as_str())
    {
        errors.push("row.state_code".to_string());
    }
    errors
}

fn packet_admission_audit_errors(
    packets: &[HiveLoopPacket],
    admissions: &[HiveLoopAdmission],
) -> Vec<serde_json::Value> {
    let mut admission_counts = HashMap::new();
    for admission in admissions {
        *admission_counts
            .entry(admission.packet_id)
            .or_insert(0usize) += 1;
    }

    packets
        .iter()
        .filter_map(|packet| {
            let count = admission_counts
                .get(&packet.id)
                .copied()
                .unwrap_or_default();
            let errors = match count {
                0 => vec!["packet.admission_missing"],
                1 => Vec::new(),
                _ => vec!["packet.admission_duplicate"],
            };
            (!errors.is_empty()).then(|| {
                serde_json::json!({
                    "scope": "packet_admission",
                    "packet_id": packet.id,
                    "round": packet.round,
                    "object_kind": packet.object_kind,
                    "writer_role": packet.writer_role,
                    "admission_count": count,
                    "errors": errors
                })
            })
        })
        .collect()
}

fn admission_audit_errors(
    admission: &HiveLoopAdmission,
    packet_by_id: &HashMap<i64, &HiveLoopPacket>,
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let packet = packet_by_id.get(&admission.packet_id).copied();
    if admission
        .policy
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(ADMISSION_SCHEMA_VERSION)
    {
        errors.push("schema_version".to_string());
    }
    if packet.is_none() {
        errors.push("packet.link".to_string());
    }
    let envelope_valid = admission
        .policy
        .pointer("/packet/envelope/valid")
        .and_then(|value| value.as_bool());
    if envelope_valid != Some(true) {
        errors.push("packet.envelope".to_string());
    }
    if admission
        .policy
        .get("result")
        .and_then(|value| value.as_str())
        != Some(admission.result.as_str())
    {
        errors.push("result.binding".to_string());
    }
    if !matches!(admission.result.as_str(), "admitted" | "rejected") {
        errors.push("result.value".to_string());
    }

    let gate_name = admission
        .policy
        .pointer("/gate/name")
        .and_then(|value| value.as_str());
    if let Some(gate_name) = gate_name {
        if gate_spec(gate_name).is_none() {
            errors.push("gate.unknown".to_string());
        }
    }
    let gate_passed = admission
        .policy
        .pointer("/gate/passed")
        .and_then(|value| value.as_bool());
    let policy_missing = admission
        .policy
        .get("policy")
        .map_or(true, serde_json::Value::is_null);
    if policy_missing && admission.result == "admitted" {
        errors.push("policy.missing".to_string());
    }
    if let Some(packet) = packet {
        if packet.loop_id != admission.loop_id {
            errors.push("packet.loop_id".to_string());
        }
        if admission
            .policy
            .pointer("/packet/id")
            .and_then(|value| value.as_i64())
            != Some(packet.id)
        {
            errors.push("packet.id".to_string());
        }
        if admission_field(&admission.policy, "/packet/object_kind")
            != Some(packet.object_kind.as_str())
        {
            errors.push("packet.object_kind".to_string());
        }
        if admission_field(&admission.policy, "/packet/writer_role")
            != Some(packet.writer_role.as_str())
        {
            errors.push("packet.writer_role".to_string());
        }
        if admission_field(&admission.policy, "/packet/route_from")
            != Some(packet.route_from.as_str())
        {
            errors.push("packet.route_from".to_string());
        }
        if admission_field(&admission.policy, "/packet/route_to") != Some(packet.route_to.as_str())
        {
            errors.push("packet.route_to".to_string());
        }
        if admission_field(&admission.policy, "/packet/state_code")
            != Some(packet.state_code.as_str())
        {
            errors.push("packet.state_code".to_string());
        }
        if envelope_valid != Some(typed_packet_envelope_valid(&packet.payload)) {
            errors.push("packet.envelope_binding".to_string());
        }

        let expected_required = receipt_requirements_for_packet(&packet.object_kind)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let declared_required = packet_receipt_requirements(&packet.payload);
        let receipt_required = string_array_at(&admission.policy, "/receipt/required");
        if declared_required != expected_required {
            errors.push("packet.receipt_requirements".to_string());
        }
        if receipt_required != expected_required {
            errors.push("receipt.required_binding".to_string());
        }
    }

    if admission
        .policy
        .pointer("/receipt/required")
        .and_then(|value| value.as_array())
        .is_none()
    {
        errors.push("receipt.required".to_string());
    }
    let receipt_missing_array = admission
        .policy
        .pointer("/receipt/missing")
        .and_then(|value| value.as_array());
    if receipt_missing_array.is_none() {
        errors.push("receipt.missing".to_string());
    }
    let receipt_missing = string_array_at(&admission.policy, "/receipt/missing");
    let receipt_satisfied = admission
        .policy
        .pointer("/receipt/satisfied")
        .and_then(|value| value.as_bool());
    if receipt_satisfied.is_none() {
        errors.push("receipt.satisfied".to_string());
    }
    if receipt_satisfied != Some(receipt_missing.is_empty()) {
        errors.push("receipt.satisfied_binding".to_string());
    }

    if let Some(policy) = admission
        .policy
        .get("policy")
        .filter(|value| !value.is_null())
    {
        if policy
            .get("schema_version")
            .and_then(|value| value.as_str())
            != Some(POLICY_SCHEMA_VERSION)
        {
            errors.push("policy.schema_version".to_string());
        }
        if policy.get("status").and_then(|value| value.as_str()) != Some("active") {
            errors.push("policy.status".to_string());
        }
        let policy_gate = policy.get("gate").and_then(|value| value.as_str());
        if policy_gate != gate_name {
            errors.push("policy.gate_binding".to_string());
        }
        if let Some(packet) = packet {
            if policy.get("object_kind").and_then(|value| value.as_str())
                != Some(packet.object_kind.as_str())
            {
                errors.push("policy.object_kind".to_string());
            }
            if policy.get("writer_role").and_then(|value| value.as_str())
                != Some(packet.writer_role.as_str())
            {
                errors.push("policy.writer_role".to_string());
            }
            if policy.get("route_from").and_then(|value| value.as_str())
                != Some(packet.route_from.as_str())
            {
                errors.push("policy.route_from".to_string());
            }
            if policy.get("route_to").and_then(|value| value.as_str())
                != Some(packet.route_to.as_str())
            {
                errors.push("policy.route_to".to_string());
            }
        }
        if let Some(policy_gate) = policy_gate {
            admission_gate_spec_errors(
                policy,
                "/gate_spec",
                policy_gate,
                packet,
                "policy.gate_spec",
                &mut errors,
            );
        }
    }

    if let Some(gate_name) = gate_name {
        admission_gate_spec_errors(
            &admission.policy,
            "/gate/spec",
            gate_name,
            packet,
            "gate.spec",
            &mut errors,
        );
    }
    match (
        admission.result.as_str(),
        gate_passed,
        receipt_satisfied,
        envelope_valid,
        policy_missing,
    ) {
        ("admitted", Some(true), Some(true), Some(true), false) => {}
        ("admitted", _, _, _, _) => errors.push("result.admission_conditions".to_string()),
        ("rejected", Some(true), Some(true), Some(true), false) => {
            errors.push("gate.result_binding".to_string());
        }
        ("rejected", _, _, _, _) => {}
        _ => {}
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "admission_id": admission.id,
            "packet_id": admission.packet_id,
            "errors": errors
        }))
    }
}

fn admission_field<'a>(value: &'a serde_json::Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(|value| value.as_str())
}

fn admission_gate_spec_errors(
    value: &serde_json::Value,
    pointer: &str,
    gate_name: &str,
    packet: Option<&HiveLoopPacket>,
    prefix: &str,
    errors: &mut Vec<String>,
) {
    let Some(spec_value) = value.pointer(pointer) else {
        errors.push(format!("{prefix}.missing"));
        return;
    };
    let Some(spec) = gate_spec(gate_name) else {
        return;
    };
    if spec_value
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(POLICY_SCHEMA_VERSION)
    {
        errors.push(format!("{prefix}.schema_version"));
    }
    if spec_value.get("name").and_then(|value| value.as_str()) != Some(gate_name) {
        errors.push(format!("{prefix}.name"));
    }
    if spec_value
        .get("expected_object_kind")
        .and_then(|value| value.as_str())
        != spec.expected_object_kind
    {
        errors.push(format!("{prefix}.expected_object_kind"));
    }
    if string_array_at(spec_value, "/required_receipts")
        != spec
            .required_receipts
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    {
        errors.push(format!("{prefix}.required_receipts"));
    }
    if let Some(packet) = packet {
        if spec
            .expected_object_kind
            .is_some_and(|expected| expected != packet.object_kind)
        {
            errors.push(format!("{prefix}.packet_object_kind"));
        }
        if spec.required_receipts != receipt_requirements_for_packet(&packet.object_kind).as_slice()
        {
            errors.push(format!("{prefix}.packet_receipts"));
        }
    }
}

fn worker_receipt_audit_errors(packet: &HiveLoopPacket) -> Option<serde_json::Value> {
    let Some(worker) = packet_role_worker(&packet.payload) else {
        return None;
    };
    let mut errors = Vec::new();
    if worker
        .get("kind")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("kind".to_string());
    }
    if worker
        .get("mode")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("mode".to_string());
    }
    let worker_role = worker.get("role").and_then(|value| value.as_str());
    if worker_role.map_or(true, |value| value.trim().is_empty()) {
        errors.push("role".to_string());
    }
    if worker_role.is_some_and(|role| role != packet.writer_role) {
        errors.push("role_binding".to_string());
    }
    if worker.get("ok").and_then(|value| value.as_bool()).is_none() {
        errors.push("ok".to_string());
    }
    match worker.get("timeout_secs").and_then(|value| value.as_u64()) {
        Some(1..=MAX_WORKER_TIMEOUT_SECS) => {}
        _ => errors.push("timeout_secs".to_string()),
    }
    let attempt_count = worker.get("attempt_count").and_then(|value| value.as_u64());
    let max_attempts = worker.get("max_attempts").and_then(|value| value.as_u64());
    match max_attempts {
        Some(1..=MAX_WORKER_ATTEMPTS) => {}
        _ => errors.push("max_attempts".to_string()),
    }
    match (attempt_count, max_attempts) {
        (Some(count), Some(max)) if count <= max => {}
        _ => errors.push("attempt_count".to_string()),
    }
    if worker.get("ok").and_then(|value| value.as_bool()) == Some(true) {
        match worker_structured_receipt(worker) {
            Some(receipt) => errors.extend(
                worker_receipt_contract_errors(&receipt, Some(&packet.writer_role))
                    .into_iter()
                    .map(|field| format!("receipt.{field}")),
            ),
            None => errors.push("receipt".to_string()),
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "packet_id": packet.id,
            "object_kind": packet.object_kind,
            "writer_role": packet.writer_role,
            "worker_role": worker_role,
            "errors": errors
        }))
    }
}

fn runtime_policy_audit_errors(
    contract: &HiveLoopContract,
    packets: &[HiveLoopPacket],
) -> Vec<serde_json::Value> {
    let registry = runtime_policy_registry();
    let mut errors = Vec::new();
    if runtime_policy_spec(&registry, &contract.runtime).is_none() {
        errors.push(serde_json::json!({
            "scope": "contract",
            "runtime": contract.runtime,
            "errors": ["runtime.unsupported"]
        }));
    }

    for packet in packets
        .iter()
        .filter(|packet| packet.round == contract.current_round)
    {
        for (receipt, worker) in packet_worker_receipts(packet) {
            let worker_errors =
                runtime_worker_policy_errors(&registry, worker, &packet.writer_role);
            if !worker_errors.is_empty() {
                errors.push(serde_json::json!({
                    "scope": "worker_receipt",
                    "packet_id": packet.id,
                    "object_kind": packet.object_kind,
                    "writer_role": packet.writer_role,
                    "receipt": receipt,
                    "kind": worker.get("kind").and_then(|value| value.as_str()),
                    "worker_role": worker.get("role").and_then(|value| value.as_str()),
                    "errors": worker_errors
                }));
            }
        }
    }
    errors
}

fn packet_worker_receipts<'a>(
    packet: &'a HiveLoopPacket,
) -> Vec<(&'static str, &'a serde_json::Value)> {
    let body = packet_body(&packet.payload);
    let mut workers = Vec::new();
    if let Some(worker) = body.get("role_worker") {
        workers.push(("role_worker", worker));
    }
    if let Some(worker) = body.get("runtime_worker") {
        workers.push(("runtime_worker", worker));
    }
    workers
}

fn runtime_worker_policy_errors(
    registry: &RuntimePolicyRegistry,
    worker: &serde_json::Value,
    expected_role: &str,
) -> Vec<String> {
    let mut errors = Vec::new();
    let kind = worker.get("kind").and_then(|value| value.as_str());
    match kind.and_then(|kind| runtime_policy_spec(registry, kind)) {
        Some(spec) => {
            if worker.get("mode").and_then(|value| value.as_str()) != Some(spec.mode.as_str()) {
                errors.push("mode".to_string());
            }
            for field in &spec.required_worker_context {
                if !worker_context_field_present(worker, field) {
                    errors.push(format!("context.{field}"));
                }
            }
        }
        None if kind.is_some() => errors.push("kind.unsupported".to_string()),
        None => errors.push("kind".to_string()),
    }

    let role = worker.get("role").and_then(|value| value.as_str());
    if role.map_or(true, |value| value.trim().is_empty()) {
        errors.push("role".to_string());
    }
    if role.is_some_and(|value| value != expected_role) {
        errors.push("role_binding".to_string());
    }

    match worker.get("timeout_secs").and_then(|value| value.as_u64()) {
        Some(1..=MAX_WORKER_TIMEOUT_SECS) => {}
        _ => errors.push("timeout_secs".to_string()),
    }
    let attempt_count = worker.get("attempt_count").and_then(|value| value.as_u64());
    let max_attempts = worker.get("max_attempts").and_then(|value| value.as_u64());
    match max_attempts {
        Some(1..=MAX_WORKER_ATTEMPTS) => {}
        _ => errors.push("max_attempts".to_string()),
    }
    match (attempt_count, max_attempts) {
        (Some(count), Some(max)) if count <= max => {}
        _ => errors.push("attempt_count".to_string()),
    }
    if worker.get("ok").and_then(|value| value.as_bool()) == Some(true) {
        match worker_structured_receipt(worker) {
            Some(receipt) => errors.extend(
                worker_receipt_contract_errors(&receipt, Some(expected_role))
                    .into_iter()
                    .map(|field| format!("receipt.{field}")),
            ),
            None => errors.push("receipt".to_string()),
        }
    }
    errors
}

fn worker_context_field_present(worker: &serde_json::Value, field: &str) -> bool {
    match field {
        "command" | "cwd" | "output_last_message_path" => worker
            .get(field)
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty()),
        "prompt_chars" => worker
            .get(field)
            .and_then(|value| value.as_u64())
            .is_some_and(|value| value > 0),
        _ => worker.get(field).is_some_and(|value| !value.is_null()),
    }
}

fn runtime_policy_spec<'a>(
    registry: &'a RuntimePolicyRegistry,
    runtime: &str,
) -> Option<&'a RuntimePolicySpec> {
    registry.supported.iter().find(|spec| spec.name == runtime)
}

fn active_policy_audit_errors(active_policies: &[&HiveLoopPolicy]) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let expected_policies = expected_loop_policies_for_active(active_policies);
    if active_policies.len() != expected_policies.len() {
        errors.push(serde_json::json!({
            "scope": "active_policy_set",
            "expected": expected_policies.len(),
            "actual": active_policies.len(),
            "errors": ["active_policy_count"]
        }));
    }

    for expected in expected_policies {
        let matches = active_policies
            .iter()
            .filter(|policy| policy_matches_expected_route(policy, expected))
            .collect::<Vec<_>>();
        match matches.len() {
            0 => errors.push(serde_json::json!({
                "scope": "expected_policy",
                "object_kind": expected.object_kind,
                "writer_role": expected.writer_role,
                "route_from": expected.route_from,
                "route_to": expected.route_to,
                "gate": expected.gate,
                "errors": ["policy.missing"]
            })),
            1 => {
                let policy = matches[0];
                if policy.gate != expected.gate {
                    errors.push(serde_json::json!({
                        "scope": "policy",
                        "policy_id": policy.id,
                        "object_kind": policy.object_kind,
                        "writer_role": policy.writer_role,
                        "route_from": policy.route_from,
                        "route_to": policy.route_to,
                        "gate": policy.gate,
                        "expected_gate": expected.gate,
                        "errors": ["policy.gate"]
                    }));
                }
            }
            _ => errors.push(serde_json::json!({
                "scope": "expected_policy",
                "object_kind": expected.object_kind,
                "writer_role": expected.writer_role,
                "route_from": expected.route_from,
                "route_to": expected.route_to,
                "gate": expected.gate,
                "policy_ids": matches.iter().map(|policy| policy.id).collect::<Vec<_>>(),
                "errors": ["policy.duplicate"]
            })),
        }
    }

    for policy in active_policies {
        let mut policy_errors = Vec::new();
        match gate_spec(&policy.gate) {
            Some(spec) => {
                if spec
                    .expected_object_kind
                    .is_some_and(|expected| expected != policy.object_kind)
                {
                    policy_errors.push("gate.expected_object_kind".to_string());
                }
                if spec.required_receipts
                    != receipt_requirements_for_packet(&policy.object_kind).as_slice()
                {
                    policy_errors.push("gate.required_receipts".to_string());
                }
            }
            None => policy_errors.push("gate.unknown".to_string()),
        }
        if !expected_policies
            .iter()
            .any(|expected| policy_matches_expected_route(policy, expected))
        {
            policy_errors.push("policy.route".to_string());
        }
        if !policy_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "policy",
                "policy_id": policy.id,
                "object_kind": policy.object_kind,
                "writer_role": policy.writer_role,
                "route_from": policy.route_from,
                "route_to": policy.route_to,
                "gate": policy.gate,
                "errors": policy_errors
            }));
        }
    }

    errors
}

fn expected_loop_policies_for_active(
    active_policies: &[&HiveLoopPolicy],
) -> &'static [LoopPolicySpec] {
    let legacy_match_count = LEGACY_LOOP_POLICIES
        .iter()
        .filter(|expected| {
            active_policies
                .iter()
                .any(|policy| policy_matches_expected_route(policy, expected))
        })
        .count();
    let current_match_count = DEFAULT_LOOP_POLICIES
        .iter()
        .filter(|expected| {
            active_policies
                .iter()
                .any(|policy| policy_matches_expected_route(policy, expected))
        })
        .count();
    if legacy_match_count > current_match_count {
        LEGACY_LOOP_POLICIES
    } else {
        DEFAULT_LOOP_POLICIES
    }
}

fn policy_matches_expected_route(policy: &HiveLoopPolicy, expected: &LoopPolicySpec) -> bool {
    policy.object_kind == expected.object_kind
        && policy.writer_role == expected.writer_role
        && policy.route_from == expected.route_from
        && policy.route_to == expected.route_to
}

fn verdict_sequence_audit_errors(
    contract: &HiveLoopContract,
    verdicts: &[HiveLoopVerdict],
) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let mut verdicts_by_round: HashMap<i64, Vec<&HiveLoopVerdict>> = HashMap::new();
    for verdict in verdicts {
        if verdict.round < 1 || verdict.round > contract.current_round {
            errors.push(serde_json::json!({
                "scope": "verdict_row",
                "verdict_id": verdict.id,
                "round": verdict.round,
                "current_round": contract.current_round,
                "errors": ["verdict.round"]
            }));
        }
        verdicts_by_round
            .entry(verdict.round)
            .or_default()
            .push(verdict);
    }

    for (round, round_verdicts) in verdicts_by_round {
        if round_verdicts.len() > 1 {
            errors.push(serde_json::json!({
                "scope": "verdict_round",
                "round": round,
                "verdict_ids": round_verdicts.iter().map(|verdict| verdict.id).collect::<Vec<_>>(),
                "decisions": round_verdicts
                    .iter()
                    .map(|verdict| verdict.decision.as_str())
                    .collect::<Vec<_>>(),
                "errors": ["verdict.round_duplicate"]
            }));
        }
    }

    if terminal_contract_status(&contract.status)
        && !verdicts
            .iter()
            .any(|verdict| verdict.round == contract.current_round)
    {
        errors.push(serde_json::json!({
            "scope": "verdict_round",
            "round": contract.current_round,
            "status": contract.status,
            "errors": ["verdict.current_round_missing"]
        }));
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn terminal_contract_status(status: &str) -> bool {
    matches!(status, "kept" | "rejected" | "needs-review" | "blocked")
}

fn verdict_audit_errors(verdict: &HiveLoopVerdict) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let score_decision = verdict
        .score
        .get("decision")
        .and_then(|value| value.as_str());
    let evidence_decision = verdict
        .evidence
        .get("decision")
        .and_then(|value| value.as_str());
    let score_reason = verdict
        .score
        .get("reason_code")
        .and_then(|value| value.as_str());
    let evidence_reason = verdict
        .evidence
        .get("reason_code")
        .and_then(|value| value.as_str());
    if verdict
        .score
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(VERDICT_SCHEMA_VERSION)
    {
        errors.push("score.schema_version".to_string());
    }
    if !decision_label_allowed(&verdict.decision) {
        errors.push("decision".to_string());
    }
    if score_decision != Some(verdict.decision.as_str()) {
        errors.push("score.decision_binding".to_string());
    }
    if evidence_decision != Some(verdict.decision.as_str()) {
        errors.push("evidence.decision_binding".to_string());
    }
    if score_reason.is_none() {
        errors.push("score.reason_code".to_string());
    }
    if evidence_reason.is_none() {
        errors.push("evidence.reason_code".to_string());
    }
    if score_reason.is_some() && evidence_reason.is_some() && score_reason != evidence_reason {
        errors.push("reason_code.binding".to_string());
    }
    if verdict.score.get("gate_results").map_or(true, |value| {
        !value.is_object() || value.as_object().is_some_and(serde_json::Map::is_empty)
    }) {
        errors.push("score.gate_results".to_string());
    }
    match verdict.score.get("score_vector") {
        Some(score_vector) if score_vector.is_object() => {
            errors.extend(verdict_score_vector_errors(
                score_vector,
                verdict.decision.as_str(),
            ));
        }
        _ => errors.push("score.score_vector".to_string()),
    }
    match verdict
        .score
        .get("gates_passed")
        .and_then(|value| value.as_bool())
    {
        Some(value) if value == (verdict.decision == "keep") => {}
        _ => errors.push("score.gates_passed".to_string()),
    }
    match verdict
        .score
        .get("operator_review_needed")
        .and_then(|value| value.as_bool())
    {
        Some(value) if value == (verdict.decision != "keep") => {}
        _ => errors.push("score.operator_review_needed".to_string()),
    }
    if human_options(&verdict.score) != expected_human_options_for_decision(&verdict.decision) {
        errors.push("score.human_options".to_string());
    }
    if verdict
        .evidence
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(VERDICT_SCHEMA_VERSION)
    {
        errors.push("evidence.schema_version".to_string());
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "verdict_id": verdict.id,
            "round": verdict.round,
            "errors": errors
        }))
    }
}

fn verdict_evidence_binding_audit_errors(
    contract: &HiveLoopContract,
    verdicts: &[HiveLoopVerdict],
    packets: &[HiveLoopPacket],
    admissions: &[HiveLoopAdmission],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let packets_by_id = packets
        .iter()
        .map(|packet| (packet.id, packet))
        .collect::<HashMap<_, _>>();
    let admissions_by_id = admissions
        .iter()
        .map(|admission| (admission.id, admission))
        .collect::<HashMap<_, _>>();
    let evidence_by_id = evidence
        .iter()
        .map(|row| (row.id, row))
        .collect::<HashMap<_, _>>();
    let mut errors = Vec::new();

    for verdict in verdicts {
        let reason_code = verdict
            .evidence
            .get("reason_code")
            .and_then(|value| value.as_str());
        let verdict_errors = if reason_code == Some("admission_rejected") {
            admission_rejection_verdict_binding_errors(
                contract,
                verdict,
                &packets_by_id,
                &admissions_by_id,
                &evidence_by_id,
            )
        } else {
            standard_verdict_binding_errors(verdict, packets, evidence)
        };
        if !verdict_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "verdict_evidence",
                "verdict_id": verdict.id,
                "round": verdict.round,
                "decision": verdict.decision,
                "reason_code": reason_code,
                "errors": verdict_errors
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("verdict_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
        )
    });
    errors
}

fn standard_verdict_binding_errors(
    verdict: &HiveLoopVerdict,
    packets: &[HiveLoopPacket],
    evidence: &[HiveLoopEvidence],
) -> Vec<String> {
    let mut errors = Vec::new();
    let round_stage_evidence = evidence
        .iter()
        .filter(|row| row.round == verdict.round && stage_bound_evidence_kind(&row.kind))
        .collect::<Vec<_>>();
    let expected_evidence_count = round_stage_evidence.len() as i64;
    match verdict
        .evidence
        .get("evidence_count")
        .and_then(|value| value.as_i64())
    {
        Some(count) if count == expected_evidence_count => {}
        _ => errors.push("evidence.count".to_string()),
    }

    let evidence_before_verdict = round_stage_evidence
        .iter()
        .filter(|row| row.kind != "verdict_packet")
        .count() as i64;
    match verdict
        .evidence
        .pointer("/source/round_evidence_before_verdict")
        .and_then(|value| value.as_i64())
    {
        Some(count) if count == evidence_before_verdict => {}
        _ => errors.push("evidence.source_round_count".to_string()),
    }

    let score_runtime_ready = verdict
        .score
        .pointer("/gate_results/runtime_ready")
        .and_then(|value| value.as_bool());
    let evidence_runtime_ready = verdict
        .evidence
        .get("runtime_ready")
        .and_then(|value| value.as_bool());
    match (evidence_runtime_ready, score_runtime_ready) {
        (Some(evidence_value), Some(score_value)) if evidence_value == score_value => {}
        _ => errors.push("evidence.runtime_ready".to_string()),
    }

    let reviewer_packet = packets.iter().find(|packet| {
        packet.round == verdict.round
            && packet.object_kind == "VERDICT_PACKET"
            && matches!(packet.writer_role.as_str(), "reviewer" | "evaluator")
    });
    let expected_worker = reviewer_packet.and_then(|packet| packet_role_worker(&packet.payload));
    match (verdict.evidence.get("role_worker"), expected_worker) {
        (Some(actual), Some(expected)) if actual == expected => {}
        (Some(_), Some(_)) => errors.push("evidence.role_worker_binding".to_string()),
        _ => errors.push("evidence.role_worker".to_string()),
    }
    let source_reviewer = verdict
        .evidence
        .pointer("/source/reviewer")
        .and_then(|value| value.as_str());
    let source_evaluator = verdict
        .evidence
        .pointer("/source/evaluator")
        .and_then(|value| value.as_str());
    if source_reviewer != Some("hive-loop-control") && source_evaluator != Some("hive-loop-control")
    {
        errors.push("evidence.source_reviewer".to_string());
    }

    errors
}

fn admission_rejection_verdict_binding_errors(
    contract: &HiveLoopContract,
    verdict: &HiveLoopVerdict,
    packets_by_id: &HashMap<i64, &HiveLoopPacket>,
    admissions_by_id: &HashMap<i64, &HiveLoopAdmission>,
    evidence_by_id: &HashMap<i64, &HiveLoopEvidence>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let evidence_id = verdict
        .evidence
        .get("evidence_id")
        .and_then(|value| value.as_i64());
    let admission_id = verdict
        .evidence
        .get("admission_id")
        .and_then(|value| value.as_i64());
    let packet_id = verdict
        .evidence
        .get("packet_id")
        .and_then(|value| value.as_i64());
    let phase = verdict
        .evidence
        .get("phase")
        .and_then(|value| value.as_str());

    let linked_evidence = evidence_id.and_then(|evidence_id| evidence_by_id.get(&evidence_id));
    match linked_evidence {
        Some(row) if row.kind == "admission_rejection" && row.round == verdict.round => {}
        Some(_) => errors.push("evidence.link".to_string()),
        None => errors.push("evidence.link".to_string()),
    }

    let linked_admission =
        admission_id.and_then(|admission_id| admissions_by_id.get(&admission_id));
    match linked_admission {
        Some(admission) if admission.result == "rejected" => {}
        Some(_) => errors.push("admission.result".to_string()),
        None => errors.push("admission.link".to_string()),
    }

    match (linked_admission, packet_id) {
        (Some(admission), Some(packet_id)) if admission.packet_id == packet_id => {}
        _ => errors.push("admission.packet_binding".to_string()),
    }
    if !packet_id.is_some_and(|packet_id| packets_by_id.contains_key(&packet_id)) {
        errors.push("packet.link".to_string());
    }

    if let Some(row) = linked_evidence {
        if row.payload.get("phase").and_then(|value| value.as_str()) != phase {
            errors.push("evidence.phase_binding".to_string());
        }
        if row
            .payload
            .get("admission_id")
            .and_then(|value| value.as_i64())
            != admission_id
        {
            errors.push("evidence.admission_binding".to_string());
        }
        if row
            .payload
            .get("packet_id")
            .and_then(|value| value.as_i64())
            != packet_id
        {
            errors.push("evidence.packet_binding".to_string());
        }
    }
    if contract.status == "blocked"
        && verdict.round == contract.current_round
        && phase != Some(contract.active_phase.as_str())
    {
        errors.push("evidence.phase".to_string());
    }
    if let Some(admission) = linked_admission {
        if verdict.evidence.pointer("/source/admission_receipt") != Some(&admission.policy) {
            errors.push("evidence.admission_receipt_binding".to_string());
        }
    }
    let source_reviewer = verdict
        .evidence
        .pointer("/source/reviewer")
        .and_then(|value| value.as_str());
    let source_evaluator = verdict
        .evidence
        .pointer("/source/evaluator")
        .and_then(|value| value.as_str());
    if source_reviewer != Some("hive-loop-control") && source_evaluator != Some("hive-loop-control")
    {
        errors.push("evidence.source_reviewer".to_string());
    }

    errors
}

fn verdict_score_vector_errors(score_vector: &serde_json::Value, decision: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for metric in VERDICT_SCORE_METRICS {
        let value = score_vector.get(*metric);
        if *metric == "runtime_readiness"
            && decision == "blocked"
            && value == Some(&serde_json::Value::Null)
        {
            continue;
        }
        match value.and_then(|value| value.as_f64()) {
            Some(value) if (0.0..=1.0).contains(&value) => {}
            _ => errors.push(format!("score.score_vector.{metric}")),
        }
    }
    errors
}

fn expected_human_options_for_decision(decision: &str) -> Vec<String> {
    match decision {
        "keep" => option_list(&["comment"]),
        "reject" => option_list(&["comment", "retry"]),
        "needs-review" => option_list(&["comment", "retry", "cancel"]),
        "blocked" => option_list(&["comment", "retry", "request-review", "cancel"]),
        _ => Vec::new(),
    }
}

#[derive(Default)]
struct IssueSurfaceAudit {
    comment_count: usize,
    action_count: usize,
    operator_evidence_count: usize,
    errors: Vec<serde_json::Value>,
}

fn issue_surface_audit(
    store: &Store,
    contract: &HiveLoopContract,
    issues: &[HiveIssue],
    evidence: &[HiveLoopEvidence],
) -> Result<IssueSurfaceAudit> {
    let mut audit = IssueSurfaceAudit::default();
    let mut comments_by_id = HashMap::new();
    if issues.is_empty() {
        audit.errors.push(serde_json::json!({
            "scope": "loop",
            "loop_id": contract.id,
            "errors": ["issue.missing"]
        }));
    }

    for issue in issues {
        let mut issue_errors = Vec::new();
        if issue.loop_id != Some(contract.id) {
            issue_errors.push("issue.loop_id".to_string());
        }
        if !issue_status_allowed(&issue.status) {
            issue_errors.push("issue.status".to_string());
        }
        let expected_status = issue_status_for_contract_status(&contract.status);
        if expected_status.is_some_and(|expected| issue.status != expected) {
            issue_errors.push("issue.contract_status_binding".to_string());
        }
        if issue.title.trim().is_empty() {
            issue_errors.push("issue.title".to_string());
        }

        let comments = store.list_hive_comments(issue.id)?;
        audit.comment_count += comments.len();
        if comments.is_empty() {
            issue_errors.push("comment.missing".to_string());
        }
        if !issue_errors.is_empty() {
            audit.errors.push(serde_json::json!({
                "scope": "issue",
                "issue_id": issue.id,
                "contract_status": contract.status,
                "expected_status": expected_status,
                "actual_status": issue.status,
                "errors": issue_errors
            }));
        }
        if let Some(loop_id) = issue.loop_id.filter(|loop_id| *loop_id == contract.id) {
            let trace = issue_trace_summary_without_audit(store, loop_id, Some(issue))?;
            let doctor = issue_doctor_summary(store, loop_id, issue, &trace)?;
            let actions = issue_actions(issue, Some(&trace), Some(&doctor));
            audit.action_count += actions.len();
            if let Some(error) = issue_action_audit_error(issue, contract, &trace, &actions) {
                audit.errors.push(error);
            }
        }

        for comment in &comments {
            comments_by_id.insert(comment.id, comment.clone());
            if let Some(error) = issue_comment_audit_error(&comment, issue, evidence) {
                audit.errors.push(error);
            }
        }
    }

    for row in evidence
        .iter()
        .filter(|row| row.kind == "operator_comment" || row.kind == "operator_decision")
    {
        audit.operator_evidence_count += 1;
        if let Some(error) = operator_evidence_audit_error(row, issues, &comments_by_id) {
            audit.errors.push(error);
        }
    }

    Ok(audit)
}

fn issue_status_for_contract_status(status: &str) -> Option<&'static str> {
    match status {
        "todo" => Some("Todo"),
        "running" => Some("Doing"),
        "blocked" => Some("Blocked"),
        "needs-review" => Some("Needs Review"),
        "rejected" => Some("Canceled"),
        "kept" => Some("Done"),
        _ => None,
    }
}

fn issue_action_audit_error(
    issue: &HiveIssue,
    contract: &HiveLoopContract,
    trace: &IssueTraceSummary,
    actions: &[IssueAction],
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let mut expected_actions = Vec::new();
    if issue.loop_id == Some(contract.id) && issue.status == "Todo" {
        expected_actions.push("run".to_string());
    }
    expected_actions.extend(trace.human_options.clone());
    let action_names = actions
        .iter()
        .map(|action| action.action.clone())
        .collect::<Vec<_>>();
    if action_names != expected_actions {
        errors.push("action.sequence".to_string());
    }
    let mut seen = HashMap::new();
    for action in actions {
        *seen.entry(action.action.as_str()).or_insert(0usize) += 1;
        issue_action_field_errors(issue, contract, action, &expected_actions, &mut errors);
    }
    if actions.is_empty() {
        errors.push("action.missing".to_string());
    }
    if seen.values().any(|count| *count > 1) {
        errors.push("action.duplicate".to_string());
    }

    if errors.is_empty() {
        return None;
    }
    errors.sort();
    errors.dedup();
    Some(serde_json::json!({
        "scope": "issue_action",
        "issue_id": issue.id,
        "status": issue.status,
        "expected_actions": expected_actions,
        "actual_actions": action_names,
        "errors": errors
    }))
}

fn issue_action_field_errors(
    issue: &HiveIssue,
    contract: &HiveLoopContract,
    action: &IssueAction,
    expected_actions: &[String],
    errors: &mut Vec<String>,
) {
    if action.schema_version != ISSUE_ACTION_SCHEMA_VERSION {
        errors.push("action.schema_version".to_string());
    }
    if !matches!(
        action.action.as_str(),
        "run" | "comment" | "retry" | "request-review" | "cancel"
    ) {
        errors.push("action.name".to_string());
    }
    if !expected_actions
        .iter()
        .any(|expected| expected == &action.action)
    {
        errors.push("action.unexpected".to_string());
    }
    let expected_label = match action.action.as_str() {
        "run" => Some("Run"),
        "comment" => Some("Comment"),
        "retry" => Some("Retry"),
        "request-review" => Some("Review"),
        "cancel" => Some("Cancel"),
        _ => None,
    };
    if expected_label.is_some_and(|label| action.label != label) {
        errors.push("action.label".to_string());
    }
    let expected_source = if action.action == "run" {
        "runtime"
    } else {
        "human_options"
    };
    if action.source != expected_source {
        errors.push("action.source".to_string());
    }
    let expected_input = match action.action.as_str() {
        "run" => Some("none"),
        "comment" => Some("body"),
        "retry" | "request-review" | "cancel" => Some("note"),
        _ => None,
    };
    if expected_input.is_some_and(|input| action.input != input) {
        errors.push("action.input".to_string());
    }
    if action.destructive != (action.action == "cancel") {
        errors.push("action.destructive".to_string());
    }
    match action.action.as_str() {
        "run" => {
            if action.runtime.as_deref() != Some(contract.runtime.as_str()) {
                errors.push("action.runtime".to_string());
            }
            if !action
                .command
                .starts_with(&format!("entrance hive issue run {}", issue.id))
                || !action.command.contains("--compact")
                || !action
                    .command
                    .contains(&format!("--runtime {}", contract.runtime))
            {
                errors.push("action.command".to_string());
            }
        }
        "comment" => {
            if action.runtime.is_some() {
                errors.push("action.runtime".to_string());
            }
            if action.command
                != format!(
                    "entrance hive issue comment {} --body <text> --compact",
                    issue.id
                )
            {
                errors.push("action.command".to_string());
            }
        }
        "retry" => {
            if action.runtime.as_deref() != Some(contract.runtime.as_str()) {
                errors.push("action.runtime".to_string());
            }
            if !action
                .command
                .starts_with(&format!("entrance hive issue retry-run {}", issue.id))
                || !action.command.contains("--body <note>")
                || !action.command.contains("--compact")
            {
                errors.push("action.command".to_string());
            }
            if contract.runtime == "codex"
                && (!action.command.contains("--runtime codex")
                    || !action.command.contains("--worker-attempts 2"))
            {
                errors.push("action.command".to_string());
            }
        }
        "request-review" => {
            if action.runtime.is_some() {
                errors.push("action.runtime".to_string());
            }
            if action.command
                != format!(
                    "entrance hive issue decide {} request-review --body <note> --compact",
                    issue.id
                )
            {
                errors.push("action.command".to_string());
            }
        }
        "cancel" => {
            if action.runtime.is_some() {
                errors.push("action.runtime".to_string());
            }
            if action.command
                != format!(
                    "entrance hive issue decide {} cancel --body <note> --compact",
                    issue.id
                )
            {
                errors.push("action.command".to_string());
            }
        }
        _ => {}
    }
}

fn issue_comment_audit_error(
    comment: &HiveComment,
    issue: &HiveIssue,
    evidence: &[HiveLoopEvidence],
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    if comment.issue_id != issue.id {
        errors.push("comment.issue_id".to_string());
    }
    if comment.author.trim().is_empty() {
        errors.push("comment.author".to_string());
    }
    if comment.body.trim().is_empty() {
        errors.push("comment.body".to_string());
    }
    let schema = schema_version(&comment.payload);
    let source = comment
        .payload
        .get("source")
        .and_then(|value| value.as_str());
    let expected_schema = expected_comment_schema(comment);
    if source.is_none() {
        errors.push("comment.payload.source".to_string());
    }
    if !comment_schema_allowed(schema.as_deref()) {
        errors.push("comment.payload.schema_version".to_string());
    }
    if expected_schema.is_some() && schema.as_deref() != expected_schema {
        errors.push("comment.payload.schema_binding".to_string());
    }
    match expected_schema {
        Some(SYSTEM_COMMENT_SCHEMA_VERSION) => {
            errors.extend(system_comment_audit_errors(comment, issue, evidence));
        }
        Some(OPERATOR_COMMENT_SCHEMA_VERSION) => {
            if !evidence.iter().any(|row| {
                row.kind == "operator_comment"
                    && row
                        .payload
                        .pointer("/issue/comment_id")
                        .and_then(|value| value.as_i64())
                        == Some(comment.id)
            }) {
                errors.push("comment.operator_evidence".to_string());
            }
        }
        Some(OPERATOR_DECISION_SCHEMA_VERSION) => {
            if comment
                .payload
                .get("action")
                .and_then(|value| value.as_str())
                .is_none()
            {
                errors.push("comment.payload.action".to_string());
            }
            if !evidence.iter().any(|row| {
                row.kind == "operator_decision"
                    && row
                        .payload
                        .pointer("/issue/comment_id")
                        .and_then(|value| value.as_i64())
                        == Some(comment.id)
            }) {
                errors.push("comment.operator_evidence".to_string());
            }
        }
        _ => {}
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "scope": "comment",
            "issue_id": issue.id,
            "comment_id": comment.id,
            "author": comment.author,
            "source": source,
            "schema_version": schema,
            "errors": errors
        }))
    }
}

fn system_comment_audit_errors(
    comment: &HiveComment,
    issue: &HiveIssue,
    evidence: &[HiveLoopEvidence],
) -> Vec<String> {
    let mut errors = Vec::new();
    let payload = &comment.payload;

    if let Some(loop_id) = issue.loop_id {
        let comment_loop_id = payload.get("loop_id").and_then(|value| value.as_i64());
        if comment_loop_id != Some(loop_id) {
            errors.push("comment.payload.loop_id".to_string());
        }
    }

    let has_stage_fields = ["stage_role", "evidence_kind", "worker"]
        .iter()
        .any(|field| payload.get(*field).is_some());
    if !has_stage_fields {
        return errors;
    }

    let round = payload.get("round").and_then(|value| value.as_i64());
    if !round.is_some_and(|round| round >= 1) {
        errors.push("comment.stage.round".to_string());
    }

    let stage_role = payload.get("stage_role").and_then(|value| value.as_str());
    let valid_stage_role = stage_role.filter(|role| canonical_stage_roles().contains(role));
    if valid_stage_role.is_none() {
        errors.push("comment.stage.role".to_string());
    }

    let evidence_kind = payload
        .get("evidence_kind")
        .and_then(|value| value.as_str());
    if let Some(role) = valid_stage_role {
        if canonical_stage_evidence_kind(role) != evidence_kind {
            errors.push("comment.stage.evidence_kind".to_string());
        }
    } else if evidence_kind.is_none() {
        errors.push("comment.stage.evidence_kind".to_string());
    }
    let evidence_id = payload.get("evidence_id").and_then(|value| value.as_i64());
    if !evidence_id.is_some_and(|id| id > 0) {
        errors.push("comment.stage.evidence_id".to_string());
    }

    let admission = payload.get("admission").and_then(|value| value.as_str());
    if admission != Some("admitted") {
        errors.push("comment.stage.admission".to_string());
    }

    let worker = payload.get("worker");
    if !worker.is_some_and(|worker| worker.is_object()) {
        errors.push("comment.stage.worker".to_string());
    }
    if let (Some(worker), Some(role)) = (worker, valid_stage_role) {
        if worker.get("role").and_then(|value| value.as_str()) != Some(role) {
            errors.push("comment.stage.worker_role".to_string());
        }
    }

    if let (Some(round), Some(evidence_id), Some(evidence_kind), Some(admission), Some(worker)) =
        (round, evidence_id, evidence_kind, admission, worker)
    {
        let has_evidence_binding = evidence.iter().any(|row| {
            row.id == evidence_id
                && row.round == round
                && row.kind == evidence_kind
                && row
                    .payload
                    .get("admission")
                    .and_then(|value| value.as_str())
                    == Some(admission)
                && row.payload.get("worker") == Some(worker)
        });
        if !has_evidence_binding {
            errors.push("comment.stage.evidence_binding".to_string());
        }
    }

    errors
}

fn operator_evidence_audit_error(
    row: &HiveLoopEvidence,
    issues: &[HiveIssue],
    comments_by_id: &HashMap<i64, HiveComment>,
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let expected_schema = match row.kind.as_str() {
        "operator_comment" => OPERATOR_COMMENT_SCHEMA_VERSION,
        "operator_decision" => OPERATOR_DECISION_SCHEMA_VERSION,
        _ => return None,
    };
    if row.payload.get("source").and_then(|value| value.as_str()) != Some("issue/status/comment") {
        errors.push("evidence.source".to_string());
    }
    if schema_version(&row.payload).as_deref() != Some(expected_schema) {
        errors.push("evidence.schema_version".to_string());
    }
    let issue_id = row
        .payload
        .pointer("/issue/id")
        .and_then(|value| value.as_i64());
    if !issue_id.is_some_and(|issue_id| issues.iter().any(|issue| issue.id == issue_id)) {
        errors.push("evidence.issue_id".to_string());
    }
    let comment_id = row
        .payload
        .pointer("/issue/comment_id")
        .and_then(|value| value.as_i64());
    if comment_id.is_none() {
        errors.push("evidence.comment_id".to_string());
    }
    let linked_comment = comment_id.and_then(|comment_id| comments_by_id.get(&comment_id));
    if comment_id.is_some() && linked_comment.is_none() {
        errors.push("evidence.comment_link".to_string());
    }
    if row
        .payload
        .pointer("/loop/id")
        .and_then(|value| value.as_i64())
        != Some(row.loop_id)
    {
        errors.push("evidence.loop_id_binding".to_string());
    }
    let evidence_round = row
        .payload
        .pointer("/loop/round")
        .and_then(|value| value.as_i64());
    if evidence_round != Some(row.round) {
        errors.push("evidence.loop_round_binding".to_string());
    }

    if let Some(comment) = linked_comment {
        if issue_id != Some(comment.issue_id) {
            errors.push("evidence.comment_issue_id".to_string());
        }
        if expected_comment_schema(comment) != Some(expected_schema) {
            errors.push("evidence.comment_schema_binding".to_string());
        }
        if row
            .payload
            .pointer("/operator/author")
            .and_then(|value| value.as_str())
            != Some(comment.author.as_str())
        {
            errors.push("evidence.author_binding".to_string());
        }
        if row
            .payload
            .pointer("/operator/comment_body")
            .and_then(|value| value.as_str())
            != Some(comment.body.as_str())
        {
            errors.push("evidence.comment_body_binding".to_string());
        }
        if comment
            .payload
            .get("loop_id")
            .and_then(|value| value.as_i64())
            != Some(row.loop_id)
        {
            errors.push("evidence.comment_loop_binding".to_string());
        }
        if row.kind == "operator_comment" {
            if comment
                .payload
                .get("round")
                .and_then(|value| value.as_i64())
                != evidence_round
            {
                errors.push("evidence.comment_round_binding".to_string());
            }
            if comment
                .payload
                .get("status")
                .and_then(|value| value.as_str())
                != row
                    .payload
                    .pointer("/issue/status")
                    .and_then(|value| value.as_str())
            {
                errors.push("evidence.comment_status_binding".to_string());
            }
            if comment
                .payload
                .get("phase")
                .and_then(|value| value.as_str())
                != row
                    .payload
                    .pointer("/loop/phase")
                    .and_then(|value| value.as_str())
            {
                errors.push("evidence.comment_phase_binding".to_string());
            }
        }
        if row.kind == "operator_decision" {
            let evidence_action = row
                .payload
                .pointer("/operator/action")
                .and_then(|value| value.as_str());
            let comment_action = comment
                .payload
                .get("action")
                .and_then(|value| value.as_str());
            if evidence_action != comment_action {
                errors.push("evidence.action_binding".to_string());
            }
            if comment
                .payload
                .get("next_round")
                .and_then(|value| value.as_i64())
                != evidence_round
            {
                errors.push("evidence.comment_next_round_binding".to_string());
            }
            match evidence_action.map(parse_issue_decision_action) {
                Some(Ok(action)) => {
                    if row
                        .payload
                        .pointer("/issue/to_status")
                        .and_then(|value| value.as_str())
                        != Some(action.issue_status())
                    {
                        errors.push("evidence.issue_status_binding".to_string());
                    }
                    if row
                        .payload
                        .pointer("/loop/next_status")
                        .and_then(|value| value.as_str())
                        != Some(action.contract_status())
                    {
                        errors.push("evidence.loop_status_binding".to_string());
                    }
                    if row
                        .payload
                        .pointer("/loop/next_phase")
                        .and_then(|value| value.as_str())
                        != Some(action.contract_phase())
                    {
                        errors.push("evidence.loop_phase_binding".to_string());
                    }
                    if action == IssueDecisionAction::Retry
                        && !evidence_round.is_some_and(|round| round > 1)
                    {
                        errors.push("evidence.retry_round".to_string());
                    }
                }
                _ => errors.push("evidence.action".to_string()),
            }
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "scope": "operator_evidence",
            "evidence_id": row.id,
            "kind": row.kind,
            "issue_id": issue_id,
            "errors": errors
        }))
    }
}

fn expected_comment_schema(comment: &HiveComment) -> Option<&'static str> {
    let source = comment
        .payload
        .get("source")
        .and_then(|value| value.as_str());
    let action = comment
        .payload
        .get("action")
        .and_then(|value| value.as_str());
    match source {
        Some("operator") if action.is_some() => Some(OPERATOR_DECISION_SCHEMA_VERSION),
        Some("operator") => Some(OPERATOR_COMMENT_SCHEMA_VERSION),
        Some("hive" | "compiler") => Some(SYSTEM_COMMENT_SCHEMA_VERSION),
        _ => None,
    }
}

fn comment_schema_allowed(schema: Option<&str>) -> bool {
    matches!(
        schema,
        Some(OPERATOR_COMMENT_SCHEMA_VERSION)
            | Some(OPERATOR_DECISION_SCHEMA_VERSION)
            | Some(SYSTEM_COMMENT_SCHEMA_VERSION)
    )
}

fn issue_status_allowed(value: &str) -> bool {
    matches!(
        value,
        "Todo" | "Doing" | "Blocked" | "Needs Review" | "Done" | "Canceled"
    )
}

fn decision_label_allowed(value: &str) -> bool {
    matches!(value, "keep" | "reject" | "needs-review" | "blocked")
}

pub fn panel(store: &Store) -> Result<Vec<IssueCard>> {
    store
        .list_hive_issues()?
        .into_iter()
        .map(|issue| issue_card_from_issue(store, issue))
        .collect()
}

pub fn issue(store: &Store, issue_id: i64) -> Result<IssueCard> {
    issue_card(store, issue_id)
}

pub fn issue_mirror(store: &Store, issue_id: i64) -> Result<IssueMirrorReport> {
    let card = issue_card(store, issue_id)?;
    let loop_contract = card
        .issue
        .loop_id
        .map(|loop_id| store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let review_surface = loop_contract
        .as_ref()
        .map(|contract| contract.review_surface.clone())
        .unwrap_or_else(|| "local-hive-panel".to_string());
    let provider = issue_mirror_provider(&review_surface);
    let external_key = match card.issue.loop_id {
        Some(loop_id) => format!("hive-loop-{loop_id}-issue-{}", card.issue.id),
        None => format!("hive-issue-{}", card.issue.id),
    };

    Ok(IssueMirrorReport {
        schema_version: ISSUE_MIRROR_SCHEMA_VERSION.to_string(),
        provider,
        review_surface,
        external_key,
        issue: card.issue,
        loop_contract,
        comments: card.comments,
        actions: card.actions,
        trace: card.trace,
        doctor: card.doctor,
    })
}

fn issue_mirror_provider(review_surface: &str) -> String {
    let value = review_surface.trim();
    if value.is_empty() {
        return "local-hive-panel".to_string();
    }
    value
        .split([':', '/', '#', '?'])
        .next()
        .filter(|provider| !provider.trim().is_empty())
        .unwrap_or("local-hive-panel")
        .trim()
        .to_string()
}

pub fn add_comment(store: &Store, request: IssueCommentRequest) -> Result<IssueCard> {
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let author = default_text(request.author, "human");
    let body = request.body.trim().to_string();
    if body.is_empty() {
        anyhow::bail!("hive issue comment requires a non-empty body");
    }
    let contract = issue
        .loop_id
        .map(|loop_id| store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let comment_id = store.insert_hive_comment(HiveCommentCreate {
        issue_id: request.issue_id,
        author: author.clone(),
        body: body.clone(),
        payload: serde_json::json!({
            "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
            "source": "operator",
            "loop_id": issue.loop_id,
            "round": contract.as_ref().map(|contract| contract.current_round),
            "status": issue.status,
            "phase": contract.as_ref().map(|contract| contract.active_phase.as_str())
        }),
    })?;
    record_operator_comment_evidence(store, &issue, comment_id, &author, &body)?;

    issue_card_from_issue(store, issue)
}

pub fn decide_issue(store: &Store, request: IssueDecisionRequest) -> Result<IssueCard> {
    let action = parse_issue_decision_action(&request.action)?;
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    ensure_issue_decision_allowed(store, &issue, action)?;
    let author = default_text(request.author, "human");
    let note = request.body.as_deref().unwrap_or_default().trim();
    let mut next_round = None;

    if let Some(loop_id) = issue.loop_id {
        let contract = store
            .get_hive_loop_contract(loop_id)?
            .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
        let round = match action {
            IssueDecisionAction::Retry => contract.current_round + 1,
            IssueDecisionAction::RequestReview | IssueDecisionAction::Cancel => {
                contract.current_round
            }
        };
        store.update_hive_loop_contract_state(
            loop_id,
            action.contract_status(),
            action.contract_phase(),
            round,
        )?;
        next_round = Some(round);
    }

    let issue_summary = action.issue_summary(next_round);
    let comment_body = action.comment_body(next_round, note);

    store.update_hive_issue_status(issue.id, action.issue_status(), Some(&issue_summary))?;
    let comment_id = store.insert_hive_comment(HiveCommentCreate {
        issue_id: issue.id,
        author: author.clone(),
        body: comment_body.clone(),
        payload: serde_json::json!({
            "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
            "source": "operator",
            "action": action.as_str(),
            "loop_id": issue.loop_id,
            "next_round": next_round,
            "note": note
        }),
    })?;
    record_operator_decision_evidence(
        store,
        &issue,
        action,
        comment_id,
        &author,
        note,
        next_round,
        &issue_summary,
        &comment_body,
    )?;

    issue_card(store, issue.id)
}

pub fn run_issue(store: &Store, request: IssueRunRequest) -> Result<HiveLoopReport> {
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let loop_id = issue
        .loop_id
        .with_context(|| format!("hive issue #{} is not linked to a loop", issue.id))?;

    if request.retry {
        decide_issue(
            store,
            IssueDecisionRequest {
                issue_id: issue.id,
                action: "retry".to_string(),
                author: default_text(request.author, "human"),
                body: request.body.clone(),
            },
        )?;
    } else if issue.status != "Todo" {
        anyhow::bail!(
            "hive issue run requires issue #{} to be `Todo`; current status is `{}`. Use `hive issue retry-run {}` to record a retry decision first.",
            issue.id,
            issue.status,
            issue.id
        );
    }

    run(
        store,
        HiveLoopRunRequest {
            loop_id,
            runtime: request.runtime,
            decision: request.decision,
            worker_timeout_secs: request.worker_timeout_secs,
            worker_attempts: request.worker_attempts,
        },
    )
}

fn ensure_issue_decision_allowed(
    store: &Store,
    issue: &HiveIssue,
    action: IssueDecisionAction,
) -> Result<()> {
    let options = issue
        .loop_id
        .map(|loop_id| {
            issue_trace_summary(store, loop_id, Some(issue)).map(|trace| trace.human_options)
        })
        .transpose()?
        .unwrap_or_else(|| issue_human_options(Some(issue), &[], &[]));
    if options.iter().any(|option| option == action.as_str()) {
        return Ok(());
    }

    anyhow::bail!(
        "issue decision `{}` is not allowed when issue #{} is `{}`; allowed options: {}",
        action.as_str(),
        issue.id,
        issue.status,
        options.join(", ")
    )
}

fn record_operator_decision_evidence(
    store: &Store,
    issue: &HiveIssue,
    action: IssueDecisionAction,
    comment_id: i64,
    author: &str,
    note: &str,
    next_round: Option<i64>,
    summary: &str,
    comment_body: &str,
) -> Result<()> {
    let Some(loop_id) = issue.loop_id else {
        return Ok(());
    };
    let round = match next_round {
        Some(round) => round,
        None => store
            .get_hive_loop_contract(loop_id)?
            .map(|contract| contract.current_round)
            .unwrap_or(1),
    };

    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id,
        stage_id: None,
        round,
        kind: "operator_decision".to_string(),
        summary: summary.to_string(),
        path: None,
        payload: serde_json::json!({
            "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
            "source": "issue/status/comment",
            "issue": {
                "id": issue.id,
                "comment_id": comment_id,
                "from_status": issue.status,
                "to_status": action.issue_status()
            },
            "loop": {
                "id": loop_id,
                "next_status": action.contract_status(),
                "next_phase": action.contract_phase(),
                "round": round
            },
            "operator": {
                "author": author,
                "action": action.as_str(),
                "note": note,
                "comment_body": comment_body
            }
        }),
    })?;
    Ok(())
}

fn record_operator_comment_evidence(
    store: &Store,
    issue: &HiveIssue,
    comment_id: i64,
    author: &str,
    body: &str,
) -> Result<()> {
    let Some(loop_id) = issue.loop_id else {
        return Ok(());
    };
    let Some(contract) = store.get_hive_loop_contract(loop_id)? else {
        return Ok(());
    };

    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id,
        stage_id: None,
        round: contract.current_round,
        kind: "operator_comment".to_string(),
        summary: body.to_string(),
        path: None,
        payload: serde_json::json!({
            "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
            "source": "issue/status/comment",
            "issue": {
                "id": issue.id,
                "status": issue.status,
                "comment_id": comment_id
            },
            "loop": {
                "id": loop_id,
                "status": contract.status,
                "phase": contract.active_phase,
                "round": contract.current_round
            },
            "operator": {
                "author": author,
                "comment_body": body
            }
        }),
    })?;
    Ok(())
}

fn insert_stage(
    store: &Store,
    contract: &HiveLoopContract,
    role: &str,
    summary: &str,
    input: serde_json::Value,
    output: serde_json::Value,
) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    store.insert_hive_loop_stage(HiveLoopStageCreate {
        loop_id: contract.id,
        round: contract.current_round,
        role: role.to_string(),
        status: "done".to_string(),
        summary: Some(summary.to_string()),
        input,
        output,
        started_at: Some(now.clone()),
        completed_at: Some(now),
    })
}

fn issue_card(store: &Store, issue_id: i64) -> Result<IssueCard> {
    let issue = store
        .get_hive_issue(issue_id)?
        .with_context(|| format!("unknown hive issue `{issue_id}`"))?;
    issue_card_from_issue(store, issue)
}

fn issue_card_from_issue(store: &Store, issue: HiveIssue) -> Result<IssueCard> {
    let comments = store.list_hive_comments(issue.id)?;
    let trace = issue
        .loop_id
        .map(|loop_id| issue_trace_summary(store, loop_id, Some(&issue)))
        .transpose()?;
    let doctor = issue
        .loop_id
        .zip(trace.as_ref())
        .map(|(loop_id, trace)| issue_doctor_summary(store, loop_id, &issue, trace))
        .transpose()?;
    let actions = issue_actions(&issue, trace.as_ref(), doctor.as_ref());
    Ok(IssueCard {
        issue,
        comments,
        actions,
        trace,
        doctor,
    })
}

fn issue_actions(
    issue: &HiveIssue,
    trace: Option<&IssueTraceSummary>,
    doctor: Option<&IssueDoctorSummary>,
) -> Vec<IssueAction> {
    let mut actions = Vec::new();
    let runtime = doctor
        .map(|doctor| doctor.runtime.as_str())
        .filter(|runtime| !runtime.is_empty());

    if issue.loop_id.is_some() && issue.status == "Todo" {
        actions.push(issue_action(
            "run",
            "Run",
            issue_run_action_command(issue, doctor, runtime),
            "runtime",
            "none",
            false,
            runtime,
        ));
    }

    let source = if trace.is_some() {
        "human_options"
    } else {
        "status_fallback"
    };
    let options = trace
        .map(|trace| trace.human_options.clone())
        .unwrap_or_else(|| issue_human_options(Some(issue), &[], &[]));
    for option in options {
        match option.as_str() {
            "comment" => actions.push(issue_action(
                "comment",
                "Comment",
                format!(
                    "entrance hive issue comment {} --body <text> --compact",
                    issue.id
                ),
                source,
                "body",
                false,
                None,
            )),
            "retry" => actions.push(issue_action(
                "retry",
                "Retry",
                issue_retry_action_command(issue.id, doctor, runtime),
                source,
                "note",
                false,
                runtime,
            )),
            "request-review" => actions.push(issue_action(
                "request-review",
                "Review",
                format!(
                    "entrance hive issue decide {} request-review --body <note> --compact",
                    issue.id
                ),
                source,
                "note",
                false,
                None,
            )),
            "cancel" => actions.push(issue_action(
                "cancel",
                "Cancel",
                format!(
                    "entrance hive issue decide {} cancel --body <note> --compact",
                    issue.id
                ),
                source,
                "note",
                true,
                None,
            )),
            _ => {}
        }
    }

    actions
}

fn issue_action(
    action: &str,
    label: &str,
    command: String,
    source: &str,
    input: &str,
    destructive: bool,
    runtime: Option<&str>,
) -> IssueAction {
    IssueAction {
        schema_version: ISSUE_ACTION_SCHEMA_VERSION.to_string(),
        action: action.to_string(),
        label: label.to_string(),
        command,
        source: source.to_string(),
        input: input.to_string(),
        destructive,
        runtime: runtime.map(ToOwned::to_owned),
    }
}

fn issue_run_action_command(
    issue: &HiveIssue,
    doctor: Option<&IssueDoctorSummary>,
    runtime: Option<&str>,
) -> String {
    doctor
        .and_then(|doctor| {
            doctor
                .next_actions
                .iter()
                .find(|action| action.contains("entrance hive issue run"))
                .cloned()
        })
        .unwrap_or_else(|| match runtime {
            Some(runtime) => format!(
                "entrance hive issue run {} --runtime {} --compact",
                issue.id, runtime
            ),
            None => format!("entrance hive issue run {} --compact", issue.id),
        })
}

fn issue_retry_action_command(
    issue_id: i64,
    doctor: Option<&IssueDoctorSummary>,
    runtime: Option<&str>,
) -> String {
    doctor
        .and_then(|doctor| {
            doctor
                .next_actions
                .iter()
                .find(|action| action.contains("entrance hive issue retry-run"))
                .cloned()
        })
        .unwrap_or_else(|| match runtime {
            Some(runtime) => retry_run_command(issue_id, runtime),
            None => format!("entrance hive issue retry-run {issue_id} --body <note> --compact"),
        })
}

fn issue_doctor_summary(
    store: &Store,
    loop_id: i64,
    issue: &HiveIssue,
    trace: &IssueTraceSummary,
) -> Result<IssueDoctorSummary> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let audit_passed = trace.audit_passed.unwrap_or(false);
    let worker_failures = doctor_worker_failures(trace);
    let health = doctor_health(
        &contract.status,
        Some(issue.status.as_str()),
        trace.last_decision.as_deref(),
        audit_passed,
        !worker_failures.is_empty(),
    )
    .to_string();
    Ok(IssueDoctorSummary {
        schema_version: DOCTOR_SCHEMA_VERSION.to_string(),
        health: health.clone(),
        summary: doctor_summary(
            &contract,
            Some(issue.status.as_str()),
            trace,
            audit_passed,
            trace.audit_failed_count,
            &health,
        ),
        current_round: contract.current_round,
        next_actions: doctor_next_actions(
            &health,
            loop_id,
            Some(issue.id),
            &contract.runtime,
            audit_passed,
        ),
        runtime: contract.runtime.clone(),
        counts: doctor_counts(trace),
        failed_checks: trace.audit_failed_checks.clone(),
        audit_failure_details: trace.audit_failure_details.clone(),
        missing_receipts: doctor_missing_receipts(trace),
        worker_failures,
    })
}

fn issue_trace_summary(
    store: &Store,
    loop_id: i64,
    issue: Option<&HiveIssue>,
) -> Result<IssueTraceSummary> {
    issue_trace_summary_inner(store, loop_id, issue, true)
}

fn issue_trace_summary_without_audit(
    store: &Store,
    loop_id: i64,
    issue: Option<&HiveIssue>,
) -> Result<IssueTraceSummary> {
    issue_trace_summary_inner(store, loop_id, issue, false)
}

fn issue_trace_summary_inner(
    store: &Store,
    loop_id: i64,
    issue: Option<&HiveIssue>,
    include_audit: bool,
) -> Result<IssueTraceSummary> {
    let current_round = store
        .get_hive_loop_contract(loop_id)?
        .map(|contract| contract.current_round)
        .unwrap_or(1);
    let packets = store.list_hive_loop_packets(loop_id)?;
    let admissions = store.list_hive_loop_admissions(loop_id)?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let evidence = store.list_hive_loop_evidence(loop_id)?;
    let verdicts = store.list_hive_loop_verdicts(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let packet_rounds = packets
        .iter()
        .map(|packet| (packet.id, packet.round))
        .collect::<HashMap<_, _>>();
    let admission_in_current_round = |admission: &HiveLoopAdmission| {
        packet_rounds
            .get(&admission.packet_id)
            .is_some_and(|round| *round == current_round)
    };
    let last_admission = admissions
        .iter()
        .rev()
        .find(|admission| admission_in_current_round(admission));
    let last_verdict = verdicts
        .iter()
        .rev()
        .find(|verdict| verdict.round == current_round);
    let execution_evidence = evidence
        .iter()
        .rev()
        .find(|row| row.round == current_round && row.kind == "execution_packet");
    let worker = execution_evidence.and_then(|row| row.payload.get("worker"));
    let round_admissions = admissions
        .iter()
        .filter(|admission| admission_in_current_round(admission))
        .collect::<Vec<_>>();
    let role_worker_count = packets
        .iter()
        .filter(|packet| packet_role_worker(&packet.payload).is_some())
        .count();
    let role_worker_ok_count = packets
        .iter()
        .filter_map(|packet| packet_role_worker(&packet.payload))
        .filter(|worker| worker_ok(worker))
        .count();
    let round_role_worker_count = packets
        .iter()
        .filter(|packet| {
            packet.round == current_round && packet_role_worker(&packet.payload).is_some()
        })
        .count();
    let round_role_worker_ok_count = packets
        .iter()
        .filter(|packet| packet.round == current_round)
        .filter_map(|packet| packet_role_worker(&packet.payload))
        .filter(|worker| worker_ok(worker))
        .count();
    let audit_report = if include_audit {
        audit(store, loop_id).ok()
    } else {
        None
    };
    let audit_failed_checks = audit_report
        .as_ref()
        .map(|report| {
            report
                .checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| check.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let audit_failure_details = audit_report
        .as_ref()
        .map(audit_failure_details)
        .unwrap_or_default();
    let round_evidence = evidence
        .iter()
        .filter(|row| row.round == current_round)
        .map(|row| issue_evidence_summary(row, &stage_roles))
        .collect::<Vec<_>>();
    let round_worker_duration_ms = round_evidence
        .iter()
        .filter_map(|row| row.worker_duration_ms)
        .sum();
    let round_worker_timeout_count = round_evidence
        .iter()
        .filter(|row| row.worker_timed_out == Some(true))
        .count();
    let round_worker_retry_exhausted_count = round_evidence
        .iter()
        .filter(|row| row.worker_retry_exhausted == Some(true))
        .count();
    let verdict_human_options = last_verdict
        .map(|verdict| human_options(&verdict.score))
        .unwrap_or_default();
    let operator_events = evidence
        .iter()
        .filter(|row| row.kind == "operator_comment" || row.kind == "operator_decision")
        .map(issue_operator_summary)
        .collect::<Vec<_>>();
    let round_operator_events = operator_events
        .iter()
        .filter(|event| event.round == current_round)
        .cloned()
        .collect::<Vec<_>>();
    let last_operator_event = operator_events.last().cloned();
    let rounds = issue_round_summaries(
        current_round,
        &evidence,
        &admissions,
        &packet_rounds,
        &verdicts,
    );

    Ok(IssueTraceSummary {
        current_round,
        rounds,
        packet_count: packets.len(),
        admission_count: admissions.len(),
        evidence_count: evidence.len(),
        verdict_count: verdicts.len(),
        round_packet_count: packets
            .iter()
            .filter(|packet| packet.round == current_round)
            .count(),
        round_admission_count: round_admissions.len(),
        round_evidence_count: evidence
            .iter()
            .filter(|row| row.round == current_round)
            .count(),
        round_verdict_count: verdicts
            .iter()
            .filter(|verdict| verdict.round == current_round)
            .count(),
        receipt_required_count: admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/required"))
            .sum(),
        receipt_missing_count: admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
            .sum(),
        round_receipt_required_count: round_admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/required"))
            .sum(),
        round_receipt_missing_count: round_admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
            .sum(),
        role_worker_count,
        role_worker_ok_count,
        round_role_worker_count,
        round_role_worker_ok_count,
        round_worker_duration_ms,
        round_worker_timeout_count,
        round_worker_retry_exhausted_count,
        packet_schema: packets
            .iter()
            .rev()
            .find(|packet| packet.round == current_round)
            .and_then(|packet| schema_version(&packet.payload)),
        policy_schema: last_admission
            .and_then(|admission| admission.policy.pointer("/policy/schema_version"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_schema: last_admission.and_then(|admission| schema_version(&admission.policy)),
        verdict_schema: last_verdict.and_then(|verdict| schema_version(&verdict.score)),
        last_admission_gate: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/name"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        last_gate_description: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/spec/description"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        last_gate_expected_object_kind: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/spec/expected_object_kind"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        last_admission_passed: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/passed"))
            .and_then(|value| value.as_bool()),
        last_decision: last_verdict.map(|verdict| verdict.decision.clone()),
        reason_code: last_verdict
            .and_then(|verdict| {
                verdict
                    .score
                    .get("reason_code")
                    .and_then(|value| value.as_str())
                    .or_else(|| {
                        verdict
                            .evidence
                            .get("reason_code")
                            .and_then(|value| value.as_str())
                    })
            })
            .map(ToOwned::to_owned),
        score_vector: last_verdict
            .map(|verdict| score_vector(&verdict.score))
            .unwrap_or_default(),
        human_options: issue_human_options(issue, &verdict_human_options, &round_evidence),
        operator_event_count: operator_events.len(),
        round_operator_event_count: round_operator_events.len(),
        last_operator_event,
        operator_events: round_operator_events,
        worker_kind: worker
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_mode: worker
            .and_then(|value| value.get("mode"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_ok: worker
            .and_then(|value| value.get("ok"))
            .and_then(|value| value.as_bool()),
        audit_schema: audit_report
            .as_ref()
            .map(|report| report.schema_version.clone()),
        audit_passed: audit_report.as_ref().map(|report| report.passed),
        audit_failed_count: audit_report
            .as_ref()
            .map(|report| report.failed_count)
            .unwrap_or_default(),
        audit_failed_checks,
        audit_failure_details,
        evidence: round_evidence,
        stages: issue_stage_summaries(&stages, &evidence, current_round),
    })
}

fn stage_role_map(stages: &[HiveLoopStage]) -> HashMap<i64, String> {
    stages
        .iter()
        .map(|stage| (stage.id, stage.role.clone()))
        .collect()
}

fn issue_round_summaries(
    current_round: i64,
    evidence: &[HiveLoopEvidence],
    admissions: &[HiveLoopAdmission],
    packet_rounds: &HashMap<i64, i64>,
    verdicts: &[HiveLoopVerdict],
) -> Vec<IssueRoundSummary> {
    let mut rounds = Vec::new();
    rounds.push(current_round);
    rounds.extend(evidence.iter().map(|row| row.round));
    rounds.extend(verdicts.iter().map(|verdict| verdict.round));
    rounds.extend(
        admissions
            .iter()
            .filter_map(|admission| packet_rounds.get(&admission.packet_id).copied()),
    );
    rounds.sort_unstable();
    rounds.dedup();
    rounds
        .into_iter()
        .map(|round| {
            let round_evidence = evidence.iter().filter(|row| row.round == round);
            let evidence_count = evidence.iter().filter(|row| row.round == round).count();
            let rejected_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter(|row| evidence_row_rejected(row))
                .count();
            let worker_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter(|row| row.payload.get("worker").is_some())
                .count();
            let worker_ok_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter_map(|row| row.payload.get("worker"))
                .filter(|worker| worker_ok(worker))
                .count();
            let worker_timeout_count = round_evidence
                .clone()
                .filter_map(|row| row.payload.get("worker"))
                .filter(|worker| {
                    worker.get("timed_out").and_then(|value| value.as_bool()) == Some(true)
                })
                .count();
            let worker_retry_exhausted_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter_map(|row| row.payload.get("worker"))
                .filter(|worker| {
                    worker
                        .get("retry_exhausted")
                        .and_then(|value| value.as_bool())
                        == Some(true)
                })
                .count();
            let round_admissions = admissions.iter().filter(|admission| {
                packet_rounds
                    .get(&admission.packet_id)
                    .is_some_and(|packet_round| *packet_round == round)
            });
            let receipt_required_count = round_admissions
                .clone()
                .map(|admission| receipt_array_len(&admission.policy, "/receipt/required"))
                .sum();
            let receipt_missing_count = admissions
                .iter()
                .filter(|admission| {
                    packet_rounds
                        .get(&admission.packet_id)
                        .is_some_and(|packet_round| *packet_round == round)
                })
                .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
                .sum();
            let decision = verdicts
                .iter()
                .rev()
                .find(|verdict| verdict.round == round)
                .map(|verdict| verdict.decision.clone());
            let status = issue_round_status(
                decision.as_deref(),
                rejected_count,
                receipt_missing_count,
                worker_count,
                worker_ok_count,
            )
            .to_string();
            IssueRoundSummary {
                round,
                status,
                decision,
                evidence_count,
                rejected_count,
                receipt_required_count,
                receipt_missing_count,
                worker_count,
                worker_ok_count,
                worker_timeout_count,
                worker_retry_exhausted_count,
            }
        })
        .collect()
}

fn evidence_row_rejected(row: &HiveLoopEvidence) -> bool {
    row.kind == "admission_rejection"
        || row
            .payload
            .get("admission")
            .or_else(|| row.payload.get("result"))
            .and_then(|value| value.as_str())
            == Some("rejected")
}

fn issue_round_status(
    decision: Option<&str>,
    rejected_count: usize,
    receipt_missing_count: usize,
    worker_count: usize,
    worker_ok_count: usize,
) -> &'static str {
    match decision {
        Some("keep") => "kept",
        Some("reject") => "rejected",
        Some("needs-review") => "needs_review",
        Some("blocked") => "blocked",
        _ if rejected_count > 0 || receipt_missing_count > 0 || worker_ok_count < worker_count => {
            "blocked"
        }
        _ if worker_count > 0 => "ran",
        _ => "pending",
    }
}

fn issue_evidence_summary(
    row: &HiveLoopEvidence,
    stage_roles: &HashMap<i64, String>,
) -> IssueEvidenceSummary {
    let worker = row.payload.get("worker");
    let worker_receipt = worker.and_then(worker_structured_receipt);
    let stage_role = row
        .stage_id
        .and_then(|stage_id| stage_roles.get(&stage_id).cloned());
    let worker_receipt_errors = worker
        .map(|worker| worker_receipt_errors_for_summary(worker, stage_role.as_deref()))
        .unwrap_or_default();
    IssueEvidenceSummary {
        id: row.id,
        round: row.round,
        stage_role,
        kind: row.kind.clone(),
        summary: row.summary.clone(),
        schema_version: schema_version(&row.payload),
        admission_result: row
            .payload
            .get("admission")
            .or_else(|| row.payload.get("result"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        blocked_phase: row
            .payload
            .get("phase")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        missing_receipts: string_array_at(&row.payload, "/admission_receipt/receipt/missing"),
        packet_envelope_errors: string_array_at(
            &row.payload,
            "/admission_receipt/packet/envelope/errors",
        ),
        operator_options: string_array_at(&row.payload, "/operator_options"),
        operator_author: string_at(&row.payload, "/operator/author"),
        operator_action: string_at(&row.payload, "/operator/action"),
        worker_kind: worker
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_mode: worker
            .and_then(|value| value.get("mode"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_ok: worker
            .and_then(|value| value.get("ok"))
            .and_then(|value| value.as_bool()),
        worker_receipt_ok: worker
            .and_then(|value| value.get("receipt_ok"))
            .and_then(|value| value.as_bool()),
        worker_timed_out: worker
            .and_then(|value| value.get("timed_out"))
            .and_then(|value| value.as_bool()),
        worker_status: worker
            .and_then(|value| value.get("status"))
            .and_then(|value| value.as_i64()),
        worker_duration_ms: worker
            .and_then(|value| value.get("duration_ms"))
            .and_then(|value| value.as_u64()),
        worker_timeout_secs: worker
            .and_then(|value| value.get("timeout_secs"))
            .and_then(|value| value.as_u64()),
        worker_attempt_count: worker
            .and_then(|value| value.get("attempt_count"))
            .and_then(|value| value.as_u64()),
        worker_max_attempts: worker
            .and_then(|value| value.get("max_attempts"))
            .and_then(|value| value.as_u64()),
        worker_retry_exhausted: worker
            .and_then(|value| value.get("retry_exhausted"))
            .and_then(|value| value.as_bool()),
        worker_command: worker
            .and_then(|value| value.get("command"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_cwd: worker
            .and_then(|value| value.get("cwd"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_action: worker_receipt
            .as_ref()
            .and_then(|receipt| receipt.get("action"))
            .and_then(|value| value.as_str())
            .or_else(|| {
                worker
                    .and_then(|value| value.pointer("/packet/action"))
                    .and_then(|value| value.as_str())
            })
            .map(ToOwned::to_owned),
        worker_evidence_summary: worker_receipt
            .as_ref()
            .and_then(|receipt| receipt.get("evidence_summary"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_gate_count: worker_receipt
            .as_ref()
            .and_then(|receipt| receipt.get("gates"))
            .and_then(|value| value.as_object())
            .map(serde_json::Map::len),
        worker_receipt_errors,
        transcript_excerpt: worker
            .and_then(worker_transcript_excerpt)
            .map(|value| truncate_text(&value, 240)),
    }
}

fn worker_receipt_errors_for_summary(
    worker: &serde_json::Value,
    expected_role: Option<&str>,
) -> Vec<String> {
    let stored_errors = string_array_at(worker, "/receipt_errors");
    if !stored_errors.is_empty() {
        return stored_errors;
    }
    match worker_structured_receipt(worker) {
        Some(receipt) => worker_receipt_contract_errors(&receipt, expected_role),
        None if worker.get("ok").and_then(|value| value.as_bool()) == Some(true) => {
            vec!["receipt".to_string()]
        }
        None => Vec::new(),
    }
}

fn issue_operator_summary(row: &HiveLoopEvidence) -> IssueOperatorSummary {
    IssueOperatorSummary {
        id: row.id,
        round: row.round,
        kind: row.kind.clone(),
        author: string_at(&row.payload, "/operator/author"),
        action: string_at(&row.payload, "/operator/action"),
        issue_status: string_at(&row.payload, "/issue/to_status")
            .or_else(|| string_at(&row.payload, "/issue/status")),
        loop_status: string_at(&row.payload, "/loop/next_status")
            .or_else(|| string_at(&row.payload, "/loop/status")),
        note: string_at(&row.payload, "/operator/note")
            .or_else(|| string_at(&row.payload, "/operator/comment_body"))
            .map(|value| truncate_text(&value, 180)),
        summary: truncate_text(&row.summary, 180),
    }
}

fn worker_transcript_excerpt(worker: &serde_json::Value) -> Option<String> {
    ["last_message", "error", "stderr", "stdout"]
        .into_iter()
        .filter_map(|key| worker.get(key).and_then(|value| value.as_str()))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn issue_stage_summaries(
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
    current_round: i64,
) -> Vec<IssueStageSummary> {
    stages
        .iter()
        .filter(|stage| stage.round == current_round)
        .map(|stage| {
            let stage_evidence = evidence
                .iter()
                .rev()
                .find(|row| row.stage_id == Some(stage.id));
            let worker = stage_evidence
                .and_then(|row| row.payload.get("worker"))
                .or_else(|| stage.output.get("role_worker"))
                .or_else(|| stage.output.get("runtime_worker"));
            IssueStageSummary {
                role: stage.role.clone(),
                status: stage.status.clone(),
                summary: stage.summary.clone(),
                evidence_kind: stage_evidence.map(|row| row.kind.clone()),
                evidence_summary: stage_evidence.map(|row| row.summary.clone()),
                admission_result: stage_evidence
                    .and_then(|row| {
                        row.payload
                            .get("admission")
                            .or_else(|| row.payload.get("result"))
                    })
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                worker_kind: worker
                    .and_then(|value| value.get("kind"))
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                worker_mode: worker
                    .and_then(|value| value.get("mode"))
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                worker_ok: worker
                    .and_then(|value| value.get("ok"))
                    .and_then(|value| value.as_bool()),
            }
        })
        .collect()
}

fn schema_version(value: &serde_json::Value) -> Option<String> {
    value
        .get("schema_version")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn receipt_array_len(value: &serde_json::Value, pointer: &str) -> usize {
    value
        .pointer(pointer)
        .and_then(|value| value.as_array())
        .map(Vec::len)
        .unwrap_or_default()
}

fn string_array_at(value: &serde_json::Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn string_at(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn human_options(value: &serde_json::Value) -> Vec<String> {
    value
        .get("human_options")
        .and_then(|value| value.as_array())
        .map(|options| {
            options
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn issue_human_options(
    issue: Option<&HiveIssue>,
    verdict_options: &[String],
    evidence: &[IssueEvidenceSummary],
) -> Vec<String> {
    let Some(issue) = issue else {
        return verdict_options.to_vec();
    };
    match issue.status.as_str() {
        "Todo" => option_list(&["comment", "cancel"]),
        "Doing" | "Done" => option_list(&["comment"]),
        "Blocked" => option_list(&["comment", "retry", "request-review", "cancel"]),
        "Needs Review" => option_list(&["comment", "retry", "cancel"]),
        "Canceled" if latest_operator_action(evidence) == Some("cancel") => {
            option_list(&["comment"])
        }
        "Canceled" if verdict_options.iter().any(|option| option == "retry") => {
            option_list(&["comment", "retry"])
        }
        "Canceled" => option_list(&["comment"]),
        _ if verdict_options.is_empty() => option_list(&["comment"]),
        _ => verdict_options.to_vec(),
    }
}

fn latest_operator_action(evidence: &[IssueEvidenceSummary]) -> Option<&str> {
    evidence
        .iter()
        .rev()
        .find(|row| row.kind == "operator_decision")
        .and_then(|row| row.operator_action.as_deref())
}

fn option_list(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn score_vector(value: &serde_json::Value) -> Vec<ScoreVectorMetric> {
    let Some(metrics) = value
        .get("score_vector")
        .and_then(|value| value.as_object())
    else {
        return Vec::new();
    };
    let preferred_order = [
        "stage_completeness",
        "runtime_readiness",
        "evidence_presence",
        "admission_integrity",
    ];
    let mut output = preferred_order
        .into_iter()
        .filter_map(|name| {
            metrics.get(name).map(|value| ScoreVectorMetric {
                name: name.to_string(),
                value: value.as_f64(),
            })
        })
        .collect::<Vec<_>>();
    let mut extra_names = metrics
        .keys()
        .filter(|name| !preferred_order.contains(&name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    extra_names.sort();
    output.extend(extra_names.into_iter().map(|name| ScoreVectorMetric {
        value: metrics.get(&name).and_then(|value| value.as_f64()),
        name,
    }));
    output
}

fn add_system_comment(
    store: &Store,
    issue_id: i64,
    body: &str,
    payload: serde_json::Value,
) -> Result<()> {
    store.insert_hive_comment(HiveCommentCreate {
        issue_id,
        author: "hive".to_string(),
        body: body.to_string(),
        payload: system_comment_payload("hive", payload),
    })?;
    Ok(())
}

fn add_stage_system_comment(
    store: &Store,
    issue_id: i64,
    loop_id: i64,
    round: i64,
    role: &str,
    evidence_kind: &str,
    evidence_id: i64,
    body: &str,
    admission: &str,
    worker: &serde_json::Value,
) -> Result<()> {
    add_system_comment(
        store,
        issue_id,
        body,
        serde_json::json!({
            "loop_id": loop_id,
            "round": round,
            "phase": role,
            "stage_role": role,
            "evidence_kind": evidence_kind,
            "evidence_id": evidence_id,
            "admission": admission,
            "worker": worker
        }),
    )
}

fn system_comment_payload(source: &str, payload: serde_json::Value) -> serde_json::Value {
    let mut typed = serde_json::Map::new();
    typed.insert(
        "schema_version".to_string(),
        serde_json::Value::String(SYSTEM_COMMENT_SCHEMA_VERSION.to_string()),
    );
    typed.insert(
        "source".to_string(),
        serde_json::Value::String(source.to_string()),
    );
    match payload {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if key != "schema_version" && key != "source" {
                    typed.insert(key, value);
                }
            }
        }
        other => {
            typed.insert("details".to_string(), other);
        }
    }
    serde_json::Value::Object(typed)
}

fn block_on_admission_rejection(
    store: &Store,
    contract: &HiveLoopContract,
    issue_id: Option<i64>,
    phase: &str,
    stage_id: Option<i64>,
    admission: &HiveLoopAdmission,
) -> Result<HiveLoopReport> {
    let summary = format!(
        "Compiler admission blocked at {phase}: {}.",
        admission.reason
    );
    let rejected_packet = store.get_hive_loop_packet(admission.packet_id)?;
    let rejected_worker = rejected_packet
        .as_ref()
        .and_then(|packet| packet_role_worker(&packet.payload))
        .cloned();
    let evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id,
        round: contract.current_round,
        kind: "admission_rejection".to_string(),
        summary: summary.clone(),
        path: None,
        payload: serde_json::json!({
            "phase": phase,
            "admission_id": admission.id,
            "packet_id": admission.packet_id,
            "result": admission.result,
            "reason": admission.reason,
            "worker": rejected_worker,
            "admission_receipt": admission.policy.clone(),
            "operator_options": ["fix-policy", "retry", "request-human-review"]
        }),
    })?;

    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: "blocked".to_string(),
        summary: summary.clone(),
        score: admission_rejection_score_payload(phase),
        evidence: admission_rejection_verdict_evidence_payload(evidence_id, phase, admission),
    })?;

    store.update_hive_loop_contract_state(contract.id, "blocked", phase, contract.current_round)?;

    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(issue_id, "Blocked", Some(&summary))?;
        add_system_comment(
            store,
            issue_id,
            &summary,
            serde_json::json!({
                "loop_id": contract.id,
                "phase": phase,
                "decision": "blocked",
                "reason_code": "admission_rejected",
                "admission": {
                    "id": admission.id,
                    "packet_id": admission.packet_id,
                    "result": admission.result,
                    "reason": admission.reason
                },
                "operator_options": ["fix-policy", "retry", "request-human-review"]
            }),
        )?;
    }

    report(store, contract.id)
}

fn stage_completeness_for_phase(phase: &str) -> f64 {
    match phase {
        "explorer" => 0.33,
        "developer" => 0.66,
        "reviewer" => 1.0,
        "doer" => 0.66,
        "evaluator" => 1.0,
        _ => 0.0,
    }
}

fn admission_rejection_score_payload(phase: &str) -> serde_json::Value {
    let stage_completeness = stage_completeness_for_phase(phase);
    serde_json::json!({
        "schema_version": VERDICT_SCHEMA_VERSION,
        "decision": "blocked",
        "reason_code": "admission_rejected",
        "gates_passed": false,
        "operator_review_needed": true,
        "score_vector": {
            "stage_completeness": stage_completeness,
            "runtime_readiness": serde_json::Value::Null,
            "evidence_presence": 1.0,
            "admission_integrity": 0.0
        },
        "gate_results": {
            "admission_passed": false,
            "blocked_phase": phase
        },
        "human_options": ["comment", "retry", "request-review", "cancel"]
    })
}

fn admission_rejection_verdict_evidence_payload(
    evidence_id: i64,
    phase: &str,
    admission: &HiveLoopAdmission,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": VERDICT_SCHEMA_VERSION,
        "decision": "blocked",
        "reason_code": "admission_rejected",
        "evidence_id": evidence_id,
        "admission_id": admission.id,
        "packet_id": admission.packet_id,
        "phase": phase,
        "source": {
            "reviewer": "hive-loop-control",
            "admission_receipt": admission.policy.clone()
        }
    })
}

impl IssueDecisionAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::RequestReview => "request-review",
            Self::Cancel => "cancel",
        }
    }

    fn issue_status(self) -> &'static str {
        match self {
            Self::Retry => "Todo",
            Self::RequestReview => "Needs Review",
            Self::Cancel => "Canceled",
        }
    }

    fn contract_status(self) -> &'static str {
        match self {
            Self::Retry => "todo",
            Self::RequestReview => "needs-review",
            Self::Cancel => "rejected",
        }
    }

    fn contract_phase(self) -> &'static str {
        match self {
            Self::Retry => "explorer",
            Self::RequestReview => "human-review",
            Self::Cancel => "complete",
        }
    }

    fn issue_summary(self, next_round: Option<i64>) -> String {
        match self {
            Self::Retry => format!(
                "Human chose retry; loop returned to Explorer for round {}.",
                next_round.unwrap_or(1)
            ),
            Self::RequestReview => "Human requested review before the loop continues.".to_string(),
            Self::Cancel => "Human canceled this loop issue.".to_string(),
        }
    }

    fn comment_body(self, next_round: Option<i64>, note: &str) -> String {
        let base = self.issue_summary(next_round);
        if note.is_empty() {
            base
        } else {
            format!("{base} Note: {note}")
        }
    }
}

fn parse_issue_decision_action(value: &str) -> Result<IssueDecisionAction> {
    match value {
        "retry" => Ok(IssueDecisionAction::Retry),
        "request-review" => Ok(IssueDecisionAction::RequestReview),
        "cancel" => Ok(IssueDecisionAction::Cancel),
        other => anyhow::bail!(
            "unsupported issue decision `{other}`; expected retry, request-review, or cancel"
        ),
    }
}

impl VerdictDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Reject => "reject",
            Self::NeedsReview => "needs-review",
            Self::Blocked => "blocked",
        }
    }

    fn contract_status(self) -> &'static str {
        match self {
            Self::Keep => "kept",
            Self::Reject => "rejected",
            Self::NeedsReview => "needs-review",
            Self::Blocked => "blocked",
        }
    }

    fn issue_status(self) -> &'static str {
        match self {
            Self::Keep => "Done",
            Self::Reject => "Canceled",
            Self::NeedsReview => "Needs Review",
            Self::Blocked => "Blocked",
        }
    }

    fn operator_review_required(self) -> bool {
        !matches!(self, Self::Keep)
    }

    fn gates_passed(self) -> bool {
        matches!(self, Self::Keep)
    }

    fn human_options(self) -> Vec<&'static str> {
        match self {
            Self::Keep => vec!["comment"],
            Self::Reject => vec!["comment", "retry"],
            Self::NeedsReview => vec!["comment", "retry", "cancel"],
            Self::Blocked => vec!["comment", "retry", "request-review", "cancel"],
        }
    }
}

impl TypedVerdict {
    fn score_payload(&self) -> serde_json::Value {
        let runtime_readiness = if self.runtime_ready { 1.0 } else { 0.0 };
        serde_json::json!({
            "schema_version": VERDICT_SCHEMA_VERSION,
            "decision": self.decision.as_str(),
            "reason_code": self.reason_code,
            "gates_passed": self.decision.gates_passed(),
            "operator_review_needed": self.decision.operator_review_required(),
            "score_vector": {
                "stage_completeness": 1.0,
                "runtime_readiness": runtime_readiness,
                "evidence_presence": if self.evidence_count > 0 { 1.0 } else { 0.0 },
                "admission_integrity": 1.0
            },
            "gate_results": {
                "three_stages_recorded": true,
                "evidence_recorded": self.evidence_count > 0,
                "runtime_ready": self.runtime_ready,
                "decision_allowed": matches!(
                    self.decision,
                    VerdictDecision::Keep
                        | VerdictDecision::Reject
                        | VerdictDecision::NeedsReview
                        | VerdictDecision::Blocked
                )
            },
            "human_options": self.decision.human_options()
        })
    }

    fn evidence_payload(
        &self,
        runtime: &str,
        reviewer_worker: &serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "schema_version": VERDICT_SCHEMA_VERSION,
            "decision": self.decision.as_str(),
            "reason_code": self.reason_code,
            "evidence_count": self.evidence_count + 1,
            "runtime": runtime,
            "runtime_ready": self.runtime_ready,
            "role_worker": reviewer_worker,
            "source": {
                "reviewer": "hive-loop-control",
                "round_evidence_before_verdict": self.evidence_count
            }
        })
    }

    fn packet_payload(&self, reviewer_worker: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "decision": self.decision.as_str(),
            "summary": self.summary,
            "reason_code": self.reason_code,
            "score": self.score_payload(),
            "role_worker": reviewer_worker
        })
    }
}

fn parse_decision_override(value: Option<&str>) -> Result<Option<VerdictDecision>> {
    value
        .map(|value| match value {
            "keep" => Ok(VerdictDecision::Keep),
            "reject" => Ok(VerdictDecision::Reject),
            "needs-review" => Ok(VerdictDecision::NeedsReview),
            "blocked" => Ok(VerdictDecision::Blocked),
            other => anyhow::bail!(
                "unsupported reviewer decision `{other}`; expected keep, reject, needs-review, or blocked"
            ),
        })
        .transpose()
}

fn build_verdict(
    decision_override: Option<VerdictDecision>,
    runtime_ready: bool,
    runtime_failure: Option<RuntimeFailure>,
    runtime: &str,
    evidence_count: usize,
    round: i64,
) -> TypedVerdict {
    if !runtime_ready {
        let reason_code = runtime_failure
            .unwrap_or(RuntimeFailure::Worker)
            .reason_code();
        return TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code,
            summary: format!(
                "Reviewer blocked the candidate: `{runtime}` {}.",
                runtime_failure
                    .unwrap_or(RuntimeFailure::Worker)
                    .summary_fragment()
            ),
            runtime_ready,
            evidence_count,
        };
    }

    if decision_override == Some(VerdictDecision::Reject) && round >= REVIEWER_INVALID_ROUND_BUDGET
    {
        return TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code: "review_budget_exhausted",
            summary: format!(
                "Reviewer blocked the issue: candidate was still invalid after {REVIEWER_INVALID_ROUND_BUDGET} review rounds."
            ),
            runtime_ready,
            evidence_count,
        };
    }

    match decision_override.unwrap_or(VerdictDecision::Keep) {
        VerdictDecision::Keep => TypedVerdict {
            decision: VerdictDecision::Keep,
            reason_code: "all_gates_passed",
            summary: "Reviewer kept the candidate: all MVP gates passed.".to_string(),
            runtime_ready,
            evidence_count,
        },
        VerdictDecision::Reject => TypedVerdict {
            decision: VerdictDecision::Reject,
            reason_code: "quality_gate_failed",
            summary: "Reviewer rejected the candidate: quality gate failed.".to_string(),
            runtime_ready,
            evidence_count,
        },
        VerdictDecision::NeedsReview => TypedVerdict {
            decision: VerdictDecision::NeedsReview,
            reason_code: "human_review_required",
            summary: "Reviewer requested human review for this candidate.".to_string(),
            runtime_ready,
            evidence_count,
        },
        VerdictDecision::Blocked => TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code: "operator_blocked",
            summary: "Reviewer blocked the candidate by operator decision.".to_string(),
            runtime_ready,
            evidence_count,
        },
    }
}

impl RuntimeFailure {
    fn reason_code(self) -> &'static str {
        match self {
            Self::Probe => "runtime_probe_failed",
            Self::Worker => "runtime_worker_failed",
        }
    }

    fn summary_fragment(self) -> &'static str {
        match self {
            Self::Probe => "runtime probe failed",
            Self::Worker => "worker execution failed",
        }
    }
}

impl GateCheck {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReceiptRequirementsSatisfied => "receipt_requirements_satisfied",
            Self::BodyFieldPresent(_) => "body_field_present",
            Self::DecisionPresent => "decision_present",
            Self::ExternalReceiptCurrent => "external_receipt_current",
        }
    }
}

impl From<GateSpec> for PolicyGateSpec {
    fn from(spec: GateSpec) -> Self {
        Self {
            schema_version: POLICY_SCHEMA_VERSION.to_string(),
            name: spec.name.to_string(),
            description: spec.description.to_string(),
            expected_object_kind: spec.expected_object_kind.map(ToOwned::to_owned),
            required_receipts: spec
                .required_receipts
                .iter()
                .map(|receipt| (*receipt).to_string())
                .collect(),
            check: spec.check.as_str().to_string(),
        }
    }
}

fn runtime_failure(
    runtime_probe: &serde_json::Value,
    runtime_worker: &serde_json::Value,
) -> Option<RuntimeFailure> {
    if !runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        Some(RuntimeFailure::Probe)
    } else if !runtime_worker
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        Some(RuntimeFailure::Worker)
    } else {
        None
    }
}

fn runtime_worker_summary(runtime_worker: &serde_json::Value) -> String {
    let kind = runtime_worker
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let mode = runtime_worker
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let ok = runtime_worker
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let role = runtime_worker
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("worker");
    format!("{role}:{kind}/{mode} ok={ok}")
}

fn worker_ok(worker: &serde_json::Value) -> bool {
    worker
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn worker_timeout_secs(requested: Option<u64>) -> Result<u64> {
    let value = match requested {
        Some(value) => value,
        None => match std::env::var("ENTRANCE_HIVE_WORKER_TIMEOUT_SECS") {
            Ok(value) if !value.trim().is_empty() => value
                .trim()
                .parse::<u64>()
                .with_context(|| "ENTRANCE_HIVE_WORKER_TIMEOUT_SECS must be a positive integer")?,
            _ => DEFAULT_WORKER_TIMEOUT_SECS,
        },
    };
    if value == 0 {
        anyhow::bail!("worker timeout must be at least 1 second");
    }
    if value > MAX_WORKER_TIMEOUT_SECS {
        anyhow::bail!(
            "worker timeout must be <= {} seconds",
            MAX_WORKER_TIMEOUT_SECS
        );
    }
    Ok(value)
}

fn worker_attempts(requested: Option<u64>) -> Result<u64> {
    let value = match requested {
        Some(value) => value,
        None => match std::env::var("ENTRANCE_HIVE_WORKER_ATTEMPTS") {
            Ok(value) if !value.trim().is_empty() => value
                .trim()
                .parse::<u64>()
                .with_context(|| "ENTRANCE_HIVE_WORKER_ATTEMPTS must be a positive integer")?,
            _ => DEFAULT_WORKER_ATTEMPTS,
        },
    };
    if value == 0 {
        anyhow::bail!("worker attempts must be at least 1");
    }
    if value > MAX_WORKER_ATTEMPTS {
        anyhow::bail!("worker attempts must be <= {}", MAX_WORKER_ATTEMPTS);
    }
    Ok(value)
}

fn seed_default_policies(store: &Store, loop_id: i64) -> Result<()> {
    for policy in DEFAULT_LOOP_POLICIES {
        store.insert_hive_loop_policy(HiveLoopPolicyCreate {
            loop_id,
            object_kind: policy.object_kind.to_string(),
            writer_role: policy.writer_role.to_string(),
            route_from: policy.route_from.to_string(),
            route_to: policy.route_to.to_string(),
            gate: policy.gate.to_string(),
            status: "active".to_string(),
        })?;
    }
    Ok(())
}

fn emit_and_admit(
    store: &Store,
    contract: &HiveLoopContract,
    object_kind: &str,
    writer_role: &str,
    route_from: &str,
    route_to: &str,
    payload: serde_json::Value,
) -> Result<HiveLoopAdmission> {
    let packet_payload = typed_packet_payload(
        contract,
        object_kind,
        writer_role,
        route_from,
        route_to,
        payload,
    );
    let packet_id = store.insert_hive_loop_packet(HiveLoopPacketCreate {
        loop_id: contract.id,
        round: contract.current_round,
        object_kind: object_kind.to_string(),
        writer_role: writer_role.to_string(),
        route_from: route_from.to_string(),
        route_to: route_to.to_string(),
        state_code: "submitted".to_string(),
        payload: packet_payload.clone(),
    })?;
    let packet = store
        .get_hive_loop_packet(packet_id)?
        .expect("newly created packet should exist");
    let policies = store.list_hive_loop_policies(contract.id)?;
    let matching_policy = policies.iter().find(|policy| {
        policy.status == "active"
            && policy.object_kind == packet.object_kind
            && policy.writer_role == packet.writer_role
            && policy.route_from == packet.route_from
            && policy.route_to == packet.route_to
    });

    let (result, reason, gate_name, gate_passed) = match matching_policy {
        Some(policy) => {
            let passed = gate_passes(&policy.gate, &packet_payload);
            let result = if passed { "admitted" } else { "rejected" };
            let reason = if passed {
                format!("{} passed", policy.gate)
            } else {
                gate_failure_reason(&policy.gate, &packet_payload)
            };
            (
                result.to_string(),
                reason,
                Some(policy.gate.as_str()),
                Some(passed),
            )
        }
        None => (
            "rejected".to_string(),
            "no active policy matched packet writer and route".to_string(),
            None,
            None,
        ),
    };
    let admission_receipt = typed_admission_receipt(
        &packet,
        &packet_payload,
        matching_policy,
        &result,
        &reason,
        gate_name,
        gate_passed,
    );

    let admission_id = store.insert_hive_loop_admission(HiveLoopAdmissionCreate {
        loop_id: contract.id,
        packet_id,
        result: result.clone(),
        reason: reason.clone(),
        policy: admission_receipt,
    })?;
    let admission = store
        .list_hive_loop_admissions(contract.id)?
        .into_iter()
        .find(|admission| admission.id == admission_id)
        .expect("newly created admission should exist");

    Ok(admission)
}

fn typed_packet_payload(
    contract: &HiveLoopContract,
    object_kind: &str,
    writer_role: &str,
    route_from: &str,
    route_to: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": PACKET_SCHEMA_VERSION,
        "loop_id": contract.id,
        "round": contract.current_round,
        "object_kind": object_kind,
        "writer": {
            "role": writer_role
        },
        "route": {
            "from": route_from,
            "to": route_to
        },
        "state_code": "submitted",
        "body": body,
        "receipt_requirements": receipt_requirements_for_packet(object_kind)
    })
}

fn receipt_requirements_for_packet(object_kind: &str) -> Vec<&'static str> {
    match object_kind {
        "EXPLORATION_PACKET" => vec!["candidate", "constraints", "role_worker"],
        "EXECUTION_PACKET" => vec!["runtime_probe", "runtime_worker", "artifact", "role_worker"],
        "VERDICT_PACKET" => vec!["decision", "summary", "score", "role_worker"],
        _ => Vec::new(),
    }
}

fn gate_spec(gate: &str) -> Option<GateSpec> {
    match gate {
        "candidate_receipts_present" => Some(GateSpec {
            name: "candidate_receipts_present",
            description: "Explorer packets must carry the candidate, constraints, and role worker receipt.",
            expected_object_kind: Some("EXPLORATION_PACKET"),
            required_receipts: &["candidate", "constraints", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        "runtime_receipts_present" => Some(GateSpec {
            name: "runtime_receipts_present",
            description: "Developer packets must carry runtime probe, runtime worker, artifact, and role worker receipts.",
            expected_object_kind: Some("EXECUTION_PACKET"),
            required_receipts: &["runtime_probe", "runtime_worker", "artifact", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        "verdict_receipts_present" => Some(GateSpec {
            name: "verdict_receipts_present",
            description: "Reviewer packets must carry decision, summary, score, and role worker receipts.",
            expected_object_kind: Some("VERDICT_PACKET"),
            required_receipts: &["decision", "summary", "score", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        "candidate_present" => Some(GateSpec {
            name: "candidate_present",
            description: "Packet body must include a non-empty candidate.",
            expected_object_kind: None,
            required_receipts: &["candidate"],
            check: GateCheck::BodyFieldPresent("candidate"),
        }),
        "runtime_probe_present" => Some(GateSpec {
            name: "runtime_probe_present",
            description: "Packet body must include runtime probe evidence.",
            expected_object_kind: None,
            required_receipts: &["runtime_probe"],
            check: GateCheck::BodyFieldPresent("runtime_probe"),
        }),
        "decision_present" => Some(GateSpec {
            name: "decision_present",
            description: "Packet body must include an allowed reviewer decision.",
            expected_object_kind: None,
            required_receipts: &["decision"],
            check: GateCheck::DecisionPresent,
        }),
        CONNECTOR_MIRROR_RECEIPT_GATE => Some(GateSpec {
            name: CONNECTOR_MIRROR_RECEIPT_GATE,
            description: "Connector mirror receipts must match the current Hive issue mirror before external issue/status/comment surfaces trust them.",
            expected_object_kind: Some(CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND),
            required_receipts: &[
                "mirror_file_current",
                "receipt_schema",
                "receipt_binding",
                "receipt_digest",
            ],
            check: GateCheck::ExternalReceiptCurrent,
        }),
        _ => None,
    }
}

fn all_gate_specs() -> Vec<GateSpec> {
    [
        "candidate_receipts_present",
        "runtime_receipts_present",
        "verdict_receipts_present",
        "candidate_present",
        "runtime_probe_present",
        "decision_present",
        CONNECTOR_MIRROR_RECEIPT_GATE,
    ]
    .into_iter()
    .filter_map(gate_spec)
    .collect()
}

fn gate_spec_payload(gate: &str) -> serde_json::Value {
    gate_spec(gate)
        .map(|spec| {
            serde_json::json!({
                "schema_version": POLICY_SCHEMA_VERSION,
                "name": spec.name,
                "description": spec.description,
                "expected_object_kind": spec.expected_object_kind,
                "required_receipts": spec.required_receipts,
                "check": spec.check.as_str()
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

fn typed_admission_receipt(
    packet: &HiveLoopPacket,
    packet_payload: &serde_json::Value,
    policy: Option<&HiveLoopPolicy>,
    result: &str,
    reason: &str,
    gate_name: Option<&str>,
    gate_passed: Option<bool>,
) -> serde_json::Value {
    let (required_receipts, missing_receipts) = receipt_requirement_status(packet_payload);
    let receipt_satisfied = missing_receipts.is_empty();
    let packet_envelope_errors = typed_packet_envelope_errors(packet_payload);
    serde_json::json!({
        "schema_version": ADMISSION_SCHEMA_VERSION,
        "result": result,
        "reason": reason,
        "packet": {
            "id": packet.id,
            "schema_version": packet_payload
                .get("schema_version")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown"),
            "object_kind": &packet.object_kind,
            "writer_role": &packet.writer_role,
            "route_from": &packet.route_from,
            "route_to": &packet.route_to,
            "state_code": &packet.state_code,
            "envelope": {
                "valid": packet_envelope_errors.is_empty(),
                "errors": packet_envelope_errors
            }
        },
        "policy": policy.map(|policy| serde_json::json!({
            "schema_version": POLICY_SCHEMA_VERSION,
            "id": policy.id,
            "object_kind": &policy.object_kind,
            "writer_role": &policy.writer_role,
            "route_from": &policy.route_from,
            "route_to": &policy.route_to,
            "gate": &policy.gate,
            "status": &policy.status,
            "gate_spec": gate_spec_payload(&policy.gate)
        })),
        "gate": {
            "name": gate_name,
            "passed": gate_passed,
            "spec": gate_name
                .map(gate_spec_payload)
                .unwrap_or(serde_json::Value::Null)
        },
        "receipt": {
            "required": required_receipts,
            "missing": missing_receipts,
            "satisfied": receipt_satisfied
        }
    })
}

fn gate_passes(gate: &str, payload: &serde_json::Value) -> bool {
    if !typed_packet_envelope_valid(payload) {
        return false;
    }
    let Some(spec) = gate_spec(gate) else {
        return false;
    };
    if spec
        .expected_object_kind
        .is_some_and(|expected| packet_object_kind(payload) != Some(expected))
    {
        return false;
    }
    let body = packet_body(payload);
    match spec.check {
        GateCheck::ReceiptRequirementsSatisfied => receipt_requirements_satisfied(payload),
        GateCheck::BodyFieldPresent(field) => body.get(field).is_some_and(|value| match value {
            serde_json::Value::Null => false,
            serde_json::Value::String(text) => !text.trim().is_empty(),
            serde_json::Value::Array(values) => !values.is_empty(),
            serde_json::Value::Object(values) => !values.is_empty(),
            serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
        }),
        GateCheck::DecisionPresent => body
            .get("decision")
            .and_then(|value| value.as_str())
            .is_some_and(|value| matches!(value, "keep" | "reject" | "needs-review" | "blocked")),
        GateCheck::ExternalReceiptCurrent => receipt_requirements_satisfied(payload),
    }
}

fn receipt_requirements_satisfied(payload: &serde_json::Value) -> bool {
    let (_required, missing) = receipt_requirement_status(payload);
    missing.is_empty()
}

fn gate_failure_reason(gate: &str, payload: &serde_json::Value) -> String {
    let envelope_errors = typed_packet_envelope_errors(payload);
    if !envelope_errors.is_empty() {
        return format!(
            "{gate} failed: typed packet envelope invalid: {}",
            envelope_errors.join(", ")
        );
    }
    let (_required, missing) = receipt_requirement_status(payload);
    if missing.is_empty() {
        format!("{gate} failed")
    } else {
        format!(
            "{gate} failed: missing or invalid receipts {}",
            missing.join(", ")
        )
    }
}

fn receipt_requirement_status(payload: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let required = packet_receipt_requirements(payload);
    let body = packet_body(payload);
    let expected_worker_role = payload
        .pointer("/writer/role")
        .and_then(|value| value.as_str());
    let missing = required
        .iter()
        .filter(|requirement| !receipt_value_present(body, requirement, expected_worker_role))
        .cloned()
        .collect::<Vec<_>>();
    (required, missing)
}

fn packet_receipt_requirements(payload: &serde_json::Value) -> Vec<String> {
    let declared = payload
        .get("receipt_requirements")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !declared.is_empty() {
        return declared;
    }
    packet_object_kind(payload)
        .map(receipt_requirements_for_packet)
        .unwrap_or_default()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn receipt_value_present(
    body: &serde_json::Value,
    requirement: &str,
    expected_worker_role: Option<&str>,
) -> bool {
    if matches!(requirement, "role_worker" | "runtime_worker") {
        return body
            .get(requirement)
            .is_some_and(|worker| worker_receipt_valid(worker, expected_worker_role));
    }
    body.get(requirement).is_some_and(|value| match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        serde_json::Value::Array(values) => !values.is_empty(),
        serde_json::Value::Object(values) => !values.is_empty(),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    })
}

fn worker_receipt_valid(worker: &serde_json::Value, expected_role: Option<&str>) -> bool {
    worker_ok(worker)
        && worker_structured_receipt(worker).is_some_and(|receipt| {
            worker_receipt_contract_errors(&receipt, expected_role).is_empty()
        })
}

fn packet_object_kind(payload: &serde_json::Value) -> Option<&str> {
    payload.get("object_kind").and_then(|value| value.as_str())
}

fn typed_packet_envelope_valid(payload: &serde_json::Value) -> bool {
    typed_packet_envelope_errors(payload).is_empty()
}

fn typed_packet_envelope_errors(payload: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();
    if payload
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(PACKET_SCHEMA_VERSION)
    {
        errors.push("schema_version".to_string());
    }
    if payload
        .get("loop_id")
        .and_then(|value| value.as_i64())
        .is_none()
    {
        errors.push("loop_id".to_string());
    }
    if payload
        .get("round")
        .and_then(|value| value.as_i64())
        .is_none()
    {
        errors.push("round".to_string());
    }
    if payload
        .get("object_kind")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("object_kind".to_string());
    }
    if payload
        .pointer("/writer/role")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("writer.role".to_string());
    }
    if payload
        .pointer("/route/from")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("route.from".to_string());
    }
    if payload
        .pointer("/route/to")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("route.to".to_string());
    }
    if payload.get("state_code").and_then(|value| value.as_str()) != Some("submitted") {
        errors.push("state_code".to_string());
    }
    if payload.get("body").is_none() {
        errors.push("body".to_string());
    }
    errors
}

fn packet_body(payload: &serde_json::Value) -> &serde_json::Value {
    payload.get("body").unwrap_or(payload)
}

fn packet_role_worker(payload: &serde_json::Value) -> Option<&serde_json::Value> {
    packet_body(payload).get("role_worker")
}

fn probe_runtime(runtime: &str) -> serde_json::Value {
    match runtime {
        "local" => serde_json::json!({
            "ok": true,
            "kind": "local",
            "detail": "local deterministic runtime"
        }),
        "codex" => match Command::new("codex").arg("--version").output() {
            Ok(output) => serde_json::json!({
                "ok": output.status.success(),
                "kind": "codex",
                "status": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout).trim(),
                "stderr": String::from_utf8_lossy(&output.stderr).trim()
            }),
            Err(error) => serde_json::json!({
                "ok": false,
                "kind": "codex",
                "error": error.to_string()
            }),
        },
        other => serde_json::json!({
            "ok": false,
            "kind": "unsupported",
            "runtime": other,
            "error": "unsupported runtime"
        }),
    }
}

fn run_role_worker(
    runtime: &str,
    role: &str,
    contract: &HiveLoopContract,
    runtime_probe: &serde_json::Value,
    timeout_secs: u64,
    max_attempts: u64,
) -> serde_json::Value {
    match runtime {
        "local" => serde_json::json!({
            "ok": true,
            "kind": "local",
            "mode": "deterministic-worker",
            "role": role,
            "receipt_schema_version": WORKER_RECEIPT_SCHEMA_VERSION,
            "receipt_ok": true,
            "duration_ms": 0,
            "timeout_secs": timeout_secs,
            "attempt_count": 1,
            "max_attempts": max_attempts,
            "last_message": format!(
                "Local {role} worker accepted loop #{} round {}.",
                contract.id,
                contract.current_round
            ),
            "packet": {
                "loop_id": contract.id,
                "round": contract.current_round,
                "role": role,
                "action": role_worker_action(role)
            },
            "receipt": {
                "ok": true,
                "role": role,
                "action": role_worker_action(role),
                "evidence_summary": format!(
                    "Local {role} worker accepted loop #{} round {}.",
                    contract.id,
                    contract.current_round
                ),
                "gates": {
                    "packet_received": true,
                    "role_bound": true,
                    "deterministic_runtime": true
                }
            }
        }),
        "codex" => {
            if !runtime_probe
                .get("ok")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return serde_json::json!({
                    "ok": false,
                    "kind": "codex",
                    "mode": "codex-exec",
                    "role": role,
                    "skipped": true,
                    "timeout_secs": timeout_secs,
                    "attempt_count": 0,
                    "max_attempts": max_attempts,
                    "error": "codex probe failed"
                });
            }
            run_codex_worker(contract, role, timeout_secs, max_attempts)
        }
        other => serde_json::json!({
            "ok": false,
            "kind": "unsupported",
            "mode": "unsupported",
            "role": role,
            "runtime": other,
            "skipped": true,
            "timeout_secs": timeout_secs,
            "attempt_count": 0,
            "max_attempts": max_attempts,
            "error": "unsupported runtime"
        }),
    }
}

fn role_worker_action(role: &str) -> &'static str {
    match role {
        "explorer" => "compile-candidate",
        "developer" => "implement-admitted-candidate",
        "reviewer" => "review-evidence-and-verdict-envelope",
        "doer" => "record-local-loop-ledger",
        "evaluator" => "check-gates-and-verdict-envelope",
        _ => "unknown-role-action",
    }
}

fn run_codex_worker(
    contract: &HiveLoopContract,
    role: &str,
    timeout_secs: u64,
    max_attempts: u64,
) -> serde_json::Value {
    let mut attempts = Vec::new();
    for attempt in 1..=max_attempts {
        let attempt_result = run_codex_worker_attempt(contract, role, timeout_secs, attempt);
        let attempt_ok = worker_ok(&attempt_result);
        attempts.push(attempt_result);
        if attempt_ok {
            break;
        }
    }

    let mut worker = attempts.last().cloned().unwrap_or_else(|| {
        serde_json::json!({
            "ok": false,
            "kind": "codex",
            "mode": "codex-exec",
            "role": role,
            "timeout_secs": timeout_secs,
            "error": "no worker attempts were recorded"
        })
    });
    let attempt_count = attempts.len() as u64;
    let ok = worker_ok(&worker);
    if let Some(payload) = worker.as_object_mut() {
        payload.insert(
            "attempt_count".to_string(),
            serde_json::json!(attempt_count),
        );
        payload.insert("max_attempts".to_string(), serde_json::json!(max_attempts));
        payload.insert("attempts".to_string(), serde_json::json!(attempts));
        payload.insert(
            "retry_exhausted".to_string(),
            serde_json::json!(!ok && attempt_count >= max_attempts),
        );
    }
    worker
}

fn run_codex_worker_attempt(
    contract: &HiveLoopContract,
    role: &str,
    timeout_secs: u64,
    attempt: u64,
) -> serde_json::Value {
    let output_path = std::env::temp_dir().join(format!(
        "entrance-hive-codex-worker-{}-{}-{}-{}.txt",
        contract.id,
        role,
        attempt,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let prompt = codex_worker_prompt(contract, role);
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let command_display = codex_command_display(&cwd, &output_path);
    let prompt_chars = prompt.chars().count();

    let mut command = Command::new("codex");
    command
        .arg("-a")
        .arg("never")
        .arg("exec")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("read-only")
        .arg("-C")
        .arg(&cwd)
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("--json")
        .arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let started_at = chrono::Utc::now().to_rfc3339();
    let result = run_command_with_timeout(command, Duration::from_secs(timeout_secs));
    let last_message = std::fs::read_to_string(&output_path).unwrap_or_default();
    let _ = std::fs::remove_file(&output_path);

    match result {
        Ok(output) => {
            let receipt = parse_worker_receipt(&last_message);
            let receipt_errors = receipt
                .as_ref()
                .map(|receipt| worker_receipt_contract_errors(receipt, None))
                .unwrap_or_else(|| vec!["receipt.parse".to_string()]);
            let receipt_ok = worker_receipt_ok(&last_message);
            let worker_ok = codex_worker_success(&output, receipt_ok);
            serde_json::json!({
                "ok": worker_ok,
                "kind": "codex",
                "mode": "codex-exec",
                "role": role,
                "receipt_schema_version": WORKER_RECEIPT_SCHEMA_VERSION,
                "attempt": attempt,
                "started_at": started_at,
                "completed_at": chrono::Utc::now().to_rfc3339(),
                "timed_out": output.timed_out,
                "status": output.status_code,
                "duration_ms": output.duration_ms,
                "timeout_secs": timeout_secs,
                "command": command_display,
                "cwd": cwd,
                "output_last_message_path": output_path.display().to_string(),
                "prompt_chars": prompt_chars,
                "receipt_ok": receipt_ok,
                "receipt": receipt,
                "receipt_errors": receipt_errors,
                "stdout": truncate_text(&output.stdout, 12000),
                "stderr": truncate_text(&output.stderr, 4000),
                "last_message": truncate_text(&last_message, 4000)
            })
        }
        Err(error) => serde_json::json!({
            "ok": false,
            "kind": "codex",
            "mode": "codex-exec",
            "role": role,
            "attempt": attempt,
            "started_at": started_at,
            "completed_at": chrono::Utc::now().to_rfc3339(),
            "timeout_secs": timeout_secs,
            "command": command_display,
            "cwd": cwd,
            "output_last_message_path": output_path.display().to_string(),
            "prompt_chars": prompt_chars,
            "error": error.to_string(),
            "last_message": truncate_text(&last_message, 4000)
        }),
    }
}

fn codex_command_display(cwd: &str, output_path: &std::path::Path) -> String {
    format!(
        "codex -a never exec --ephemeral --skip-git-repo-check --sandbox read-only -C {} --output-last-message {} --json <prompt>",
        cwd,
        output_path.display()
    )
}

fn codex_worker_success(output: &TimedCommandOutput, receipt_ok: Option<bool>) -> bool {
    output.status_success && !output.timed_out && receipt_ok == Some(true)
}

struct TimedCommandOutput {
    status_success: bool,
    status_code: Option<i32>,
    timed_out: bool,
    duration_ms: u64,
    stdout: String,
    stderr: String,
}

fn run_command_with_timeout(mut command: Command, timeout: Duration) -> Result<TimedCommandOutput> {
    let mut child = command.spawn().context("failed to spawn runtime worker")?;
    let started = Instant::now();
    let mut timed_out = false;

    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let output = child.wait_with_output()?;
    let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    Ok(TimedCommandOutput {
        status_success: output.status.success(),
        status_code: output.status.code(),
        timed_out,
        duration_ms,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn codex_worker_prompt(contract: &HiveLoopContract, role: &str) -> String {
    let expected_action = role_worker_action(role);
    let role_duty = match role {
        "explorer" => {
            "compile the goal into a bounded candidate and confirm the constraints are explicit"
        }
        "developer" => {
            "validate that you received the admitted development packet and confirm the implementation path is bounded"
        }
        "reviewer" => {
            "inspect the evidence and gate contract, then confirm the verdict envelope can be reviewed"
        }
        "doer" => {
            "validate that you received the accepted execution packet and confirm it can be executed"
        }
        "evaluator" => {
            "inspect the gate contract and confirm the verdict envelope can be evaluated"
        }
        _ => "acknowledge the typed loop packet",
    };
    format!(
        r#"Entrance Hive {role_name} worker packet.

You are the {role_name} role inside a constrained Explorer -> Developer -> Reviewer loop.
Rules:
- Do not modify files.
- Do not make network calls.
- Keep the response compact.
- Return a single JSON object only. Do not use markdown fences or prose.
- The receipt schema is strict:
  {{
    "ok": true,
    "role": "{role_name}",
    "action": "{expected_action}",
    "evidence_summary": "one compact sentence",
    "gates": {{"packet_received": true, "role_bound": true}}
  }}
- `ok` must be a boolean.
- `role` must be the string "{role_name}".
- `action` must be a non-empty JSON string. Never return `action` as an object,
  array, or nested structure. Put details in `evidence_summary` instead.
- `evidence_summary` must be a non-empty string.
- `gates` must be a non-empty JSON object.
- Your role duty is: {role_duty}.
- This is a runtime receipt: validate that you received the typed packet,
  summarize the accepted action for your role, and set ok=true unless you cannot process it.
- Do not set ok=false merely because the surrounding Hive runtime persists the
  evidence after you return.

Loop id: {id}
Round: {round}
Title: {title}
Goal: {goal}
Boundary: {boundary}
Approach space: {approach}
Eval space: {eval}
Accepted candidate: Run the local Hive loop MVP packet and report whether the runtime can execute it.
"#,
        role_name = role,
        expected_action = expected_action,
        role_duty = role_duty,
        id = contract.id,
        round = contract.current_round,
        title = contract.title,
        goal = contract.goal,
        boundary = contract.boundary,
        approach = serde_json::to_string(&contract.approach_space).unwrap_or_default(),
        eval = serde_json::to_string(&contract.eval_space).unwrap_or_default()
    )
}

fn worker_receipt_ok(value: &str) -> Option<bool> {
    let receipt = parse_worker_receipt(value)?;
    if receipt.get("ok").and_then(|value| value.as_bool()) != Some(true) {
        return Some(false);
    }
    Some(worker_receipt_contract_errors(&receipt, None).is_empty())
}

fn parse_worker_receipt(value: &str) -> Option<serde_json::Value> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .or_else(|_| {
            let start = trimmed.find('{').unwrap_or(0);
            let end = trimmed
                .rfind('}')
                .map(|index| index + 1)
                .unwrap_or(trimmed.len());
            serde_json::from_str::<serde_json::Value>(&trimmed[start..end])
        })
        .ok()
}

fn worker_structured_receipt(worker: &serde_json::Value) -> Option<serde_json::Value> {
    worker
        .get("receipt")
        .filter(|value| value.is_object())
        .cloned()
        .or_else(|| {
            worker
                .get("last_message")
                .and_then(|value| value.as_str())
                .and_then(parse_worker_receipt)
        })
}

fn worker_receipt_contract_errors(
    receipt: &serde_json::Value,
    expected_role: Option<&str>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if receipt
        .get("ok")
        .and_then(|value| value.as_bool())
        .is_none()
    {
        errors.push("ok".to_string());
    }
    let role = receipt.get("role").and_then(|value| value.as_str());
    if role.map_or(true, |value| value.trim().is_empty()) {
        errors.push("role".to_string());
    }
    if expected_role.is_some_and(|expected| role.is_some_and(|role| role != expected)) {
        errors.push("role_binding".to_string());
    }
    if receipt
        .get("action")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("action".to_string());
    }
    if receipt
        .get("evidence_summary")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("evidence_summary".to_string());
    }
    match receipt.get("gates") {
        Some(serde_json::Value::Object(gates)) if !gates.is_empty() => {}
        _ => errors.push("gates".to_string()),
    }
    errors
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            break;
        }
        output.push(ch);
    }
    output
}

fn default_text(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn default_vec(values: Vec<String>, fallback: &str) -> Vec<String> {
    let values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        vec![fallback.to_string()]
    } else {
        values
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use entrance_core::{Store, StoreSchemaStatus};

    use super::*;

    #[test]
    fn policy_registry_and_loop_policies_expose_typed_gate_specs() {
        let registry = policy_registry();
        assert_eq!(registry.schema_version, POLICY_SCHEMA_VERSION);
        assert!(registry.gates.len() >= 7);
        assert_eq!(registry.runtime.schema_version, POLICY_SCHEMA_VERSION);
        let connector_gate = registry
            .gates
            .iter()
            .find(|gate| gate.name == CONNECTOR_MIRROR_RECEIPT_GATE)
            .expect("connector mirror receipt gate should be registered");
        assert_eq!(
            connector_gate.expected_object_kind.as_deref(),
            Some(CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND)
        );
        assert_eq!(connector_gate.check, "external_receipt_current");
        assert!(connector_gate
            .required_receipts
            .iter()
            .any(|receipt| receipt == "mirror_file_current"));
        assert!(connector_gate
            .required_receipts
            .iter()
            .any(|receipt| receipt == "receipt_binding"));
        assert_eq!(registry.connector.schema_version, POLICY_SCHEMA_VERSION);
        assert_eq!(
            registry.connector.admission.gate,
            CONNECTOR_MIRROR_RECEIPT_GATE
        );
        assert_eq!(
            registry.connector.admission.expected_object_kind,
            CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND
        );
        assert_eq!(
            registry.connector.admission.required_checks,
            CONNECTOR_ADMISSION_REQUIRED_CHECKS
                .iter()
                .map(|check| (*check).to_string())
                .collect::<Vec<_>>()
        );
        assert!(registry
            .connector
            .admission
            .required_checks
            .iter()
            .any(|check| check == "retry_policy_bound"));
        assert_eq!(
            registry
                .connector
                .admission
                .check_registry
                .iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>()
                .as_slice(),
            CONNECTOR_ADMISSION_REQUIRED_CHECKS
        );
        let retry_check = registry
            .connector
            .admission
            .check_registry
            .iter()
            .find(|check| check.name == "retry_policy_bound")
            .expect("retry policy check should be structured");
        assert_eq!(retry_check.severity, "blocker");
        assert_eq!(retry_check.owner, "retry-policy");
        assert!(retry_check
            .required_evidence
            .iter()
            .any(|evidence| evidence == "connector_remote_contract.retry"));
        let github_retry = registry
            .connector
            .retry
            .iter()
            .find(|policy| policy.provider == "github")
            .expect("GitHub retry policy should be registered");
        assert_eq!(github_retry.transport, "rest");
        assert_eq!(github_retry.max_attempts, CONNECTOR_RETRY_MAX_ATTEMPTS);
        assert_eq!(
            github_retry.base_backoff_ms,
            CONNECTOR_RETRY_BASE_BACKOFF_MS
        );
        assert!(github_retry.retryable_http_statuses.contains(&503));
        let linear_retry = registry
            .connector
            .retry
            .iter()
            .find(|policy| policy.provider == "linear")
            .expect("Linear retry policy should be registered");
        assert_eq!(linear_retry.transport, "graphql");
        assert!(linear_retry.rate_limit_http_statuses.contains(&429));
        let connector_registry = connector_registry();
        assert!(connector_registry
            .provider_admissions
            .iter()
            .all(|admission| admission.required_checks
                == registry.connector.admission.required_checks));
        assert!(connector_registry
            .provider_admissions
            .iter()
            .all(|admission| admission
                .check_registry
                .iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>()
                .as_slice()
                == CONNECTOR_ADMISSION_REQUIRED_CHECKS));
        assert_eq!(
            registry.runtime.worker.default_timeout_secs,
            DEFAULT_WORKER_TIMEOUT_SECS
        );
        assert_eq!(
            registry.runtime.worker.max_timeout_secs,
            MAX_WORKER_TIMEOUT_SECS
        );
        assert_eq!(registry.runtime.worker.max_attempts, MAX_WORKER_ATTEMPTS);
        let codex_runtime = registry
            .runtime
            .supported
            .iter()
            .find(|runtime| runtime.name == "codex")
            .expect("codex runtime policy should be registered");
        assert_eq!(codex_runtime.mode, "codex-exec");
        assert_eq!(codex_runtime.sandbox.filesystem, "read-only");
        assert!(codex_runtime
            .required_worker_context
            .iter()
            .any(|field| field == "command"));
        assert!(codex_runtime
            .required_worker_context
            .iter()
            .any(|field| field == "cwd"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "timeout_secs"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "role"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "receipt.action"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "receipt.gates"));
        let verdict_gate = registry
            .gates
            .iter()
            .find(|gate| gate.name == "verdict_receipts_present")
            .expect("verdict gate should be registered");
        assert_eq!(
            verdict_gate.expected_object_kind.as_deref(),
            Some("VERDICT_PACKET")
        );
        assert_eq!(verdict_gate.check, "receipt_requirements_satisfied");
        assert!(verdict_gate
            .required_receipts
            .iter()
            .any(|receipt| receipt == "score"));

        let root = std::env::temp_dir().join(format!(
            "entrance-hive-policy-registry-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");
        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Policy loop".to_string(),
                goal: "Expose active loop policies".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let report = policies(&store, created.contract.id).expect("loop policies should resolve");
        assert_eq!(report.loop_id, created.contract.id);
        assert_eq!(report.policies.len(), 3);
        assert!(report.policies.iter().all(|card| card
            .gate_spec
            .as_ref()
            .is_some_and(|spec| spec.schema_version == POLICY_SCHEMA_VERSION)));
        assert_eq!(
            report.policies[0]
                .gate_spec
                .as_ref()
                .expect("candidate gate spec should exist")
                .required_receipts,
            vec!["candidate", "constraints", "role_worker"]
        );
        let audit_report =
            super::audit(&store, created.contract.id).expect("policy audit should resolve");
        let policy_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "active_policy_registry")
            .expect("policy registry check should exist");
        assert!(policy_check.passed);
        assert!(policy_check
            .details
            .pointer("/policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.is_empty()));

        let explorer_policy = report
            .policies
            .iter()
            .find(|card| card.policy.object_kind == "EXPLORATION_PACKET")
            .expect("explorer policy should exist");
        store
            .update_hive_loop_policy_gate(explorer_policy.policy.id, "runtime_receipts_present")
            .expect("policy gate should update");
        let bad_audit =
            super::audit(&store, created.contract.id).expect("bad policy audit should resolve");
        let bad_policy_check = bad_audit
            .checks
            .iter()
            .find(|check| check.name == "active_policy_registry")
            .expect("policy registry check should exist");
        assert!(!bad_policy_check.passed);
        assert!(bad_policy_check
            .details
            .pointer("/policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("gate.expected_object_kind"))))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn loop_audit_and_doctor_gate_on_store_schema_health() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-schema-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");
        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Schema audit loop".to_string(),
                goal: "Gate loop audit on SQLite schema health".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let schema_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "store_schema")
            .expect("store schema audit check should exist");
        assert!(schema_check.passed);
        assert_eq!(
            schema_check.details.pointer("/missing_tables"),
            Some(&serde_json::json!([]))
        );
        assert_eq!(
            schema_check.details.pointer("/missing_columns"),
            Some(&serde_json::json!([]))
        );
        assert_eq!(
            schema_check.details.pointer("/missing_indexes"),
            Some(&serde_json::json!([]))
        );
        assert!(schema_check
            .details
            .pointer("/expected_index_count")
            .and_then(|value| value.as_u64())
            .is_some_and(|count| count > 0));

        let doctor_report =
            super::doctor(&store, created.contract.id).expect("doctor should resolve");
        assert!(doctor_report
            .checks
            .iter()
            .any(|check| check.name == "store_schema" && check.passed));
        assert!(!doctor_report
            .failed_checks
            .iter()
            .any(|check| check == "store_schema"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn store_schema_audit_check_fails_closed_when_schema_drifts() {
        let check = super::store_schema_audit_check(&StoreSchemaStatus {
            schema_version: "entrance.sqlite.core.v1".to_string(),
            db_path: "/tmp/drifted.db".to_string(),
            user_version: 0,
            expected_user_version: 1,
            healthy: false,
            tables: Vec::new(),
            indexes: Vec::new(),
            missing_tables: vec!["hive_loop_packets".to_string()],
            missing_columns: vec!["hive_loop_packets.payload".to_string()],
            missing_indexes: vec!["idx_hive_loop_packets_loop_round".to_string()],
            generated_at: "2026-06-01T00:00:00Z".to_string(),
        });

        assert_eq!(check.name, "store_schema");
        assert!(!check.passed);
        assert_eq!(
            check.details.pointer("/errors"),
            Some(&serde_json::json!([
                "schema.user_version",
                "schema.missing_tables",
                "schema.missing_columns",
                "schema.missing_indexes"
            ]))
        );
    }

    #[test]
    fn connector_registry_exposes_active_and_planned_providers() {
        let registry = connector_registry();
        assert_eq!(registry.schema_version, CONNECTOR_REGISTRY_SCHEMA_VERSION);
        assert_eq!(registry.admission.gate, CONNECTOR_MIRROR_RECEIPT_GATE);
        assert_eq!(
            registry.admission.expected_object_kind,
            CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND
        );
        assert!(registry
            .admission
            .required_receipts
            .iter()
            .any(|receipt| receipt == "mirror_file_current"));
        assert_eq!(registry.provider_admissions.len(), registry.providers.len());
        let local_panel = registry
            .providers
            .iter()
            .find(|provider| provider.name == "local-hive-panel")
            .expect("local panel connector should be registered");
        assert_eq!(local_panel.status, "active");
        assert!(local_panel.configured);
        assert!(local_panel.supports_publish);
        assert!(local_panel.supports_readback);
        assert!(local_panel.supports_admission);
        let local_panel_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "local-hive-panel")
            .expect("local panel admission should be registered");
        assert_eq!(local_panel_admission.status, "ready");
        assert_eq!(
            local_panel_admission.route_to.as_deref(),
            Some("local_issue_surface")
        );
        let file = registry
            .providers
            .iter()
            .find(|provider| provider.name == "file")
            .expect("file connector should be registered");
        assert_eq!(file.status, "active");
        assert!(file.configured);
        assert!(file.supports_publish);
        assert!(file.supports_readback);
        assert!(file.supports_admission);
        let file_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "file")
            .expect("file admission should be registered");
        assert_eq!(file_admission.status, "ready");
        assert_eq!(
            file_admission.route_to.as_deref(),
            Some("external_issue_surface")
        );
        assert!(file_admission.blockers.is_empty());
        let remote_fixture = registry
            .providers
            .iter()
            .find(|provider| provider.name == "remote-fixture")
            .expect("remote fixture connector should be registered");
        assert_eq!(remote_fixture.status, "active");
        assert_eq!(remote_fixture.mode, "remote-issue-api-fixture");
        assert!(remote_fixture.supports_publish);
        assert!(remote_fixture.supports_readback);
        let remote_fixture_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "remote-fixture")
            .expect("remote fixture admission should be registered");
        assert_eq!(remote_fixture_admission.status, "ready");
        assert_eq!(
            remote_fixture_admission.route_to.as_deref(),
            Some("external_issue_surface")
        );
        let linear = registry
            .providers
            .iter()
            .find(|provider| provider.name == "linear")
            .expect("linear connector should be registered");
        assert_eq!(linear.status, "planned");
        assert!(linear.auth_required);
        assert!(!linear.configured);
        let linear_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "linear")
            .expect("linear admission should be registered");
        assert_eq!(linear_admission.status, "blocked");
        assert!(linear_admission
            .blockers
            .iter()
            .any(|blocker| blocker == "provider_not_active"));
        assert!(linear_admission
            .blockers
            .iter()
            .any(|blocker| blocker == "connector_not_configured"));
        assert!(linear_admission
            .blockers
            .iter()
            .any(|blocker| blocker == "admission_not_supported"));
    }

    #[test]
    fn connector_registry_applies_runtime_config_overrides() {
        let config = ConnectorsConfig {
            file: ConnectorProviderConfig {
                enabled: Some(false),
                ..ConnectorProviderConfig::default()
            },
            linear: ConnectorProviderConfig {
                enabled: Some(true),
                auth_env: vec!["ENTRANCE_TEST_LINEAR_TOKEN".to_string()],
                storage: Some("linear-dry-run".to_string()),
                ..ConnectorProviderConfig::default()
            },
            github: ConnectorProviderConfig::default(),
        };

        let registry = connector_registry_with_config(&config);

        let file = registry
            .providers
            .iter()
            .find(|provider| provider.name == "file")
            .expect("file connector should be registered");
        assert_eq!(file.status, "disabled");
        assert!(!file.configured);
        assert!(!file.supports_publish);

        let linear = registry
            .providers
            .iter()
            .find(|provider| provider.name == "linear")
            .expect("linear connector should be registered");
        assert_eq!(linear.status, "planned");
        assert_eq!(linear.storage, "linear-dry-run");
        assert_eq!(linear.auth_env, vec!["ENTRANCE_TEST_LINEAR_TOKEN"]);
        assert!(!linear.configured);
        assert!(linear.notes.contains("Configured from entrance.toml."));
        let linear_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "linear")
            .expect("linear admission should be registered");
        assert_eq!(linear_admission.status, "blocked");
        assert!(linear_admission
            .blockers
            .iter()
            .any(|blocker| blocker == "provider_not_active"));
        assert!(linear_admission
            .blockers
            .iter()
            .any(|blocker| blocker == "connector_not_configured"));
    }

    #[test]
    fn linear_connector_can_activate_from_config_and_token_env() {
        let token_env = "ENTRANCE_TEST_LINEAR_TOKEN_ACTIVATE";
        std::env::set_var(token_env, "test-token");
        let config = ConnectorsConfig {
            linear: ConnectorProviderConfig {
                enabled: Some(true),
                auth_env: vec![token_env.to_string()],
                ..ConnectorProviderConfig::default()
            },
            ..ConnectorsConfig::default()
        };

        let registry = connector_registry_with_config(&config);
        std::env::remove_var(token_env);

        let linear = registry
            .providers
            .iter()
            .find(|provider| provider.name == "linear")
            .expect("linear connector should be registered");
        assert_eq!(linear.status, "active");
        assert!(linear.configured);
        assert!(linear.supports_publish);
        assert!(linear.supports_readback);
        assert!(linear.supports_admission);
        assert_eq!(linear.storage, "linear-graphql-api");
        assert!(linear
            .notes
            .contains("GraphQL publish/readback connector is active"));
        let linear_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "linear")
            .expect("linear admission should be registered");
        assert_eq!(linear_admission.status, "ready");
        assert!(linear_admission.blockers.is_empty());
    }

    #[test]
    fn github_connector_can_activate_from_config_and_token_env() {
        let token_env = "ENTRANCE_TEST_GITHUB_TOKEN_ACTIVATE";
        std::env::set_var(token_env, "test-token");
        let config = ConnectorsConfig {
            github: ConnectorProviderConfig {
                enabled: Some(true),
                auth_env: vec![token_env.to_string()],
                ..ConnectorProviderConfig::default()
            },
            ..ConnectorsConfig::default()
        };

        let registry = connector_registry_with_config(&config);
        std::env::remove_var(token_env);

        let github = registry
            .providers
            .iter()
            .find(|provider| provider.name == "github")
            .expect("github connector should be registered");
        assert_eq!(github.status, "active");
        assert!(github.configured);
        assert!(github.supports_publish);
        assert!(github.supports_readback);
        assert!(github.supports_admission);
        assert_eq!(github.storage, "github-rest-api");
        assert!(github
            .notes
            .contains("REST publish/readback connector is active"));
        let github_admission = registry
            .provider_admissions
            .iter()
            .find(|admission| admission.provider == "github")
            .expect("github admission should be registered");
        assert_eq!(github_admission.status, "ready");
        assert!(github_admission.blockers.is_empty());
    }

    #[test]
    fn pending_next_actions_prefer_issue_compact_run() {
        assert_eq!(
            doctor_next_actions("pending", 4, Some(9), "codex", true),
            vec!["entrance hive issue run 9 --runtime codex --compact"]
        );
        assert_eq!(
            doctor_next_actions("pending", 4, None, "local", true),
            vec!["entrance hive loop run 4 --runtime local --compact"]
        );
    }

    #[test]
    fn doctor_next_actions_prefer_compact_audit_gate() {
        assert_eq!(
            doctor_next_actions("ok", 4, Some(9), "codex", true),
            vec![
                "entrance hive loop audit 4 --compact",
                "entrance hive loop trace 4",
                "entrance hive loop evidence 4"
            ]
        );
        assert_eq!(
            doctor_next_actions("audit_failed", 4, Some(9), "codex", false),
            vec![
                "entrance hive loop audit 4 --compact",
                "entrance hive loop evidence 4"
            ]
        );
    }

    #[test]
    fn local_loop_records_stages_evidence_verdict_and_issue() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Test loop".to_string(),
                goal: "Run the local loop".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(7),
                worker_attempts: Some(2),
            },
        )
        .expect("loop should run");

        assert_eq!(report.contract.status, "kept");
        assert_eq!(report.contract.active_phase, "complete");
        assert_eq!(report.policies.len(), 3);
        assert_eq!(report.policies[0].gate, "candidate_receipts_present");
        assert_eq!(report.policies[1].gate, "runtime_receipts_present");
        assert_eq!(report.policies[2].gate, "verdict_receipts_present");
        assert_eq!(report.packets.len(), 3);
        assert!(report.packets.iter().all(|packet| packet
            .payload
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(PACKET_SCHEMA_VERSION)));
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/role_worker/role")
                .and_then(|value| value.as_str()),
            Some("explorer")
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/candidate")
                .and_then(|value| value.as_str()),
            Some("Run a local MVP loop through Hive")
        );
        assert_eq!(
            report.packets[1]
                .payload
                .pointer("/receipt_requirements/3")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );
        assert_eq!(
            report.packets[2]
                .payload
                .pointer("/body/role_worker/role")
                .and_then(|value| value.as_str()),
            Some("reviewer")
        );
        assert_eq!(report.admissions.len(), 3);
        assert!(report
            .admissions
            .iter()
            .all(|admission| admission.result == "admitted"));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(ADMISSION_SCHEMA_VERSION)));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/policy/schema_version")
            .and_then(|value| value.as_str())
            == Some(POLICY_SCHEMA_VERSION)));
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/spec/check")
                .and_then(|value| value.as_str()),
            Some("receipt_requirements_satisfied")
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/spec/expected_object_kind")
                .and_then(|value| value.as_str()),
            Some("EXPLORATION_PACKET")
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/packet/schema_version")
                .and_then(|value| value.as_str()),
            Some(PACKET_SCHEMA_VERSION)
        );
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/packet/envelope/valid")
            .and_then(|value| value.as_bool())
            == Some(true)));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/packet/envelope/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.is_empty())));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/receipt/satisfied")
            .and_then(|value| value.as_bool())
            == Some(true)));
        assert_eq!(
            report.admissions[1]
                .policy
                .pointer("/receipt/required/3")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );
        assert_eq!(
            report.admissions[1]
                .policy
                .pointer("/receipt/missing")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(report.stages.len(), 3);
        assert_eq!(report.evidence.len(), 3);
        let execution_evidence = report
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("execution evidence should exist");
        assert_eq!(
            execution_evidence
                .payload
                .pointer("/worker/kind")
                .and_then(|value| value.as_str()),
            Some("local")
        );
        assert_eq!(
            execution_evidence
                .payload
                .pointer("/worker/receipt/action")
                .and_then(|value| value.as_str()),
            Some("implement-admitted-candidate")
        );
        assert_eq!(
            execution_evidence
                .payload
                .pointer("/worker/receipt/gates/role_bound")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "keep");
        let trace = report.issues[0]
            .trace
            .as_ref()
            .expect("issue trace should be present");
        assert_eq!(trace.current_round, 1);
        assert_eq!(trace.packet_count, 3);
        assert_eq!(trace.admission_count, 3);
        assert_eq!(trace.verdict_count, 1);
        assert_eq!(trace.round_packet_count, 3);
        assert_eq!(trace.round_admission_count, 3);
        assert_eq!(trace.round_evidence_count, 3);
        assert_eq!(trace.round_verdict_count, 1);
        assert_eq!(trace.receipt_required_count, 11);
        assert_eq!(trace.receipt_missing_count, 0);
        assert_eq!(trace.round_receipt_required_count, 11);
        assert_eq!(trace.round_receipt_missing_count, 0);
        assert_eq!(trace.role_worker_count, 3);
        assert_eq!(trace.role_worker_ok_count, 3);
        assert_eq!(trace.round_role_worker_count, 3);
        assert_eq!(trace.round_role_worker_ok_count, 3);
        assert_eq!(trace.round_worker_duration_ms, 0);
        assert_eq!(trace.round_worker_timeout_count, 0);
        assert_eq!(trace.round_worker_retry_exhausted_count, 0);
        assert_eq!(trace.packet_schema.as_deref(), Some(PACKET_SCHEMA_VERSION));
        assert_eq!(trace.policy_schema.as_deref(), Some(POLICY_SCHEMA_VERSION));
        assert_eq!(
            trace.admission_schema.as_deref(),
            Some(ADMISSION_SCHEMA_VERSION)
        );
        assert_eq!(
            trace.verdict_schema.as_deref(),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            trace.last_admission_gate.as_deref(),
            Some("verdict_receipts_present")
        );
        assert_eq!(
            trace.last_gate_expected_object_kind.as_deref(),
            Some("VERDICT_PACKET")
        );
        assert!(trace
            .last_gate_description
            .as_deref()
            .is_some_and(|description| description.contains("Reviewer packets")));
        assert_eq!(trace.last_admission_passed, Some(true));
        assert_eq!(trace.last_decision.as_deref(), Some("keep"));
        assert_eq!(trace.score_vector.len(), 4);
        assert_eq!(
            trace
                .score_vector
                .iter()
                .find(|metric| metric.name == "runtime_readiness")
                .and_then(|metric| metric.value),
            Some(1.0)
        );
        assert_eq!(trace.human_options, vec!["comment"]);
        assert_eq!(trace.operator_event_count, 0);
        assert_eq!(trace.round_operator_event_count, 0);
        assert!(trace.last_operator_event.is_none());
        assert!(trace.operator_events.is_empty());
        assert_eq!(trace.worker_kind.as_deref(), Some("local"));
        assert_eq!(trace.worker_ok, Some(true));
        assert_eq!(trace.evidence.len(), 3);
        let doer_evidence = trace
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("developer evidence summary should exist");
        assert_eq!(doer_evidence.stage_role.as_deref(), Some("developer"));
        assert_eq!(doer_evidence.admission_result.as_deref(), Some("admitted"));
        assert_eq!(doer_evidence.worker_kind.as_deref(), Some("local"));
        assert_eq!(doer_evidence.worker_ok, Some(true));
        assert_eq!(doer_evidence.worker_duration_ms, Some(0));
        assert_eq!(doer_evidence.worker_timeout_secs, Some(7));
        assert_eq!(doer_evidence.worker_attempt_count, Some(1));
        assert_eq!(doer_evidence.worker_max_attempts, Some(2));
        assert_eq!(doer_evidence.worker_retry_exhausted, None);
        assert_eq!(
            doer_evidence.worker_action.as_deref(),
            Some("implement-admitted-candidate")
        );
        assert!(doer_evidence
            .worker_evidence_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("Local developer worker")));
        assert_eq!(doer_evidence.worker_gate_count, Some(3));
        assert!(doer_evidence.worker_receipt_errors.is_empty());
        let mut receipt_error_trace = trace.clone();
        receipt_error_trace
            .evidence
            .iter_mut()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("developer evidence summary should exist")
            .worker_receipt_errors = vec!["action".to_string()];
        assert!(doctor_worker_failures(&receipt_error_trace)
            .iter()
            .any(|failure| failure.contains("receipt_errors=action")));
        assert_eq!(
            doctor_health("kept", Some("Done"), Some("keep"), true, true),
            "worker_failed"
        );
        assert!(doer_evidence
            .transcript_excerpt
            .as_deref()
            .is_some_and(|excerpt| excerpt.contains("Local developer worker")));
        assert_eq!(
            trace
                .stages
                .iter()
                .map(|stage| stage.role.as_str())
                .collect::<Vec<_>>(),
            vec!["explorer", "developer", "reviewer"]
        );
        let doer_trace = trace
            .stages
            .iter()
            .find(|stage| stage.role == "developer")
            .expect("developer stage trace should exist");
        assert_eq!(
            doer_trace.evidence_kind.as_deref(),
            Some("execution_packet")
        );
        assert_eq!(doer_trace.admission_result.as_deref(), Some("admitted"));
        assert_eq!(doer_trace.worker_kind.as_deref(), Some("local"));
        assert_eq!(doer_trace.worker_ok, Some(true));
        let shown_issue =
            issue(&store, report.issues[0].issue.id).expect("single issue report should resolve");
        assert_eq!(shown_issue.issue.id, report.issues[0].issue.id);
        assert_eq!(
            shown_issue
                .trace
                .as_ref()
                .expect("shown issue should include trace")
                .stages
                .len(),
            3
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/runtime_readiness")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            report.verdicts[0]
                .evidence
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(report.issues[0].issue.status, "Done");
        assert!(report.issues[0].comments.len() >= 3);
        assert_eq!(
            report.issues[0]
                .comments
                .iter()
                .filter_map(|comment| comment
                    .payload
                    .get("stage_role")
                    .and_then(|value| value.as_str()))
                .collect::<Vec<_>>(),
            vec!["explorer", "developer", "reviewer"]
        );
        assert_eq!(
            report.issues[0]
                .comments
                .iter()
                .filter_map(|comment| comment
                    .payload
                    .get("evidence_id")
                    .and_then(|value| value.as_i64()))
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(report.issues[0].comments.iter().any(|comment| {
            comment.body == "Explorer admitted a candidate for this round."
                && comment
                    .payload
                    .get("evidence_kind")
                    .and_then(|value| value.as_str())
                    == Some("exploration_packet")
        }));
        assert!(report.issues[0].comments.iter().any(|comment| {
            comment.body == "Developer admitted the execution packet."
                && comment
                    .payload
                    .get("evidence_kind")
                    .and_then(|value| value.as_str())
                    == Some("execution_packet")
        }));
        assert!(report.issues[0].comments.iter().any(|comment| {
            comment.body == "Reviewer admitted the verdict packet."
                && comment
                    .payload
                    .get("evidence_kind")
                    .and_then(|value| value.as_str())
                    == Some("verdict_packet")
        }));
        assert!(report.issues[0].comments.iter().all(|comment| comment
            .payload
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(SYSTEM_COMMENT_SCHEMA_VERSION)));
        let issue_doctor = report.issues[0]
            .doctor
            .as_ref()
            .expect("issue card should include doctor summary");
        assert_eq!(issue_doctor.health, "ok");
        assert_eq!(issue_doctor.runtime, "local");
        assert_eq!(issue_doctor.counts.round_role_worker_ok_count, 3);
        assert!(issue_doctor.worker_failures.is_empty());
        let trace_report =
            super::trace(&store, created.contract.id).expect("loop trace report should resolve");
        assert_eq!(trace_report.contract.status, "kept");
        assert_eq!(
            trace_report
                .issue
                .as_ref()
                .map(|issue| issue.status.as_str()),
            Some("Done")
        );
        assert_eq!(trace_report.trace.last_decision.as_deref(), Some("keep"));
        assert_eq!(trace_report.trace.round_receipt_missing_count, 0);
        assert_eq!(trace_report.trace.score_vector.len(), 4);
        assert_eq!(
            trace_report.trace.last_gate_expected_object_kind.as_deref(),
            Some("VERDICT_PACKET")
        );
        assert_eq!(
            trace_report.trace.audit_schema.as_deref(),
            Some(AUDIT_SCHEMA_VERSION)
        );
        assert_eq!(trace_report.trace.audit_passed, Some(true));
        assert_eq!(trace_report.trace.audit_failed_count, 0);
        assert!(trace_report.trace.audit_failed_checks.is_empty());
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("loop evidence report should resolve");
        assert_eq!(evidence_report.evidence.len(), 3);
        assert!(evidence_report.evidence.iter().any(|evidence| {
            evidence.stage_role.as_deref() == Some("reviewer")
                && evidence.kind == "verdict_packet"
                && evidence.worker_ok == Some(true)
        }));
        let audit_report =
            super::audit(&store, created.contract.id).expect("loop audit should resolve");
        assert_eq!(audit_report.schema_version, AUDIT_SCHEMA_VERSION);
        assert!(audit_report.passed);
        assert_eq!(audit_report.failed_count, 0);
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "packet_envelopes"
                && check.passed
                && check
                    .details
                    .pointer("/packet_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "worker_receipts"
                && check.passed
                && check
                    .details
                    .pointer("/worker_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "runtime_policy"
                && check.passed
                && check
                    .details
                    .pointer("/runtime_policy_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "issue_surface"
                && check.passed
                && check
                    .details
                    .pointer("/comment_count")
                    .and_then(|value| value.as_u64())
                    .is_some_and(|count| count >= 3)
                && check
                    .details
                    .pointer("/issue_surface_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        let doctor_report =
            super::doctor(&store, created.contract.id).expect("loop doctor should resolve");
        assert_eq!(doctor_report.schema_version, DOCTOR_SCHEMA_VERSION);
        assert_eq!(doctor_report.health, "ok");
        assert_eq!(doctor_report.status, "kept");
        assert_eq!(doctor_report.decision.as_deref(), Some("keep"));
        assert_eq!(doctor_report.counts.round_packet_count, 3);
        assert_eq!(doctor_report.counts.round_role_worker_ok_count, 3);
        assert_eq!(doctor_report.counts.round_receipt_missing_count, 0);
        assert_eq!(doctor_report.counts.round_worker_duration_ms, 0);
        assert_eq!(doctor_report.counts.round_worker_timeout_count, 0);
        assert_eq!(doctor_report.counts.round_worker_retry_exhausted_count, 0);
        assert_eq!(doctor_report.counts.audit_failed_count, 0);
        assert!(doctor_report.failed_checks.is_empty());
        assert!(doctor_report.missing_receipts.is_empty());
        assert!(doctor_report.worker_failures.is_empty());
        assert!(doctor_report.next_actions.iter().any(
            |action| action == &format!("entrance hive loop evidence {}", created.contract.id)
        ));

        let rerun = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("completed loop run should be idempotent");
        assert_eq!(rerun.contract.status, "kept");
        assert_eq!(rerun.packets.len(), report.packets.len());
        assert_eq!(rerun.admissions.len(), report.admissions.len());
        assert_eq!(rerun.evidence.len(), report.evidence.len());
        assert_eq!(rerun.verdicts.len(), report.verdicts.len());
        assert_eq!(
            rerun.issues[0].comments.len(),
            report.issues[0].comments.len()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_ok_reads_final_json_receipt() {
        assert_eq!(
            worker_receipt_ok(
                r#"{"ok":true,"role":"doer","action":"execute","evidence_summary":"done","gates":{"accepted":true}}"#
            ),
            Some(true)
        );
        assert_eq!(worker_receipt_ok(r#"{"ok":true}"#), Some(false));
        assert_eq!(
            worker_receipt_ok(
                r#"{"ok":true,"role":"doer","action":{"accepted":"execute"},"evidence_summary":"done","gates":{"accepted":true}}"#
            ),
            Some(false)
        );
        let object_action_receipt = serde_json::json!({
            "ok": true,
            "role": "doer",
            "action": { "accepted": "execute" },
            "evidence_summary": "done",
            "gates": { "accepted": true }
        });
        assert_eq!(
            worker_receipt_contract_errors(&object_action_receipt, Some("doer")),
            vec!["action"]
        );
        assert_eq!(
            worker_receipt_ok("prefix {\"ok\":false,\"reason\":\"blocked\"} suffix"),
            Some(false)
        );
        assert_eq!(worker_receipt_ok("not json"), None);
    }

    #[test]
    fn codex_worker_prompt_declares_strict_receipt_schema() {
        let contract = HiveLoopContract {
            id: 42,
            title: "Prompt contract".to_string(),
            goal: "Keep worker receipts typed".to_string(),
            boundary: "No writes".to_string(),
            approach_space: vec!["Use strict JSON".to_string()],
            eval_space: vec!["action is a string".to_string()],
            review_surface: "local-hive-panel".to_string(),
            autonomy_level: "run-approved-candidates".to_string(),
            runtime: "codex".to_string(),
            status: "todo".to_string(),
            active_phase: "explorer".to_string(),
            current_round: 1,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let prompt = codex_worker_prompt(&contract, "explorer");

        assert!(prompt.contains(r#""role": "explorer""#));
        assert!(prompt.contains(r#""action": "compile-candidate""#));
        assert!(prompt.contains("action` must be a non-empty JSON string"));
        assert!(prompt.contains("Never return `action` as an object"));
    }

    #[test]
    fn evidence_summary_exposes_codex_worker_command_context() {
        let evidence = HiveLoopEvidence {
            id: 9,
            loop_id: 3,
            stage_id: None,
            round: 1,
            kind: "execution_packet".to_string(),
            summary: "Doer ran `codex` runtime worker.".to_string(),
            path: None,
            payload: serde_json::json!({
                "worker": {
                    "ok": true,
                    "kind": "codex",
                    "mode": "codex-exec",
                    "role": "doer",
                    "command": "codex -a never exec --sandbox read-only <prompt>",
                    "cwd": "/tmp/entrance-src",
                    "receipt_ok": true,
                    "receipt": {
                        "ok": true,
                        "role": "doer",
                        "action": "record-local-loop-ledger",
                        "evidence_summary": "codex accepted the packet",
                        "gates": { "packet_received": true }
                    }
                }
            }),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let summary = issue_evidence_summary(&evidence, &HashMap::new());

        assert_eq!(
            summary.worker_command.as_deref(),
            Some("codex -a never exec --sandbox read-only <prompt>")
        );
        assert_eq!(summary.worker_cwd.as_deref(), Some("/tmp/entrance-src"));
        assert_eq!(
            summary.worker_action.as_deref(),
            Some("record-local-loop-ledger")
        );
    }

    #[test]
    fn codex_worker_requires_explicit_ok_receipt() {
        let output = TimedCommandOutput {
            status_success: true,
            status_code: Some(0),
            timed_out: false,
            duration_ms: 12,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(codex_worker_success(&output, Some(true)));
        assert!(!codex_worker_success(&output, Some(false)));
        assert!(!codex_worker_success(&output, None));

        let timed_out = TimedCommandOutput {
            timed_out: true,
            ..output
        };
        assert!(!codex_worker_success(&timed_out, Some(true)));
    }

    #[test]
    fn doctor_retry_action_uses_more_attempts_for_codex() {
        assert_eq!(
            retry_run_command(7, "codex"),
            "entrance hive issue retry-run 7 --body <note> --runtime codex --worker-attempts 2 --compact"
        );
        assert_eq!(
            retry_run_command(7, "local"),
            "entrance hive issue retry-run 7 --body <note> --compact"
        );
    }

    #[test]
    fn verdict_audit_rejects_inconsistent_score_contract() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-verdict-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict audit loop".to_string(),
                goal: "Detect inconsistent typed verdict score contracts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");

        let mut verdict = report.verdicts[0].clone();
        verdict.score["gates_passed"] = serde_json::json!(false);
        verdict.score["human_options"] = serde_json::json!(["comment", "retry"]);
        verdict.score["score_vector"]["runtime_readiness"] = serde_json::json!(1.5);
        verdict.evidence["decision"] = serde_json::json!("blocked");
        verdict.evidence["reason_code"] = serde_json::json!("different_reason");

        let errors = verdict_audit_errors(&verdict).expect("verdict should fail audit");
        let fields = errors
            .get("errors")
            .and_then(|value| value.as_array())
            .expect("verdict audit should return error fields")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"score.gates_passed"));
        assert!(fields.contains(&"score.human_options"));
        assert!(fields.contains(&"score.score_vector.runtime_readiness"));
        assert!(fields.contains(&"evidence.decision_binding"));
        assert!(fields.contains(&"reason_code.binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verdict_audit_rejects_drifted_evidence_bindings() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-verdict-evidence-binding-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict evidence binding loop".to_string(),
                goal: "Detect drifted verdict evidence bindings".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let verdict = report
            .verdicts
            .first()
            .expect("run should record a verdict");
        let mut evidence = verdict.evidence.clone();
        evidence["evidence_count"] = serde_json::json!(999);
        evidence["runtime_ready"] = serde_json::json!(false);
        evidence["role_worker"]["ok"] = serde_json::json!(false);
        store
            .insert_hive_loop_verdict(HiveLoopVerdictCreate {
                loop_id: verdict.loop_id,
                round: verdict.round,
                decision: verdict.decision.clone(),
                summary: verdict.summary.clone(),
                score: verdict.score.clone(),
                evidence,
            })
            .expect("drifted verdict should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let verdict_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "verdict_packets")
            .expect("verdict audit should exist");
        assert!(!verdict_check.passed);
        for expected in [
            "evidence.count",
            "evidence.runtime_ready",
            "evidence.role_worker_binding",
        ] {
            assert!(
                verdict_check
                    .details
                    .pointer("/verdict_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.iter().any(|error| error
                        .pointer("/errors")
                        .and_then(|value| value.as_array())
                        .is_some_and(|fields| fields
                            .iter()
                            .any(|field| field.as_str() == Some(expected))))),
                "expected verdict evidence binding error {expected}"
            );
        }
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "verdict_packets:verdict_evidence:evidence.count"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verdict_audit_rejects_replayed_round_verdicts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-verdict-replay-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict replay audit loop".to_string(),
                goal: "Catch replayed verdicts in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let verdict = report
            .verdicts
            .first()
            .expect("run should record a verdict");
        store
            .insert_hive_loop_verdict(HiveLoopVerdictCreate {
                loop_id: verdict.loop_id,
                round: verdict.round,
                decision: verdict.decision.clone(),
                summary: verdict.summary.clone(),
                score: verdict.score.clone(),
                evidence: verdict.evidence.clone(),
            })
            .expect("replayed verdict should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let verdict_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "verdict_packets")
            .expect("verdict audit should exist");
        assert!(!verdict_check.passed);
        assert!(verdict_check
            .details
            .pointer("/verdict_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("verdict.round_duplicate"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "verdict_packets:verdict_round:verdict.round_duplicate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_rejects_packets_missing_required_receipts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-receipt-gate-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Receipt gate loop".to_string(),
                goal: "Reject incomplete execution packets".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let admission = emit_and_admit(
            &store,
            &created.contract,
            "EXECUTION_PACKET",
            "developer",
            "developer",
            "reviewer",
            serde_json::json!({
                "runtime_probe": {
                    "ok": true,
                    "kind": "local"
                },
                "artifact": "hive-loop-ledger"
            }),
        )
        .expect("admission should be recorded");

        assert_eq!(admission.result, "rejected");
        assert_eq!(
            admission.reason,
            "runtime_receipts_present failed: missing or invalid receipts runtime_worker, role_worker"
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/satisfied")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/missing/0")
                .and_then(|value| value.as_str()),
            Some("runtime_worker")
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/missing/1")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_rejects_success_worker_with_incomplete_structured_receipt() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-worker-receipt-gate-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Worker receipt gate loop".to_string(),
                goal: "Reject incomplete successful worker receipts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let runtime_probe = serde_json::json!({
            "ok": true,
            "kind": "local"
        });
        let runtime_worker = run_role_worker(
            "local",
            "developer",
            &created.contract,
            &runtime_probe,
            DEFAULT_WORKER_TIMEOUT_SECS,
            DEFAULT_WORKER_ATTEMPTS,
        );
        let mut role_worker = runtime_worker.clone();
        role_worker
            .pointer_mut("/receipt")
            .and_then(|value| value.as_object_mut())
            .expect("role worker receipt should be an object")
            .remove("action");

        let admission = emit_and_admit(
            &store,
            &created.contract,
            "EXECUTION_PACKET",
            "developer",
            "developer",
            "reviewer",
            serde_json::json!({
                "runtime_probe": runtime_probe,
                "runtime_worker": runtime_worker,
                "role_worker": role_worker,
                "artifact": "hive-loop-ledger"
            }),
        )
        .expect("admission should be recorded");

        assert_eq!(admission.result, "rejected");
        assert_eq!(
            admission.reason,
            "runtime_receipts_present failed: missing or invalid receipts role_worker"
        );
        assert_eq!(
            string_array_at(&admission.policy, "/receipt/missing"),
            vec!["role_worker"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_audit_rejects_corrupt_receipt_policy_and_gate_bindings() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-admission-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Admission audit loop".to_string(),
                goal: "Detect drift between admission receipt, policy, gate, and packet"
                    .to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let packet_by_id = report
            .packets
            .iter()
            .map(|packet| (packet.id, packet))
            .collect::<HashMap<_, _>>();
        let mut bad_admission = report.admissions[0].clone();
        bad_admission.policy["packet"]["object_kind"] = serde_json::json!("EXECUTION_PACKET");
        bad_admission.policy["policy"]["route_to"] = serde_json::json!("complete");
        bad_admission.policy["policy"]["gate"] = serde_json::json!("runtime_receipts_present");
        bad_admission.policy["gate"]["passed"] = serde_json::json!(false);
        bad_admission.policy["receipt"]["required"] = serde_json::json!(["candidate"]);
        bad_admission.policy["receipt"]["missing"] = serde_json::json!(["constraints"]);
        bad_admission.policy["receipt"]["satisfied"] = serde_json::json!(true);

        let errors = admission_audit_errors(&bad_admission, &packet_by_id)
            .expect("corrupt admission should fail audit");
        let fields = errors
            .get("errors")
            .and_then(|value| value.as_array())
            .expect("admission audit should return error fields")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"packet.object_kind"));
        assert!(fields.contains(&"policy.route_to"));
        assert!(fields.contains(&"policy.gate_binding"));
        assert!(fields.contains(&"receipt.required_binding"));
        assert!(fields.contains(&"receipt.satisfied_binding"));
        assert!(fields.contains(&"result.admission_conditions"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typed_packet_envelope_diagnostics_explain_schema_breaks() {
        let malformed = serde_json::json!({
            "schema_version": "entrance.hive.packet.v0",
            "object_kind": " ",
            "writer": {
                "role": ""
            },
            "route": {
                "from": "explorer"
            },
            "state_code": "draft",
            "body": {
                "candidate": "local-loop-mvp"
            }
        });
        assert_eq!(
            typed_packet_envelope_errors(&malformed),
            vec![
                "schema_version",
                "loop_id",
                "round",
                "object_kind",
                "writer.role",
                "route.to",
                "state_code"
            ]
        );
        assert!(!typed_packet_envelope_valid(&malformed));
        assert!(!gate_passes("candidate_receipts_present", &malformed));
        assert_eq!(
            gate_failure_reason("candidate_receipts_present", &malformed),
            "candidate_receipts_present failed: typed packet envelope invalid: schema_version, loop_id, round, object_kind, writer.role, route.to, state_code"
        );

        let packet = HiveLoopPacket {
            id: 42,
            loop_id: 7,
            round: 3,
            object_kind: "EXPLORATION_PACKET".to_string(),
            writer_role: "explorer".to_string(),
            route_from: "explorer".to_string(),
            route_to: "doer".to_string(),
            state_code: "submitted".to_string(),
            payload: malformed.clone(),
            created_at: "2026-05-31T00:00:00Z".to_string(),
        };
        let receipt = typed_admission_receipt(
            &packet,
            &malformed,
            None,
            "rejected",
            "bad packet",
            None,
            None,
        );
        assert_eq!(
            receipt
                .pointer("/packet/envelope/valid")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            receipt
                .pointer("/packet/envelope/errors/0")
                .and_then(|value| value.as_str()),
            Some("schema_version")
        );
        assert_eq!(
            receipt
                .pointer("/packet/envelope/errors/6")
                .and_then(|value| value.as_str()),
            Some("state_code")
        );
    }

    #[test]
    fn unsupported_runtime_records_blocked_verdict_and_issue() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-blocked-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Blocked loop".to_string(),
                goal: "Block unsupported runtime".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("loop should be created");

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(2),
            },
        )
        .expect("blocked loop should still return a report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "explorer");
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "blocked");
        assert_eq!(
            report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("admission_rejected")
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/admission_integrity")
                .and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(report.issues[0].issue.status, "Blocked");
        let issue_doctor = report.issues[0]
            .doctor
            .as_ref()
            .expect("blocked issue should include doctor summary");
        assert_eq!(issue_doctor.health, "blocked");
        assert!(issue_doctor
            .missing_receipts
            .iter()
            .any(|receipt| receipt == "role_worker"));
        assert!(report
            .issues
            .first()
            .expect("issue should exist")
            .comments
            .iter()
            .any(|comment| comment.body.contains("role_worker")));
        assert_eq!(report.admissions.len(), 1);
        assert_eq!(report.admissions[0].result, "rejected");
        assert!(report.admissions[0].reason.contains("role_worker"));
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/receipt/missing/0")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );
        assert_eq!(report.packets.len(), 1);
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/role_worker/ok")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("blocked evidence report should resolve");
        let blocked_evidence = evidence_report
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "admission_rejection")
            .expect("admission rejection evidence should be summarized");
        assert_eq!(blocked_evidence.blocked_phase.as_deref(), Some("explorer"));
        assert_eq!(blocked_evidence.missing_receipts, vec!["role_worker"]);
        assert_eq!(blocked_evidence.worker_kind.as_deref(), Some("unsupported"));
        assert_eq!(blocked_evidence.worker_ok, Some(false));
        assert_eq!(blocked_evidence.worker_timeout_secs, Some(5));
        assert_eq!(blocked_evidence.worker_attempt_count, Some(0));
        assert_eq!(blocked_evidence.worker_max_attempts, Some(2));
        assert_eq!(blocked_evidence.worker_retry_exhausted, None);
        assert!(blocked_evidence
            .operator_options
            .iter()
            .any(|option| option == "request-human-review"));
        let doctor_report =
            super::doctor(&store, created.contract.id).expect("blocked doctor should resolve");
        assert_eq!(doctor_report.health, "blocked");
        assert_eq!(doctor_report.status, "blocked");
        assert_eq!(doctor_report.issue_status.as_deref(), Some("Blocked"));
        assert_eq!(doctor_report.decision.as_deref(), Some("blocked"));
        assert!(doctor_report
            .missing_receipts
            .iter()
            .any(|receipt| receipt == "role_worker"));
        assert!(doctor_report
            .worker_failures
            .iter()
            .any(|failure| failure.contains("worker=unsupported")));
        assert!(doctor_report
            .next_actions
            .iter()
            .any(|action| action.contains("issue retry-run")));
        assert!(doctor_report.next_actions.iter().any(|action| action
            == &format!(
                "entrance hive issue decide {} request-review --body <note> --compact",
                doctor_report
                    .issue_id
                    .expect("blocked doctor should have issue")
            )));
        let audit_report =
            super::audit(&store, created.contract.id).expect("blocked audit should resolve");
        let runtime_policy_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "runtime_policy")
            .expect("runtime policy audit should be present");
        assert!(!runtime_policy_check.passed);
        assert!(runtime_policy_check
            .details
            .pointer("/runtime_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| !errors.is_empty()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn decision_override_records_reject_and_needs_review_verdicts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-decision-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let rejected = create(
            &store,
            HiveLoopCreateRequest {
                title: "Rejected loop".to_string(),
                goal: "Reject a candidate".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let rejected_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: rejected.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("reject loop should run");

        assert_eq!(rejected_report.contract.status, "rejected");
        assert_eq!(rejected_report.verdicts[0].decision, "reject");
        assert_eq!(rejected_report.issues[0].issue.status, "Canceled");
        assert_eq!(
            rejected_report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("quality_gate_failed")
        );
        assert_eq!(
            rejected_report.verdicts[0]
                .score
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            rejected_report.verdicts[0]
                .score
                .pointer("/human_options/1")
                .and_then(|value| value.as_str()),
            Some("retry")
        );
        assert_eq!(
            rejected_report.issues[0]
                .trace
                .as_ref()
                .expect("rejected issue trace should exist")
                .human_options,
            vec!["comment", "retry"]
        );

        let review = create(
            &store,
            HiveLoopCreateRequest {
                title: "Review loop".to_string(),
                goal: "Ask for human review".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let review_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: review.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("needs-review".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("review loop should run");

        assert_eq!(review_report.contract.status, "needs-review");
        assert_eq!(review_report.verdicts[0].decision, "needs-review");
        assert_eq!(review_report.issues[0].issue.status, "Needs Review");
        assert_eq!(
            review_report.verdicts[0]
                .score
                .get("operator_review_needed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            review_report.verdicts[0]
                .score
                .pointer("/human_options/2")
                .and_then(|value| value.as_str()),
            Some("cancel")
        );
        assert_eq!(
            review_report.issues[0]
                .trace
                .as_ref()
                .expect("review issue trace should exist")
                .human_options,
            vec!["comment", "retry", "cancel"]
        );

        let exhausted = create(
            &store,
            HiveLoopCreateRequest {
                title: "Exhausted review loop".to_string(),
                goal: "Block after repeated invalid reviews".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        store
            .update_hive_loop_contract_state(
                exhausted.contract.id,
                "todo",
                "explorer",
                REVIEWER_INVALID_ROUND_BUDGET,
            )
            .expect("test should move loop to budget round");
        let exhausted_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: exhausted.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("exhausted reject loop should run");

        assert_eq!(exhausted_report.contract.status, "blocked");
        assert_eq!(exhausted_report.contract.current_round, 3);
        assert_eq!(exhausted_report.verdicts[0].decision, "blocked");
        assert_eq!(exhausted_report.issues[0].issue.status, "Blocked");
        assert_eq!(
            exhausted_report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("review_budget_exhausted")
        );
        assert!(exhausted_report.verdicts[0]
            .summary
            .contains("still invalid after 3 review rounds"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_audit_rejects_role_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-worker-role-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Worker role audit loop".to_string(),
                goal: "Catch worker receipt role drift".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        let mut payload = doer_packet.payload.clone();
        *payload
            .pointer_mut("/body/role_worker/role")
            .expect("role worker role should exist") = serde_json::json!("explorer");
        *payload
            .pointer_mut("/body/runtime_worker/role")
            .expect("runtime worker role should exist") = serde_json::json!("explorer");
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: report.contract.id,
                round: report.contract.current_round,
                object_kind: "EXECUTION_PACKET".to_string(),
                writer_role: "doer".to_string(),
                route_from: "doer".to_string(),
                route_to: "evaluator".to_string(),
                state_code: "submitted".to_string(),
                payload,
            })
            .expect("drifted packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let worker_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "worker_receipts")
            .expect("worker receipt audit should exist");
        assert!(!worker_check.passed);
        assert!(worker_check
            .details
            .pointer("/worker_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("role_binding"))))));
        let runtime_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "runtime_policy")
            .expect("runtime policy audit should exist");
        assert!(!runtime_check.passed);
        assert!(runtime_check
            .details
            .pointer("/runtime_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("role_binding"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "worker_receipts:role_binding"));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "runtime_policy:worker_receipt:role_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_audit_rejects_missing_structured_receipt_fields() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-worker-receipt-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Worker receipt audit loop".to_string(),
                goal: "Catch incomplete structured worker receipts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        let mut payload = doer_packet.payload.clone();
        payload
            .pointer_mut("/body/role_worker/receipt")
            .and_then(|value| value.as_object_mut())
            .expect("role worker receipt should be an object")
            .remove("action");
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: report.contract.id,
                round: report.contract.current_round,
                object_kind: "EXECUTION_PACKET".to_string(),
                writer_role: "doer".to_string(),
                route_from: "doer".to_string(),
                route_to: "evaluator".to_string(),
                state_code: "submitted".to_string(),
                payload,
            })
            .expect("drifted packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let worker_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "worker_receipts")
            .expect("worker receipt audit should exist");
        assert!(!worker_check.passed);
        assert!(worker_check
            .details
            .pointer("/worker_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("receipt.action"))))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_policy_audit_rejects_codex_workers_missing_command_context() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-codex-context-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Codex context audit loop".to_string(),
                goal: "Catch codex worker context drift".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        let mut payload = doer_packet.payload.clone();
        for pointer in ["/body/role_worker", "/body/runtime_worker"] {
            let worker = payload
                .pointer_mut(pointer)
                .and_then(|value| value.as_object_mut())
                .expect("worker should be an object");
            worker.insert("kind".to_string(), serde_json::json!("codex"));
            worker.insert("mode".to_string(), serde_json::json!("codex-exec"));
        }
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: report.contract.id,
                round: report.contract.current_round,
                object_kind: "EXECUTION_PACKET".to_string(),
                writer_role: "doer".to_string(),
                route_from: "doer".to_string(),
                route_to: "evaluator".to_string(),
                state_code: "submitted".to_string(),
                payload,
            })
            .expect("drifted packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let runtime_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "runtime_policy")
            .expect("runtime policy audit should exist");

        assert!(!runtime_check.passed);
        assert!(runtime_check
            .details
            .pointer("/runtime_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("context.command"))))));
        assert!(audit_failure_details(&audit_report)
            .iter()
            .any(|detail| detail == "runtime_policy:worker_receipt:context.command"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_evidence_audit_rejects_codex_evidence_missing_command_context() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-codex-evidence-context-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Codex evidence context loop".to_string(),
                goal: "Catch codex evidence context drift".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_stage = report
            .stages
            .iter()
            .find(|stage| stage.role == "developer")
            .expect("developer stage should exist");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: report.contract.id,
                stage_id: Some(doer_stage.id),
                round: report.contract.current_round,
                kind: "execution_packet".to_string(),
                summary: "Drifted codex evidence without command context.".to_string(),
                path: None,
                payload: serde_json::json!({
                    "runtime": "codex",
                    "worker": {
                        "ok": true,
                        "kind": "codex",
                        "mode": "codex-exec",
                        "role": "developer",
                        "timeout_secs": 60,
                        "attempt_count": 1,
                        "max_attempts": 1,
                        "receipt_ok": true,
                        "receipt": {
                            "ok": true,
                            "role": "developer",
                            "action": "implement-admitted-candidate",
                            "evidence_summary": "codex evidence drifted",
                            "gates": { "packet_received": true }
                        }
                    }
                }),
            })
            .expect("drifted evidence should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let evidence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "stage_evidence")
            .expect("stage evidence audit should exist");

        assert!(!evidence_check.passed);
        assert!(evidence_check
            .details
            .pointer("/stage_evidence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .get("scope")
                .and_then(|value| value.as_str())
                == Some("evidence_worker")
                && error
                    .pointer("/errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|fields| fields
                        .iter()
                        .any(|field| field.as_str() == Some("context.command"))))));
        assert!(audit_failure_details(&audit_report)
            .iter()
            .any(|detail| detail == "stage_evidence:evidence_worker:context.command"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_evidence_audit_rejects_duplicate_stage_evidence() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-stage-evidence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Stage evidence audit loop".to_string(),
                goal: "Catch duplicated stage evidence in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_evidence = report
            .evidence
            .iter()
            .find(|row| row.kind == "execution_packet")
            .expect("doer evidence should exist");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: doer_evidence.loop_id,
                stage_id: doer_evidence.stage_id,
                round: doer_evidence.round,
                kind: doer_evidence.kind.clone(),
                summary: doer_evidence.summary.clone(),
                path: doer_evidence.path.clone(),
                payload: doer_evidence.payload.clone(),
            })
            .expect("duplicated evidence should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let evidence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "stage_evidence")
            .expect("stage evidence audit should exist");
        assert!(!evidence_check.passed);
        assert!(evidence_check
            .details
            .pointer("/stage_evidence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.stage_duplicate"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| { detail == "stage_evidence:evidence_stage:evidence.stage_duplicate" }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_sequence_audit_rejects_replayed_stages() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-stage-sequence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Stage replay audit loop".to_string(),
                goal: "Catch replayed stages in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_stage = report
            .stages
            .iter()
            .find(|stage| stage.role == "developer")
            .expect("developer stage should exist");
        store
            .insert_hive_loop_stage(HiveLoopStageCreate {
                loop_id: doer_stage.loop_id,
                round: doer_stage.round,
                role: doer_stage.role.clone(),
                status: doer_stage.status.clone(),
                summary: doer_stage.summary.clone(),
                input: doer_stage.input.clone(),
                output: doer_stage.output.clone(),
                started_at: doer_stage.started_at.clone(),
                completed_at: doer_stage.completed_at.clone(),
            })
            .expect("replayed stage should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let sequence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "stage_sequence")
            .expect("stage sequence audit should exist");
        assert!(!sequence_check.passed);
        assert!(sequence_check
            .details
            .pointer("/stage_sequence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("stage.role_duplicate"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "stage_sequence:stage_role:stage.role_duplicate"));
        let doctor_report = super::doctor(&store, created.contract.id)
            .expect("doctor should include audit details");
        assert!(doctor_report
            .failed_checks
            .iter()
            .any(|check| check == "stage_sequence"));
        assert!(doctor_report
            .audit_failure_details
            .iter()
            .any(|detail| detail == "stage_sequence:stage_role:stage.role_duplicate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn packet_sequence_audit_rejects_replayed_packets() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-packet-sequence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Packet replay audit loop".to_string(),
                goal: "Catch replayed packets in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: doer_packet.loop_id,
                round: doer_packet.round,
                object_kind: doer_packet.object_kind.clone(),
                writer_role: doer_packet.writer_role.clone(),
                route_from: doer_packet.route_from.clone(),
                route_to: doer_packet.route_to.clone(),
                state_code: doer_packet.state_code.clone(),
                payload: doer_packet.payload.clone(),
            })
            .expect("replayed packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let sequence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "packet_sequence")
            .expect("packet sequence audit should exist");
        assert!(!sequence_check.passed);
        assert!(sequence_check
            .details
            .pointer("/packet_sequence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("packet.route_duplicate"))))));
        let admission_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "admission_receipts")
            .expect("admission audit should exist");
        assert!(!admission_check.passed);
        assert!(admission_check
            .details
            .pointer("/admission_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("packet.admission_missing"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "packet_sequence:packet_route:packet.route_duplicate"));
        assert!(
            trace_report
                .trace
                .audit_failure_details
                .iter()
                .any(|detail| detail
                    == "admission_receipts:packet_admission:packet.admission_missing")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejected_admission_records_blocked_report_and_issue() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-admission-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Admission loop".to_string(),
                goal: "Block on a policy gate".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let execution_policy = created
            .policies
            .iter()
            .find(|policy| policy.object_kind == "EXECUTION_PACKET")
            .expect("execution policy should exist");
        store
            .update_hive_loop_policy_gate(execution_policy.id, "unknown_gate")
            .expect("policy gate should be updated");

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("admission rejection should still return a report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "developer");
        assert_eq!(report.issues[0].issue.status, "Blocked");
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "blocked");
        assert_eq!(
            report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("admission_rejected")
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/admission_integrity")
                .and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(
            report.verdicts[0]
                .evidence
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert!(report
            .admissions
            .iter()
            .any(|admission| admission.result == "rejected"
                && admission.reason == "unknown_gate failed"));
        let rejected_admission = report
            .admissions
            .iter()
            .find(|admission| admission.result == "rejected")
            .expect("rejected admission should be recorded");
        assert_eq!(
            rejected_admission
                .policy
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(ADMISSION_SCHEMA_VERSION)
        );
        assert_eq!(
            rejected_admission
                .policy
                .pointer("/packet/object_kind")
                .and_then(|value| value.as_str()),
            Some("EXECUTION_PACKET")
        );
        assert_eq!(
            rejected_admission
                .policy
                .pointer("/gate/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert!(report
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "admission_rejection"));
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("blocked evidence report should resolve");
        let blocked_evidence = evidence_report
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "admission_rejection")
            .expect("admission rejection evidence should be summarized");
        assert_eq!(blocked_evidence.blocked_phase.as_deref(), Some("developer"));
        assert!(blocked_evidence
            .operator_options
            .iter()
            .any(|option| option == "retry"));
        assert!(blocked_evidence.missing_receipts.is_empty());
        assert!(report
            .issues
            .first()
            .expect("issue should exist")
            .comments
            .iter()
            .any(|comment| comment
                .body
                .contains("Compiler admission blocked at developer")));
        let blocked_trace = report.issues[0]
            .trace
            .as_ref()
            .expect("blocked issue should include trace");
        assert_eq!(blocked_trace.audit_passed, Some(false));
        assert!(blocked_trace
            .audit_failed_checks
            .iter()
            .any(|check| check == "active_policy_registry"));
        assert!(blocked_trace
            .audit_failed_checks
            .iter()
            .any(|check| check == "admission_receipts"));
        let audit_report =
            super::audit(&store, created.contract.id).expect("loop audit should resolve");
        assert!(!audit_report.passed);
        assert!(audit_report
            .checks
            .iter()
            .any(|check| check.name == "active_policy_registry" && !check.passed));
        assert!(audit_report
            .checks
            .iter()
            .any(|check| check.name == "admission_receipts" && !check.passed));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_decisions_update_issue_comment_and_loop_contract() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-decision-action-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Human decision loop".to_string(),
                goal: "Exercise issue decisions".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("loop should be created");
        let blocked = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should block");
        let issue_id = blocked.issues[0].issue.id;
        assert_eq!(
            blocked.issues[0]
                .actions
                .iter()
                .map(|action| action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["comment", "retry", "request-review", "cancel"]
        );
        let review_action = blocked.issues[0]
            .actions
            .iter()
            .find(|action| action.action == "request-review")
            .expect("blocked issue should expose review action");
        assert_eq!(review_action.schema_version, ISSUE_ACTION_SCHEMA_VERSION);
        assert_eq!(review_action.source, "human_options");
        assert_eq!(review_action.input, "note");
        let mut corrupt_actions = blocked.issues[0].actions.clone();
        corrupt_actions.retain(|action| action.action != "request-review");
        corrupt_actions[0].schema_version = "bad.schema".to_string();
        corrupt_actions[1].source = "status_fallback".to_string();
        corrupt_actions
            .iter_mut()
            .find(|action| action.action == "cancel")
            .expect("cancel action should exist")
            .destructive = false;
        let action_error = issue_action_audit_error(
            &blocked.issues[0].issue,
            &blocked.contract,
            blocked.issues[0]
                .trace
                .as_ref()
                .expect("blocked issue should include trace"),
            &corrupt_actions,
        )
        .expect("corrupt action metadata should fail audit");
        let action_error_fields = action_error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .expect("action errors should be listed")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(action_error_fields.contains(&"action.sequence"));
        assert!(action_error_fields.contains(&"action.schema_version"));
        assert!(action_error_fields.contains(&"action.source"));
        assert!(action_error_fields.contains(&"action.destructive"));

        let review_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "request-review".to_string(),
                author: "human".to_string(),
                body: Some("Need policy owner".to_string()),
            },
        )
        .expect("issue should move to review");
        let review_contract = store
            .get_hive_loop_contract(created.contract.id)
            .expect("contract query should succeed")
            .expect("contract should exist");
        assert_eq!(review_card.issue.status, "Needs Review");
        assert_eq!(review_contract.status, "needs-review");
        assert_eq!(review_contract.active_phase, "human-review");
        assert_eq!(
            review_card
                .actions
                .iter()
                .map(|action| action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["comment", "retry", "cancel"]
        );
        let review_doctor = review_card
            .doctor
            .as_ref()
            .expect("review card should include doctor summary");
        assert_eq!(review_doctor.health, "needs_review");
        assert_eq!(review_doctor.counts.audit_failed_count, 1);
        assert_eq!(review_doctor.failed_checks, vec!["runtime_policy"]);
        assert!(review_doctor
            .next_actions
            .iter()
            .any(|action| action.contains("issue retry-run")));
        assert!(review_doctor
            .next_actions
            .iter()
            .any(|action| action.contains("issue show") && action.contains("--compact")));
        assert!(!review_doctor
            .next_actions
            .iter()
            .any(|action| action.contains("request-review")));
        assert!(review_card
            .comments
            .iter()
            .any(|comment| comment.body.contains("Need policy owner")));
        assert!(review_card.comments.iter().any(|comment| {
            comment.body.contains("Need policy owner")
                && comment
                    .payload
                    .get("schema_version")
                    .and_then(|value| value.as_str())
                    == Some(OPERATOR_DECISION_SCHEMA_VERSION)
                && comment
                    .payload
                    .get("action")
                    .and_then(|value| value.as_str())
                    == Some("request-review")
        }));
        let review_evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        assert!(review_evidence.iter().any(|evidence| {
            evidence.kind == "operator_decision"
                && evidence
                    .payload
                    .get("schema_version")
                    .and_then(|value| value.as_str())
                    == Some(OPERATOR_DECISION_SCHEMA_VERSION)
                && evidence
                    .payload
                    .pointer("/operator/action")
                    .and_then(|value| value.as_str())
                    == Some("request-review")
        }));
        let review_trace = review_card
            .trace
            .as_ref()
            .expect("review card should retain loop trace");
        assert_eq!(review_trace.audit_failed_count, 1);
        assert_eq!(review_trace.operator_event_count, 1);
        assert_eq!(review_trace.round_operator_event_count, 1);
        assert_eq!(
            review_trace
                .last_operator_event
                .as_ref()
                .and_then(|event| event.action.as_deref()),
            Some("request-review")
        );
        assert_eq!(
            review_trace
                .operator_events
                .first()
                .and_then(|event| event.issue_status.as_deref()),
            Some("Needs Review")
        );
        let review_decision_summary = review_trace
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "operator_decision")
            .expect("review decision should be summarized");
        assert_eq!(
            review_decision_summary.operator_author.as_deref(),
            Some("human")
        );
        assert_eq!(
            review_decision_summary.operator_action.as_deref(),
            Some("request-review")
        );
        let review_audit =
            super::audit(&store, created.contract.id).expect("review audit should resolve");
        assert!(!review_audit.passed);
        assert!(review_audit
            .checks
            .iter()
            .any(|check| check.name == "stage_sequence" && check.passed));
        assert!(review_audit
            .checks
            .iter()
            .any(|check| check.name == "stage_evidence" && check.passed));
        assert!(review_audit
            .checks
            .iter()
            .any(|check| check.name == "runtime_policy" && !check.passed));

        let retry_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: None,
            },
        )
        .expect("issue should retry");
        let retry_contract = store
            .get_hive_loop_contract(created.contract.id)
            .expect("contract query should succeed")
            .expect("contract should exist");
        assert_eq!(retry_card.issue.status, "Todo");
        assert_eq!(retry_contract.status, "todo");
        assert_eq!(retry_contract.active_phase, "explorer");
        assert_eq!(
            retry_contract.current_round,
            blocked.contract.current_round + 1
        );
        let retry_trace = retry_card
            .trace
            .as_ref()
            .expect("retry card should retain loop trace");
        assert_eq!(retry_trace.current_round, retry_contract.current_round);
        assert_eq!(retry_trace.packet_count, 1);
        assert_eq!(retry_trace.admission_count, 1);
        assert_eq!(retry_trace.evidence_count, 3);
        assert_eq!(retry_trace.verdict_count, 1);
        assert_eq!(retry_trace.round_packet_count, 0);
        assert_eq!(retry_trace.round_admission_count, 0);
        assert_eq!(retry_trace.round_evidence_count, 1);
        assert_eq!(retry_trace.round_verdict_count, 0);
        assert_eq!(retry_trace.round_receipt_required_count, 0);
        assert_eq!(retry_trace.round_receipt_missing_count, 0);
        assert_eq!(retry_trace.role_worker_count, 1);
        assert_eq!(retry_trace.role_worker_ok_count, 0);
        assert_eq!(retry_trace.round_role_worker_count, 0);
        assert_eq!(retry_trace.round_role_worker_ok_count, 0);
        assert_eq!(retry_trace.round_worker_duration_ms, 0);
        assert_eq!(retry_trace.round_worker_timeout_count, 0);
        assert_eq!(retry_trace.round_worker_retry_exhausted_count, 0);
        assert_eq!(retry_trace.verdict_schema, None);
        assert_eq!(retry_trace.last_decision, None);
        assert_eq!(retry_trace.worker_kind, None);
        assert_eq!(retry_trace.human_options, vec!["comment", "cancel"]);
        assert_eq!(retry_trace.operator_event_count, 2);
        assert_eq!(retry_trace.round_operator_event_count, 1);
        assert_eq!(
            retry_trace
                .last_operator_event
                .as_ref()
                .and_then(|event| event.action.as_deref()),
            Some("retry")
        );

        let cancel_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "cancel".to_string(),
                author: "human".to_string(),
                body: None,
            },
        )
        .expect("issue should cancel");
        let cancel_contract = store
            .get_hive_loop_contract(created.contract.id)
            .expect("contract query should succeed")
            .expect("contract should exist");
        assert_eq!(cancel_card.issue.status, "Canceled");
        assert_eq!(cancel_contract.status, "rejected");
        assert_eq!(cancel_contract.active_phase, "complete");
        assert_eq!(
            cancel_card
                .trace
                .as_ref()
                .expect("cancel card should retain trace")
                .human_options,
            vec!["comment"]
        );
        let cancel_trace = cancel_card
            .trace
            .as_ref()
            .expect("cancel card should retain trace");
        assert_eq!(cancel_trace.operator_event_count, 3);
        assert_eq!(cancel_trace.round_operator_event_count, 2);
        assert_eq!(
            cancel_trace
                .last_operator_event
                .as_ref()
                .and_then(|event| event.action.as_deref()),
            Some("cancel")
        );
        assert_eq!(
            cancel_trace
                .operator_events
                .iter()
                .filter_map(|event| event.action.as_deref())
                .collect::<Vec<_>>(),
            vec!["retry", "cancel"]
        );
        assert!(cancel_card
            .comments
            .iter()
            .any(|comment| comment.body.contains("Human canceled")));
        let decision_evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        assert!(decision_evidence.iter().any(|evidence| {
            evidence.kind == "operator_decision"
                && evidence.round == retry_contract.current_round
                && evidence
                    .payload
                    .pointer("/issue/comment_id")
                    .and_then(|value| value.as_i64())
                    .is_some()
                && evidence
                    .payload
                    .pointer("/operator/action")
                    .and_then(|value| value.as_str())
                    == Some("cancel")
        }));
        let audit_report =
            super::audit(&store, created.contract.id).expect("decision audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(issue_surface_check.passed);
        let retry_after_cancel = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: None,
            },
        );
        assert!(retry_after_cancel
            .expect_err("human-canceled issue should not retry")
            .to_string()
            .contains("not allowed"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_run_executes_todo_and_retry_control_flow() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-run-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let todo = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue run todo loop".to_string(),
                goal: "Run a Todo issue from the issue surface".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("todo loop should be created");
        let todo_report = run_issue(
            &store,
            IssueRunRequest {
                issue_id: todo.issues[0].issue.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
                retry: false,
                author: "human".to_string(),
                body: None,
            },
        )
        .expect("todo issue should run");
        assert_eq!(todo_report.contract.status, "kept");
        assert_eq!(todo_report.issues[0].issue.status, "Done");

        let blocked = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue retry-run loop".to_string(),
                goal: "Retry a blocked issue from the issue surface".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("blocked loop should be created");
        let blocked_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: blocked.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("unsupported runtime should block");
        let blocked_issue_id = blocked_report.issues[0].issue.id;
        let blocked_run = run_issue(
            &store,
            IssueRunRequest {
                issue_id: blocked_issue_id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
                retry: false,
                author: "human".to_string(),
                body: None,
            },
        );
        assert!(blocked_run
            .expect_err("blocked issue should require retry-run")
            .to_string()
            .contains("retry-run"));

        let retry_report = run_issue(
            &store,
            IssueRunRequest {
                issue_id: blocked_issue_id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
                retry: true,
                author: "human".to_string(),
                body: Some("Retry with local runtime".to_string()),
            },
        )
        .expect("retry-run should record decision and execute");
        assert_eq!(retry_report.contract.status, "kept");
        assert_eq!(retry_report.contract.current_round, 2);
        assert_eq!(retry_report.issues[0].issue.status, "Done");
        assert!(retry_report.issues[0]
            .comments
            .iter()
            .any(|comment| comment.body.contains("Retry with local runtime")));
        let operator_decisions = store
            .list_hive_loop_evidence(blocked.contract.id)
            .expect("evidence should list")
            .into_iter()
            .filter(|evidence| evidence.kind == "operator_decision")
            .collect::<Vec<_>>();
        assert_eq!(operator_decisions.len(), 1);
        assert_eq!(operator_decisions[0].round, 2);
        let audit_report =
            super::audit(&store, blocked.contract.id).expect("retry audit should resolve");
        assert!(audit_report.passed);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_comments_record_operator_evidence() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-comment-evidence-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Human comment loop".to_string(),
                goal: "Capture issue comments as loop evidence".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("loop should be created");
        let blocked = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should block");
        let issue_id = blocked.issues[0].issue.id;

        let comment_card = add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "operator".to_string(),
                body: "  Please inspect the missing role worker receipt.  ".to_string(),
            },
        )
        .expect("comment should be recorded");
        let operator_comment = comment_card
            .comments
            .iter()
            .find(|comment| comment.author == "operator")
            .expect("operator comment should be visible");
        assert_eq!(
            operator_comment.body,
            "Please inspect the missing role worker receipt."
        );
        assert_eq!(
            operator_comment
                .payload
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(OPERATOR_COMMENT_SCHEMA_VERSION)
        );
        assert_eq!(
            operator_comment
                .payload
                .get("loop_id")
                .and_then(|value| value.as_i64()),
            Some(created.contract.id)
        );
        assert_eq!(
            operator_comment
                .payload
                .get("round")
                .and_then(|value| value.as_i64()),
            Some(blocked.contract.current_round)
        );
        assert_eq!(
            operator_comment
                .payload
                .get("status")
                .and_then(|value| value.as_str()),
            Some("Blocked")
        );
        assert_eq!(
            operator_comment
                .payload
                .get("phase")
                .and_then(|value| value.as_str()),
            Some(blocked.contract.active_phase.as_str())
        );

        let evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        let comment_evidence = evidence
            .iter()
            .find(|evidence| evidence.kind == "operator_comment")
            .expect("operator comment should be ledger evidence");
        assert_eq!(comment_evidence.round, blocked.contract.current_round);
        assert_eq!(
            comment_evidence
                .payload
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(OPERATOR_COMMENT_SCHEMA_VERSION)
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/operator/comment_body")
                .and_then(|value| value.as_str()),
            Some("Please inspect the missing role worker receipt.")
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/issue/comment_id")
                .and_then(|value| value.as_i64()),
            Some(operator_comment.id)
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/loop/round")
                .and_then(|value| value.as_i64()),
            Some(blocked.contract.current_round)
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/loop/phase")
                .and_then(|value| value.as_str()),
            Some(blocked.contract.active_phase.as_str())
        );
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("evidence report should resolve");
        assert!(evidence_report.evidence.iter().any(|evidence| {
            evidence.kind == "operator_comment"
                && evidence.summary == "Please inspect the missing role worker receipt."
                && evidence.schema_version.as_deref() == Some(OPERATOR_COMMENT_SCHEMA_VERSION)
                && evidence.operator_author.as_deref() == Some("operator")
        }));
        let audit_report =
            super::audit(&store, created.contract.id).expect("comment audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(issue_surface_check.passed);
        assert!(issue_surface_check
            .details
            .pointer("/operator_evidence_count")
            .and_then(|value| value.as_u64())
            .is_some_and(|count| count >= 1));
        assert!(add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "operator".to_string(),
                body: "   ".to_string(),
            },
        )
        .is_err());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_mirror_exports_review_surface_and_actions() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-mirror-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Mirror export loop".to_string(),
                goal: "Export issue/status/comment for an external board".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: "linear:ENT-13".to_string(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;
        add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "human".to_string(),
                body: "Mirror me".to_string(),
            },
        )
        .expect("comment should be recorded before mirror export");

        let mirror = issue_mirror(&store, issue_id).expect("mirror should export");

        assert_eq!(mirror.schema_version, ISSUE_MIRROR_SCHEMA_VERSION);
        assert_eq!(mirror.provider, "linear");
        assert_eq!(mirror.review_surface, "linear:ENT-13");
        assert_eq!(
            mirror.external_key,
            format!("hive-loop-{}-issue-{issue_id}", created.contract.id)
        );
        assert_eq!(
            mirror
                .loop_contract
                .as_ref()
                .map(|contract| contract.review_surface.as_str()),
            Some("linear:ENT-13")
        );
        assert!(mirror.comments.iter().any(|comment| {
            comment.author == "human"
                && comment.body == "Mirror me"
                && comment
                    .payload
                    .get("schema_version")
                    .and_then(|value| value.as_str())
                    == Some(OPERATOR_COMMENT_SCHEMA_VERSION)
        }));
        assert!(mirror.actions.iter().any(|action| {
            action.action == "comment"
                && action.command
                    == format!("entrance hive issue comment {issue_id} --body <text> --compact")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_untyped_comments() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-surface-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue surface audit loop".to_string(),
                goal: "Detect untyped control-plane comments".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;
        store
            .insert_hive_comment(HiveCommentCreate {
                issue_id,
                author: "human".to_string(),
                body: "legacy untyped note".to_string(),
                payload: serde_json::json!({}),
            })
            .expect("legacy comment should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        assert!(issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("comment.payload.schema_version"))))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_issue_contract_status_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-status-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue status drift loop".to_string(),
                goal: "Detect drift between contract and issue status".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let run_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let issue_id = run_report.issues[0].issue.id;
        assert_eq!(run_report.contract.status, "kept");
        assert_eq!(run_report.issues[0].issue.status, "Done");

        store
            .update_hive_issue_status(issue_id, "Todo", Some("drifted issue status"))
            .expect("issue status should be mutated for audit probe");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        let errors = issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .expect("issue surface errors should be listed");
        assert!(errors.iter().any(|error| {
            error
                .pointer("/expected_status")
                .and_then(|value| value.as_str())
                == Some("Done")
                && error
                    .pointer("/actual_status")
                    .and_then(|value| value.as_str())
                    == Some("Todo")
                && error
                    .pointer("/errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|fields| {
                        fields
                            .iter()
                            .any(|field| field.as_str() == Some("issue.contract_status_binding"))
                    })
        }));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:issue:issue.contract_status_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_stage_system_comment_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-stage-comment-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Stage comment audit loop".to_string(),
                goal: "Detect drift between stage comments and stage evidence".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;
        let run_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_evidence = run_report
            .evidence
            .iter()
            .find(|row| row.kind == "execution_packet")
            .expect("developer evidence should exist");
        let doer_worker = doer_evidence
            .payload
            .get("worker")
            .cloned()
            .expect("developer evidence should carry a worker receipt");

        store
            .insert_hive_comment(HiveCommentCreate {
                issue_id,
                author: "hive".to_string(),
                body: "Developer admitted the execution packet.".to_string(),
                payload: serde_json::json!({
                    "schema_version": SYSTEM_COMMENT_SCHEMA_VERSION,
                    "source": "hive",
                    "loop_id": created.contract.id,
                    "round": 1,
                    "phase": "developer",
                    "stage_role": "developer",
                    "evidence_kind": "verdict_packet",
                    "evidence_id": doer_evidence.id,
                    "admission": "admitted",
                    "worker": doer_worker
                }),
            })
            .expect("drifted stage comment should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        let errors = issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .expect("issue surface errors should be listed");
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("comment.stage.evidence_kind"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("comment.stage.evidence_binding")))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:comment:comment.stage.evidence_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_operator_evidence_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-operator-evidence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Operator evidence audit loop".to_string(),
                goal: "Detect drift between operator comments and evidence".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;

        let comment_card = add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "human".to_string(),
                body: "Keep the operator trail honest".to_string(),
            },
        )
        .expect("operator comment should be recorded");
        let operator_comment = comment_card
            .comments
            .iter()
            .find(|comment| comment.author == "human")
            .expect("operator comment should be visible");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_comment".to_string(),
                summary: "drifted comment evidence".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "status": comment_card.issue.status,
                        "comment_id": operator_comment.id
                    },
                    "loop": {
                        "id": created.contract.id,
                        "status": created.contract.status,
                        "phase": created.contract.active_phase,
                        "round": created.contract.current_round
                    },
                    "operator": {
                        "author": "different-human",
                        "comment_body": "drifted body"
                    }
                }),
            })
            .expect("drifted operator comment evidence should insert");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_comment".to_string(),
                summary: "drifted comment loop binding".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "status": "Drifted",
                        "comment_id": operator_comment.id
                    },
                    "loop": {
                        "id": created.contract.id + 99,
                        "status": "blocked",
                        "phase": "doer",
                        "round": created.contract.current_round + 99
                    },
                    "operator": {
                        "author": "human",
                        "comment_body": operator_comment.body
                    }
                }),
            })
            .expect("drifted operator comment binding evidence should insert");

        let cancel_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "cancel".to_string(),
                author: "human".to_string(),
                body: Some("No longer needed".to_string()),
            },
        )
        .expect("todo issue should cancel");
        let cancel_comment = cancel_card
            .comments
            .iter()
            .find(|comment| {
                comment
                    .payload
                    .get("action")
                    .and_then(|value| value.as_str())
                    == Some("cancel")
            })
            .expect("cancel decision comment should be visible");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_decision".to_string(),
                summary: "drifted decision evidence".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "comment_id": cancel_comment.id,
                        "from_status": "Todo",
                        "to_status": "Todo"
                    },
                    "loop": {
                        "id": created.contract.id,
                        "next_status": "todo",
                        "next_phase": "explorer",
                        "round": created.contract.current_round
                    },
                    "operator": {
                        "author": "human",
                        "action": "retry",
                        "note": "wrong action",
                        "comment_body": cancel_comment.body
                    }
                }),
            })
            .expect("drifted operator decision evidence should insert");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_decision".to_string(),
                summary: "drifted retry round binding".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "comment_id": cancel_comment.id,
                        "from_status": "Todo",
                        "to_status": "Canceled"
                    },
                    "loop": {
                        "id": created.contract.id + 99,
                        "next_status": "rejected",
                        "next_phase": "explorer",
                        "round": created.contract.current_round + 99
                    },
                    "operator": {
                        "author": "human",
                        "action": "cancel",
                        "note": "wrong round",
                        "comment_body": cancel_comment.body
                    }
                }),
            })
            .expect("drifted decision round evidence should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        let errors = issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .expect("issue surface errors should be listed");
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.author_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.comment_body_binding")))));
        assert!(errors.iter().any(|error| {
            error.pointer("/kind").and_then(|value| value.as_str()) == Some("operator_comment")
                && error
                    .pointer("/errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|fields| {
                        fields
                            .iter()
                            .any(|field| field.as_str() == Some("evidence.loop_id_binding"))
                            && fields
                                .iter()
                                .any(|field| field.as_str() == Some("evidence.loop_round_binding"))
                            && fields.iter().any(|field| {
                                field.as_str() == Some("evidence.comment_round_binding")
                            })
                            && fields.iter().any(|field| {
                                field.as_str() == Some("evidence.comment_status_binding")
                            })
                            && fields.iter().any(|field| {
                                field.as_str() == Some("evidence.comment_phase_binding")
                            })
                    })
        }));
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.action_binding")))));
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.loop_id_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.loop_round_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.loop_phase_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.comment_next_round_binding")))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.author_binding"));
        assert!(trace_report.trace.audit_failure_details.iter().any(
            |detail| detail == "issue_surface:operator_evidence:evidence.comment_body_binding"
        ));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.action_binding"));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.loop_round_binding"));
        assert!(trace_report.trace.audit_failure_details.iter().any(
            |detail| detail == "issue_surface:operator_evidence:evidence.comment_round_binding"
        ));
        let doctor_report = super::doctor(&store, created.contract.id)
            .expect("doctor should include audit details");
        assert!(doctor_report.audit_failure_details.iter().any(
            |detail| detail == "issue_surface:operator_evidence:evidence.comment_body_binding"
        ));
        let issue_card = issue(&store, issue_id).expect("issue card should include doctor details");
        let issue_doctor = issue_card
            .doctor
            .expect("issue doctor should be present for linked loop");
        assert!(issue_doctor
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.action_binding"));

        let _ = fs::remove_dir_all(root);
    }
}
