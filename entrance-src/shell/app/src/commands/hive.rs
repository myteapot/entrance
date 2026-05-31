use anyhow::{bail, Result};
use entrance_hive::{
    HiveCallbackRequest, HiveDispatchRequest, HiveLoopCreateRequest, HiveLoopRunRequest, IssueCard,
    IssueCommentRequest, IssueDecisionRequest, IssueRunRequest, ReviewDecision,
};

use crate::{app::AppServices, cli, print_json};

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>\n  entrance hive loop create --title <text> --goal <text> [--runtime local|codex]\n  entrance hive loop run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop show <id>\n  entrance hive loop trace <id>\n  entrance hive loop evidence <id>\n  entrance hive loop audit <id>\n  entrance hive loop doctor <id>\n  entrance hive loop policies <id>\n  entrance hive loop list\n  entrance hive policy registry\n  entrance hive issue list [--compact]\n  entrance hive issue show <id> [--compact]\n  entrance hive issue comment <id> --body <text> [--compact]\n  entrance hive issue decide <id> <retry|request-review|cancel> [--body <text>] [--compact]\n  entrance hive issue run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive issue retry-run <id> [--body <text>] [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]"
            );
            Ok(())
        }
        [flag] if cli::is_help(flag) => run(services, &[]),
        [command] if command == "list" => print_json(&services.hive.list()?),
        [command] if command == "summary" => print_json(&services.hive.summary()?),
        [command, flag, title] if command == "dispatch" && flag == "--title" => {
            let report = services.hive.dispatch(HiveDispatchRequest {
                title: title.clone(),
                project_dir: None,
                summary: None,
                payload_json: "{}".to_string(),
            })?;
            print_json(&report)
        }
        [command, flag, title, flag2, project]
            if command == "dispatch" && flag == "--title" && flag2 == "--project" =>
        {
            let report = services.hive.dispatch(HiveDispatchRequest {
                title: title.clone(),
                project_dir: Some(project.clone()),
                summary: None,
                payload_json: "{}".to_string(),
            })?;
            print_json(&report)
        }
        [command, flag, title, flag2, project, flag3, summary]
            if command == "dispatch"
                && flag == "--title"
                && flag2 == "--project"
                && flag3 == "--summary" =>
        {
            let report = services.hive.dispatch(HiveDispatchRequest {
                title: title.clone(),
                project_dir: Some(project.clone()),
                summary: Some(summary.clone()),
                payload_json: "{}".to_string(),
            })?;
            print_json(&report)
        }
        [command, id] if command == "engine" => {
            let id = id.parse::<i64>()?;
            print_json(&services.hive.engine_report(id)?)
        }
        [command, id, status] if command == "callback" => {
            let id = id.parse::<i64>()?;
            print_json(&services.hive.callback(HiveCallbackRequest {
                run_id: id,
                status: status.clone(),
                summary: None,
            })?)
        }
        [command, id, status, summary] if command == "callback" => {
            let id = id.parse::<i64>()?;
            print_json(&services.hive.callback(HiveCallbackRequest {
                run_id: id,
                status: status.clone(),
                summary: Some(summary.clone()),
            })?)
        }
        [command, id, decision] if command == "review" => {
            let id = id.parse::<i64>()?;
            let decision = match decision.as_str() {
                "approve" => ReviewDecision::Approve,
                "return" => ReviewDecision::Return,
                "integrate" => ReviewDecision::Integrate,
                _ => bail!("unsupported review decision"),
            };
            print_json(&services.hive.review(id, decision)?)
        }
        [scope, action] if scope == "loop" && action == "list" => {
            print_json(&services.hive.loop_list()?)
        }
        [scope, action, id] if scope == "loop" && action == "show" => {
            print_json(&services.hive.loop_report(id.parse::<i64>()?)?)
        }
        [scope, action, id] if scope == "loop" && action == "trace" => {
            print_json(&services.hive.loop_trace(id.parse::<i64>()?)?)
        }
        [scope, action, id] if scope == "loop" && action == "evidence" => {
            print_json(&services.hive.loop_evidence(id.parse::<i64>()?)?)
        }
        [scope, action, id] if scope == "loop" && action == "audit" => {
            print_json(&services.hive.loop_audit(id.parse::<i64>()?)?)
        }
        [scope, action, id] if scope == "loop" && action == "doctor" => {
            print_json(&services.hive.loop_doctor(id.parse::<i64>()?)?)
        }
        [scope, action, id] if scope == "loop" && action == "policies" => {
            print_json(&services.hive.loop_policies(id.parse::<i64>()?)?)
        }
        [scope, action] if scope == "policy" && action == "registry" => {
            print_json(&services.hive.policy_registry())
        }
        [scope, action, id, rest @ ..] if scope == "loop" && action == "run" => {
            let loop_id = id.parse::<i64>()?;
            let report = services.hive.loop_run(HiveLoopRunRequest {
                loop_id,
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                decision: flag_value(rest, "--decision").map(ToOwned::to_owned),
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
            })?;
            if flag_present(rest, "--compact") {
                print_json(&services.hive.loop_doctor(loop_id)?)
            } else {
                print_json(&report)
            }
        }
        [scope, action, rest @ ..] if scope == "loop" && action == "create" => {
            let title = flag_value(rest, "--title").unwrap_or("Untitled loop");
            let goal = flag_value(rest, "--goal").unwrap_or(title);
            let request = HiveLoopCreateRequest {
                title: title.to_string(),
                goal: goal.to_string(),
                boundary: flag_value(rest, "--boundary")
                    .unwrap_or_default()
                    .to_string(),
                approach_space: csv_values(flag_value(rest, "--approach")),
                eval_space: csv_values(flag_value(rest, "--eval")),
                review_surface: flag_value(rest, "--review-surface")
                    .unwrap_or("local-hive-panel")
                    .to_string(),
                autonomy_level: flag_value(rest, "--autonomy")
                    .unwrap_or("run-approved-candidates")
                    .to_string(),
                runtime: flag_value(rest, "--runtime").unwrap_or("local").to_string(),
            };
            print_json(&services.hive.loop_create(request)?)
        }
        [scope, action, rest @ ..] if scope == "issue" && action == "list" => {
            let cards = services.hive.panel()?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_board(&cards))
            } else {
                print_json(&cards)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "show" => {
            let card = services.hive.issue_report(id.parse::<i64>()?)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "comment" => {
            let body = flag_value(rest, "--body").unwrap_or_default();
            let card = services.hive.issue_comment(IssueCommentRequest {
                issue_id: id.parse::<i64>()?,
                author: flag_value(rest, "--author").unwrap_or("human").to_string(),
                body: body.to_string(),
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id, decision, rest @ ..] if scope == "issue" && action == "decide" => {
            let card = services.hive.issue_decide(IssueDecisionRequest {
                issue_id: id.parse::<i64>()?,
                action: decision.to_string(),
                author: flag_value(rest, "--author").unwrap_or("human").to_string(),
                body: flag_value(rest, "--body").map(ToOwned::to_owned),
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id, rest @ ..]
            if scope == "issue" && (action == "run" || action == "retry-run") =>
        {
            let issue_id = id.parse::<i64>()?;
            let report = services.hive.issue_run(IssueRunRequest {
                issue_id,
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                decision: flag_value(rest, "--decision").map(ToOwned::to_owned),
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
                retry: action == "retry-run",
                author: flag_value(rest, "--author").unwrap_or("human").to_string(),
                body: flag_value(rest, "--body").map(ToOwned::to_owned),
            })?;
            if flag_present(rest, "--compact") {
                let card = services.hive.issue_report(issue_id)?;
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&report)
            }
        }
        _ => bail!("unsupported hive command"),
    }
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|values| values[0] == flag)
        .map(|values| values[1].as_str())
}

fn flag_present(args: &[String], flag: &str) -> bool {
    args.iter().any(|value| value == flag)
}

fn csv_values(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn compact_issue_board(cards: &[IssueCard]) -> serde_json::Value {
    const STATUSES: &[&str] = &[
        "Todo",
        "Doing",
        "Blocked",
        "Needs Review",
        "Done",
        "Canceled",
    ];
    let columns = STATUSES
        .iter()
        .map(|status| {
            let issues = cards
                .iter()
                .filter(|card| card.issue.status == *status)
                .map(compact_issue_card)
                .collect::<Vec<_>>();
            serde_json::json!({
                "status": status,
                "count": issues.len(),
                "issues": issues
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": "entrance.hive.issue_board.compact.v1",
        "total": cards.len(),
        "columns": columns
    })
}

fn compact_issue_detail(card: &IssueCard) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue.compact.v1",
        "issue": compact_issue_card(card),
        "recent_comments": compact_recent_comments(card, 5),
        "recent_evidence": compact_recent_evidence(card, 5),
        "stages": compact_stage_rows(card)
    })
}

fn compact_issue_card(card: &IssueCard) -> serde_json::Value {
    let latest_comment = card.comments.last().map(|comment| {
        serde_json::json!({
            "author": comment.author,
            "body": compact_text(&comment.body, 180),
            "created_at": comment.created_at
        })
    });
    serde_json::json!({
        "id": card.issue.id,
        "loop_id": card.issue.loop_id,
        "title": card.issue.title,
        "status": card.issue.status,
        "summary": card.issue.summary,
        "doctor": card.doctor.as_ref().map(|doctor| serde_json::json!({
            "health": doctor.health,
            "runtime": doctor.runtime.as_str(),
            "summary": doctor.summary,
            "next_actions": doctor.next_actions.iter().take(3).collect::<Vec<_>>()
        })),
        "trace": card.trace.as_ref().map(|trace| serde_json::json!({
            "round": trace.current_round,
            "decision": trace.last_decision,
            "reason_code": trace.reason_code,
            "human_options": trace.human_options,
            "receipts": {
                "required": trace.round_receipt_required_count,
                "missing": trace.round_receipt_missing_count
            },
            "workers": {
                "ok": trace.round_role_worker_ok_count,
                "total": trace.round_role_worker_count
            },
            "audit_failed": trace.audit_failed_count
        })),
        "comment_count": card.comments.len(),
        "latest_comment": latest_comment,
        "actions": compact_issue_actions(card)
    })
}

fn compact_recent_comments(card: &IssueCard, limit: usize) -> Vec<serde_json::Value> {
    let mut comments = card.comments.iter().rev().take(limit).collect::<Vec<_>>();
    comments.reverse();
    comments
        .into_iter()
        .map(|comment| {
            serde_json::json!({
                "id": comment.id,
                "author": comment.author,
                "body": compact_text(&comment.body, 220),
                "created_at": comment.created_at,
                "action": comment.payload.get("action").and_then(|value| value.as_str()),
                "source": comment.payload.get("source").and_then(|value| value.as_str())
            })
        })
        .collect()
}

fn compact_recent_evidence(card: &IssueCard, limit: usize) -> Vec<serde_json::Value> {
    let Some(trace) = &card.trace else {
        return Vec::new();
    };
    let mut evidence = trace.evidence.iter().rev().take(limit).collect::<Vec<_>>();
    evidence.reverse();
    evidence
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "round": row.round,
                "role": row.stage_role,
                "kind": row.kind,
                "summary": compact_text(&row.summary, 220),
                "admission": row.admission_result,
                "worker": row.worker_kind.as_ref().map(|kind| serde_json::json!({
                    "kind": kind,
                    "command": row.worker_command.as_ref().map(|command| compact_text(command, 220)),
                    "cwd": row.worker_cwd.as_ref().map(|cwd| compact_text(cwd, 180)),
                    "ok": row.worker_ok,
                    "receipt_ok": row.worker_receipt_ok,
                    "duration_ms": row.worker_duration_ms,
                    "action": row.worker_action.as_ref().map(|action| compact_text(action, 180))
                }))
            })
        })
        .collect()
}

