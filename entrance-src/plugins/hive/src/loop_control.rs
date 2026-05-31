use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use entrance_core::{
    HiveComment, HiveCommentCreate, HiveIssue, HiveIssueCreate, HiveLoopAdmission,
    HiveLoopAdmissionCreate, HiveLoopContract, HiveLoopContractCreate, HiveLoopEvidence,
    HiveLoopEvidenceCreate, HiveLoopPacket, HiveLoopPacketCreate, HiveLoopPolicy,
    HiveLoopPolicyCreate, HiveLoopStage, HiveLoopStageCreate, HiveLoopVerdict,
    HiveLoopVerdictCreate, Store,
};
use serde::{Deserialize, Serialize};

const PACKET_SCHEMA_VERSION: &str = "entrance.hive.packet.v1";
const ADMISSION_SCHEMA_VERSION: &str = "entrance.hive.admission.v1";
const VERDICT_SCHEMA_VERSION: &str = "entrance.hive.verdict.v1";

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
pub struct IssueCard {
    pub issue: HiveIssue,
    pub comments: Vec<HiveComment>,
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
        payload: serde_json::json!({
            "loop_id": loop_id,
            "goal": request.goal,
            "next_phase": "explorer",
            "policy_count": 3
        }),
    })?;

    report(store, loop_id)
}

