const PACKET_SCHEMA_VERSION: &str = "entrance.hive.packet.v1";
const POLICY_SCHEMA_VERSION: &str = "entrance.hive.policy.v1";
const ADMISSION_SCHEMA_VERSION: &str = "entrance.hive.admission.v1";
const VERDICT_SCHEMA_VERSION: &str = "entrance.hive.verdict.v1";
const WORKER_RECEIPT_SCHEMA_VERSION: &str = "entrance.hive.worker_receipt.v1";
const OPERATOR_DECISION_SCHEMA_VERSION: &str = "entrance.hive.operator_decision.v1";
const OPERATOR_COMMENT_SCHEMA_VERSION: &str = "entrance.hive.operator_comment.v1";
pub const OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION: &str =
    "entrance.hive.operator_confirmation_receipt.v1";
const ISSUE_ACTION_SCHEMA_VERSION: &str = "entrance.hive.issue_action.v1";
pub const OPERATOR_ACTION_POLICY_SCHEMA_VERSION: &str = "entrance.hive.operator_action_policy.v1";
pub const OPERATOR_ACTION_CONFIRMATION_ARG: &str = "operator_confirmed";
const SYSTEM_COMMENT_SCHEMA_VERSION: &str = "entrance.hive.system_comment.v1";
const AUDIT_SCHEMA_VERSION: &str = "entrance.hive.audit.v1";
const DOCTOR_SCHEMA_VERSION: &str = "entrance.hive.doctor.v1";
const WORKER_LIFECYCLE_SCHEMA_VERSION: &str = "entrance.hive.worker_lifecycle.v1";
const RUNTIME_PREFLIGHT_SCHEMA_VERSION: &str = "entrance.hive.runtime_preflight.v1";
const RUNTIME_CAPABILITY_PREVIEW_SCHEMA_VERSION: &str =
    "entrance.hive.runtime_capability_preview.v1";
const LOOP_DASHBOARD_SCHEMA_VERSION: &str = "entrance.hive.loop_dashboard.v1";
const EVIDENCE_DRILLDOWN_SCHEMA_VERSION: &str = "entrance.hive.evidence_drilldown.v1";
const EVIDENCE_MANIFEST_SCHEMA_VERSION: &str = "entrance.hive.evidence_manifest.v1";
const ISSUE_TIMELINE_SCHEMA_VERSION: &str = "entrance.hive.issue_timeline.v1";
const ISSUE_TIMELINE_ITEM_SCHEMA_VERSION: &str = "entrance.hive.issue_timeline_item.v1";
const ISSUE_TRANSITION_POLICY_SCHEMA_VERSION: &str = "entrance.hive.issue_transition_policy.v1";
const ISSUE_TRANSITION_ADMISSION_SCHEMA_VERSION: &str =
    "entrance.hive.issue_transition_admission.v1";
const VERDICT_SCORE_METRICS: &[&str] = &[
    "stage_completeness",
    "runtime_readiness",
    "evidence_presence",
    "admission_integrity",
    "target_alignment",
    "goal_alignment",
    "acceptance_evidence",
    "implementation_specificity",
    "regression_risk",
];
const DEFAULT_WORKER_TIMEOUT_SECS: u64 = 60;
const MAX_WORKER_TIMEOUT_SECS: u64 = 600;
const DEFAULT_WORKER_ATTEMPTS: u64 = 1;
const MAX_WORKER_ATTEMPTS: u64 = 3;

#[derive(Debug, Clone, Copy)]
struct GateSpec {
    name: &'static str,
    description: &'static str,
    expected_object_kind: Option<&'static str>,
    required_receipts: &'static [&'static str],
    check: GateCheck,
}

#[derive(Debug, Clone, Copy)]
struct GateEvaluationContext<'a> {
    packets: &'a [HiveLoopPacket],
    admissions: &'a [HiveLoopAdmission],
}

