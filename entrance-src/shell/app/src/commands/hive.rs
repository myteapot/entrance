use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use entrance_hive::{
    HiveCallbackRequest, HiveDispatchRequest, HiveLoopAuditCheck, HiveLoopAuditReport,
    HiveLoopCreateRequest, HiveLoopRunRequest, IssueAction, IssueCard, IssueCommentRequest,
    IssueDecisionRequest, IssueMirrorReport, IssueRunRequest, ReviewDecision,
};

use crate::{app::AppServices, cli, print_json};

const ISSUE_MIRROR_SYNC_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_sync.v1";

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>\n  entrance hive loop create --title <text> --goal <text> [--runtime local|codex] [--compact]\n  entrance hive loop run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop show <id>\n  entrance hive loop trace <id>\n  entrance hive loop evidence <id>\n  entrance hive loop audit <id> [--compact]\n  entrance hive loop doctor <id>\n  entrance hive loop policies <id>\n  entrance hive loop list\n  entrance hive policy registry\n  entrance hive issue list [--compact]\n  entrance hive issue show <id> [--compact]\n  entrance hive issue mirror <id> [--compact]\n  entrance hive issue mirror-sync <id> [--out <path>]\n  entrance hive issue comment <id> --body <text> [--compact]\n  entrance hive issue decide <id> <retry|request-review|cancel> [--body <text>] [--compact]\n  entrance hive issue run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive issue retry-run <id> [--body <text>] [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]"
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
        [scope, action, id, rest @ ..] if scope == "loop" && action == "audit" => {
            let report = services.hive.loop_audit(id.parse::<i64>()?)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_loop_audit(&report))
            } else {
                print_json(&report)
            }
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
            let report = services.hive.loop_create(request)?;
            if flag_present(rest, "--compact") {
                if let Some(card) = report.issues.first() {
                    print_json(&compact_issue_detail(card))
                } else {
                    print_json(&report)
                }
            } else {
                print_json(&report)
            }
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
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror" => {
            let mirror = services.hive.issue_mirror(id.parse::<i64>()?)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_mirror(&mirror))
            } else {
                print_json(&mirror)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-sync" => {
            let report =
                sync_issue_mirror_to_file(services, id.parse::<i64>()?, flag_value(rest, "--out"))?;
            print_json(&report)
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

fn compact_issue_mirror(mirror: &IssueMirrorReport) -> serde_json::Value {
    let loop_contract = mirror.loop_contract.as_ref().map(|contract| {
        serde_json::json!({
            "id": contract.id,
            "status": contract.status,
            "phase": contract.active_phase,
            "round": contract.current_round,
            "runtime": contract.runtime,
            "review_surface": contract.review_surface
        })
    });
    serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror.compact.v1",
        "source_schema_version": mirror.schema_version,
        "provider": mirror.provider,
        "review_surface": mirror.review_surface,
        "external_key": mirror.external_key,
        "refresh_command": format!("entrance hive issue mirror {} --compact", mirror.issue.id),
        "issue": {
            "id": mirror.issue.id,
            "loop_id": mirror.issue.loop_id,
            "title": mirror.issue.title,
            "status": mirror.issue.status,
            "summary": mirror.issue.summary
        },
        "loop": loop_contract,
        "counts": {
            "actions": mirror.actions.len(),
            "comments": mirror.comments.len(),
            "evidence": mirror.trace.as_ref().map(|trace| trace.evidence_count).unwrap_or_default(),
            "operator_events": mirror.trace.as_ref().map(|trace| trace.operator_event_count).unwrap_or_default()
        },
        "trace": mirror.trace.as_ref().map(|trace| serde_json::json!({
            "round": trace.current_round,
            "decision": trace.last_decision,
            "reason_code": trace.reason_code,
            "human_options": trace.human_options,
            "audit": {
                "passed": trace.audit_passed,
                "failed": trace.audit_failed_count,
                "failed_checks": trace.audit_failed_checks,
                "failure_details": trace.audit_failure_details.iter().take(5).collect::<Vec<_>>()
            },
            "receipts": {
                "required": trace.round_receipt_required_count,
                "missing": trace.round_receipt_missing_count
            },
            "workers": {
                "ok": trace.round_role_worker_ok_count,
                "total": trace.round_role_worker_count,
                "duration_ms": trace.round_worker_duration_ms,
                "timeouts": trace.round_worker_timeout_count,
                "retry_exhausted": trace.round_worker_retry_exhausted_count
            }
        })),
        "doctor": mirror.doctor.as_ref().map(|doctor| serde_json::json!({
            "health": doctor.health,
            "summary": doctor.summary,
            "next_actions": doctor.next_actions.iter().take(5).collect::<Vec<_>>(),
            "worker_failures": doctor.worker_failures.iter().take(5).collect::<Vec<_>>()
        })),
        "comments": compact_mirror_comments(mirror, 8),
        "actions": mirror.actions.iter().map(compact_issue_action).collect::<Vec<_>>()
    })
}

