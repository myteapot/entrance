use anyhow::{Context, Result};
use entrance_core::{HiveCommentCreate, HiveLoopEvidenceCreate, Store};
use serde::{Deserialize, Serialize};

use crate::{HiveLoopReport, IssueCard, IssueRunRequest};

const AUTO_ADVANCE_SCHEMA_VERSION: &str = "entrance.hive.auto_advance.v1";
const DEFAULT_ADVANCE_MAX_STEPS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAdvanceRequest {
    pub issue_id: i64,
    pub mode: Option<String>,
    pub runtime: Option<String>,
    pub max_steps: Option<usize>,
    pub worker_timeout_secs: Option<u64>,
    pub worker_attempts: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAdvanceStep {
    pub schema_version: String,
    pub index: usize,
    pub issue_id: i64,
    pub loop_id: i64,
    pub round: i64,
    pub status_before: String,
    pub status_after: String,
    pub contract_status_after: String,
    pub decision: Option<String>,
    pub reason_code: Option<String>,
    pub summary: String,
    pub prepared_next_round: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAdvanceReport {
    pub schema_version: String,
    pub issue: IssueCard,
    pub mode: String,
    pub stop_reason: String,
    pub steps: Vec<IssueAdvanceStep>,
    pub next_actions: Vec<String>,
}

pub fn advance_issue(store: &Store, request: IssueAdvanceRequest) -> Result<IssueAdvanceReport> {
    let mode = request
        .mode
        .clone()
        .unwrap_or_else(|| "one_step".to_string());
    if !matches!(mode.as_str(), "one_step" | "until_wait") {
        anyhow::bail!("issue advance mode must be one_step or until_wait");
    }
    let max_steps = request
        .max_steps
        .unwrap_or_else(|| {
            if mode == "one_step" {
                1
            } else {
                DEFAULT_ADVANCE_MAX_STEPS
            }
        })
        .max(1);
    let mut steps = Vec::new();
    let mut stop_reason = None;

    for index in 1..=max_steps {
        let issue = store
            .get_hive_issue(request.issue_id)?
            .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
        if let Some(reason) = preflight_stop_reason(&issue.status) {
            stop_reason = Some(reason.to_string());
            break;
        }
        let loop_id = issue
            .loop_id
            .with_context(|| format!("hive issue #{} is not linked to a loop", issue.id))?;
        let contract = store
            .get_hive_loop_contract(loop_id)?
            .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
        let status_before = issue.status.clone();
        let report = run_advance_step(store, &issue, &contract.status, &request)?;
        let mut step = summarize_step(index, &status_before, &report)?;
        let invalid_retry = step.decision.as_deref() == Some("reject")
            && step.reason_code.as_deref() != Some("review_budget_exhausted");
        if invalid_retry && mode == "until_wait" {
            let next_round = step.round + 1;
            store.update_hive_loop_contract_state(loop_id, "todo", "explorer", next_round)?;
            store.update_hive_issue_status(
                request.issue_id,
                "Todo",
                Some("Auto-advance prepared the next Developer -> Reviewer round."),
            )?;
            step.status_after = "Todo".to_string();
            step.contract_status_after = "todo".to_string();
            step.prepared_next_round = true;
        }
        record_advance_step(store, &step)?;
        stop_reason = Some(step_stop_reason(&step, &mode));
        steps.push(step);
        if stop_reason.as_deref() != Some("continue") {
            break;
        }
    }

    if stop_reason.as_deref() == Some("continue") || stop_reason.is_none() {
        stop_reason = Some("max_steps".to_string());
    }
    let issue = crate::loop_control::issue(store, request.issue_id)?;
    let next_actions =
        issue_advance_next_actions(&issue, stop_reason.as_deref().unwrap_or("unknown"));
    Ok(IssueAdvanceReport {
        schema_version: AUTO_ADVANCE_SCHEMA_VERSION.to_string(),
        issue,
        mode,
        stop_reason: stop_reason.unwrap_or_else(|| "unknown".to_string()),
        steps,
        next_actions,
    })
}

pub fn issue_advance_next_action(card: &IssueCard) -> Option<String> {
    match card.issue.status.as_str() {
        "Todo" | "Doing" => Some(format!(
            "entrance hive issue advance {} --until-wait --compact",
            card.issue.id
        )),
        "Blocked" | "Needs Review" => Some(format!(
            "entrance hive issue decide {} retry --human-confirmed --compact",
            card.issue.id
        )),
        _ => None,
    }
}

fn run_advance_step(
    store: &Store,
    issue: &entrance_core::HiveIssue,
    contract_status: &str,
    request: &IssueAdvanceRequest,
) -> Result<HiveLoopReport> {
    if issue.status == "Doing" && contract_status == "todo" {
        return crate::runner::run(
            store,
            crate::HiveLoopRunRequest {
                loop_id: issue.loop_id.expect("advance issue should be loop-linked"),
                runtime: request.runtime.clone(),
                decision: None,
                worker_timeout_secs: request.worker_timeout_secs,
                worker_attempts: request.worker_attempts,
            },
        );
    }
    crate::kernel::run_issue(
        store,
        IssueRunRequest {
            issue_id: issue.id,
            runtime: request.runtime.clone(),
            decision: None,
            worker_timeout_secs: request.worker_timeout_secs,
            worker_attempts: request.worker_attempts,
            retry: false,
            author: "auto-advance".to_string(),
            body: None,
            confirmation_receipt: None,
        },
    )
}

fn summarize_step(
    index: usize,
    status_before: &str,
    report: &HiveLoopReport,
) -> Result<IssueAdvanceStep> {
    let issue = report
        .issues
        .first()
        .with_context(|| format!("loop #{} did not return an issue card", report.contract.id))?;
    let verdict = report.verdicts.last();
    let reason_code = verdict.and_then(|verdict| {
        verdict
            .score
            .get("reason_code")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    });
    Ok(IssueAdvanceStep {
        schema_version: AUTO_ADVANCE_SCHEMA_VERSION.to_string(),
        index,
        issue_id: issue.issue.id,
        loop_id: report.contract.id,
        round: report.contract.current_round,
        status_before: status_before.to_string(),
        status_after: issue.issue.status.clone(),
        contract_status_after: report.contract.status.clone(),
        decision: verdict.map(|verdict| verdict.decision.clone()),
        reason_code,
        summary: verdict
            .map(|verdict| verdict.summary.clone())
            .unwrap_or_else(|| report.contract.status.clone()),
        prepared_next_round: false,
    })
}

fn record_advance_step(store: &Store, step: &IssueAdvanceStep) -> Result<()> {
    let payload = serde_json::to_value(step)?;
    let comment_id = store.insert_hive_comment(HiveCommentCreate {
        issue_id: step.issue_id,
        author: "auto-advance".to_string(),
        body: step.summary.clone(),
        payload: serde_json::json!({
            "schema_version": AUTO_ADVANCE_SCHEMA_VERSION,
            "source": "kernel",
            "advance_step": payload.clone()
        }),
    })?;
    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: step.loop_id,
        stage_id: None,
        round: step.round,
        kind: "auto_advance".to_string(),
        summary: step.summary.clone(),
        path: None,
        payload: serde_json::json!({
            "schema_version": AUTO_ADVANCE_SCHEMA_VERSION,
            "source": "kernel",
            "comment_id": comment_id,
            "advance_step": payload
        }),
    })?;
    Ok(())
}