fn compact_stage_rows(card: &IssueCard) -> Vec<serde_json::Value> {
    let Some(trace) = &card.trace else {
        return Vec::new();
    };
    trace
        .stages
        .iter()
        .map(|stage| {
            serde_json::json!({
                "role": stage.role,
                "status": stage.status,
                "summary": stage.summary.as_ref().map(|summary| compact_text(summary, 180)),
                "admission": stage.admission_result,
                "worker": stage.worker_kind.as_ref().map(|kind| serde_json::json!({
                    "kind": kind,
                    "ok": stage.worker_ok
                }))
            })
        })
        .collect()
}

fn compact_issue_actions(card: &IssueCard) -> Vec<serde_json::Value> {
    let mut actions = Vec::new();
    if card.issue.loop_id.is_some() && card.issue.status == "Todo" {
        actions.push(compact_issue_action(
            "run",
            "Run",
            compact_run_command(card),
        ));
    }
    actions.extend(
        compact_issue_options(card)
            .into_iter()
            .filter_map(|option| compact_issue_option_action(card, &option)),
    );
    actions
}

fn compact_issue_options(card: &IssueCard) -> Vec<String> {
    if let Some(trace) = &card.trace {
        return trace.human_options.clone();
    }
    match card.issue.status.as_str() {
        "Todo" => vec!["comment", "cancel"],
        "Blocked" => vec!["comment", "retry", "request-review", "cancel"],
        "Needs Review" => vec!["comment", "retry", "cancel"],
        "Doing" | "Done" | "Canceled" => vec!["comment"],
        _ => vec!["comment"],
    }
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}