pub(crate) fn sync_issue_mirror_to_file(
    services: &AppServices,
    issue_id: i64,
    out_path: Option<&str>,
) -> Result<serde_json::Value> {
    let mirror = services.hive.issue_mirror(issue_id)?;
    let path = out_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_issue_mirror_path(&services.kernel.root, &mirror.external_key));
    let bytes = write_issue_mirror_file(&mirror, &path)?;
    Ok(compact_issue_mirror_sync(&mirror, &path, bytes))
}

fn default_issue_mirror_path(app_root: &Path, external_key: &str) -> PathBuf {
    app_root
        .join("connectors")
        .join("issue-mirrors")
        .join(format!("{}.json", sanitize_mirror_key(external_key)))
}

fn sanitize_mirror_key(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    while output.contains("--") {
        output = output.replace("--", "-");
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "issue-mirror".to_string()
    } else {
        trimmed
    }
}

fn write_issue_mirror_file(mirror: &IssueMirrorReport, path: &Path) -> Result<u64> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mirror sink directory {}",
                parent.display()
            )
        })?;
    }
    let payload = serde_json::to_vec_pretty(mirror)?;
    fs::write(path, &payload)
        .with_context(|| format!("failed to write issue mirror {}", path.display()))?;
    Ok(payload.len() as u64)
}

fn compact_issue_mirror_sync(
    mirror: &IssueMirrorReport,
    path: &Path,
    bytes: u64,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_SYNC_SCHEMA_VERSION,
        "mirror_schema_version": mirror.schema_version.as_str(),
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": mirror.issue.id,
        "issue_status": mirror.issue.status.as_str(),
        "path": path.display().to_string(),
        "bytes": bytes,
        "refresh_command": format!("entrance hive issue mirror {} --compact", mirror.issue.id),
        "sync_command": format!("entrance hive issue mirror-sync {}", mirror.issue.id)
    })
}

fn compact_loop_audit(report: &HiveLoopAuditReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.audit.compact.v1",
        "loop_id": report.loop_id,
        "passed": report.passed,
        "failed_count": report.failed_count,
        "failed_checks": report
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| check.name.as_str())
            .collect::<Vec<_>>(),
        "checks": report
            .checks
            .iter()
            .map(compact_audit_check)
            .collect::<Vec<_>>(),
        "actions": [
            compact_loop_action("doctor", "Doctor", format!("entrance hive loop doctor {}", report.loop_id)),
            compact_loop_action("trace", "Trace", format!("entrance hive loop trace {}", report.loop_id)),
            compact_loop_action("evidence", "Evidence", format!("entrance hive loop evidence {}", report.loop_id)),
            compact_loop_action("full-audit", "Full audit", format!("entrance hive loop audit {}", report.loop_id))
        ]
    })
}

fn compact_audit_check(check: &HiveLoopAuditCheck) -> serde_json::Value {
    let errors = compact_audit_errors(&check.details);
    serde_json::json!({
        "name": check.name,
        "passed": check.passed,
        "summary": compact_text(&check.summary, 180),
        "error_count": compact_audit_error_count(&check.details),
        "errors": errors,
        "counts": compact_audit_counts(&check.details)
    })
}