fn preflight_stop_reason(status: &str) -> Option<&'static str> {
    match status {
        "Done" => Some("done"),
        "Blocked" => Some("blocked"),
        "Canceled" => Some("canceled"),
        "Needs Review" => Some("needs_review"),
        _ => None,
    }
}

fn step_stop_reason(step: &IssueAdvanceStep, mode: &str) -> String {
    if mode == "one_step" {
        return "one_step_complete".to_string();
    }
    match step.status_after.as_str() {
        "Done" => "done".to_string(),
        "Blocked" => "blocked".to_string(),
        "Needs Review" => "needs_review".to_string(),
        "Canceled" => "canceled".to_string(),
        "Todo" if step.prepared_next_round => "continue".to_string(),
        _ => "wait".to_string(),
    }
}

fn issue_advance_next_actions(card: &IssueCard, stop_reason: &str) -> Vec<String> {
    let mut actions = Vec::new();
    if let Some(action) = issue_advance_next_action(card) {
        actions.push(action);
    }
    actions.push(format!(
        "entrance hive issue show {} --compact",
        card.issue.id
    ));
    if let Some(loop_id) = card.issue.loop_id {
        actions.push(format!("entrance hive loop control {loop_id}"));
    }
    if stop_reason == "blocked" || card.issue.status == "Blocked" {
        actions.push(format!(
            "entrance hive issue decide {} retry --human-confirmed --compact",
            card.issue.id
        ));
    }
    actions
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use entrance_core::Store;

    use super::*;
    use crate::HiveLoopCreateRequest;

    fn temp_store(name: &str) -> (std::path::PathBuf, Store) {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-advance-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");
        (root, store)
    }

    fn create_issue_loop(store: &Store, title: &str, goal: &str) -> IssueCard {
        crate::kernel::create(
            store,
            HiveLoopCreateRequest {
                title: title.to_string(),
                goal: goal.to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: "local-hive-panel".to_string(),
                autonomy_level: "developer-reviewer".to_string(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created")
        .issues
        .into_iter()
        .next()
        .expect("loop should create an issue")
    }

    #[test]
    fn advance_one_step_records_step_and_stops() {
        let (root, store) = temp_store("one-step");
        let issue = create_issue_loop(
            &store,
            "Advance one step",
            "Complete a deterministic local Developer and Reviewer round",
        );

        let report = advance_issue(
            &store,
            IssueAdvanceRequest {
                issue_id: issue.issue.id,
                mode: Some("one_step".to_string()),
                runtime: Some("local".to_string()),
                max_steps: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("issue should advance one step");

        assert_eq!(report.schema_version, AUTO_ADVANCE_SCHEMA_VERSION);
        assert_eq!(report.stop_reason, "one_step_complete");
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].schema_version, AUTO_ADVANCE_SCHEMA_VERSION);
        assert_eq!(report.steps[0].status_before, "Todo");
        assert_eq!(report.steps[0].decision.as_deref(), Some("keep"));
        assert_eq!(report.issue.issue.status, "Done");
        assert!(store
            .list_hive_comments(issue.issue.id)
            .expect("comments should load")
            .iter()
            .any(|comment| comment
                .payload
                .pointer("/schema_version")
                .and_then(|value| value.as_str())
                == Some(AUTO_ADVANCE_SCHEMA_VERSION)));
        assert!(store
            .list_hive_loop_evidence(issue.issue.loop_id.expect("issue should be loop-linked"))
            .expect("evidence should load")
            .iter()
            .any(|evidence| evidence.kind == "auto_advance"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn advance_until_wait_retries_invalid_rounds_then_blocks_on_third() {
        let (root, store) = temp_store("invalid-budget");
        let issue = create_issue_loop(&store, "Invalid budget", "go");

        let report = advance_issue(
            &store,
            IssueAdvanceRequest {
                issue_id: issue.issue.id,
                mode: Some("until_wait".to_string()),
                runtime: Some("local".to_string()),
                max_steps: Some(4),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("issue should advance until blocked");

        assert_eq!(report.stop_reason, "blocked");
        assert_eq!(report.steps.len(), 3);
        assert!(report.steps[0].prepared_next_round);
        assert!(report.steps[1].prepared_next_round);
        assert!(!report.steps[2].prepared_next_round);
        assert_eq!(
            report.steps[2].reason_code.as_deref(),
            Some("review_budget_exhausted")
        );
        assert_eq!(report.issue.issue.status, "Blocked");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn advance_stops_on_done_blocked_canceled_needs_review() {
        for status in ["Done", "Blocked", "Canceled", "Needs Review"] {
            let (root, store) = temp_store(&format!("stop-{status}"));
            let issue =
                create_issue_loop(&store, &format!("Stop {status}"), "Stop without running");
            store
                .update_hive_issue_status(issue.issue.id, status, Some("preset terminal status"))
                .expect("issue status should update");

            let report = advance_issue(
                &store,
                IssueAdvanceRequest {
                    issue_id: issue.issue.id,
                    mode: Some("until_wait".to_string()),
                    runtime: Some("local".to_string()),
                    max_steps: Some(3),
                    worker_timeout_secs: None,
                    worker_attempts: None,
                },
            )
            .expect("advance should stop before running terminal statuses");

            assert!(report.steps.is_empty());
            assert_eq!(report.issue.issue.status, status);
            assert_eq!(
                report.stop_reason,
                match status {
                    "Done" => "done",
                    "Blocked" => "blocked",
                    "Canceled" => "canceled",
                    "Needs Review" => "needs_review",
                    _ => unreachable!(),
                }
            );

            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn semantic_reviewer_failure_counts_against_budget() {
        let (root, store) = temp_store("semantic-budget");
        let issue = create_issue_loop(&store, "Semantic budget", "go");

        let report = advance_issue(
            &store,
            IssueAdvanceRequest {
                issue_id: issue.issue.id,
                mode: Some("until_wait".to_string()),
                runtime: Some("local".to_string()),
                max_steps: Some(3),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("semantic failure should count toward reviewer budget");

        let loop_id = issue.issue.loop_id.expect("issue should be loop-linked");
        let verdicts = store
            .list_hive_loop_verdicts(loop_id)
            .expect("verdicts should load");
        let last_verdict = verdicts.last().expect("verdict should exist");
        assert_eq!(report.stop_reason, "blocked");
        assert_eq!(last_verdict.decision, "blocked");
        assert_eq!(
            last_verdict
                .score
                .pointer("/score_vector/goal_alignment")
                .and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(
            last_verdict
                .score
                .pointer("/gate_results/semantic_gates_passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert!(last_verdict
            .score
            .pointer("/gate_results/failure_reasons")
            .and_then(|value| value.as_array())
            .is_some_and(|reasons| reasons.iter().any(|reason| reason
                .as_str()
                .is_some_and(|reason| reason.starts_with("goal_alignment=0.00<")))));

        let _ = fs::remove_dir_all(root);
    }
}