pub fn run(store: &Store, request: HiveLoopRunRequest) -> Result<HiveLoopReport> {
    let mut contract = store
        .get_hive_loop_contract(request.loop_id)?
        .with_context(|| format!("unknown hive loop `{}`", request.loop_id))?;
    let runtime = request.runtime.unwrap_or_else(|| contract.runtime.clone());
    let issues = store.list_hive_issues_for_loop(contract.id)?;
    let issue_id = issues.first().map(|issue| issue.id);

    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(
            issue_id,
            "Doing",
            Some("Explorer, Doer, and Evaluator are running."),
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
            "constraints": [
                "keep work in SQLite/Hive",
                "separate explorer, doer, evaluator stages",
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
        "doer",
        serde_json::json!({
            "candidate": "Run a local MVP loop through Hive",
            "constraints": [
                "keep work in SQLite/Hive",
                "separate explorer, doer, evaluator stages",
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
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(explorer_stage),
        round: contract.current_round,
        kind: "exploration_packet".to_string(),
        summary: "Explorer produced a concrete local-loop candidate.".to_string(),
        path: None,
        payload: serde_json::json!({
            "candidate": "local-loop-mvp",
            "approach_count": contract.approach_space.len(),
            "admission": explorer_admission.result
        }),
    })?;

    store.update_hive_loop_contract_state(
        contract.id,
        "running",
        "doer",
        contract.current_round,
    )?;
    let runtime_probe = probe_runtime(&runtime);
    let runtime_worker = run_runtime_worker(&runtime, &contract, &runtime_probe);
    let doer_stage = insert_stage(
        store,
        &contract,
        "doer",
        "Doer executed the accepted MVP action and captured runtime evidence.",
        serde_json::json!({
            "candidate": "local-loop-mvp",
            "runtime": runtime
        }),
        serde_json::json!({
            "runtime_probe": runtime_probe,
            "runtime_worker": runtime_worker,
            "artifact": "hive-loop-ledger"
        }),
    )?;
    let doer_admission = emit_and_admit(
        store,
        &contract,
        "EXECUTION_PACKET",
        "doer",
        "doer",
        "evaluator",
        serde_json::json!({
            "runtime": runtime,
            "runtime_probe": runtime_probe,
            "runtime_worker": runtime_worker,
            "artifact": "hive-loop-ledger"
        }),
    )?;
    if doer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "doer",
            Some(doer_stage),
            &doer_admission,
        );
    }
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(doer_stage),
        round: contract.current_round,
        kind: "execution_packet".to_string(),
        summary: format!("Doer ran `{runtime}` runtime worker."),
        path: None,
        payload: serde_json::json!({
            "runtime": runtime,
            "probe": runtime_probe,
            "worker": runtime_worker,
            "admission": doer_admission.result
        }),
    })?;

    store.update_hive_loop_contract_state(
        contract.id,
        "evaluating",
        "evaluator",
        contract.current_round,
    )?;
    let evidence = store.list_hive_loop_evidence(contract.id)?;
    let runtime_ready = runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        && runtime_worker
            .get("ok")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let runtime_failure = runtime_failure(&runtime_probe, &runtime_worker);
    let decision_override = parse_decision_override(request.decision.as_deref())?;
    let typed_verdict = build_verdict(
        decision_override,
        runtime_ready,
        runtime_failure,
        &runtime,
        evidence.len(),
    );
    let evaluator_stage = insert_stage(
        store,
        &contract,
        "evaluator",
        &typed_verdict.summary,
        serde_json::json!({
            "evidence_count": evidence.len(),
            "eval_space": contract.eval_space
        }),
        serde_json::json!({
            "decision": typed_verdict.decision.as_str(),
            "gates": {
                "three_stages_recorded": true,
                "evidence_recorded": !evidence.is_empty(),
                "runtime_ready": typed_verdict.runtime_ready
            }
        }),
    )?;
    let evaluator_admission = emit_and_admit(
        store,
        &contract,
        "VERDICT_PACKET",
        "evaluator",
        "evaluator",
        "complete",
        typed_verdict.packet_payload(),
    )?;
    if evaluator_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "evaluator",
            Some(evaluator_stage),
            &evaluator_admission,
        );
    }
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(evaluator_stage),
        round: contract.current_round,
        kind: "verdict_packet".to_string(),
        summary: typed_verdict.summary.clone(),
        path: None,
        payload: serde_json::json!({
            "decision": typed_verdict.decision.as_str(),
            "reason_code": typed_verdict.reason_code,
            "runtime_ready": typed_verdict.runtime_ready,
            "admission": evaluator_admission.result
        }),
    })?;
    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: typed_verdict.decision.as_str().to_string(),
        summary: typed_verdict.summary.clone(),
        score: typed_verdict.score_payload(),
        evidence: typed_verdict.evidence_payload(&runtime),
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
            "{} Worker: {}. Admissions: explorer={}, doer={}, evaluator={}.",
            typed_verdict.summary,
            runtime_worker_summary(&runtime_worker),
            explorer_admission.result,
            doer_admission.result,
            evaluator_admission.result
        );
        add_system_comment(
            store,
            issue_id,
            &admission_summary,
            serde_json::json!({
                "loop_id": contract.id,
                "decision": typed_verdict.decision.as_str(),
                "reason_code": typed_verdict.reason_code,
                "phase": "evaluator",
                "runtime_worker": runtime_worker,
                "admissions": {
                    "explorer": explorer_admission.result,
                    "doer": doer_admission.result,
                    "evaluator": evaluator_admission.result
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
        .map(|issue| {
            let comments = store.list_hive_comments(issue.id)?;
            Ok(IssueCard { issue, comments })
        })
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

pub fn panel(store: &Store) -> Result<Vec<IssueCard>> {
    store
        .list_hive_issues()?
        .into_iter()
        .map(|issue| {
            let comments = store.list_hive_comments(issue.id)?;
            Ok(IssueCard { issue, comments })
        })
        .collect()
}

pub fn add_comment(store: &Store, request: IssueCommentRequest) -> Result<IssueCard> {
    store.insert_hive_comment(HiveCommentCreate {
        issue_id: request.issue_id,
        author: default_text(request.author, "human"),
        body: request.body,
        payload: serde_json::json!({ "source": "operator" }),
    })?;

    let issue = store
        .list_hive_issues()?
        .into_iter()
        .find(|issue| issue.id == request.issue_id)
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let comments = store.list_hive_comments(issue.id)?;
    Ok(IssueCard { issue, comments })
}

pub fn decide_issue(store: &Store, request: IssueDecisionRequest) -> Result<IssueCard> {
    let action = parse_issue_decision_action(&request.action)?;
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
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

    store.update_hive_issue_status(
        issue.id,
        action.issue_status(),
        Some(action.issue_summary(next_round).as_str()),
    )?;
    store.insert_hive_comment(HiveCommentCreate {
        issue_id: issue.id,
        author,
        body: action.comment_body(next_round, note),
        payload: serde_json::json!({
            "source": "operator",
            "action": action.as_str(),
            "loop_id": issue.loop_id,
            "next_round": next_round,
            "note": note
        }),
    })?;

    issue_card(store, issue.id)
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
    let comments = store.list_hive_comments(issue.id)?;
    Ok(IssueCard { issue, comments })
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
        payload,
    })?;
    Ok(())
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
            "evaluator": "hive-loop-control",
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

    fn evidence_payload(&self, runtime: &str) -> serde_json::Value {
        serde_json::json!({
            "schema_version": VERDICT_SCHEMA_VERSION,
            "decision": self.decision.as_str(),
            "reason_code": self.reason_code,
            "evidence_count": self.evidence_count + 1,
            "runtime": runtime,
            "runtime_ready": self.runtime_ready,
            "source": {
                "evaluator": "hive-loop-control",
                "round_evidence_before_verdict": self.evidence_count
            }
        })
    }

    fn packet_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "decision": self.decision.as_str(),
            "summary": self.summary,
            "reason_code": self.reason_code,
            "score": self.score_payload()
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
                "unsupported evaluator decision `{other}`; expected keep, reject, needs-review, or blocked"
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
) -> TypedVerdict {
    if !runtime_ready {
        let reason_code = runtime_failure
            .unwrap_or(RuntimeFailure::Worker)
            .reason_code();
        return TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code,
            summary: format!(
                "Evaluator blocked the candidate: `{runtime}` {}.",
                runtime_failure
                    .unwrap_or(RuntimeFailure::Worker)
                    .summary_fragment()
            ),
            runtime_ready,
            evidence_count,
        };
    }

    match decision_override.unwrap_or(VerdictDecision::Keep) {
        VerdictDecision::Keep => TypedVerdict {
            decision: VerdictDecision::Keep,
            reason_code: "all_gates_passed",
            summary: "Evaluator kept the candidate: all MVP gates passed.".to_string(),
            runtime_ready,
            evidence_count,
        },
        VerdictDecision::Reject => TypedVerdict {
            decision: VerdictDecision::Reject,
            reason_code: "quality_gate_failed",
            summary: "Evaluator rejected the candidate: quality gate failed.".to_string(),
            runtime_ready,
            evidence_count,
        },
        VerdictDecision::NeedsReview => TypedVerdict {
            decision: VerdictDecision::NeedsReview,
            reason_code: "human_review_required",
            summary: "Evaluator requested human review for this candidate.".to_string(),
            runtime_ready,
            evidence_count,
        },
        VerdictDecision::Blocked => TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code: "operator_blocked",
            summary: "Evaluator blocked the candidate by operator decision.".to_string(),
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
    format!("{kind}/{mode} ok={ok}")
}

fn seed_default_policies(store: &Store, loop_id: i64) -> Result<()> {
    for policy in [
        (
            "EXPLORATION_PACKET",
            "explorer",
            "explorer",
            "doer",
            "candidate_present",
        ),
        (
            "EXECUTION_PACKET",
            "doer",
            "doer",
            "evaluator",
            "runtime_probe_present",
        ),
        (
            "VERDICT_PACKET",
            "evaluator",
            "evaluator",
            "complete",
            "decision_present",
        ),
    ] {
        store.insert_hive_loop_policy(HiveLoopPolicyCreate {
            loop_id,
            object_kind: policy.0.to_string(),
            writer_role: policy.1.to_string(),
            route_from: policy.2.to_string(),
            route_to: policy.3.to_string(),
            gate: policy.4.to_string(),
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
            let outcome = if passed { "passed" } else { "failed" };
            (
                result.to_string(),
                format!("{} {outcome}", policy.gate),
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
        "EXPLORATION_PACKET" => vec!["candidate", "constraints"],
        "EXECUTION_PACKET" => vec!["runtime_probe", "runtime_worker", "artifact"],
        "VERDICT_PACKET" => vec!["decision", "summary", "score"],
        _ => Vec::new(),
    }
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
            "state_code": &packet.state_code
        },
        "policy": policy.map(|policy| serde_json::json!({
            "id": policy.id,
            "object_kind": &policy.object_kind,
            "writer_role": &policy.writer_role,
            "route_from": &policy.route_from,
            "route_to": &policy.route_to,
            "gate": &policy.gate,
            "status": &policy.status
        })),
        "gate": {
            "name": gate_name,
            "passed": gate_passed
        }
    })
}

fn gate_passes(gate: &str, payload: &serde_json::Value) -> bool {
    if !typed_packet_envelope_valid(payload) {
        return false;
    }
    let body = packet_body(payload);
    match gate {
        "candidate_present" => body
            .get("candidate")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty()),
        "runtime_probe_present" => body.get("runtime_probe").is_some(),
        "decision_present" => body
            .get("decision")
            .and_then(|value| value.as_str())
            .is_some_and(|value| matches!(value, "keep" | "reject" | "needs-review" | "blocked")),
        _ => false,
    }
}

fn typed_packet_envelope_valid(payload: &serde_json::Value) -> bool {
    payload
        .get("schema_version")
        .and_then(|value| value.as_str())
        == Some(PACKET_SCHEMA_VERSION)
        && payload
            .get("object_kind")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
        && payload
            .pointer("/writer/role")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
        && payload
            .pointer("/route/from")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
        && payload
            .pointer("/route/to")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
        && payload.get("body").is_some()
}

fn packet_body(payload: &serde_json::Value) -> &serde_json::Value {
    payload.get("body").unwrap_or(payload)
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

fn run_runtime_worker(
    runtime: &str,
    contract: &HiveLoopContract,
    runtime_probe: &serde_json::Value,
) -> serde_json::Value {
    match runtime {
        "local" => serde_json::json!({
            "ok": true,
            "kind": "local",
            "mode": "deterministic-worker",
            "last_message": format!(
                "Local worker accepted loop #{} and recorded the task packet.",
                contract.id
            ),
            "packet": {
                "loop_id": contract.id,
                "round": contract.current_round,
                "role": "doer",
                "action": "record-local-loop-ledger"
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
                    "skipped": true,
                    "error": "codex probe failed"
                });
            }
            run_codex_worker(contract)
        }
        other => serde_json::json!({
            "ok": false,
            "kind": "unsupported",
            "runtime": other,
            "skipped": true,
            "error": "unsupported runtime"
        }),
    }
}

fn run_codex_worker(contract: &HiveLoopContract) -> serde_json::Value {
    let output_path = std::env::temp_dir().join(format!(
        "entrance-hive-codex-worker-{}-{}.txt",
        contract.id,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let prompt = codex_worker_prompt(contract);
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| ".".to_string());

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
    let result = run_command_with_timeout(command, Duration::from_secs(60));
    let last_message = std::fs::read_to_string(&output_path).unwrap_or_default();
    let _ = std::fs::remove_file(&output_path);

    match result {
        Ok(output) => {
            let receipt_ok = worker_receipt_ok(&last_message);
            serde_json::json!({
                "ok": output.status_success && !output.timed_out && receipt_ok.unwrap_or(true),
                "kind": "codex",
                "mode": "codex-exec",
                "started_at": started_at,
                "completed_at": chrono::Utc::now().to_rfc3339(),
                "timed_out": output.timed_out,
                "status": output.status_code,
                "receipt_ok": receipt_ok,
                "stdout": truncate_text(&output.stdout, 12000),
                "stderr": truncate_text(&output.stderr, 4000),
                "last_message": truncate_text(&last_message, 4000)
            })
        }
        Err(error) => serde_json::json!({
            "ok": false,
            "kind": "codex",
            "mode": "codex-exec",
            "started_at": started_at,
            "completed_at": chrono::Utc::now().to_rfc3339(),
            "error": error.to_string(),
            "last_message": truncate_text(&last_message, 4000)
        }),
    }
}

struct TimedCommandOutput {
    status_success: bool,
    status_code: Option<i32>,
    timed_out: bool,
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
    Ok(TimedCommandOutput {
        status_success: output.status.success(),
        status_code: output.status.code(),
        timed_out,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn codex_worker_prompt(contract: &HiveLoopContract) -> String {
    format!(
        r#"Entrance Hive Doer worker packet.

You are the Doer role inside a constrained Explorer -> Doer -> Evaluator loop.
Rules:
- Do not modify files.
- Do not make network calls.
- Keep the response compact.
- Return only JSON with keys: ok, role, action, evidence_summary, gates.
- The MVP execution is this receipt: validate that you received the typed packet,
  summarize the accepted action, and set ok=true unless you cannot process it.
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
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let json_value = serde_json::from_str::<serde_json::Value>(trimmed)
        .or_else(|_| {
            let start = trimmed.find('{').unwrap_or(0);
            let end = trimmed
                .rfind('}')
                .map(|index| index + 1)
                .unwrap_or(trimmed.len());
            serde_json::from_str::<serde_json::Value>(&trimmed[start..end])
        })
        .ok()?;
    json_value.get("ok").and_then(|value| value.as_bool())
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
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use entrance_core::Store;

    use super::*;

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
            },
        )
        .expect("loop should run");

        assert_eq!(report.contract.status, "kept");
        assert_eq!(report.contract.active_phase, "complete");
        assert_eq!(report.policies.len(), 3);
        assert_eq!(report.packets.len(), 3);
        assert!(report.packets.iter().all(|packet| packet
            .payload
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(PACKET_SCHEMA_VERSION)));
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
                .pointer("/receipt_requirements/1")
                .and_then(|value| value.as_str()),
            Some("runtime_worker")
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
                .pointer("/packet/schema_version")
                .and_then(|value| value.as_str()),
            Some(PACKET_SCHEMA_VERSION)
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
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "keep");
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

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_ok_reads_final_json_receipt() {
        assert_eq!(worker_receipt_ok(r#"{"ok":true}"#), Some(true));
        assert_eq!(
            worker_receipt_ok("prefix {\"ok\":false,\"reason\":\"blocked\"} suffix"),
            Some(false)
        );
        assert_eq!(worker_receipt_ok("not json"), None);
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
            },
        )
        .expect("blocked loop should still return a report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "complete");
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "blocked");
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
            Some(0.0)
        );
        assert_eq!(report.issues[0].issue.status, "Blocked");
        assert!(report
            .issues
            .first()
            .expect("issue should exist")
            .comments
            .iter()
            .any(|comment| comment.body.contains("unsupported-agent")));
        assert_eq!(report.admissions.len(), 3);
        assert!(report
            .admissions
            .iter()
            .all(|admission| admission.result == "admitted"));

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
            },
        )
        .expect("admission rejection should still return a report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "doer");
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
        assert!(report
            .issues
            .first()
            .expect("issue should exist")
            .comments
            .iter()
            .any(|comment| comment.body.contains("Compiler admission blocked at doer")));

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
            },
        )
        .expect("loop should block");
        let issue_id = blocked.issues[0].issue.id;

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
        assert!(review_card
            .comments
            .iter()
            .any(|comment| comment.body.contains("Need policy owner")));

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
        assert!(cancel_card
            .comments
            .iter()
            .any(|comment| comment.body.contains("Human canceled")));

        let _ = fs::remove_dir_all(root);
    }
}
