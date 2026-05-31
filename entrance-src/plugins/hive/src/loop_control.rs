use std::process::Command;

use anyhow::{Context, Result};
use entrance_core::{
    HiveComment, HiveCommentCreate, HiveIssue, HiveIssueCreate, HiveLoopContract,
    HiveLoopContractCreate, HiveLoopEvidence, HiveLoopEvidenceCreate, HiveLoopStage,
    HiveLoopStageCreate, HiveLoopVerdict, HiveLoopVerdictCreate, Store,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLoopReport {
    pub contract: HiveLoopContract,
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

    let issue_id = store.insert_hive_issue(HiveIssueCreate {
        loop_id: Some(loop_id),
        title: format!("Loop #{loop_id}: {}", request.title),
        status: "Todo".to_string(),
        summary: Some("Loop contract created; waiting for Explorer.".to_string()),
    })?;

    store.insert_hive_comment(HiveCommentCreate {
        issue_id,
        author: "compiler".to_string(),
        body: "Loop contract admitted into Hive.".to_string(),
        payload: serde_json::json!({
            "loop_id": loop_id,
            "goal": request.goal,
            "next_phase": "explorer"
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
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(explorer_stage),
        round: contract.current_round,
        kind: "exploration".to_string(),
        summary: "Explorer produced a concrete local-loop candidate.".to_string(),
        path: None,
        payload: serde_json::json!({
            "candidate": "local-loop-mvp",
            "approach_count": contract.approach_space.len()
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
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(doer_stage),
        round: contract.current_round,
        kind: "runtime".to_string(),
        summary: format!("Doer probed `{runtime}` runtime."),
        path: None,
        payload: serde_json::json!({
            "runtime": runtime,
            "probe": runtime_probe
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
    let decision = if runtime == "codex" && !runtime_ready {
        "blocked"
    } else {
        "keep"
    };
    let verdict_summary = if decision == "keep" {
        "Evaluator kept the candidate: all MVP gates passed.".to_string()
    } else {
        format!("Evaluator blocked the candidate: `{runtime}` runtime probe failed.")
    };
    let evaluator_stage = insert_stage(
        store,
        &contract,
        "evaluator",
        &verdict_summary,
        serde_json::json!({
            "evidence_count": evidence.len(),
            "eval_space": contract.eval_space
        }),
        serde_json::json!({
            "decision": decision,
            "gates": {
                "three_stages_recorded": true,
                "evidence_recorded": !evidence.is_empty(),
                "runtime_ready": runtime_ready
            }
        }),
    )?;
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(evaluator_stage),
        round: contract.current_round,
        kind: "evaluation".to_string(),
        summary: verdict_summary.clone(),
        path: None,
        payload: serde_json::json!({
            "decision": decision,
            "runtime_ready": runtime_ready
        }),
    })?;
    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: decision.to_string(),
        summary: verdict_summary.clone(),
        score: serde_json::json!({
            "gates_passed": decision == "keep",
            "stage_completeness": 1.0,
            "runtime_readiness": if runtime_ready { 1.0 } else { 0.0 },
            "operator_review_needed": decision != "keep"
        }),
        evidence: serde_json::json!({
            "evidence_count": evidence.len() + 1,
            "runtime": runtime
        }),
    })?;

    let final_status = if decision == "keep" {
        "kept"
    } else {
        "blocked"
    };
    let issue_status = if decision == "keep" {
        "Done"
    } else {
        "Blocked"
    };
    store.update_hive_loop_contract_state(
        contract.id,
        final_status,
        "complete",
        contract.current_round,
    )?;
    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(issue_id, issue_status, Some(&verdict_summary))?;
        add_system_comment(
            store,
            issue_id,
            &verdict_summary,
            serde_json::json!({
                "loop_id": contract.id,
                "decision": decision,
                "phase": "evaluator"
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

fn probe_runtime(runtime: &str) -> serde_json::Value {
    if runtime != "codex" {
        return serde_json::json!({
            "ok": true,
            "kind": "local",
            "detail": "local deterministic runtime"
        });
    }

    match Command::new("codex").arg("--version").output() {
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
            },
        )
        .expect("loop should run");

        assert_eq!(report.contract.status, "kept");
        assert_eq!(report.contract.active_phase, "complete");
        assert_eq!(report.stages.len(), 3);
        assert_eq!(report.evidence.len(), 3);
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "keep");
        assert_eq!(report.issues[0].issue.status, "Done");
        assert!(report.issues[0].comments.len() >= 3);

        let _ = fs::remove_dir_all(root);
    }
}
