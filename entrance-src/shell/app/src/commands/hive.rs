use anyhow::{bail, Context, Result};
use entrance_core::StoreSchemaStatus;
use entrance_hive::{
    issue_advance_next_action, HiveCallbackRequest, HiveDispatchRequest, HiveLoopCreateRequest,
    HiveLoopReport, HiveLoopRunRequest, IssueAdvanceRequest, IssueCard, IssueClaimRequest,
    IssueCommentRequest, IssueDecisionRequest, IssueRunRequest, IssueTransitionPolicyReport,
    OperatorConfirmationActor, OperatorConfirmationClient, OperatorConfirmationReceipt,
    PolicyRegistryReport, ReviewDecision, OPERATOR_ACTION_CONFIRMATION_ARG,
    OPERATOR_ACTION_POLICY_SCHEMA_VERSION, OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
};

use crate::{app::AppServices, cli, mcp::loop_control_packet, print_json};

const ISSUE_STATUSES: &[&str] = &[
    "Todo",
    "Doing",
    "Blocked",
    "Needs Review",
    "Done",
    "Canceled",
];

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive schema [--compact]\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>\n  entrance hive loop create --title <text> --goal <text> [--runtime local|codex] [--compact]\n  entrance hive loop run <id> [--runtime local|codex] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop control <id>\n  entrance hive loop list\n  entrance hive issue create --title <text> --goal <text> [--runtime local|codex] [--compact]\n  entrance hive issue list [--compact]\n  entrance hive issue show <id> [--compact]\n  entrance hive issue claim <id> --agent <name> [--role developer|reviewer] [--compact]\n  entrance hive issue comment <id> --body <text> [--author <name>] [--compact]\n  entrance hive issue run <id> [--runtime local|codex] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive issue advance <id> [--until-wait] [--runtime local|codex] [--max-steps <n>] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive issue review <id> --decision keep|reject|blocked [--body <text>] [--compact]\n  entrance hive issue retry <id> --human-confirmed [--body <text>] [--compact]\n  entrance hive issue decide <id> retry|request-review|cancel --human-confirmed [--body <text>] [--compact]\n  entrance hive issue control <id>\n  entrance hive review-queue\n  entrance hive policy registry [--compact]"
            );
            Ok(())
        }
        [flag] if cli::is_help(flag) => run(services, &[]),
        [command] if command == "list" => print_json(&services.hive.list()?),
        [command] if command == "summary" => print_json(&services.hive.summary()?),
        [command, rest @ ..] if command == "schema" => {
            let status = services.kernel.store.schema_status()?;
            if flag_present(rest, "--compact") {
                print_json(&compact_store_schema_status(&status))
            } else {
                print_json(&status)
            }
        }
        [command, rest @ ..] if command == "dispatch" => {
            let title = flag_value(rest, "--title").context("hive dispatch requires --title")?;
            let report = services.hive.dispatch(HiveDispatchRequest {
                title: title.to_string(),
                project_dir: flag_value(rest, "--project").map(ToOwned::to_owned),
                summary: flag_value(rest, "--summary").map(ToOwned::to_owned),
                payload_json: "{}".to_string(),
            })?;
            print_json(&report)
        }
        [command, id] if command == "engine" => {
            print_json(&services.hive.engine_report(id.parse::<i64>()?)?)
        }
        [command, id, status] if command == "callback" => {
            print_json(&services.hive.callback(HiveCallbackRequest {
                run_id: id.parse::<i64>()?,
                status: status.clone(),
                summary: None,
            })?)
        }
        [command, id, status, summary] if command == "callback" => {
            print_json(&services.hive.callback(HiveCallbackRequest {
                run_id: id.parse::<i64>()?,
                status: status.clone(),
                summary: Some(summary.clone()),
            })?)
        }
        [command, id, decision] if command == "review" => {
            let decision = match decision.as_str() {
                "approve" => ReviewDecision::Approve,
                "return" => ReviewDecision::Return,
                "integrate" => ReviewDecision::Integrate,
                _ => bail!("unsupported review decision"),
            };
            print_json(&services.hive.review(id.parse::<i64>()?, decision)?)
        }
        [scope, action] if scope == "loop" && action == "list" => {
            print_json(&services.hive.loop_list()?)
        }
        [scope, action, id] if scope == "loop" && action == "show" => {
            print_json(&services.hive.loop_report(id.parse::<i64>()?)?)
        }
        [scope, action, rest @ ..] if scope == "loop" && action == "create" => {
            let report = services
                .hive
                .loop_create(loop_create_request_from_flags(rest)?)?;
            print_loop_create_report(services, report, flag_present(rest, "--compact"))
        }
        [scope, action, id, rest @ ..] if scope == "loop" && action == "run" => {
            let loop_id = id.parse::<i64>()?;
            let report = services.hive.loop_run(HiveLoopRunRequest {
                loop_id,
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                decision: None,
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_loop_report(services, &report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id] if scope == "loop" && action == "control" => {
            print_json(&loop_control_packet(services, id.parse::<i64>()?)?)
        }
        [scope, action, rest @ ..] if scope == "policy" && action == "registry" => {
            let report = services.hive.policy_registry();
            if flag_present(rest, "--compact") {
                print_json(&compact_policy_registry(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, rest @ ..] if scope == "issue" && action == "create" => {
            let report = services
                .hive
                .loop_create(loop_create_request_from_flags(rest)?)?;
            print_loop_create_report(services, report, flag_present(rest, "--compact"))
        }
        [scope, action, rest @ ..] if scope == "issue" && action == "list" => {
            let cards = services.hive.panel()?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_board(&cards))
            } else {
                print_json(&cards)
            }
        }
        [scope, action] if scope == "review-queue" && action.is_empty() => unreachable!(),
        [scope] if scope == "review-queue" => print_json(&review_queue(services)?),
        [scope, action, id, rest @ ..] if scope == "issue" && action == "show" => {
            let card = services.hive.issue_report(id.parse::<i64>()?)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id] if scope == "issue" && action == "control" => {
            print_json(&issue_control_packet(services, id.parse::<i64>()?)?)
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "claim" => {
            let agent = flag_value(rest, "--agent").context("issue claim requires --agent")?;
            let role = flag_value(rest, "--role").unwrap_or("developer");
            if !matches!(role, "developer" | "reviewer") {
                bail!("issue claim role must be developer or reviewer");
            }
            let report = services.hive.issue_claim(IssueClaimRequest {
                issue_id: id.parse::<i64>()?,
                agent: agent.to_string(),
                role: Some(role.to_string()),
                source: Some("cli".to_string()),
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&report.issue))
            } else {
                print_json(&report)
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
        [scope, action, id, rest @ ..] if scope == "issue" && action == "run" => {
            let issue_id = id.parse::<i64>()?;
            let report = services.hive.issue_run(IssueRunRequest {
                issue_id,
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                decision: None,
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
                retry: false,
                author: flag_value(rest, "--author").unwrap_or("human").to_string(),
                body: None,
                confirmation_receipt: None,
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_loop_report(services, &report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "advance" => {
            let report = services.hive.issue_advance(IssueAdvanceRequest {
                issue_id: id.parse::<i64>()?,
                mode: Some(if flag_present(rest, "--until-wait") {
                    "until_wait".to_string()
                } else {
                    "one_step".to_string()
                }),
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                max_steps: flag_value(rest, "--max-steps")
                    .map(str::parse)
                    .transpose()?,
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
            })?;
            if flag_present(rest, "--compact") {
                print_json(&serde_json::json!({
                    "schema_version": report.schema_version,
                    "mode": report.mode,
                    "stop_reason": report.stop_reason,
                    "issue": compact_issue_card(&report.issue),
                    "steps": report.steps,
                    "next_actions": report.next_actions
                }))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "retry" => {
            let issue_id = id.parse::<i64>()?;
            let author = flag_value(rest, "--author").unwrap_or("human").to_string();
            let report = services.hive.issue_run(IssueRunRequest {
                issue_id,
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                decision: None,
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
                retry: true,
                author: author.clone(),
                body: flag_value(rest, "--body").map(ToOwned::to_owned),
                confirmation_receipt: cli_human_confirmation_receipt(
                    "retry",
                    &author,
                    flag_present(rest, "--human-confirmed"),
                ),
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_loop_report(services, &report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "review" => {
            let decision =
                flag_value(rest, "--decision").context("issue review requires --decision")?;
            let action = match decision {
                "keep" => "request-review",
                "reject" | "blocked" => "request-review",
                _ => bail!("issue review decision must be keep, reject, or blocked"),
            };
            let author = flag_value(rest, "--author")
                .unwrap_or("reviewer")
                .to_string();
            let card = services.hive.issue_decide(IssueDecisionRequest {
                issue_id: id.parse::<i64>()?,
                action: action.to_string(),
                author: author.clone(),
                body: flag_value(rest, "--body")
                    .map(ToOwned::to_owned)
                    .or_else(|| Some(format!("Reviewer decision: {decision}."))),
                confirmation_receipt: cli_human_confirmation_receipt(action, &author, true),
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id, decision, rest @ ..] if scope == "issue" && action == "decide" => {
            let author = flag_value(rest, "--author").unwrap_or("human").to_string();
            let card = services.hive.issue_decide(IssueDecisionRequest {
                issue_id: id.parse::<i64>()?,
                action: decision.to_string(),
                author: author.clone(),
                body: flag_value(rest, "--body").map(ToOwned::to_owned),
                confirmation_receipt: cli_human_confirmation_receipt(
                    decision,
                    &author,
                    flag_present(rest, "--human-confirmed"),
                ),
            })?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail(&card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "transition-policy" => {
            let report = services.hive.issue_transition_policy(id.parse::<i64>()?)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_transition_policy(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id] if scope == "issue" && action == "timeline" => {
            print_json(&services.hive.issue_timeline(id.parse::<i64>()?)?)
        }
        [scope, action, id, item_id] if scope == "issue" && action == "timeline-item" => {
            print_json(
                &services
                    .hive
                    .issue_timeline_item(id.parse::<i64>()?, item_id)?,
            )
        }
        _ => bail!("unsupported hive command"),
    }
}

pub(crate) fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|values| values[0] == flag)
        .map(|values| values[1].as_str())
}

pub(crate) fn flag_present(args: &[String], flag: &str) -> bool {
    args.iter().any(|value| value == flag)
}

pub(crate) fn cli_human_confirmation_receipt(
    action: &str,
    author: &str,
    human_confirmed: bool,
) -> Option<OperatorConfirmationReceipt> {
    human_confirmed.then(|| OperatorConfirmationReceipt {
        schema_version: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
        source: "cli".to_string(),
        policy_schema_version: OPERATOR_ACTION_POLICY_SCHEMA_VERSION.to_string(),
        confirmation_arg: OPERATOR_ACTION_CONFIRMATION_ARG.to_string(),
        human_confirmed: true,
        action: action.to_string(),
        author: author.to_string(),
        marker: format!(
            "CLI confirmation: human_confirmed=true; action={action}; author={author}; policy={OPERATOR_ACTION_POLICY_SCHEMA_VERSION}"
        ),
        client: Some(OperatorConfirmationClient {
            name: "entrance-cli".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            source: "cli".to_string(),
        }),
        actor: Some(OperatorConfirmationActor {
            id: author.to_string(),
            label: author.to_string(),
            source: "cli".to_string(),
            trust: "self-reported".to_string(),
            verified: false,
        }),
    })
}

pub(crate) fn loop_create_request_from_flags(args: &[String]) -> Result<HiveLoopCreateRequest> {
    let title = flag_value(args, "--title").context("loop create requires --title <text>")?;
    let goal = flag_value(args, "--goal").unwrap_or(title);
    Ok(HiveLoopCreateRequest {
        title: title.to_string(),
        goal: goal.to_string(),
        boundary: flag_value(args, "--boundary")
            .unwrap_or("Local MCP-native issue workbench boundary.")
            .to_string(),
        approach_space: flag_values(args, "--approach"),
        eval_space: flag_values(args, "--eval"),
        review_surface: "local-hive-panel".to_string(),
        autonomy_level: "run-approved-candidates".to_string(),
        runtime: flag_value(args, "--runtime").unwrap_or("local").to_string(),
    })
}

fn flag_values(args: &[String], flag: &str) -> Vec<String> {
    args.windows(2)
        .filter(|values| values[0] == flag)
        .map(|values| values[1].clone())
        .collect()
}

fn print_loop_create_report(
    services: &AppServices,
    report: HiveLoopReport,
    compact: bool,
) -> Result<()> {
    if compact {
        print_json(&compact_loop_report(services, &report))
    } else {
        print_json(&report)
    }
}

pub(crate) fn compact_issue_board(cards: &[IssueCard]) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_board.compact.v1",
        "statuses": ISSUE_STATUSES,
        "counts": ISSUE_STATUSES.iter().map(|status| {
            serde_json::json!({
                "status": status,
                "count": cards.iter().filter(|card| card.issue.status == *status).count()
            })
        }).collect::<Vec<_>>(),
        "issues": cards.iter().map(compact_issue_card).collect::<Vec<_>>()
    })
}

pub(crate) fn compact_issue_detail(card: &IssueCard) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_detail.compact.v1",
        "issue": compact_issue_card(card),
        "comments": card.comments.iter().map(|comment| serde_json::json!({
            "id": comment.id,
            "author": comment.author,
            "body": comment.body,
            "created_at": comment.created_at
        })).collect::<Vec<_>>(),
        "actions": card.actions.iter().map(|action| serde_json::json!({
            "action": action.action,
            "label": action.label,
            "command": action.command,
            "confirmation_required": action.confirmation_required
        })).collect::<Vec<_>>(),
        "trace": card.trace.as_ref().map(|trace| serde_json::json!({
            "current_round": trace.current_round,
            "evidence_count": trace.evidence_count,
            "verdict_count": trace.verdict_count,
            "last_decision": trace.last_decision,
            "reason_code": trace.reason_code,
            "human_options": trace.human_options,
            "worker_ok": trace.worker_ok,
            "audit_passed": trace.audit_passed,
            "audit_failed_checks": trace.audit_failed_checks
        })),
        "doctor": card.doctor.as_ref().map(|doctor| serde_json::json!({
            "health": doctor.health,
            "failed_checks": doctor.failed_checks
        }))
    })
}

fn compact_issue_card(card: &IssueCard) -> serde_json::Value {
    serde_json::json!({
        "id": card.issue.id,
        "loop_id": card.issue.loop_id,
        "title": card.issue.title,
        "status": card.issue.status,
        "summary": card.issue.summary,
        "assignee": card.issue.assignee,
        "claim_role": card.issue.claim_role,
        "claim_source": card.issue.claim_source,
        "claimed_at": card.issue.claimed_at,
        "round": card.trace.as_ref().map(|trace| trace.current_round),
        "latest_decision": card.trace.as_ref().and_then(|trace| trace.last_decision.clone()),
        "evidence_count": card.trace.as_ref().map(|trace| trace.evidence_count).unwrap_or_default(),
        "comment_count": card.comments.len()
    })
}

pub(crate) fn compact_loop_report(
    services: &AppServices,
    report: &HiveLoopReport,
) -> serde_json::Value {
    let issue = report.issues.first().map(compact_issue_card);
    let control = loop_control_packet(services, report.contract.id).ok();
    serde_json::json!({
        "schema_version": "entrance.hive.loop.compact.v1",
        "loop": {
            "id": report.contract.id,
            "title": report.contract.title,
            "status": report.contract.status,
            "active_phase": report.contract.active_phase,
            "current_round": report.contract.current_round,
            "runtime": report.contract.runtime
        },
        "issue": issue,
        "control": control
    })
}

pub(crate) fn issue_control_packet(
    services: &AppServices,
    issue_id: i64,
) -> Result<serde_json::Value> {
    let card = services.hive.issue_report(issue_id)?;
    let loop_control = card
        .issue
        .loop_id
        .and_then(|loop_id| loop_control_packet(services, loop_id).ok());
    Ok(serde_json::json!({
        "schema_version": "entrance.hive.issue_control.v1",
        "issue": compact_issue_detail(&card),
        "loop_control": loop_control,
        "advance_next_action": issue_advance_next_action(&card),
        "resources": {
            "issue": format!("entrance://issues/{issue_id}"),
            "issue_control": format!("entrance://issues/{issue_id}/control"),
            "review_queue": "entrance://review-queue",
            "transition_policy": format!("entrance://issues/{issue_id}/transition-policy")
        },
        "next_actions": card.actions.iter().map(|action| serde_json::json!({
            "action": action.action,
            "label": action.label,
            "command": action.command,
            "confirmation_required": action.confirmation_required
        })).collect::<Vec<_>>()
    }))
}

pub(crate) fn review_queue(services: &AppServices) -> Result<serde_json::Value> {
    let cards = services.hive.panel()?;
    let queue = cards
        .iter()
        .filter(|card| matches!(card.issue.status.as_str(), "Blocked" | "Needs Review"))
        .map(compact_issue_card)
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "schema_version": "entrance.hive.review_queue.v1",
        "count": queue.len(),
        "issues": queue
    }))
}

fn compact_policy_registry(report: &PolicyRegistryReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.policy_registry.compact.v1",
        "runtime": report.runtime,
        "issue_transitions": report.issue_transitions
    })
}

fn compact_issue_transition_policy(report: &IssueTransitionPolicyReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_transition_policy.compact.v1",
        "issue_id": report.issue.id,
        "status": report.issue.status,
        "state_class": report.state_class,
        "allowed_actions": report.allowed_actions.iter().map(|action| serde_json::json!({
            "action": action.action,
            "to_status": action.to_status,
            "requires_confirmation": action.requires_human
        })).collect::<Vec<_>>(),
        "blocked_actions": report.blocked_actions.iter().map(|action| serde_json::json!({
            "action": action.action,
            "reason": action.reason
        })).collect::<Vec<_>>(),
        "reviewer_budget": report.reviewer_budget
    })
}

fn compact_store_schema_status(status: &StoreSchemaStatus) -> serde_json::Value {
    serde_json::json!({
        "schema_version": status.schema_version,
        "healthy": status.healthy,
        "user_version": status.user_version,
        "expected_user_version": status.expected_user_version,
        "missing_tables": status.missing_tables,
        "missing_columns": status.missing_columns,
        "missing_indexes": status.missing_indexes
    })
}