#[derive(Debug, Clone)]
struct CandidateBindingStatus {
    passed: bool,
    reason: String,
    expected_candidate: Option<String>,
    accepted_candidate: Option<String>,
    explorer_packet_id: Option<i64>,
    explorer_candidate_count: usize,
}

#[derive(Debug, Clone, Copy)]
enum GateCheck {
    ReceiptRequirementsSatisfied,
    BodyFieldPresent(&'static str),
    DecisionPresent,
    RuntimePolicyReady,
    AcceptedCandidateBound,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionPolicyRegistry {
    pub schema_version: String,
    pub owner: String,
    pub scope: String,
    pub state_classes: Vec<IssueTransitionStateClassSpec>,
    pub actions: Vec<IssueTransitionActionPolicySpec>,
    pub state_machine: Vec<IssueTransitionStateMachineSpec>,
    pub confirmation: IssueTransitionConfirmationSpec,
    pub reviewer_fallback: IssueTransitionReviewerFallbackPolicy,
    pub resource_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionStateClassSpec {
    pub class: String,
    pub statuses: Vec<String>,
    pub terminal: bool,
    pub human_decision_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionActionPolicySpec {
    pub action: String,
    pub label: String,
    pub from_statuses: Vec<String>,
    pub to_status: String,
    pub gate: String,
    pub source: String,
    pub input: String,
    pub destructive: bool,
    pub requires_confirmation: bool,
    pub runtime_required: bool,
    pub command_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionStateMachineSpec {
    pub status: String,
    pub state_class: String,
    pub terminal: bool,
    pub human_decision_required: bool,
    pub allowed_actions: Vec<IssueTransitionStateMachineActionSpec>,
    pub blocked_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionStateMachineActionSpec {
    pub action: String,
    pub label: String,
    pub to_status: String,
    pub gate: String,
    pub source: String,
    pub input: String,
    pub destructive: bool,
    pub requires_confirmation: bool,
    pub runtime_required: bool,
    pub command_template: String,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionConfirmationSpec {
    pub required_actions: Vec<String>,
    pub confirmation_arg: String,
    pub receipt_schema: String,
    pub policy_schema_version: String,
    pub policy_resource: String,
    pub actor_identity_resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionReviewerFallbackPolicy {
    pub trigger_decision: String,
    pub invalid_round_budget: i64,
    pub fallback_status: String,
    pub human_decision_statuses: Vec<String>,
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
const COMPAT_LOOP_ROLES: &[&str] = &["explorer", "doer", "evaluator"];
const REVIEWER_INVALID_ROUND_BUDGET: i64 = 3;
const ACCEPTED_CANDIDATE_BOUND_GATE: &str = "accepted_candidate_bound";
const TARGET_BINDING_SCHEMA_VERSION: &str = "entrance.hive.target_binding.v1";

const CURRENT_LOOP_POLICIES: &[LoopPolicySpec] = &[
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
        gate: ACCEPTED_CANDIDATE_BOUND_GATE,
    },
    LoopPolicySpec {
        object_kind: "VERDICT_PACKET",
        writer_role: "reviewer",
        route_from: "reviewer",
        route_to: "complete",
        gate: "verdict_receipts_present",
    },
];

const DEFAULT_LOOP_POLICIES: &[LoopPolicySpec] = &[
    LoopPolicySpec {
        object_kind: "PREFLIGHT_PACKET",
        writer_role: "kernel",
        route_from: "kernel",
        route_to: "explorer",
        gate: "runtime_policy_ready",
    },
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
        gate: ACCEPTED_CANDIDATE_BOUND_GATE,
    },
    LoopPolicySpec {
        object_kind: "VERDICT_PACKET",
        writer_role: "reviewer",
        route_from: "reviewer",
        route_to: "complete",
        gate: "verdict_receipts_present",
    },
];

const COMPAT_LOOP_POLICIES: &[LoopPolicySpec] = &[
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
    assessment: ReviewerGateAssessment,
    reviewer_invalid_rounds_used: i64,
    reviewer_invalid_budget_exhausted: bool,
}

#[derive(Debug, Clone)]
struct ReviewerGateAssessment {
    stage_completeness: f64,
    runtime_readiness: f64,
    evidence_presence: f64,
    admission_integrity: f64,
    target_alignment: f64,
    goal_alignment: f64,
    acceptance_evidence: f64,
    implementation_specificity: f64,
    regression_risk: f64,
    three_stages_recorded: bool,
    evidence_recorded: bool,
    runtime_ready: bool,
    admissions_clean: bool,
    target_bound: bool,
    semantic_gates_passed: bool,
    review_gates_passed: bool,
    observed_stage_roles: Vec<String>,
    missing_stage_roles: Vec<String>,
    expected_candidate: Option<String>,
    accepted_candidate: Option<String>,
    target_binding_reason: String,
    current_round_admission_count: usize,
    rejected_admission_count: usize,
    receipt_missing_count: usize,
    prior_stage_evidence_count: usize,
    expected_prior_stage_evidence_count: usize,
    failure_reasons: Vec<String>,
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
    pub issue_transitions: IssueTransitionPolicyRegistry,
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
pub struct HiveLoopEvidenceDrilldownReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub issue_id: Option<i64>,
    pub issue_status: Option<String>,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub runtime: String,
    pub drilldown_state: String,
    pub summary: String,
    pub evidence_count: usize,
    pub items: Vec<HiveLoopEvidenceDrilldownItem>,
    pub blockers: Vec<HiveLoopEvidenceBlocker>,
    pub human_decision: HiveLoopEvidenceHumanDecision,
    pub resources: HiveLoopEvidenceDrilldownResources,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceDrilldownItem {
    pub id: i64,
    pub round: i64,
    pub stage_role: Option<String>,
    pub kind: String,
    pub summary: String,
    pub created_at: String,
    pub path: Option<String>,
    pub schema_version: Option<String>,
    pub admission_result: Option<String>,
    pub blocked_phase: Option<String>,
    pub blocker: Option<String>,
    pub operator_options: Vec<String>,
    pub worker: Option<HiveLoopEvidenceWorkerDrilldown>,
    pub receipt: Option<HiveLoopEvidenceReceiptDrilldown>,
    pub artifacts: Vec<HiveLoopEvidenceArtifact>,
    pub payload: HiveLoopEvidencePayloadInspection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceWorkerDrilldown {
    pub kind: Option<String>,
    pub mode: Option<String>,
    pub ok: Option<bool>,
    pub receipt_ok: Option<bool>,
    pub timed_out: Option<bool>,
    pub status: Option<i64>,
    pub duration_ms: Option<u64>,
    pub timeout_secs: Option<u64>,
    pub attempt_count: Option<u64>,
    pub max_attempts: Option<u64>,
    pub retry_exhausted: Option<bool>,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub action: Option<String>,
    pub evidence_summary: Option<String>,
    pub gate_count: Option<usize>,
    pub receipt_errors: Vec<String>,
    pub transcript_excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceReceiptDrilldown {
    pub schema_version: Option<String>,
    pub role: Option<String>,
    pub action: Option<String>,
    pub ok: Option<bool>,
    pub evidence_summary: Option<String>,
    pub gates: Vec<HiveLoopEvidenceReceiptGate>,
    pub raw_excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceReceiptGate {
    pub name: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceArtifact {
    pub kind: String,
    pub path: Option<String>,
    pub summary: Option<String>,
    pub manifest: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidencePayloadInspection {
    pub top_level_keys: Vec<String>,
    pub excerpt: String,
    pub diff_from_previous: HiveLoopEvidencePayloadDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidencePayloadDiff {
    pub relative_to_evidence_id: Option<i64>,
    pub added_keys: Vec<String>,
    pub removed_keys: Vec<String>,
    pub changed_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceBlocker {
    pub evidence_id: Option<i64>,
    pub scope: String,
    pub round: i64,
    pub kind: String,
    pub phase: Option<String>,
    pub reason: String,
    pub operator_options: Vec<String>,
    pub decision_surface: HiveLoopEvidenceDecisionSurface,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceDecisionSurface {
    pub required: bool,
    pub issue_status: Option<String>,
    pub primary_action: Option<String>,
    pub actions: Vec<HiveLoopEvidenceDecisionAction>,
    pub policy_resource: String,
    pub review_queue_resource: String,
    pub confirmation_arg: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceDecisionAction {
    pub issue_action: IssueAction,
    pub recommended: bool,
    pub operator_option: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceHumanDecision {
    pub required: bool,
    pub issue_status: Option<String>,
    pub options: Vec<String>,
    pub actions: Vec<IssueAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceDrilldownResources {
    pub evidence_drilldown: String,
    pub evidence_manifest: String,
    pub loop_dashboard: String,
    pub worker_lifecycle: String,
    pub runtime_preflight: String,
    pub issue: Option<String>,
    pub issue_control: Option<String>,
    pub review_queue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceManifestReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub issue_id: Option<i64>,
    pub issue_status: Option<String>,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub runtime: String,
    pub manifest_state: String,
    pub summary: String,
    pub coverage: HiveLoopEvidenceManifestCoverage,
    pub entries: Vec<HiveLoopEvidenceManifestEntry>,
    pub resources: HiveLoopEvidenceManifestResources,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceManifestCoverage {
    pub evidence_count: usize,
    pub entry_count: usize,
    pub payload_count: usize,
    pub receipt_count: usize,
    pub transcript_count: usize,
    pub artifact_count: usize,
    pub path_count: usize,
    pub path_present_count: usize,
    pub path_missing_count: usize,
    pub path_unverified_count: usize,
    pub path_none_count: usize,
    pub digest_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceManifestEntry {
    pub id: String,
    pub evidence_id: i64,
    pub round: i64,
    pub stage_role: Option<String>,
    pub kind: String,
    pub source: String,
    pub entry_kind: String,
    pub label: String,
    pub summary: String,
    pub path: Option<String>,
    pub path_status: String,
    pub schema_version: Option<String>,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub required: bool,
    pub verified: bool,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopEvidenceManifestResources {
    pub evidence_manifest: String,
    pub evidence_drilldown: String,
    pub loop_dashboard: String,
    pub worker_lifecycle: String,
    pub runtime_preflight: String,
    pub issue: Option<String>,
    pub issue_control: Option<String>,
    pub review_queue: String,
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
pub struct HiveLoopWorkerLifecycleReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub issue_id: Option<i64>,
    pub issue_status: Option<String>,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub runtime: String,
    pub lifecycle_state: String,
    pub summary: String,
    pub policy: HiveLoopWorkerLifecyclePolicy,
    pub current: HiveLoopWorkerLifecycleRound,
    pub rounds: Vec<HiveLoopWorkerLifecycleRound>,
    pub failures: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopWorkerLifecyclePolicy {
    pub schema_version: String,
    pub expected_roles: Vec<String>,
    pub compat_roles: Vec<String>,
    pub default_timeout_secs: u64,
    pub max_timeout_secs: u64,
    pub timeout_env: String,
    pub default_attempts: u64,
    pub max_attempts: u64,
    pub attempts_env: String,
    pub reviewer_invalid_round_budget: i64,
    pub fallback_status: String,
    pub human_decision_statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopWorkerLifecycleRound {
    pub round: i64,
    pub status: String,
    pub decision: Option<String>,
    pub expected_roles: Vec<String>,
    pub observed_roles: Vec<String>,
    pub missing_roles: Vec<String>,
    pub worker_count: usize,
    pub worker_ok_count: usize,
    pub worker_timeout_count: usize,
    pub worker_retry_exhausted_count: usize,
    pub worker_duration_ms: u64,
    pub reviewer_invalid_rounds_used: i64,
    pub reviewer_invalid_budget_exhausted: bool,
    pub failures: Vec<String>,
    pub workers: Vec<HiveLoopWorkerLifecycleWorker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopWorkerLifecycleWorker {
    pub evidence_id: i64,
    pub round: i64,
    pub role: String,
    pub stage_role: Option<String>,
    pub evidence_kind: String,
    pub kind: Option<String>,
    pub mode: Option<String>,
    pub ok: Option<bool>,
    pub receipt_ok: Option<bool>,
    pub timed_out: Option<bool>,
    pub status: Option<i64>,
    pub duration_ms: Option<u64>,
    pub timeout_secs: Option<u64>,
    pub attempt_count: Option<u64>,
    pub max_attempts: Option<u64>,
    pub retry_exhausted: Option<bool>,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub action: Option<String>,
    pub evidence_summary: Option<String>,
    pub gate_count: Option<usize>,
    pub receipt_errors: Vec<String>,
    pub transcript_excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimePreflightReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub issue_id: Option<i64>,
    pub issue_status: Option<String>,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub runtime: String,
    pub preflight_state: String,
    pub summary: String,
    pub policy: HiveLoopRuntimePreflightPolicy,
    pub preview: HiveLoopRuntimePreflightPreview,
    pub current: Option<HiveLoopRuntimePreflightObservation>,
    pub failures: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimePreflightPolicy {
    pub schema_version: String,
    pub gate: String,
    pub object_kind: String,
    pub route_from: String,
    pub route_to: String,
    pub required_receipts: Vec<String>,
    pub supported_runtimes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimePreflightPreview {
    pub runtime: String,
    pub supported: bool,
    pub probe_ok: bool,
    pub blocker: Option<String>,
    pub runtime_probe: serde_json::Value,
    pub selected_policy: Option<RuntimePolicySpec>,
    pub capability_preview: HiveLoopRuntimeCapabilityPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimeCapabilityPreview {
    pub schema_version: String,
    pub runtime: String,
    pub worker_spawn_ready: bool,
    pub worker_spawn_blockers: Vec<String>,
    pub admission_scope: Vec<String>,
    pub worker_mode: Option<String>,
    pub sandbox: HiveLoopRuntimeSandboxPreview,
    pub artifact_capture: HiveLoopRuntimeArtifactCapturePreview,
    pub human_boundary: HiveLoopRuntimeHumanBoundaryPreview,
    pub worker_context: HiveLoopRuntimeWorkerContextPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimeSandboxPreview {
    pub filesystem: String,
    pub network: String,
    pub writes_artifacts: bool,
    pub process_isolation: String,
    pub write_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimeArtifactCapturePreview {
    pub expected: bool,
    pub mode: String,
    pub archive_ready: bool,
    pub resource: String,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimeHumanBoundaryPreview {
    pub review_surface: String,
    pub autonomy_level: String,
    pub confirmation_arg: String,
    pub human_decision_statuses: Vec<String>,
    pub protected_actions: Vec<String>,
    pub reviewer_invalid_round_budget: i64,
    pub fallback_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimeWorkerContextPreview {
    pub required: Vec<String>,
    pub supplied_by_driver: Vec<String>,
    pub missing_before_spawn: Vec<String>,
    pub required_receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopRuntimePreflightObservation {
    pub packet_id: i64,
    pub admission_id: Option<i64>,
    pub round: i64,
    pub result: Option<String>,
    pub reason: Option<String>,
    pub gate: Option<String>,
    pub gate_passed: Option<bool>,
    pub receipt_required: Vec<String>,
    pub receipt_missing: Vec<String>,
    pub runtime: Option<String>,
    pub supported: Option<bool>,
    pub probe_ok: Option<bool>,
    pub blocker: Option<String>,
    pub runtime_probe: Option<serde_json::Value>,
    pub capability_preview: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardReport {
    pub schema_version: String,
    pub loop_id: i64,
    pub issue: Option<HiveIssue>,
    pub status: String,
    pub active_phase: String,
    pub current_round: i64,
    pub runtime: String,
    pub dashboard_state: String,
    pub summary: String,
    pub kernel: HiveLoopDashboardKernel,
    pub agents: Vec<HiveLoopDashboardAgent>,
    pub reviewer: HiveLoopDashboardReviewer,
    pub human_decision: HiveLoopDashboardHumanDecision,
    pub health: HiveLoopDashboardHealth,
    pub rounds: Vec<HiveLoopDashboardRound>,
    pub comments_count: usize,
    pub latest_comment: Option<HiveLoopDashboardComment>,
    pub resources: HiveLoopDashboardResources,
    pub primary_next_action: Option<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardKernel {
    pub preflight_state: String,
    pub gate: String,
    pub gate_passed: Option<bool>,
    pub route_from: String,
    pub route_to: String,
    pub object_kind: String,
    pub blocker: Option<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardAgent {
    pub role: String,
    pub state: String,
    pub evidence_id: Option<i64>,
    pub worker_kind: Option<String>,
    pub worker_mode: Option<String>,
    pub ok: Option<bool>,
    pub receipt_ok: Option<bool>,
    pub timed_out: Option<bool>,
    pub retry_exhausted: Option<bool>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardReviewer {
    pub decision: Option<String>,
    pub reason_code: Option<String>,
    pub score_vector: Vec<ScoreVectorMetric>,
    pub human_options: Vec<String>,
    pub reviewer_invalid_rounds_used: i64,
    pub reviewer_invalid_round_budget: i64,
    pub reviewer_invalid_budget_exhausted: bool,
    pub fallback_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardHumanDecision {
    pub required: bool,
    pub issue_status: Option<String>,
    pub options: Vec<String>,
    pub actions: Vec<IssueAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardHealth {
    pub health: String,
    pub audit_failed_count: usize,
    pub failed_checks: Vec<String>,
    pub audit_failure_details: Vec<String>,
    pub missing_receipts: Vec<String>,
    pub worker_failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardRound {
    pub round: i64,
    pub current: bool,
    pub status: String,
    pub decision: Option<String>,
    pub reason_code: Option<String>,
    pub retry_lineage: Option<String>,
    pub blocker: Option<String>,
    pub packet_count: usize,
    pub admission_count: usize,
    pub evidence_count: usize,
    pub verdict_count: usize,
    pub rejected_count: usize,
    pub receipt_missing_count: usize,
    pub worker_count: usize,
    pub worker_ok_count: usize,
    pub groups: HiveLoopDashboardRoundGroups,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardRoundGroups {
    pub packets: Vec<HiveLoopDashboardRoundPacket>,
    pub admissions: Vec<HiveLoopDashboardRoundAdmission>,
    pub evidence: Vec<HiveLoopDashboardRoundEvidence>,
    pub verdicts: Vec<HiveLoopDashboardRoundVerdict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardRoundPacket {
    pub id: i64,
    pub object_kind: String,
    pub writer_role: String,
    pub route_from: String,
    pub route_to: String,
    pub state_code: String,
    pub admission_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardRoundAdmission {
    pub id: i64,
    pub packet_id: i64,
    pub result: String,
    pub gate: Option<String>,
    pub gate_passed: Option<bool>,
    pub reason: String,
    pub missing_receipts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardRoundEvidence {
    pub id: i64,
    pub stage_role: Option<String>,
    pub kind: String,
    pub admission_result: Option<String>,
    pub blocked_phase: Option<String>,
    pub worker_ok: Option<bool>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardRoundVerdict {
    pub id: i64,
    pub decision: String,
    pub reason_code: Option<String>,
    pub score_vector: Vec<ScoreVectorMetric>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardComment {
    pub id: i64,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopDashboardResources {
    pub loop_dashboard: String,
    pub evidence_drilldown: String,
    pub evidence_manifest: String,
    pub runtime_preflight: String,
    pub worker_lifecycle: String,
    pub issue: Option<String>,
    pub issue_control: Option<String>,
    pub review_queue: String,
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
pub struct IssueTransitionPolicyReport {
    pub schema_version: String,
    pub issue: HiveIssue,
    pub loop_id: Option<i64>,
    pub policy_owner: String,
    pub policy_scope: String,
    pub registry: IssueTransitionPolicyRegistry,
    pub state_class: String,
    pub human_decision_required: bool,
    pub summary: String,
    pub allowed_actions: Vec<IssueTransitionPolicyAction>,
    pub blocked_actions: Vec<IssueTransitionPolicyBlockedAction>,
    pub confirmation: IssueTransitionConfirmationPolicy,
    pub reviewer_budget: Option<IssueTransitionReviewerBudget>,
    pub resources: IssueTransitionPolicyResources,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTransitionPolicyAction {
    pub action: IssueAction,
    pub from_status: String,
    pub to_status: Option<String>,
    pub gate: String,
    pub requires_human: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTransitionPolicyBlockedAction {
    pub action: String,
    pub required_statuses: Vec<String>,
    pub reason: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTransitionConfirmationPolicy {
    pub required: bool,
    pub required_actions: Vec<String>,
    pub confirmation_arg: String,
    pub receipt_schema: String,
    pub policy_schema_version: String,
    pub policy_resource: String,
    pub review_queue_resource: String,
    pub actor_identity_resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTransitionReviewerBudget {
    pub current_round: i64,
    pub reviewer_invalid_rounds_used: i64,
    pub reviewer_invalid_round_budget: i64,
    pub reviewer_invalid_budget_exhausted: bool,
    pub fallback_status: String,
    pub current_decision: Option<String>,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTransitionPolicyResources {
    pub issue: String,
    pub issue_control: String,
    pub transition_policy: String,
    pub issue_timeline: String,
    pub loop_dashboard: Option<String>,
    pub worker_lifecycle: Option<String>,
    pub runtime_preflight: Option<String>,
    pub review_queue: String,
    pub policy_registry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineReport {
    pub schema_version: String,
    pub issue: HiveIssue,
    pub loop_id: Option<i64>,
    pub timeline_state: String,
    pub summary: String,
    pub counts: IssueTimelineCounts,
    pub rounds: Vec<IssueTimelineRoundGroup>,
    pub human_decision: IssueTimelineHumanDecision,
    pub decision_receipts: Vec<IssueTimelineDecisionReceipt>,
    pub items: Vec<IssueTimelineItem>,
    pub resources: IssueTimelineResources,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineItemReport {
    pub schema_version: String,
    pub issue: HiveIssue,
    pub loop_id: Option<i64>,
    pub item: IssueTimelineItem,
    pub item_index: usize,
    pub previous_item_id: Option<String>,
    pub next_item_id: Option<String>,
    pub round: Option<IssueTimelineRoundGroup>,
    pub decision_receipt: Option<IssueTimelineDecisionReceipt>,
    pub resources: IssueTimelineItemResources,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineItemResources {
    pub issue: String,
    pub issue_control: String,
    pub issue_timeline: String,
    pub item_permalink: String,
    pub linked_resource: Option<String>,
    pub review_queue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineCounts {
    pub item_count: usize,
    pub comment_count: usize,
    pub evidence_count: usize,
    pub verdict_count: usize,
    pub operator_event_count: usize,
    pub blocker_count: usize,
    pub receipt_issue_count: usize,
    pub decision_receipt_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineRoundGroup {
    pub round: Option<i64>,
    pub label: String,
    pub state: String,
    pub item_ids: Vec<String>,
    pub item_count: usize,
    pub comment_count: usize,
    pub evidence_count: usize,
    pub verdict_count: usize,
    pub operator_event_count: usize,
    pub blocker_count: usize,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub phases: Vec<String>,
    pub decisions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineHumanDecision {
    pub required: bool,
    pub issue_status: Option<String>,
    pub primary_action: Option<String>,
    pub actions: Vec<IssueTimelineDecisionAction>,
    pub receipt_count: usize,
    pub last_receipt: Option<IssueTimelineDecisionReceipt>,
    pub policy_resource: String,
    pub review_queue_resource: String,
    pub issue_control_resource: String,
    pub confirmation_arg: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineDecisionAction {
    pub issue_action: IssueAction,
    pub recommended: bool,
    pub operator_option: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineDecisionReceipt {
    pub id: String,
    pub source: String,
    pub timestamp: String,
    pub round: Option<i64>,
    pub action: Option<String>,
    pub author: Option<String>,
    pub comment_id: Option<i64>,
    pub evidence_id: Option<i64>,
    pub receipt_schema_version: Option<String>,
    pub receipt_source: Option<String>,
    pub policy_schema_version: Option<String>,
    pub confirmation_arg: Option<String>,
    pub human_confirmed: Option<bool>,
    pub client_name: Option<String>,
    pub actor_label: Option<String>,
    pub actor_trust: Option<String>,
    pub note_excerpt: Option<String>,
    pub linked_resource: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineItem {
    pub id: String,
    pub permalink: String,
    pub sequence: usize,
    pub timestamp: String,
    pub source: String,
    pub event_kind: String,
    pub actor: String,
    pub round: Option<i64>,
    pub status: Option<String>,
    pub phase: Option<String>,
    pub title: String,
    pub summary: String,
    pub body_excerpt: Option<String>,
    pub schema_version: Option<String>,
    pub comment_id: Option<i64>,
    pub evidence_id: Option<i64>,
    pub verdict_id: Option<i64>,
    pub action: Option<String>,
    pub decision: Option<String>,
    pub blocker: Option<String>,
    pub linked_resource: Option<String>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTimelineResources {
    pub issue: String,
    pub issue_control: String,
    pub issue_timeline: String,
    pub loop_dashboard: Option<String>,
    pub evidence_drilldown: Option<String>,
    pub evidence_manifest: Option<String>,
    pub runtime_preflight: Option<String>,
    pub worker_lifecycle: Option<String>,
    pub review_queue: String,
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
    #[serde(default)]
    pub confirmation_required: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_arg: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_schema: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_schema_version: Option<String>,
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
    pub reason_code: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_gate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_from_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_to_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_policy_resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_transition_policy_resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_requires_confirmation: Option<bool>,
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
    pub confirmation_receipt: Option<OperatorConfirmationReceipt>,
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
    pub confirmation_receipt: Option<OperatorConfirmationReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorConfirmationReceipt {
    pub schema_version: String,
    pub source: String,
    pub policy_schema_version: String,
    pub confirmation_arg: String,
    pub human_confirmed: bool,
    pub action: String,
    pub author: String,
    pub marker: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<OperatorConfirmationClient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<OperatorConfirmationActor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorConfirmationClient {
    pub name: String,
    pub version: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorConfirmationActor {
    pub id: String,
    pub label: String,
    pub source: String,
    pub trust: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTransitionAdmissionReceipt {
    pub schema_version: String,
    pub policy_schema_version: String,
    pub policy_owner: String,
    pub policy_scope: String,
    pub policy_resource: String,
    pub transition_policy_resource: String,
    pub action: String,
    pub gate: String,
    pub result: String,
    pub from_status: String,
    pub to_status: Option<String>,
    pub requires_confirmation: bool,
    pub allowed_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IssueDecisionAction {
    Retry,
    RequestReview,
    Cancel,
}
