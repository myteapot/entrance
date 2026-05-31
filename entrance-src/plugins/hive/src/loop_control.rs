use std::process::Command;

use anyhow::{Context, Result};
use entrance_core::{
    HiveComment, HiveCommentCreate, HiveIssue, HiveIssueCreate, HiveLoopAdmission,
    HiveLoopAdmissionCreate, HiveLoopContract, HiveLoopContractCreate, HiveLoopEvidence,
    HiveLoopEvidenceCreate, HiveLoopPacket, HiveLoopPacketCreate, HiveLoopPolicy,
    HiveLoopPolicyCreate, HiveLoopStage, HiveLoopStageCreate, HiveLoopVerdict,
    HiveLoopVerdictCreate, Store,
};
use serde::{Deserialize, Serialize};

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
            "artifact": "hive-loop-ledger"
        }),
    )?;
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(doer_stage),
        round: contract.current_round,
        kind: "execution_packet".to_string(),
        summary: format!("Doer probed `{runtime}` runtime."),
        path: None,
        payload: serde_json::json!({
            "runtime": runtime,
            "probe": runtime_probe,
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
        .unwrap_or(false);
    let decision_override = parse_decision_override(request.decision.as_deref())?;
    let typed_verdict = build_verdict(decision_override, runtime_ready, &runtime, evidence.len());
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
            "{} Admissions: explorer={}, doer={}, evaluator={}.",
            typed_verdict.summary,
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
}

impl TypedVerdict {
    fn score_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "gates_passed": self.decision.gates_passed(),
            "stage_completeness": 1.0,
            "runtime_readiness": if self.runtime_ready { 1.0 } else { 0.0 },
            "operator_review_needed": self.decision.operator_review_required(),
            "reason_code": self.reason_code
        })
    }

    fn evidence_payload(&self, runtime: &str) -> serde_json::Value {
        serde_json::json!({
            "evidence_count": self.evidence_count + 1,
            "runtime": runtime,
            "reason_code": self.reason_code
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
    runtime: &str,
    evidence_count: usize,
) -> TypedVerdict {
    if !runtime_ready {
        return TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code: "runtime_unavailable",
            summary: format!("Evaluator blocked the candidate: `{runtime}` runtime probe failed."),
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
    let packet_id = store.insert_hive_loop_packet(HiveLoopPacketCreate {
        loop_id: contract.id,
        round: contract.current_round,
        object_kind: object_kind.to_string(),
        writer_role: writer_role.to_string(),
        route_from: route_from.to_string(),
        route_to: route_to.to_string(),
        state_code: "submitted".to_string(),
        payload: payload.clone(),
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

    let (result, reason, policy_json) = match matching_policy {
        Some(policy) if gate_passes(&policy.gate, &payload) => (
            "admitted".to_string(),
            format!("{} passed", policy.gate),
            serde_json::json!(policy),
        ),
        Some(policy) => (
            "rejected".to_string(),
            format!("{} failed", policy.gate),
            serde_json::json!(policy),
        ),
        None => (
            "rejected".to_string(),
            "no active policy matched packet writer and route".to_string(),
            serde_json::json!({
                "object_kind": object_kind,
                "writer_role": writer_role,
                "route_from": route_from,
                "route_to": route_to
            }),
        ),
    };

    let admission_id = store.insert_hive_loop_admission(HiveLoopAdmissionCreate {
        loop_id: contract.id,
        packet_id,
        result: result.clone(),
        reason: reason.clone(),
        policy: policy_json,
    })?;
    let admission = store
        .list_hive_loop_admissions(contract.id)?
        .into_iter()
        .find(|admission| admission.id == admission_id)
        .expect("newly created admission should exist");

    if result != "admitted" {
        anyhow::bail!("admission rejected for {object_kind}: {reason}");
    }

    Ok(admission)
}

fn gate_passes(gate: &str, payload: &serde_json::Value) -> bool {
    match gate {
        "candidate_present" => payload
            .get("candidate")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty()),
        "runtime_probe_present" => payload.get("runtime_probe").is_some(),
        "decision_present" => payload
            .get("decision")
            .and_then(|value| value.as_str())
            .is_some_and(|value| matches!(value, "keep" | "reject" | "needs-review" | "blocked")),
        _ => false,
    }
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
        assert_eq!(report.admissions.len(), 3);
        assert!(report
            .admissions
            .iter()
            .all(|admission| admission.result == "admitted"));
        assert_eq!(report.stages.len(), 3);
        assert_eq!(report.evidence.len(), 3);
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "keep");
        assert_eq!(report.issues[0].issue.status, "Done");
        assert!(report.issues[0].comments.len() >= 3);

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
        assert_eq!(report.contract.active_phase, "complete");
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "blocked");
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

        let _ = fs::remove_dir_all(root);
    }
}