fn compact_issue_option_action(card: &IssueCard, option: &str) -> Option<serde_json::Value> {
    let issue_id = card.issue.id;
    match option {
        "comment" => Some(compact_issue_action(
            "comment",
            "Comment",
            format!("entrance hive issue comment {issue_id} --body <text> --compact"),
        )),
        "retry" => Some(compact_issue_action(
            "retry",
            "Retry",
            compact_retry_command(card).unwrap_or_else(|| {
                format!("entrance hive issue retry-run {issue_id} --body <note> --compact")
            }),
        )),
        "request-review" => Some(compact_issue_action(
            "request-review",
            "Review",
            format!("entrance hive issue decide {issue_id} request-review --body <note> --compact"),
        )),
        "cancel" => Some(compact_issue_action(
            "cancel",
            "Cancel",
            format!("entrance hive issue decide {issue_id} cancel --body <note> --compact"),
        )),
        _ => None,
    }
}

fn compact_retry_command(card: &IssueCard) -> Option<String> {
    card.doctor
        .as_ref()?
        .next_actions
        .iter()
        .find(|action| action.contains("entrance hive issue retry-run"))
        .cloned()
}

fn compact_run_command(card: &IssueCard) -> String {
    match card.doctor.as_ref().map(|doctor| doctor.runtime.as_str()) {
        Some(runtime) if !runtime.is_empty() => {
            format!(
                "entrance hive issue run {} --runtime {} --compact",
                card.issue.id, runtime
            )
        }
        _ => format!("entrance hive issue run {} --compact", card.issue.id),
    }
}

