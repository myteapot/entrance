use std::{
    collections::HashMap,
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
const POLICY_SCHEMA_VERSION: &str = "entrance.hive.policy.v1";
const ADMISSION_SCHEMA_VERSION: &str = "entrance.hive.admission.v1";
const VERDICT_SCHEMA_VERSION: &str = "entrance.hive.verdict.v1";
const OPERATOR_DECISION_SCHEMA_VERSION: &str = "entrance.hive.operator_decision.v1";

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
pub struct PolicyRegistryReport {
    pub schema_version: String,
    pub gates: Vec<PolicyGateSpec>,
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
pub struct IssueCard {
    pub issue: HiveIssue,
    pub comments: Vec<HiveComment>,
    pub trace: Option<IssueTraceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTraceSummary {
    pub current_round: i64,
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
    pub worker_kind: Option<String>,
    pub worker_mode: Option<String>,
    pub worker_ok: Option<bool>,
    pub evidence: Vec<IssueEvidenceSummary>,
    pub stages: Vec<IssueStageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreVectorMetric {
    pub name: String,
    pub value: Option<f64>,
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
    pub worker_kind: Option<String>,
    pub worker_mode: Option<String>,
    pub worker_ok: Option<bool>,
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

    if contract.status != "todo" {
        return report(store, contract.id);
    }

    let runtime_probe = probe_runtime(&runtime);

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
    let explorer_worker = run_role_worker(&runtime, "explorer", &contract, &runtime_probe);
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
            "role_worker": explorer_worker,
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
            "worker": explorer_worker,
            "admission": explorer_admission.result
        }),
    })?;

    store.update_hive_loop_contract_state(
        contract.id,
        "running",
        "doer",
        contract.current_round,
    )?;
    let runtime_worker = run_role_worker(&runtime, "doer", &contract, &runtime_probe);
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
            "role_worker": runtime_worker,
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
            "role_worker": runtime_worker,
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
    let evaluator_worker = run_role_worker(&runtime, "evaluator", &contract, &runtime_probe);
    let runtime_ready = runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        && worker_ok(&explorer_worker)
        && worker_ok(&runtime_worker)
        && worker_ok(&evaluator_worker);
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
            "role_worker": evaluator_worker,
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
        typed_verdict.packet_payload(&evaluator_worker),
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
            "worker": evaluator_worker,
            "admission": evaluator_admission.result
        }),
    })?;
    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: typed_verdict.decision.as_str().to_string(),
        summary: typed_verdict.summary.clone(),
        score: typed_verdict.score_payload(),
        evidence: typed_verdict.evidence_payload(&runtime, &evaluator_worker),
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
            "{} Workers: explorer={}, doer={}, evaluator={}. Admissions: explorer={}, doer={}, evaluator={}.",
            typed_verdict.summary,
            runtime_worker_summary(&explorer_worker),
            runtime_worker_summary(&runtime_worker),
            runtime_worker_summary(&evaluator_worker),
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
                "role_workers": {
                    "explorer": explorer_worker,
                    "doer": runtime_worker,
                    "evaluator": evaluator_worker
                },
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
        trace: issue_trace_summary(store, loop_id)?,
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
    issue_card_from_issue(store, issue)
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

    let issue_summary = action.issue_summary(next_round);
    let comment_body = action.comment_body(next_round, note);

    store.update_hive_issue_status(issue.id, action.issue_status(), Some(&issue_summary))?;
    store.insert_hive_comment(HiveCommentCreate {
        issue_id: issue.id,
        author: author.clone(),
        body: comment_body.clone(),
        payload: serde_json::json!({
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
        &author,
        note,
        next_round,
        &issue_summary,
        &comment_body,
    )?;

    issue_card(store, issue.id)
}

fn record_operator_decision_evidence(
    store: &Store,
    issue: &HiveIssue,
    action: IssueDecisionAction,
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
        .map(|loop_id| issue_trace_summary(store, loop_id))
        .transpose()?;
    Ok(IssueCard {
        issue,
        comments,
        trace,
    })
}

fn issue_trace_summary(store: &Store, loop_id: i64) -> Result<IssueTraceSummary> {
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

    Ok(IssueTraceSummary {
        current_round,
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
        human_options: last_verdict
            .map(|verdict| human_options(&verdict.score))
            .unwrap_or_default(),
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
        evidence: evidence
            .iter()
            .filter(|row| row.round == current_round)
            .map(|row| issue_evidence_summary(row, &stage_roles))
            .collect(),
        stages: issue_stage_summaries(&stages, &evidence, current_round),
    })
}

fn stage_role_map(stages: &[HiveLoopStage]) -> HashMap<i64, String> {
    stages
        .iter()
        .map(|stage| (stage.id, stage.role.clone()))
        .collect()
}

fn issue_evidence_summary(
    row: &HiveLoopEvidence,
    stage_roles: &HashMap<i64, String>,
) -> IssueEvidenceSummary {
    let worker = row.payload.get("worker");
    IssueEvidenceSummary {
        id: row.id,
        round: row.round,
        stage_role: row
            .stage_id
            .and_then(|stage_id| stage_roles.get(&stage_id).cloned()),
        kind: row.kind.clone(),
        summary: row.summary.clone(),
        schema_version: schema_version(&row.payload),
        admission_result: row
            .payload
            .get("admission")
            .or_else(|| row.payload.get("result"))
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
        transcript_excerpt: worker
            .and_then(worker_transcript_excerpt)
            .map(|value| truncate_text(&value, 240)),
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

    fn evidence_payload(
        &self,
        runtime: &str,
        evaluator_worker: &serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "schema_version": VERDICT_SCHEMA_VERSION,
            "decision": self.decision.as_str(),
            "reason_code": self.reason_code,
            "evidence_count": self.evidence_count + 1,
            "runtime": runtime,
            "runtime_ready": self.runtime_ready,
            "role_worker": evaluator_worker,
            "source": {
                "evaluator": "hive-loop-control",
                "round_evidence_before_verdict": self.evidence_count
            }
        })
    }

    fn packet_payload(&self, evaluator_worker: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "decision": self.decision.as_str(),
            "summary": self.summary,
            "reason_code": self.reason_code,
            "score": self.score_payload(),
            "role_worker": evaluator_worker
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

impl GateCheck {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReceiptRequirementsSatisfied => "receipt_requirements_satisfied",
            Self::BodyFieldPresent(_) => "body_field_present",
            Self::DecisionPresent => "decision_present",
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

fn seed_default_policies(store: &Store, loop_id: i64) -> Result<()> {
    for policy in [
        (
            "EXPLORATION_PACKET",
            "explorer",
            "explorer",
            "doer",
            "candidate_receipts_present",
        ),
        (
            "EXECUTION_PACKET",
            "doer",
            "doer",
            "evaluator",
            "runtime_receipts_present",
        ),
        (
            "VERDICT_PACKET",
            "evaluator",
            "evaluator",
            "complete",
            "verdict_receipts_present",
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
            description: "Doer packets must carry runtime probe, runtime worker, artifact, and role worker receipts.",
            expected_object_kind: Some("EXECUTION_PACKET"),
            required_receipts: &["runtime_probe", "runtime_worker", "artifact", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        "verdict_receipts_present" => Some(GateSpec {
            name: "verdict_receipts_present",
            description: "Evaluator packets must carry decision, summary, score, and role worker receipts.",
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
            description: "Packet body must include an allowed evaluator decision.",
            expected_object_kind: None,
            required_receipts: &["decision"],
            check: GateCheck::DecisionPresent,
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
    }
}

fn receipt_requirements_satisfied(payload: &serde_json::Value) -> bool {
    let (_required, missing) = receipt_requirement_status(payload);
    missing.is_empty()
}

fn gate_failure_reason(gate: &str, payload: &serde_json::Value) -> String {
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
    let missing = required
        .iter()
        .filter(|requirement| !receipt_value_present(body, requirement))
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

fn receipt_value_present(body: &serde_json::Value, requirement: &str) -> bool {
    if matches!(requirement, "role_worker" | "runtime_worker") {
        return body.get(requirement).is_some_and(worker_ok);
    }
    body.get(requirement).is_some_and(|value| match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        serde_json::Value::Array(values) => !values.is_empty(),
        serde_json::Value::Object(values) => !values.is_empty(),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    })
}

fn packet_object_kind(payload: &serde_json::Value) -> Option<&str> {
    payload.get("object_kind").and_then(|value| value.as_str())
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
) -> serde_json::Value {
    match runtime {
        "local" => serde_json::json!({
            "ok": true,
            "kind": "local",
            "mode": "deterministic-worker",
            "role": role,
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
                    "role": role,
                    "skipped": true,
                    "error": "codex probe failed"
                });
            }
            run_codex_worker(contract, role)
        }
        other => serde_json::json!({
            "ok": false,
            "kind": "unsupported",
            "role": role,
            "runtime": other,
            "skipped": true,
            "error": "unsupported runtime"
        }),
    }
}

fn role_worker_action(role: &str) -> &'static str {
    match role {
        "explorer" => "compile-candidate",
        "doer" => "record-local-loop-ledger",
        "evaluator" => "check-gates-and-verdict-envelope",
        _ => "unknown-role-action",
    }
}

fn run_codex_worker(contract: &HiveLoopContract, role: &str) -> serde_json::Value {
    let output_path = std::env::temp_dir().join(format!(
        "entrance-hive-codex-worker-{}-{}-{}.txt",
        contract.id,
        role,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let prompt = codex_worker_prompt(contract, role);
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
                "role": role,
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
            "role": role,
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

fn codex_worker_prompt(contract: &HiveLoopContract, role: &str) -> String {
    let role_duty = match role {
        "explorer" => {
            "compile the goal into a bounded candidate and confirm the constraints are explicit"
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

You are the {role_name} role inside a constrained Explorer -> Doer -> Evaluator loop.
Rules:
- Do not modify files.
- Do not make network calls.
- Keep the response compact.
- Return only JSON with keys: ok, role, action, evidence_summary, gates.
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
    fn policy_registry_and_loop_policies_expose_typed_gate_specs() {
        let registry = policy_registry();
        assert_eq!(registry.schema_version, POLICY_SCHEMA_VERSION);
        assert!(registry.gates.len() >= 6);
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

        let _ = fs::remove_dir_all(root);
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
            Some("evaluator")
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
            .is_some_and(|description| description.contains("Evaluator packets")));
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
        assert_eq!(trace.worker_kind.as_deref(), Some("local"));
        assert_eq!(trace.worker_ok, Some(true));
        assert_eq!(trace.evidence.len(), 3);
        let doer_evidence = trace
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("doer evidence summary should exist");
        assert_eq!(doer_evidence.stage_role.as_deref(), Some("doer"));
        assert_eq!(doer_evidence.admission_result.as_deref(), Some("admitted"));
        assert_eq!(doer_evidence.worker_kind.as_deref(), Some("local"));
        assert_eq!(doer_evidence.worker_ok, Some(true));
        assert!(doer_evidence
            .transcript_excerpt
            .as_deref()
            .is_some_and(|excerpt| excerpt.contains("Local doer worker")));
        assert_eq!(
            trace
                .stages
                .iter()
                .map(|stage| stage.role.as_str())
                .collect::<Vec<_>>(),
            vec!["explorer", "doer", "evaluator"]
        );
        let doer_trace = trace
            .stages
            .iter()
            .find(|stage| stage.role == "doer")
            .expect("doer stage trace should exist");
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
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("loop evidence report should resolve");
        assert_eq!(evidence_report.evidence.len(), 3);
        assert!(evidence_report.evidence.iter().any(|evidence| {
            evidence.stage_role.as_deref() == Some("evaluator")
                && evidence.kind == "verdict_packet"
                && evidence.worker_ok == Some(true)
        }));

        let rerun = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
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
        assert_eq!(worker_receipt_ok(r#"{"ok":true}"#), Some(true));
        assert_eq!(
            worker_receipt_ok("prefix {\"ok\":false,\"reason\":\"blocked\"} suffix"),
            Some(false)
        );
        assert_eq!(worker_receipt_ok("not json"), None);
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
            "doer",
            "doer",
            "evaluator",
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
        assert_eq!(
            review_report.issues[0]
                .trace
                .as_ref()
                .expect("review issue trace should exist")
                .human_options,
            vec!["comment", "retry", "cancel"]
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
        assert_eq!(retry_trace.verdict_schema, None);
        assert_eq!(retry_trace.last_decision, None);
        assert_eq!(retry_trace.worker_kind, None);

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
        let decision_evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        assert!(decision_evidence.iter().any(|evidence| {
            evidence.kind == "operator_decision"
                && evidence.round == retry_contract.current_round
                && evidence
                    .payload
                    .pointer("/operator/action")
                    .and_then(|value| value.as_str())
                    == Some("cancel")
        }));

        let _ = fs::remove_dir_all(root);
    }
}