fn compact_audit_error_count(details: &serde_json::Value) -> usize {
    let Some(object) = details.as_object() else {
        return 0;
    };
    object
        .iter()
        .filter(|(key, _)| key.contains("error"))
        .filter_map(|(_, value)| value.as_array().map(Vec::len))
        .sum()
}

fn compact_audit_errors(details: &serde_json::Value) -> Vec<String> {
    let Some(object) = details.as_object() else {
        return Vec::new();
    };
    object
        .iter()
        .filter(|(key, _)| key.contains("error"))
        .filter_map(|(key, value)| value.as_array().map(|items| (key, items)))
        .flat_map(|(key, items)| {
            items.iter().map(move |item| {
                format!("{}: {}", key, compact_text(&compact_json_value(item), 220))
            })
        })
        .take(8)
        .collect()
}

fn compact_audit_counts(details: &serde_json::Value) -> serde_json::Value {
    let Some(object) = details.as_object() else {
        return serde_json::json!({});
    };
    let mut counts = serde_json::Map::new();
    for (key, value) in object {
        if key.ends_with("_count") || key == "current_round" {
            counts.insert(key.clone(), value.clone());
        } else if let Some(items) = value.as_array() {
            counts.insert(format!("{key}_count"), serde_json::json!(items.len()));
        }
    }
    serde_json::Value::Object(counts)
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
            "counts": {
                "audit_failed": doctor.counts.audit_failed_count,
                "receipt_missing": doctor.counts.round_receipt_missing_count,
                "receipt_required": doctor.counts.round_receipt_required_count,
                "retry_exhausted": doctor.counts.round_worker_retry_exhausted_count,
                "worker_duration_ms": doctor.counts.round_worker_duration_ms,
                "worker_ok": doctor.counts.round_role_worker_ok_count,
                "worker_timeouts": doctor.counts.round_worker_timeout_count,
                "workers": doctor.counts.round_role_worker_count
            },
            "failed_checks": doctor.failed_checks.iter().take(3).collect::<Vec<_>>(),
            "missing_receipts": doctor.missing_receipts.iter().take(3).collect::<Vec<_>>(),
            "next_actions": doctor.next_actions.iter().take(3).collect::<Vec<_>>(),
            "worker_failures": doctor.worker_failures.iter().take(3).collect::<Vec<_>>()
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

fn compact_mirror_comments(mirror: &IssueMirrorReport, limit: usize) -> Vec<serde_json::Value> {
    let mut comments = mirror.comments.iter().rev().take(limit).collect::<Vec<_>>();
    comments.reverse();
    comments
        .into_iter()
        .map(|comment| {
            serde_json::json!({
                "id": comment.id,
                "author": comment.author,
                "body": compact_text(&comment.body, 260),
                "created_at": comment.created_at,
                "schema_version": comment.payload.get("schema_version").and_then(|value| value.as_str()),
                "source": comment.payload.get("source").and_then(|value| value.as_str()),
                "action": comment.payload.get("action").and_then(|value| value.as_str()),
                "round": comment.payload.get("round").and_then(|value| value.as_i64()),
                "status": comment.payload.get("status").and_then(|value| value.as_str()),
                "phase": comment.payload.get("phase").and_then(|value| value.as_str())
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
    card.actions.iter().map(compact_issue_action).collect()
}

fn compact_issue_action(action: &IssueAction) -> serde_json::Value {
    serde_json::json!({
        "schema_version": action.schema_version,
        "action": action.action,
        "label": action.label,
        "command": action.command,
        "source": action.source,
        "input": action.input,
        "destructive": action.destructive,
        "runtime": action.runtime
    })
}

fn compact_loop_action(action: &str, label: &str, command: String) -> serde_json::Value {
    serde_json::json!({
        "action": action,
        "label": label,
        "command": command
    })
}

fn compact_json_value(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
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
    use std::path::Path;

    use super::{
        compact_issue_board, compact_issue_detail, compact_issue_mirror, compact_issue_mirror_sync,
        compact_loop_audit, default_issue_mirror_path, flag_present, flag_value,
    };
    use entrance_core::{HiveComment, HiveIssue, HiveLoopContract};
    use entrance_hive::{
        HiveLoopAuditCheck, HiveLoopAuditReport, HiveLoopDoctorCounts, IssueAction, IssueCard,
        IssueDoctorSummary, IssueMirrorReport,
    };

    fn test_issue_action(action: &str, label: &str, command: &str) -> IssueAction {
        IssueAction {
            schema_version: "entrance.hive.issue_action.v1".to_string(),
            action: action.to_string(),
            label: label.to_string(),
            command: command.to_string(),
            source: "status_fallback".to_string(),
            input: match action {
                "run" => "none",
                "comment" => "body",
                _ => "note",
            }
            .to_string(),
            destructive: action == "cancel",
            runtime: None,
        }
    }

    fn test_worker_failure_doctor() -> IssueDoctorSummary {
        IssueDoctorSummary {
            schema_version: "entrance.hive.doctor.v1".to_string(),
            health: "worker_failed".to_string(),
            summary: "Loop #3 has worker timeout evidence.".to_string(),
            next_actions: vec![
                "entrance hive loop audit 3 --compact".to_string(),
                "entrance hive loop evidence 3".to_string(),
                "entrance hive issue retry-run 7 --body <note> --compact".to_string(),
            ],
            runtime: "codex".to_string(),
            current_round: 2,
            counts: HiveLoopDoctorCounts {
                packet_count: 4,
                admission_count: 4,
                evidence_count: 4,
                verdict_count: 2,
                round_packet_count: 1,
                round_admission_count: 1,
                round_evidence_count: 1,
                round_verdict_count: 1,
                receipt_required_count: 14,
                receipt_missing_count: 1,
                round_receipt_required_count: 3,
                round_receipt_missing_count: 1,
                role_worker_count: 4,
                role_worker_ok_count: 3,
                round_role_worker_count: 1,
                round_role_worker_ok_count: 0,
                round_worker_duration_ms: 1004,
                round_worker_timeout_count: 1,
                round_worker_retry_exhausted_count: 1,
                audit_failed_count: 1,
            },
            failed_checks: vec!["worker_receipts".to_string()],
            audit_failure_details: vec!["worker_receipts: role_worker".to_string()],
            missing_receipts: vec!["role_worker".to_string()],
            worker_failures: vec![
                "explorer:exploration_packet worker=codex ok=false receipt=false retry_exhausted"
                    .to_string(),
            ],
        }
    }

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
            actions: vec![
                test_issue_action(
                    "comment",
                    "Comment",
                    "entrance hive issue comment 7 --body <text> --compact",
                ),
                test_issue_action(
                    "retry",
                    "Retry",
                    "entrance hive issue retry-run 7 --body <note> --compact",
                ),
                test_issue_action(
                    "request-review",
                    "Review",
                    "entrance hive issue decide 7 request-review --body <note> --compact",
                ),
                test_issue_action(
                    "cancel",
                    "Cancel",
                    "entrance hive issue decide 7 cancel --body <note> --compact",
                ),
            ],
            trace: None,
            doctor: Some(test_worker_failure_doctor()),
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
        assert_eq!(
            blocked
                .pointer("/issues/0/doctor/counts/worker_timeouts")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/doctor/counts/retry_exhausted")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            blocked
                .pointer("/issues/0/doctor/worker_failures/0")
                .and_then(|value| value.as_str()),
            Some("explorer:exploration_packet worker=codex ok=false receipt=false retry_exhausted")
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
            actions: vec![
                test_issue_action(
                    "comment",
                    "Comment",
                    "entrance hive issue comment 7 --body <text> --compact",
                ),
                test_issue_action(
                    "retry",
                    "Retry",
                    "entrance hive issue retry-run 7 --body <note> --compact",
                ),
            ],
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

    #[test]
    fn compact_issue_mirror_exports_connector_ready_issue_surface() {
        let mirror = IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "linear".to_string(),
            review_surface: "linear:ENT-42".to_string(),
            external_key: "hive-loop-3-issue-7".to_string(),
            issue: HiveIssue {
                id: 7,
                loop_id: Some(3),
                title: "Loop #3: mirror contract".to_string(),
                status: "Done".to_string(),
                summary: Some("Evaluator kept the candidate.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:03:00Z".to_string(),
            },
            loop_contract: Some(HiveLoopContract {
                id: 3,
                title: "mirror contract".to_string(),
                goal: "Export issue/status/comment as typed mirror".to_string(),
                boundary: "No external writes".to_string(),
                approach_space: vec!["mirror local issue".to_string()],
                eval_space: vec!["compact mirror has comments".to_string()],
                review_surface: "linear:ENT-42".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "codex".to_string(),
                status: "kept".to_string(),
                active_phase: "complete".to_string(),
                current_round: 2,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:03:00Z".to_string(),
            }),
            comments: vec![HiveComment {
                id: 21,
                issue_id: 7,
                author: "human".to_string(),
                body: "Mirror this to the external board.".to_string(),
                payload: serde_json::json!({
                    "schema_version": "entrance.hive.operator_comment.v1",
                    "source": "operator",
                    "round": 2,
                    "status": "Done",
                    "phase": "complete"
                }),
                created_at: "2026-01-01T00:04:00Z".to_string(),
            }],
            actions: vec![test_issue_action(
                "comment",
                "Comment",
                "entrance hive issue comment 7 --body <text> --compact",
            )],
            trace: None,
            doctor: Some(test_worker_failure_doctor()),
        };

        let compact = compact_issue_mirror(&mirror);

        assert_eq!(
            compact
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror.compact.v1")
        );
        assert_eq!(
            compact
                .pointer("/provider")
                .and_then(|value| value.as_str()),
            Some("linear")
        );
        assert_eq!(
            compact
                .pointer("/review_surface")
                .and_then(|value| value.as_str()),
            Some("linear:ENT-42")
        );
        assert_eq!(
            compact
                .pointer("/refresh_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror 7 --compact")
        );
        assert_eq!(
            compact
                .pointer("/loop/review_surface")
                .and_then(|value| value.as_str()),
            Some("linear:ENT-42")
        );
        assert_eq!(
            compact
                .pointer("/comments/0/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.operator_comment.v1")
        );
        assert_eq!(
            compact
                .pointer("/comments/0/round")
                .and_then(|value| value.as_i64()),
            Some(2)
        );
        assert_eq!(
            compact
                .pointer("/actions/0/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue comment 7 --body <text> --compact")
        );
    }

    #[test]
    fn mirror_sink_path_sanitizes_external_key() {
        let path = default_issue_mirror_path(Path::new("/tmp/root"), "linear:ENT/42?x");

        assert_eq!(
            path,
            Path::new("/tmp/root/connectors/issue-mirrors/linear-ENT-42-x.json")
        );
    }

    #[test]
    fn compact_issue_mirror_sync_reports_written_file() {
        let mirror = IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "file".to_string(),
            review_surface: "file:local-board".to_string(),
            external_key: "hive-loop-1-issue-2".to_string(),
            issue: HiveIssue {
                id: 2,
                loop_id: Some(1),
                title: "Loop #1: mirror sync".to_string(),
                status: "Done".to_string(),
                summary: Some("Mirror written.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:01:00Z".to_string(),
            },
            loop_contract: None,
            comments: vec![],
            actions: vec![],
            trace: None,
            doctor: None,
        };

        let report = compact_issue_mirror_sync(
            &mirror,
            Path::new("/tmp/root/connectors/issue-mirrors/hive-loop-1-issue-2.json"),
            512,
        );

        assert_eq!(
            report
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_sync.v1")
        );
        assert_eq!(
            report.pointer("/provider").and_then(|value| value.as_str()),
            Some("file")
        );
        assert_eq!(
            report
                .pointer("/review_surface")
                .and_then(|value| value.as_str()),
            Some("file:local-board")
        );
        assert_eq!(
            report.pointer("/issue_id").and_then(|value| value.as_i64()),
            Some(2)
        );
        assert_eq!(
            report.pointer("/bytes").and_then(|value| value.as_u64()),
            Some(512)
        );
        assert_eq!(
            report
                .pointer("/sync_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-sync 2")
        );
    }

    #[test]
    fn compact_created_todo_issue_exposes_next_run_command() {
        let detail = compact_issue_detail(&IssueCard {
            issue: HiveIssue {
                id: 9,
                loop_id: Some(4),
                title: "Loop #4: compact create".to_string(),
                status: "Todo".to_string(),
                summary: Some("Loop contract created; waiting for Explorer.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            comments: vec![HiveComment {
                id: 13,
                issue_id: 9,
                author: "compiler".to_string(),
                body: "Loop contract admitted into Hive with 3 active policies.".to_string(),
                payload: serde_json::json!({
                    "source": "compiler",
                    "next_phase": "explorer"
                }),
                created_at: "2026-01-01T00:01:00Z".to_string(),
            }],
            actions: vec![
                test_issue_action("run", "Run", "entrance hive issue run 9 --compact"),
                test_issue_action(
                    "comment",
                    "Comment",
                    "entrance hive issue comment 9 --body <text> --compact",
                ),
            ],
            trace: None,
            doctor: None,
        });

        assert_eq!(
            detail
                .pointer("/issue/status")
                .and_then(|value| value.as_str()),
            Some("Todo")
        );
        assert_eq!(
            detail
                .pointer("/issue/actions/0/action")
                .and_then(|value| value.as_str()),
            Some("run")
        );
        assert_eq!(
            detail
                .pointer("/issue/actions/0/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue run 9 --compact")
        );
        assert_eq!(
            detail
                .pointer("/issue/actions/1/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue comment 9 --body <text> --compact")
        );
    }

    #[test]
    fn compact_loop_audit_surfaces_failed_checks_and_actions() {
        let compact = compact_loop_audit(&HiveLoopAuditReport {
            schema_version: "entrance.hive.audit.v1".to_string(),
            loop_id: 42,
            passed: false,
            failed_count: 1,
            checks: vec![
                HiveLoopAuditCheck {
                    name: "runtime_policy".to_string(),
                    passed: false,
                    summary: "Runtime `codex` inspected; 1 runtime policy issues.".to_string(),
                    details: serde_json::json!({
                        "current_round": 2,
                        "runtime_policy_errors": [
                            "runtime_policy:worker_receipt:context.command"
                        ],
                        "supported_runtimes": ["local", "codex"]
                    }),
                },
                HiveLoopAuditCheck {
                    name: "issue_surface".to_string(),
                    passed: true,
                    summary: "1 linked issues, 6 comments, and 0 operator evidence rows inspected; 0 issue surface issues.".to_string(),
                    details: serde_json::json!({
                        "comment_count": 6,
                        "operator_evidence_count": 0,
                        "issue_surface_errors": []
                    }),
                },
            ],
        });

        assert_eq!(
            compact
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.audit.compact.v1")
        );
        assert_eq!(
            compact
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("runtime_policy")
        );
        assert_eq!(
            compact
                .pointer("/checks/0/error_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            compact
                .pointer("/checks/0/counts/current_round")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            compact
                .pointer("/checks/0/errors/0")
                .and_then(|value| value.as_str()),
            Some("runtime_policy_errors: runtime_policy:worker_receipt:context.command")
        );
        assert_eq!(
            compact
                .pointer("/actions/0/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive loop doctor 42")
        );
        assert_eq!(
            compact
                .pointer("/actions/3/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive loop audit 42")
        );
    }
}