fn compact_issue_action(action: &str, label: &str, command: String) -> serde_json::Value {
    serde_json::json!({
        "action": action,
        "label": label,
        "command": command
    })
}

fn compact_text(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= limit {
        return normalized;
    }
    let mut output = normalized
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::{compact_issue_board, compact_issue_detail, flag_present, flag_value};
    use entrance_core::{HiveComment, HiveIssue};
    use entrance_hive::IssueCard;

    #[test]
    fn flags_read_values_and_presence_independently() {
        let args = vec![
            "--runtime".to_string(),
            "codex".to_string(),
            "--compact".to_string(),
        ];
        assert_eq!(flag_value(&args, "--runtime"), Some("codex"));
        assert_eq!(flag_value(&args, "--compact"), None);
        assert!(flag_present(&args, "--compact"));
        assert!(!flag_present(&args, "--missing"));
    }

    #[test]
    fn compact_issue_board_groups_status_and_latest_comment() {
        let card = IssueCard {
            issue: HiveIssue {
                id: 7,
                loop_id: Some(3),
                title: "Loop #3: compact board".to_string(),
                status: "Blocked".to_string(),
                summary: Some("Waiting for operator decision".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            comments: vec![HiveComment {
                id: 11,
                issue_id: 7,
                author: "hive".to_string(),
                body: "Compiler admission blocked at doer.".to_string(),
                payload: serde_json::json!({}),
                created_at: "2026-01-01T00:01:00Z".to_string(),
            }],
            trace: None,
            doctor: None,
        };

        let board = compact_issue_board(&[card]);

        assert_eq!(
            board
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_board.compact.v1")
        );
        assert_eq!(
            board.pointer("/total").and_then(|value| value.as_u64()),
            Some(1)
        );
        let blocked = board
            .pointer("/columns/2")
            .expect("blocked column should be present");
        assert_eq!(
            blocked.pointer("/status").and_then(|value| value.as_str()),
            Some("Blocked")
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/id")
                .and_then(|value| value.as_i64()),
            Some(7)
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/latest_comment/body")
                .and_then(|value| value.as_str()),
            Some("Compiler admission blocked at doer.")
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/actions/0/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue comment 7 --body <text> --compact")
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/actions/1/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue retry-run 7 --body <note> --compact")
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/actions/2/action")
                .and_then(|value| value.as_str()),
            Some("request-review")
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/actions/2/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue decide 7 request-review --body <note> --compact")
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/actions/3/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue decide 7 cancel --body <note> --compact")
        );
        let issue = blocked
            .pointer("/issues/0")
            .expect("blocked issue should be present");
        let detail = compact_issue_detail(&IssueCard {
            issue: HiveIssue {
                id: issue
                    .pointer("/id")
                    .and_then(|value| value.as_i64())
                    .expect("issue id should be numeric"),
                loop_id: Some(3),
                title: "Loop #3: compact board".to_string(),
                status: "Blocked".to_string(),
                summary: Some("Waiting for operator decision".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            comments: vec![HiveComment {
                id: 12,
                issue_id: 7,
                author: "human".to_string(),
                body: "Please retry with a smaller gate.".to_string(),
                payload: serde_json::json!({
                    "source": "operator",
                    "action": "comment"
                }),
                created_at: "2026-01-01T00:02:00Z".to_string(),
            }],
            trace: None,
            doctor: None,
        });
        assert_eq!(
            detail
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue.compact.v1")
        );
        assert_eq!(
            detail
                .pointer("/recent_comments/0/body")
                .and_then(|value| value.as_str()),
            Some("Please retry with a smaller gate.")
        );
        assert_eq!(
            detail
                .pointer("/issue/actions/1/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue retry-run 7 --body <note> --compact")
        );
    }
}
