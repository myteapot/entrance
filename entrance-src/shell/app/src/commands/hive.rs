use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use entrance_core::{HiveCommentCreate, HiveLoopEvidenceCreate};
use entrance_hive::{
    connector_retry_policy_for_provider, ConnectorAdmissionCheckSpec,
    ConnectorProviderAdmissionSpec, ConnectorProviderSpec, ConnectorRegistryReport,
    ConnectorRetryPolicySpec, HiveCallbackRequest, HiveDispatchRequest, HiveLoopAuditCheck,
    HiveLoopAuditReport, HiveLoopCreateRequest, HiveLoopReport, HiveLoopRunRequest, IssueAction,
    IssueCard, IssueCommentRequest, IssueDecisionRequest, IssueMirrorReport, IssueRunRequest,
    PolicyGateSpec, PolicyRegistryReport, ReviewDecision, CONNECTOR_MIRROR_RECEIPT_GATE,
    CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND,
};
use reqwest::Method;
use sha2::{Digest, Sha256};

use crate::{app::AppServices, cli, print_json};

const ISSUE_MIRROR_SYNC_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_sync.v1";
const ISSUE_MIRROR_SYNC_RECEIPT_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_sync_receipt.v1";
const ISSUE_MIRROR_PUBLISH_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_publish.v1";
const ISSUE_MIRROR_STATUS_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_status.v1";
const ISSUE_MIRROR_VERIFY_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_verify.v1";
const ISSUE_MIRROR_AUDIT_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_audit.v1";
const ISSUE_MIRROR_READBACK_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_readback.v1";
const ISSUE_MIRROR_ADMISSION_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_admission.v1";
const ISSUE_MIRROR_ROUNDTRIP_SCHEMA_VERSION: &str = "entrance.hive.issue_mirror_roundtrip.v1";
const CONNECTOR_PUBLISH_HINT_SCHEMA_VERSION: &str = "entrance.hive.connector_publish_hint.v1";
const CONNECTOR_PUBLISH_PLAN_SCHEMA_VERSION: &str = "entrance.hive.connector_publish_plan.v1";
const CONNECTOR_PUBLISH_EXECUTE_SCHEMA_VERSION: &str = "entrance.hive.connector_publish_execute.v1";
const CONNECTOR_ROUNDTRIP_PLAN_SCHEMA_VERSION: &str = "entrance.hive.connector_roundtrip_plan.v1";
const CONNECTOR_ROUNDTRIP_EXECUTE_SCHEMA_VERSION: &str =
    "entrance.hive.connector_roundtrip_execute.v1";
const CONNECTOR_WRITER_ADAPTER_SCHEMA_VERSION: &str = "entrance.hive.connector_writer_adapter.v1";
const CONNECTOR_WRITE_RECEIPT_SCHEMA_VERSION: &str = "entrance.hive.connector_write_receipt.v1";
const CONNECTOR_REMOTE_CONTRACT_SCHEMA_VERSION: &str = "entrance.hive.connector_remote_contract.v1";
const CONNECTOR_REMOTE_WRITE_RECEIPT_SCHEMA_VERSION: &str =
    "entrance.hive.connector_remote_write_receipt.v1";
const CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION: &str = "entrance.hive.connector_remote_readback.v1";
const CONNECTOR_REMOTE_TARGET_SCHEMA_VERSION: &str = "entrance.hive.connector_remote_target.v1";
const CONNECTOR_REMOTE_WRITE_PLAN_SCHEMA_VERSION: &str =
    "entrance.hive.connector_remote_write_plan.v1";
const CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION: &str =
    "entrance.hive.connector_remote_write_execute.v1";
const ISSUE_CONNECTOR_ADMISSION_PREVIEW_SCHEMA_VERSION: &str =
    "entrance.hive.issue_connector_admission_preview.v1";
const ISSUE_CONNECTOR_ADMISSION_OBJECT_KIND: &str = "ISSUE_CONNECTOR_ADMISSION";
const POLICY_SCHEMA_VERSION: &str = "entrance.hive.policy.v1";
const LOOP_DEMO_COMPACT_SCHEMA_VERSION: &str = "entrance.hive.loop_demo.compact.v1";
const GITHUB_COMMENTS_MAX_PAGES: usize = 20;
const GITHUB_COMMENT_IDEMPOTENCY_PREFIX: &str = "<!-- entrance-idempotency-key:";
const GITHUB_COMMENT_IDEMPOTENCY_SUFFIX: &str = "-->";
const LINEAR_COMMENT_IDEMPOTENCY_PREFIX: &str = "<!-- entrance-idempotency-key:";
const LINEAR_COMMENT_IDEMPOTENCY_SUFFIX: &str = "-->";
const SYSTEM_COMMENT_SCHEMA_VERSION: &str = "entrance.hive.system_comment.v1";
const ISSUE_STATUSES: &[&str] = &[
    "Todo",
    "Doing",
    "Blocked",
    "Needs Review",
    "Done",
    "Canceled",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorFileDigest {
    bytes: u64,
    sha256: String,
}

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>\n  entrance hive loop demo [--runtime local|codex] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop start --title <text> --goal <text> [--runtime local|codex] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop create --title <text> --goal <text> [--runtime local|codex] [--compact]\n  entrance hive loop run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop show <id>\n  entrance hive loop trace <id>\n  entrance hive loop evidence <id>\n  entrance hive loop audit <id> [--compact]\n  entrance hive loop doctor <id>\n  entrance hive loop policies <id>\n  entrance hive loop list\n  entrance hive policy registry [--compact]\n  entrance hive connector registry [--compact]\n  entrance hive connector queue [--provider <name>] [--compact]\n  entrance hive connector publish-plan [--provider <name>] [--compact]\n  entrance hive connector publish-execute --plan-id <sha256> [--provider <name>] [--compact]\n  entrance hive connector roundtrip-plan [--provider <name>] [--compact]\n  entrance hive connector roundtrip-execute --plan-id <sha256> [--provider <name>] [--compact]\n  entrance hive issue list [--compact]\n  entrance hive issue show <id> [--compact]\n  entrance hive issue connector-admission <id> [--path <path>] [--compact]\n  entrance hive issue mirror <id> [--compact]\n  entrance hive issue mirror-sync <id> [--out <path>]\n  entrance hive issue mirror-publish <id> [--path <path>] [--compact]\n  entrance hive issue mirror-status <id> [--path <path>] [--compact]\n  entrance hive issue mirror-verify <id> [--path <path>]\n  entrance hive issue mirror-audit <id> [--path <path>] [--compact]\n  entrance hive issue mirror-readback <id> [--path <path>] [--record] [--compact]\n  entrance hive issue mirror-admit <id> [--path <path>] [--record] [--compact]\n  entrance hive issue mirror-roundtrip <id> [--path <path>] [--no-record] [--compact]\n  entrance hive issue comment <id> --body <text> [--compact]\n  entrance hive issue decide <id> <retry|request-review|cancel> [--body <text>] [--compact]\n  entrance hive issue run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive issue retry-run <id> [--body <text>] [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]"
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
        [scope, action, rest @ ..] if scope == "policy" && action == "registry" => {
            let report = services.hive.policy_registry();
            if flag_present(rest, "--compact") {
                print_json(&compact_policy_registry(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, rest @ ..] if scope == "connector" && action == "registry" => {
            let report = services.hive.connector_registry();
            if flag_present(rest, "--compact") {
                print_json(&compact_connector_registry(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, rest @ ..] if scope == "connector" && action == "queue" => print_json(
            &connector_queue_report(services, flag_value(rest, "--provider"))?,
        ),
        [scope, action, rest @ ..] if scope == "connector" && action == "publish-plan" => {
            print_json(&connector_publish_plan_report(
                services,
                flag_value(rest, "--provider"),
            )?)
        }
        [scope, action, rest @ ..] if scope == "connector" && action == "publish-execute" => {
            let plan_id = flag_value(rest, "--plan-id")
                .context("hive connector publish-execute requires --plan-id <sha256>")?;
            print_json(&execute_connector_publish_plan(
                services,
                flag_value(rest, "--provider"),
                plan_id,
            )?)
        }
        [scope, action, rest @ ..] if scope == "connector" && action == "roundtrip-plan" => {
            let report = connector_roundtrip_plan_report(services, flag_value(rest, "--provider"))?;
            if flag_present(rest, "--compact") {
                print_json(&compact_connector_roundtrip_plan_summary(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, rest @ ..] if scope == "connector" && action == "roundtrip-execute" => {
            let plan_id = flag_value(rest, "--plan-id")
                .context("hive connector roundtrip-execute requires --plan-id <sha256>")?;
            let report = execute_connector_roundtrip_plan(
                services,
                flag_value(rest, "--provider"),
                plan_id,
            )?;
            if flag_present(rest, "--compact") {
                print_json(&compact_connector_roundtrip_execute_summary(&report))
            } else {
                print_json(&report)
            }
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
        [scope, action, rest @ ..] if scope == "loop" && action == "start" => {
            let outcome =
                run_issue_bound_loop(services, loop_create_request_from_flags(rest), rest)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_loop_start_summary(&outcome.detail))
            } else {
                print_json(&serde_json::json!({
                    "schema_version": "entrance.hive.loop_start.v1",
                    "loop_id": outcome.loop_id,
                    "issue_id": outcome.issue_id,
                    "created": outcome.created,
                    "run": outcome.run,
                    "doctor": services.hive.loop_doctor(outcome.loop_id)?,
                    "issue": outcome.card
                }))
            }
        }
        [scope, action, rest @ ..] if scope == "loop" && action == "demo" => {
            let outcome = run_issue_bound_loop(services, loop_demo_request_from_flags(rest), rest)?;
            let loop_summary = compact_loop_start_summary(&outcome.detail);
            if flag_present(rest, "--compact") {
                print_json(&compact_loop_demo_summary(services, &loop_summary))
            } else {
                print_json(&serde_json::json!({
                    "schema_version": "entrance.hive.loop_demo.v1",
                    "demo": compact_loop_demo_context(services),
                    "loop_start": {
                        "loop_id": outcome.loop_id,
                        "issue_id": outcome.issue_id,
                        "created": outcome.created,
                        "run": outcome.run,
                        "doctor": services.hive.loop_doctor(outcome.loop_id)?,
                        "issue": outcome.card
                    },
                    "compact": compact_loop_demo_summary(services, &loop_summary)
                }))
            }
        }
        [scope, action, rest @ ..] if scope == "loop" && action == "create" => {
            let request = loop_create_request_from_flags(rest);
            let report = services.hive.loop_create(request)?;
            if flag_present(rest, "--compact") {
                if let Some(card) = report.issues.first() {
                    print_json(&compact_issue_detail_with_connector_status(services, card))
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
                print_json(&compact_issue_board_with_connector_status(services, &cards))
            } else {
                print_json(&cards)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "show" => {
            let card = services.hive.issue_report(id.parse::<i64>()?)?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_detail_with_connector_status(services, &card))
            } else {
                print_json(&card)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "connector-admission" => {
            let report = issue_connector_admission_preview(
                services,
                id.parse::<i64>()?,
                flag_value(rest, "--path"),
            )?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_connector_admission_preview(&report))
            } else {
                print_json(&report)
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
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-publish" => {
            let report = publish_issue_mirror_to_file(
                services,
                id.parse::<i64>()?,
                flag_value(rest, "--path").or_else(|| flag_value(rest, "--out")),
            )?;
            print_json(&report)
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-status" => {
            let report =
                issue_mirror_status(services, id.parse::<i64>()?, flag_value(rest, "--path"))?;
            print_json(&report)
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-verify" => {
            let report =
                verify_issue_mirror_file(services, id.parse::<i64>()?, flag_value(rest, "--path"))?;
            print_json(&report)
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-audit" => {
            let report =
                audit_issue_mirror_file(services, id.parse::<i64>()?, flag_value(rest, "--path"))?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_mirror_audit_summary(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-readback" => {
            let report = readback_issue_mirror_file(
                services,
                id.parse::<i64>()?,
                flag_value(rest, "--path"),
                flag_present(rest, "--record"),
            )?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_mirror_readback_summary(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-admit" => {
            let report = admit_issue_mirror_file(
                services,
                id.parse::<i64>()?,
                flag_value(rest, "--path"),
                flag_present(rest, "--record"),
            )?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_mirror_admission_summary(&report))
            } else {
                print_json(&report)
            }
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "mirror-roundtrip" => {
            let report = roundtrip_issue_mirror_file(
                services,
                id.parse::<i64>()?,
                flag_value(rest, "--path"),
                !flag_present(rest, "--no-record"),
            )?;
            if flag_present(rest, "--compact") {
                print_json(&compact_issue_mirror_roundtrip_summary(&report))
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
                print_json(&compact_issue_detail_with_connector_status(services, &card))
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
                print_json(&compact_issue_detail_with_connector_status(services, &card))
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
                print_json(&compact_issue_detail_with_connector_status(services, &card))
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

struct LoopStartOutcome {
    loop_id: i64,
    issue_id: i64,
    created: HiveLoopReport,
    run: HiveLoopReport,
    card: IssueCard,
    detail: serde_json::Value,
}

fn run_issue_bound_loop(
    services: &AppServices,
    request: HiveLoopCreateRequest,
    rest: &[String],
) -> Result<LoopStartOutcome> {
    let created = services.hive.loop_create(request)?;
    let loop_id = created.contract.id;
    let issue_id = created
        .issues
        .first()
        .map(|card| card.issue.id)
        .with_context(|| {
            format!("hive loop start created loop `{loop_id}` without a linked issue")
        })?;
    let run = services.hive.issue_run(IssueRunRequest {
        issue_id,
        runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
        decision: flag_value(rest, "--decision").map(ToOwned::to_owned),
        worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
            .map(str::parse)
            .transpose()?,
        worker_attempts: flag_value(rest, "--worker-attempts")
            .map(str::parse)
            .transpose()?,
        retry: false,
        author: flag_value(rest, "--author").unwrap_or("human").to_string(),
        body: flag_value(rest, "--body").map(ToOwned::to_owned),
    })?;
    let card = services.hive.issue_report(issue_id)?;
    let detail = compact_issue_detail_with_connector_status(services, &card);
    Ok(LoopStartOutcome {
        loop_id,
        issue_id,
        created,
        run,
        card,
        detail,
    })
}

fn loop_create_request_from_flags(rest: &[String]) -> HiveLoopCreateRequest {
    let title = flag_value(rest, "--title").unwrap_or("Untitled loop");
    let goal = flag_value(rest, "--goal").unwrap_or(title);
    HiveLoopCreateRequest {
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
    }
}

fn loop_demo_request_from_flags(rest: &[String]) -> HiveLoopCreateRequest {
    HiveLoopCreateRequest {
        title: flag_value(rest, "--title")
            .unwrap_or("Entrance MVP demo")
            .to_string(),
        goal: flag_value(rest, "--goal")
            .unwrap_or(
                "Run the Entrance Explorer -> Doer -> Evaluator loop and expose it on the issue/status/comment panel.",
            )
            .to_string(),
        boundary: flag_value(rest, "--boundary")
            .unwrap_or(
                "Use the local Hive SQLite ledger, typed receipts, compact CLI output, and the local Panel surface.",
            )
            .to_string(),
        approach_space: csv_values_or_default(
            flag_value(rest, "--approach"),
            &[
                "Compile the natural-language goal into a typed candidate",
                "Execute only the admitted candidate",
                "Evaluate the evidence with keep/reject/block gates",
            ],
        ),
        eval_space: csv_values_or_default(
            flag_value(rest, "--eval"),
            &[
                "Explorer, Doer, and Evaluator each produce role receipts",
                "Admissions bind packets to policy gates",
                "Panel shows issue status, comments, evidence, verdict, and recovery actions",
            ],
        ),
        review_surface: flag_value(rest, "--review-surface")
            .unwrap_or("local-hive-panel")
            .to_string(),
        autonomy_level: flag_value(rest, "--autonomy")
            .unwrap_or("run-approved-candidates")
            .to_string(),
        runtime: flag_value(rest, "--runtime").unwrap_or("codex").to_string(),
    }
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

fn csv_values_or_default(value: Option<&str>, default: &[&str]) -> Vec<String> {
    let values = csv_values(value);
    if values.is_empty() {
        default.iter().map(|value| (*value).to_string()).collect()
    } else {
        values
    }
}

#[cfg(test)]
fn compact_issue_board(cards: &[IssueCard]) -> serde_json::Value {
    let columns = ISSUE_STATUSES
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

fn compact_issue_board_with_connector_status(
    services: &AppServices,
    cards: &[IssueCard],
) -> serde_json::Value {
    let issues = cards
        .iter()
        .map(|card| compact_issue_card_with_connector_status(services, card))
        .collect::<Vec<_>>();
    let columns = ISSUE_STATUSES
        .iter()
        .map(|status| {
            let column_issues = issues
                .iter()
                .filter(|issue| {
                    issue.pointer("/status").and_then(|value| value.as_str()) == Some(*status)
                })
                .cloned()
                .collect::<Vec<_>>();
            serde_json::json!({
                "status": status,
                "count": column_issues.len(),
                "issues": column_issues
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": "entrance.hive.issue_board.compact.v1",
        "total": cards.len(),
        "columns": columns,
        "connector_queue": compact_connector_queue(
            &services.hive.connector_registry(),
            &issues,
            None
        )
    })
}

#[cfg(test)]
fn compact_issue_detail(card: &IssueCard) -> serde_json::Value {
    let issue = compact_issue_card(card);
    let recent_evidence = compact_recent_evidence(card, 5);
    let recent_evidence_json = serde_json::Value::Array(recent_evidence.clone());
    let rounds = compact_issue_round_summary(&issue, &recent_evidence_json);
    let recovery = compact_issue_recovery_summary(&issue, &recent_evidence_json);
    serde_json::json!({
        "schema_version": "entrance.hive.issue.compact.v1",
        "issue": issue,
        "recent_comments": compact_recent_comments(card, 5),
        "recent_evidence": recent_evidence,
        "stages": compact_stage_rows(card),
        "rounds": rounds,
        "recovery": recovery
    })
}

fn compact_issue_detail_with_connector_status(
    services: &AppServices,
    card: &IssueCard,
) -> serde_json::Value {
    let issue = compact_issue_card_with_connector_status(services, card);
    let connector = issue
        .pointer("/connector")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let recent_evidence = compact_recent_evidence(card, 5);
    let recent_evidence_json = serde_json::Value::Array(recent_evidence.clone());
    let rounds = compact_issue_round_summary(&issue, &recent_evidence_json);
    let recovery = compact_issue_recovery_summary(&issue, &recent_evidence_json);
    serde_json::json!({
        "schema_version": "entrance.hive.issue.compact.v1",
        "issue": issue,
        "connector": connector,
        "recent_comments": compact_recent_comments(card, 5),
        "recent_evidence": recent_evidence,
        "stages": compact_stage_rows(card),
        "rounds": rounds,
        "recovery": recovery
    })
}

fn compact_loop_start_summary(issue_detail: &serde_json::Value) -> serde_json::Value {
    let raw_issue = issue_detail
        .pointer("/issue")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let connector = compact_loop_start_connector_summary(
        issue_detail
            .pointer("/connector")
            .or_else(|| raw_issue.pointer("/connector")),
    );
    let issue_id = raw_issue.pointer("/id").and_then(|value| value.as_i64());
    let loop_id = raw_issue
        .pointer("/loop_id")
        .and_then(|value| value.as_i64());
    let doctor = raw_issue
        .pointer("/doctor")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let trace = raw_issue
        .pointer("/trace")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let health = doctor
        .pointer("/health")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let runtime = doctor.pointer("/runtime").and_then(|value| value.as_str());
    let status = raw_issue
        .pointer("/status")
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown");
    let complete = health == "ok" && status == "Done";
    let recent_comments = compact_json_array_tail(issue_detail.pointer("/recent_comments"), 3);
    let recent_evidence = compact_json_array_tail(issue_detail.pointer("/recent_evidence"), 3);
    let stages = compact_json_array_tail(issue_detail.pointer("/stages"), 3);
    let recovery = compact_loop_start_recovery_summary(
        complete,
        issue_id,
        loop_id,
        runtime,
        &doctor,
        &recent_evidence,
    );
    let rounds = compact_issue_round_summary(&raw_issue, &recent_evidence);
    let retry_command = recovery
        .pointer("/retry_command")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    serde_json::json!({
        "schema_version": "entrance.hive.loop_start.compact.v1",
        "complete": complete,
        "loop_id": loop_id,
        "issue_id": issue_id,
        "status": status,
        "health": health,
        "decision": trace.pointer("/decision").and_then(|value| value.as_str()),
        "reason_code": trace.pointer("/reason_code").and_then(|value| value.as_str()),
        "runtime": runtime,
        "counts": {
            "workers": doctor.pointer("/counts/workers").and_then(|value| value.as_u64()),
            "worker_ok": doctor.pointer("/counts/worker_ok").and_then(|value| value.as_u64()),
            "worker_duration_ms": doctor.pointer("/counts/worker_duration_ms").and_then(|value| value.as_u64()),
            "receipt_required": doctor.pointer("/counts/receipt_required").and_then(|value| value.as_u64()),
            "receipt_missing": doctor.pointer("/counts/receipt_missing").and_then(|value| value.as_u64()),
            "audit_failed": doctor.pointer("/counts/audit_failed").and_then(|value| value.as_u64())
        },
        "issue": compact_loop_start_issue_summary(&raw_issue),
        "recent_comments": recent_comments,
        "recent_evidence": recent_evidence,
        "stages": stages,
        "rounds": rounds,
        "connector": connector,
        "recovery": recovery,
        "next_actions": doctor.pointer("/next_actions").cloned().unwrap_or_else(|| serde_json::json!([])),
        "commands": {
            "show": issue_id.map(|id| format!("entrance hive issue show {id} --compact")),
            "doctor": loop_id.map(|id| format!("entrance hive loop doctor {id}")),
            "board": "entrance hive issue list --compact",
            "retry": retry_command
        }
    })
}

fn compact_loop_demo_summary(
    services: &AppServices,
    loop_summary: &serde_json::Value,
) -> serde_json::Value {
    let complete = loop_summary
        .pointer("/complete")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let health = loop_summary
        .pointer("/health")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    serde_json::json!({
        "schema_version": LOOP_DEMO_COMPACT_SCHEMA_VERSION,
        "ready": complete && health == "ok",
        "demo": compact_loop_demo_context(services),
        "loop": loop_summary,
        "commands": {
            "board": "entrance hive issue list --compact",
            "show": loop_summary.pointer("/commands/show").cloned().unwrap_or(serde_json::Value::Null),
            "doctor": loop_summary.pointer("/commands/doctor").cloned().unwrap_or(serde_json::Value::Null),
            "retry": loop_summary.pointer("/commands/retry").cloned().unwrap_or(serde_json::Value::Null)
        },
        "panel": {
            "api_url": format!("http://127.0.0.1:{}", services.kernel.config.hive.http_port),
            "daemon": {
                "command": "entrance daemon http",
                "env": {
                    "ENTRANCE_APP_ROOT": services.kernel.root.display().to_string()
                }
            },
            "dev_server": {
                "command": format!("VITE_ENTRANCE_HTTP_URL=http://127.0.0.1:{} pnpm exec vite --config shell/gui/vite.config.ts --host 127.0.0.1 --port 1420", services.kernel.config.hive.http_port),
                "cwd": "entrance-src",
                "url": "http://127.0.0.1:1420/"
            }
        },
        "next_actions": if complete && health == "ok" {
            serde_json::json!([
                "entrance daemon http",
                "open the Panel Issue board",
                "entrance hive issue list --compact"
            ])
        } else {
            loop_summary
                .pointer("/next_actions")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]))
        }
    })
}

fn compact_loop_demo_context(services: &AppServices) -> serde_json::Value {
    serde_json::json!({
        "name": "Entrance local agent-loop MVP",
        "app_root": services.kernel.root.display().to_string(),
        "review_surface": "local-hive-panel",
        "loop": "Explorer -> Doer -> Evaluator",
        "surface": "issue/status/comment"
    })
}

fn compact_loop_start_recovery_summary(
    complete: bool,
    issue_id: Option<i64>,
    loop_id: Option<i64>,
    runtime: Option<&str>,
    doctor: &serde_json::Value,
    recent_evidence: &serde_json::Value,
) -> serde_json::Value {
    let next_actions = doctor
        .pointer("/next_actions")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let retry_command = first_json_string_containing(&next_actions, "issue retry-run").or_else(|| {
        issue_id.map(|id| {
            if runtime == Some("codex") {
                format!(
                    "entrance hive issue retry-run {id} --body <note> --runtime codex --worker-attempts 2 --compact"
                )
            } else {
                format!("entrance hive issue retry-run {id} --body <note> --compact")
            }
        })
    });
    let primary_action = next_actions
        .as_array()
        .and_then(|actions| actions.first())
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    serde_json::json!({
        "required": !complete,
        "primary_action": primary_action,
        "retry_command": retry_command,
        "doctor_command": loop_id.map(|id| format!("entrance hive loop doctor {id}")),
        "evidence_command": loop_id.map(|id| format!("entrance hive loop evidence {id}")),
        "audit_command": loop_id.map(|id| format!("entrance hive loop audit {id} --compact")),
        "failed_checks": doctor.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "missing_receipts": doctor.pointer("/missing_receipts").cloned().unwrap_or_else(|| serde_json::json!([])),
        "worker_failures": doctor.pointer("/worker_failures").cloned().unwrap_or_else(|| serde_json::json!([])),
        "failed_workers": compact_loop_start_failed_workers(recent_evidence)
    })
}

fn first_json_string_containing(value: &serde_json::Value, needle: &str) -> Option<String> {
    value.as_array().and_then(|values| {
        values
            .iter()
            .filter_map(|value| value.as_str())
            .find(|value| value.contains(needle))
            .map(ToOwned::to_owned)
    })
}

fn compact_loop_start_failed_workers(recent_evidence: &serde_json::Value) -> serde_json::Value {
    let Some(rows) = recent_evidence.as_array() else {
        return serde_json::json!([]);
    };
    serde_json::Value::Array(
        rows.iter()
            .filter(|row| {
                row.pointer("/worker/ok").and_then(|value| value.as_bool()) == Some(false)
                    || row
                        .pointer("/worker/receipt_ok")
                        .and_then(|value| value.as_bool())
                        == Some(false)
                    || row
                        .pointer("/worker/timed_out")
                        .and_then(|value| value.as_bool())
                        == Some(true)
                    || row
                        .pointer("/worker/retry_exhausted")
                        .and_then(|value| value.as_bool())
                        == Some(true)
                    || row
                        .pointer("/worker/receipt_errors")
                        .and_then(|value| value.as_array())
                        .map(|values| !values.is_empty())
                        .unwrap_or(false)
            })
            .map(|row| {
                serde_json::json!({
                    "role": row.pointer("/role").and_then(|value| value.as_str()),
                    "kind": row.pointer("/kind").and_then(|value| value.as_str()),
                    "worker_kind": row.pointer("/worker/kind").and_then(|value| value.as_str()),
                    "ok": row.pointer("/worker/ok").and_then(|value| value.as_bool()),
                    "receipt_ok": row.pointer("/worker/receipt_ok").and_then(|value| value.as_bool()),
                    "timed_out": row.pointer("/worker/timed_out").and_then(|value| value.as_bool()),
                    "retry_exhausted": row.pointer("/worker/retry_exhausted").and_then(|value| value.as_bool()),
                    "attempt_count": row.pointer("/worker/attempt_count").and_then(|value| value.as_u64()),
                    "max_attempts": row.pointer("/worker/max_attempts").and_then(|value| value.as_u64()),
                    "duration_ms": row.pointer("/worker/duration_ms").and_then(|value| value.as_u64()),
                    "receipt_errors": row.pointer("/worker/receipt_errors").cloned().unwrap_or_else(|| serde_json::json!([]))
                })
            })
            .collect(),
    )
}

fn compact_issue_recovery_summary(
    issue: &serde_json::Value,
    recent_evidence: &serde_json::Value,
) -> serde_json::Value {
    let status = issue
        .pointer("/status")
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown");
    let health = issue
        .pointer("/doctor/health")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let complete = health == "ok" && status == "Done";
    compact_loop_start_recovery_summary(
        complete,
        issue.pointer("/id").and_then(|value| value.as_i64()),
        issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        issue
            .pointer("/doctor/runtime")
            .and_then(|value| value.as_str()),
        issue.pointer("/doctor").unwrap_or(&serde_json::Value::Null),
        recent_evidence,
    )
}

fn compact_issue_round_summary(
    issue: &serde_json::Value,
    recent_evidence: &serde_json::Value,
) -> serde_json::Value {
    let current_round = issue
        .pointer("/trace/round")
        .and_then(|value| value.as_i64());
    if let Some(rounds) = issue
        .pointer("/trace/rounds")
        .and_then(|value| value.as_array())
    {
        let failed_rounds = rounds
            .iter()
            .filter(|round| {
                round.pointer("/status").and_then(|value| value.as_str()) != Some("kept")
                    && (round
                        .pointer("/rejected_count")
                        .and_then(|value| value.as_u64())
                        .unwrap_or_default()
                        > 0
                        || round
                            .pointer("/receipt_missing")
                            .and_then(|value| value.as_u64())
                            .unwrap_or_default()
                            > 0
                        || round
                            .pointer("/timeouts")
                            .and_then(|value| value.as_u64())
                            .unwrap_or_default()
                            > 0
                        || round
                            .pointer("/retry_exhausted")
                            .and_then(|value| value.as_u64())
                            .unwrap_or_default()
                            > 0)
            })
            .filter_map(|round| round.pointer("/round").and_then(|value| value.as_i64()))
            .collect::<Vec<_>>();
        let recovered_from_rounds = match (
            issue.pointer("/status").and_then(|value| value.as_str()),
            current_round,
        ) {
            (Some("Done"), Some(current_round)) => failed_rounds
                .iter()
                .filter(|round| **round < current_round)
                .copied()
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        return serde_json::json!({
            "current": current_round,
            "rounds": rounds,
            "evidence_rounds": rounds
                .iter()
                .filter_map(|round| round.pointer("/round").and_then(|value| value.as_i64()))
                .collect::<Vec<_>>(),
            "failed_rounds": failed_rounds,
            "recovered_from_rounds": recovered_from_rounds
        });
    }
    let evidence_rounds = compact_evidence_rounds(recent_evidence, |_| true);
    let failed_rounds = compact_evidence_rounds(recent_evidence, compact_evidence_row_failed);
    let recovered_from_rounds = match (
        issue.pointer("/status").and_then(|value| value.as_str()),
        current_round,
    ) {
        (Some("Done"), Some(current_round)) => failed_rounds
            .iter()
            .filter(|round| **round < current_round)
            .copied()
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    serde_json::json!({
        "current": current_round,
        "evidence_rounds": evidence_rounds,
        "failed_rounds": failed_rounds,
        "recovered_from_rounds": recovered_from_rounds
    })
}

fn compact_evidence_rounds<F>(recent_evidence: &serde_json::Value, predicate: F) -> Vec<i64>
where
    F: Fn(&serde_json::Value) -> bool,
{
    let mut rounds = recent_evidence
        .as_array()
        .into_iter()
        .flat_map(|rows| rows.iter())
        .filter(|row| predicate(row))
        .filter_map(|row| row.pointer("/round").and_then(|value| value.as_i64()))
        .collect::<Vec<_>>();
    rounds.sort_unstable();
    rounds.dedup();
    rounds
}

fn compact_evidence_row_failed(row: &serde_json::Value) -> bool {
    row.pointer("/admission").and_then(|value| value.as_str()) == Some("rejected")
        || row.pointer("/worker/ok").and_then(|value| value.as_bool()) == Some(false)
        || row
            .pointer("/worker/receipt_ok")
            .and_then(|value| value.as_bool())
            == Some(false)
        || row
            .pointer("/worker/timed_out")
            .and_then(|value| value.as_bool())
            == Some(true)
        || row
            .pointer("/worker/retry_exhausted")
            .and_then(|value| value.as_bool())
            == Some(true)
        || row
            .pointer("/worker/receipt_errors")
            .and_then(|value| value.as_array())
            .map(|values| !values.is_empty())
            .unwrap_or(false)
}

fn compact_loop_start_issue_summary(issue: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": issue.pointer("/id").and_then(|value| value.as_i64()),
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        "title": issue.pointer("/title").and_then(|value| value.as_str()),
        "status": issue.pointer("/status").and_then(|value| value.as_str()),
        "summary": issue.pointer("/summary").and_then(|value| value.as_str()),
        "comment_count": issue.pointer("/comment_count").and_then(|value| value.as_u64()),
        "latest_comment": issue.pointer("/latest_comment").cloned().unwrap_or(serde_json::Value::Null)
    })
}

fn compact_loop_start_connector_summary(
    connector: Option<&serde_json::Value>,
) -> serde_json::Value {
    let Some(connector) = connector else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "provider": connector.pointer("/provider").and_then(|value| value.as_str()),
        "current": connector.pointer("/current").and_then(|value| value.as_bool()),
        "publish_required": connector.pointer("/publish_required").and_then(|value| value.as_bool()),
        "reason": connector.pointer("/reason").and_then(|value| value.as_str()),
        "failed_checks": connector.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "publish_command": connector.pointer("/publish_command").and_then(|value| value.as_str()),
        "readback_command": connector.pointer("/readback_command").and_then(|value| value.as_str()),
        "admit_command": connector.pointer("/admit_command").and_then(|value| value.as_str()),
        "roundtrip_command": connector.pointer("/roundtrip_command").and_then(|value| value.as_str())
    })
}

fn compact_json_array_tail(value: Option<&serde_json::Value>, limit: usize) -> serde_json::Value {
    let Some(values) = value.and_then(|value| value.as_array()) else {
        return serde_json::json!([]);
    };
    let start = values.len().saturating_sub(limit);
    serde_json::Value::Array(values[start..].to_vec())
}

fn compact_policy_registry(report: &PolicyRegistryReport) -> serde_json::Value {
    let connector_gate = report
        .gates
        .iter()
        .find(|gate| gate.name == CONNECTOR_MIRROR_RECEIPT_GATE)
        .map(compact_policy_gate);
    serde_json::json!({
        "schema_version": "entrance.hive.policy_registry.compact.v1",
        "source_schema_version": report.schema_version.as_str(),
        "gate_count": report.gates.len(),
        "gates": report.gates.iter().map(compact_policy_gate).collect::<Vec<_>>(),
        "connector_mirror_gate": connector_gate,
        "connector": {
            "admission": {
                "schema_version": report.connector.admission.schema_version.as_str(),
                "gate": report.connector.admission.gate.as_str(),
                "route_to": report.connector.admission.route_to.as_str(),
                "expected_object_kind": report.connector.admission.expected_object_kind.as_str(),
                "check": report.connector.admission.check.as_str(),
                "required_receipts": report.connector.admission.required_receipts.iter().map(String::as_str).collect::<Vec<_>>(),
                "required_checks": report.connector.admission.required_checks.iter().map(String::as_str).collect::<Vec<_>>(),
                "check_registry": report.connector.admission.check_registry.iter().map(compact_connector_admission_check_spec).collect::<Vec<_>>(),
                "dry_run_command": report.connector.admission.dry_run_command.as_str()
            },
            "retry": report.connector.retry.iter().map(compact_connector_retry_policy).collect::<Vec<_>>()
        },
        "runtime": {
            "supported": report.runtime.supported.iter().map(|runtime| serde_json::json!({
                "name": runtime.name.as_str(),
                "mode": runtime.mode.as_str(),
                "filesystem": runtime.sandbox.filesystem.as_str(),
                "network": runtime.sandbox.network.as_str()
            })).collect::<Vec<_>>(),
            "worker": {
                "default_timeout_secs": report.runtime.worker.default_timeout_secs,
                "max_timeout_secs": report.runtime.worker.max_timeout_secs,
                "default_attempts": report.runtime.worker.default_attempts,
                "max_attempts": report.runtime.worker.max_attempts
            }
        }
    })
}

fn compact_connector_retry_policy(policy: &ConnectorRetryPolicySpec) -> serde_json::Value {
    serde_json::json!({
        "schema_version": policy.schema_version.as_str(),
        "provider": policy.provider.as_str(),
        "transport": policy.transport.as_str(),
        "applies_to": policy.applies_to.iter().map(String::as_str).collect::<Vec<_>>(),
        "max_attempts": policy.max_attempts,
        "base_backoff_ms": policy.base_backoff_ms,
        "backoff_strategy": policy.backoff_strategy.as_str(),
        "retryable_http_statuses": policy.retryable_http_statuses.clone(),
        "rate_limit_http_statuses": policy.rate_limit_http_statuses.clone(),
        "rate_limit_headers": policy.rate_limit_headers.iter().map(String::as_str).collect::<Vec<_>>(),
        "no_immediate_retry_checks": policy.no_immediate_retry_checks.iter().map(String::as_str).collect::<Vec<_>>()
    })
}

fn compact_connector_admission_check_spec(spec: &ConnectorAdmissionCheckSpec) -> serde_json::Value {
    serde_json::json!({
        "name": spec.name.as_str(),
        "severity": spec.severity.as_str(),
        "owner": spec.owner.as_str(),
        "required_evidence": spec.required_evidence.iter().map(String::as_str).collect::<Vec<_>>(),
        "summary": spec.summary.as_str()
    })
}

fn connector_remote_retry_policy(provider_name: &str) -> ConnectorRetryPolicySpec {
    connector_retry_policy_for_provider(provider_name).unwrap_or_else(|| ConnectorRetryPolicySpec {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        provider: provider_name.to_string(),
        transport: "local".to_string(),
        applies_to: vec!["remote_issue_surface".to_string()],
        max_attempts: 1,
        base_backoff_ms: 0,
        backoff_strategy: "none".to_string(),
        retryable_http_statuses: Vec::new(),
        rate_limit_http_statuses: vec![429],
        rate_limit_headers: vec!["retry-after".to_string()],
        no_immediate_retry_checks: vec!["remote_rate_limited".to_string()],
    })
}

fn github_retry_policy() -> ConnectorRetryPolicySpec {
    connector_remote_retry_policy("github")
}

fn linear_retry_policy() -> ConnectorRetryPolicySpec {
    connector_remote_retry_policy("linear")
}

fn connector_retry_max_attempts(policy: &ConnectorRetryPolicySpec) -> usize {
    policy.max_attempts.max(1) as usize
}

fn connector_retry_backoff_ms(policy: &ConnectorRetryPolicySpec, attempt_number: usize) -> u64 {
    policy.base_backoff_ms.saturating_mul(attempt_number as u64)
}

fn compact_connector_registry(report: &ConnectorRegistryReport) -> serde_json::Value {
    let active_count = report
        .providers
        .iter()
        .filter(|provider| provider.status == "active")
        .count();
    serde_json::json!({
        "schema_version": "entrance.hive.connector_registry.compact.v1",
        "source_schema_version": report.schema_version.as_str(),
        "active_count": active_count,
        "provider_count": report.providers.len(),
        "providers": report.providers.iter().map(compact_connector_provider).collect::<Vec<_>>(),
        "provider_admissions": report
            .provider_admissions
            .iter()
            .map(compact_connector_provider_admission)
            .collect::<Vec<_>>(),
        "admission": {
            "gate": report.admission.gate.as_str(),
            "route_to": report.admission.route_to.as_str(),
            "expected_object_kind": report.admission.expected_object_kind.as_str(),
            "check": report.admission.check.as_str(),
            "required_receipts": report.admission.required_receipts.iter().map(String::as_str).collect::<Vec<_>>(),
            "required_checks": report.admission.required_checks.iter().map(String::as_str).collect::<Vec<_>>(),
            "check_registry": report.admission.check_registry.iter().map(compact_connector_admission_check_spec).collect::<Vec<_>>(),
            "dry_run_command": report.admission.dry_run_command.as_str()
        }
    })
}

fn compact_connector_provider(provider: &ConnectorProviderSpec) -> serde_json::Value {
    let mut capabilities = Vec::new();
    if provider.supports_status {
        capabilities.push("status");
    }
    if provider.supports_publish {
        capabilities.push("publish");
    }
    if provider.supports_readback {
        capabilities.push("readback");
    }
    if provider.supports_admission {
        capabilities.push("admission");
    }
    serde_json::json!({
        "name": provider.name.as_str(),
        "display_name": provider.display_name.as_str(),
        "status": provider.status.as_str(),
        "mode": provider.mode.as_str(),
        "configured": provider.configured,
        "auth_required": provider.auth_required,
        "auth_env": provider.auth_env.iter().map(String::as_str).collect::<Vec<_>>(),
        "review_surface_prefixes": provider.review_surface_prefixes.iter().map(String::as_str).collect::<Vec<_>>(),
        "capabilities": capabilities,
        "storage": provider.storage.as_str(),
        "writer_adapter": compact_connector_writer_adapter(&provider.name, Some(provider)),
        "remote_contract": compact_connector_remote_contract(Some(provider)),
        "notes": compact_text(&provider.notes, 180)
    })
}

fn compact_connector_provider_admission(
    admission: &ConnectorProviderAdmissionSpec,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": admission.schema_version.as_str(),
        "provider": admission.provider.as_str(),
        "status": admission.status.as_str(),
        "gate": admission.gate.as_str(),
        "route_to": admission.route_to.as_deref(),
        "expected_object_kind": admission.expected_object_kind.as_str(),
        "check": admission.check.as_str(),
        "required_receipts": admission.required_receipts.iter().map(String::as_str).collect::<Vec<_>>(),
        "required_checks": admission.required_checks.iter().map(String::as_str).collect::<Vec<_>>(),
        "check_registry": admission.check_registry.iter().map(compact_connector_admission_check_spec).collect::<Vec<_>>(),
        "blockers": admission.blockers.iter().map(String::as_str).collect::<Vec<_>>(),
        "dry_run_command": admission.dry_run_command.as_str()
    })
}

fn compact_issue_connector_admission_preview(report: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_connector_admission_preview.compact.v1",
        "source_schema_version": report.pointer("/schema_version").and_then(|value| value.as_str()),
        "issue_id": report.pointer("/issue/id").and_then(|value| value.as_i64()),
        "loop_id": report.pointer("/issue/loop_id").and_then(|value| value.as_i64()),
        "provider": report.pointer("/provider/name").and_then(|value| value.as_str())
            .or_else(|| report.pointer("/provider_name").and_then(|value| value.as_str())),
        "provider_status": report.pointer("/provider/status").and_then(|value| value.as_str()),
        "configured": report.pointer("/provider/configured").and_then(|value| value.as_bool()),
        "provider_admission_status": report.pointer("/provider_admission/status").and_then(|value| value.as_str()),
        "provider_admission_blockers": report.pointer("/provider_admission/blockers").and_then(|value| value.as_array()).cloned().unwrap_or_default(),
        "review_surface": report.pointer("/review_surface").and_then(|value| value.as_str()),
        "mirror_current": report.pointer("/connector/current").and_then(|value| value.as_bool()),
        "publish_required": report.pointer("/connector/publish_required").and_then(|value| value.as_bool()),
        "reason": report.pointer("/connector/reason").and_then(|value| value.as_str()),
        "adapter": report.pointer("/adapter").cloned().unwrap_or_else(|| serde_json::json!({})),
        "remote_contract": report.pointer("/remote_contract").cloned().unwrap_or_else(|| serde_json::json!(null)),
        "remote_target": report.pointer("/remote_target").cloned().unwrap_or_else(|| serde_json::json!(null)),
        "writer_blockers": report.pointer("/writer_blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "checks": report.pointer("/checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "admissible": report.pointer("/decision/admissible").and_then(|value| value.as_bool()),
        "route_to": report.pointer("/decision/route_to").and_then(|value| value.as_str()),
        "blockers": report.pointer("/decision/blockers").and_then(|value| value.as_array()).cloned().unwrap_or_default(),
        "gate": report.pointer("/policy/gate").and_then(|value| value.as_str()),
        "expected_object_kind": report.pointer("/policy/expected_object_kind").and_then(|value| value.as_str()),
        "required_checks": report.pointer("/policy/required_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "check_registry": report.pointer("/policy/check_registry").cloned().unwrap_or_else(|| serde_json::json!([])),
        "commands": report.pointer("/commands")
    })
}

fn compact_policy_gate(gate: &PolicyGateSpec) -> serde_json::Value {
    serde_json::json!({
        "name": gate.name.as_str(),
        "expected_object_kind": gate.expected_object_kind.as_deref(),
        "check": gate.check.as_str(),
        "required_receipts": gate.required_receipts.iter().map(String::as_str).collect::<Vec<_>>(),
        "description": compact_text(&gate.description, 180)
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
    let registry = services.hive.connector_registry();
    let provider =
        connector_provider_for_surface(&registry, &mirror.provider, &mirror.review_surface);
    let path =
        resolve_issue_mirror_path_for_provider(&services.kernel.root, &mirror, provider, out_path);
    let receipt_path = mirror_receipt_path(&path);
    let digest = write_issue_mirror_file(&mirror, &path)?;
    write_issue_mirror_receipt(&mirror, &path, &receipt_path, &digest, provider)?;
    Ok(compact_issue_mirror_sync(
        &mirror,
        &path,
        &receipt_path,
        &digest,
        provider,
    ))
}

pub(crate) fn publish_issue_mirror_to_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let mirror = services.hive.issue_mirror(issue_id)?;
    let registry = services.hive.connector_registry();
    let provider =
        connector_provider_for_surface(&registry, &mirror.provider, &mirror.review_surface);
    let blockers =
        connector_issue_writer_blockers(provider, &mirror.review_surface, &mirror.external_key);
    let target_path =
        resolve_issue_mirror_path_for_provider(&services.kernel.root, &mirror, provider, path);
    let receipt_path = mirror_receipt_path(&target_path);
    if !blockers.is_empty() {
        return Ok(compact_issue_mirror_publish_blocked(
            &mirror,
            &target_path,
            &receipt_path,
            provider,
            &blockers,
        ));
    }
    if provider
        .map(connector_provider_is_github_remote)
        .unwrap_or(false)
    {
        return publish_issue_mirror_to_github(
            &mirror,
            provider.unwrap(),
            &target_path,
            &receipt_path,
        );
    }
    if provider
        .map(connector_provider_is_linear_remote)
        .unwrap_or(false)
    {
        return publish_issue_mirror_to_linear(
            &mirror,
            provider.unwrap(),
            &target_path,
            &receipt_path,
        );
    }
    let sync = sync_issue_mirror_to_file(services, issue_id, path)?;
    let mut publish = compact_issue_mirror_publish(&sync);
    if let Some(object) = publish.as_object_mut() {
        object.insert(
            "adapter".to_string(),
            compact_connector_writer_adapter(&mirror.provider, provider),
        );
        object.insert(
            "write_receipt".to_string(),
            connector_write_receipt(&mirror, &sync, provider),
        );
        object.insert(
            "remote_write_receipt".to_string(),
            sync.pointer("/remote_write_receipt")
                .cloned()
                .unwrap_or_else(|| serde_json::json!(null)),
        );
    }
    Ok(publish)
}

fn publish_issue_mirror_to_github(
    mirror: &IssueMirrorReport,
    provider: &ConnectorProviderSpec,
    target_path: &Path,
    receipt_path: &Path,
) -> Result<serde_json::Value> {
    let payload = mirror_payload(mirror)?;
    let digest = digest_bytes(&payload);
    let remote_target =
        connector_remote_target(Some(provider), &mirror.review_surface, &mirror.external_key);
    let issue = connector_remote_write_issue_from_mirror(mirror, &digest);
    let write_plan = connector_remote_write_plan(Some(provider), &issue, &remote_target, &[]);
    if write_plan
        .pointer("/executable")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        let blockers = write_plan
            .pointer("/blocked_by")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        return Ok(compact_issue_mirror_publish_blocked(
            mirror,
            target_path,
            receipt_path,
            Some(provider),
            &blockers,
        ));
    }

    let remote_write_execution = execute_github_remote_write_plan(provider, &write_plan);
    if remote_write_execution
        .pointer("/success")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        write_issue_mirror_receipt_with_remote_write_failure(
            mirror,
            target_path,
            receipt_path,
            &digest,
            provider,
            &remote_write_execution,
        )?;
        return Ok(compact_issue_mirror_remote_publish_failed(
            mirror,
            target_path,
            receipt_path,
            provider,
            &write_plan,
            &remote_write_execution,
        ));
    }

    let remote_write_receipt = connector_remote_write_receipt_from_execution(
        mirror,
        target_path,
        &digest,
        provider,
        &remote_target,
        &remote_write_execution,
    );
    write_issue_mirror_receipt_with_remote_execution(
        mirror,
        target_path,
        receipt_path,
        &digest,
        provider,
        &remote_write_receipt,
        &remote_write_execution,
    )?;

    let mut sync =
        compact_issue_mirror_sync(mirror, target_path, receipt_path, &digest, Some(provider));
    if let Some(object) = sync.as_object_mut() {
        object.insert(
            "remote_write_receipt".to_string(),
            remote_write_receipt.clone(),
        );
        object.insert(
            "remote_write_execution".to_string(),
            remote_write_execution.clone(),
        );
    }

    let mut publish = compact_issue_mirror_publish(&sync);
    if let Some(object) = publish.as_object_mut() {
        object.insert(
            "adapter".to_string(),
            compact_connector_writer_adapter(&mirror.provider, Some(provider)),
        );
        object.insert(
            "write_receipt".to_string(),
            connector_write_receipt(mirror, &sync, Some(provider)),
        );
        object.insert("remote_write_receipt".to_string(), remote_write_receipt);
        object.insert("remote_write_execution".to_string(), remote_write_execution);
    }
    Ok(publish)
}

fn publish_issue_mirror_to_linear(
    mirror: &IssueMirrorReport,
    provider: &ConnectorProviderSpec,
    target_path: &Path,
    receipt_path: &Path,
) -> Result<serde_json::Value> {
    let payload = mirror_payload(mirror)?;
    let digest = digest_bytes(&payload);
    let remote_target =
        connector_remote_target(Some(provider), &mirror.review_surface, &mirror.external_key);
    let issue = connector_remote_write_issue_from_mirror(mirror, &digest);
    let write_plan = connector_remote_write_plan(Some(provider), &issue, &remote_target, &[]);
    if write_plan
        .pointer("/executable")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        let blockers = write_plan
            .pointer("/blocked_by")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        return Ok(compact_issue_mirror_publish_blocked(
            mirror,
            target_path,
            receipt_path,
            Some(provider),
            &blockers,
        ));
    }

    let remote_write_execution = execute_linear_remote_write_plan(provider, &write_plan);
    if remote_write_execution
        .pointer("/success")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        write_issue_mirror_receipt_with_remote_write_failure(
            mirror,
            target_path,
            receipt_path,
            &digest,
            provider,
            &remote_write_execution,
        )?;
        return Ok(compact_issue_mirror_remote_publish_failed(
            mirror,
            target_path,
            receipt_path,
            provider,
            &write_plan,
            &remote_write_execution,
        ));
    }

    let remote_write_receipt = connector_remote_write_receipt_from_execution(
        mirror,
        target_path,
        &digest,
        provider,
        &remote_target,
        &remote_write_execution,
    );
    write_issue_mirror_receipt_with_remote_execution(
        mirror,
        target_path,
        receipt_path,
        &digest,
        provider,
        &remote_write_receipt,
        &remote_write_execution,
    )?;

    let mut sync =
        compact_issue_mirror_sync(mirror, target_path, receipt_path, &digest, Some(provider));
    if let Some(object) = sync.as_object_mut() {
        object.insert(
            "remote_write_receipt".to_string(),
            remote_write_receipt.clone(),
        );
        object.insert(
            "remote_write_execution".to_string(),
            remote_write_execution.clone(),
        );
    }

    let mut publish = compact_issue_mirror_publish(&sync);
    if let Some(object) = publish.as_object_mut() {
        object.insert(
            "adapter".to_string(),
            compact_connector_writer_adapter(&mirror.provider, Some(provider)),
        );
        object.insert(
            "write_receipt".to_string(),
            connector_write_receipt(mirror, &sync, Some(provider)),
        );
        object.insert("remote_write_receipt".to_string(), remote_write_receipt);
        object.insert("remote_write_execution".to_string(), remote_write_execution);
    }
    Ok(publish)
}

pub(crate) fn issue_mirror_status(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let readback = readback_issue_mirror_file(services, issue_id, path, false)?;
    Ok(compact_issue_mirror_status(&readback))
}

pub(crate) fn issue_connector_admission_preview(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let mirror = services.hive.issue_mirror(issue_id)?;
    let registry = services.hive.connector_registry();
    let provider =
        connector_provider_for_surface(&registry, &mirror.provider, &mirror.review_surface);
    let provider_admission = connector_provider_admission_for_surface(
        &registry,
        &mirror.provider,
        &mirror.review_surface,
    );
    let status = issue_mirror_status(services, issue_id, path)?;
    let mirror_current = status
        .pointer("/current")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let writer_blockers =
        connector_issue_writer_blockers(provider, &mirror.review_surface, &mirror.external_key);
    let remote_contract = compact_connector_remote_contract(provider);
    let remote_target =
        connector_remote_target(provider, &mirror.review_surface, &mirror.external_key);
    let remote_contract_required = !remote_contract.is_null();
    let mut blockers = Vec::new();
    if provider.is_none() {
        blockers.push("unsupported_provider".to_string());
    }
    if let Some(provider_admission) = provider_admission {
        blockers.extend(provider_admission.blockers.iter().cloned());
    }
    if remote_contract_required {
        blockers.extend(writer_blockers.iter().cloned());
        if !provider
            .map(|provider| provider.supports_readback)
            .unwrap_or(false)
        {
            blockers.push("readback_not_supported".to_string());
        }
    }
    if !mirror_current {
        blockers.push("mirror_not_current".to_string());
    }
    let failed_checks = status
        .pointer("/failed_checks")
        .and_then(|value| value.as_array())
        .map(|checks| {
            checks
                .iter()
                .filter_map(|value| value.as_str())
                .map(|check| format!("check:{check}"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    blockers.extend(failed_checks);
    let checks = connector_admission_preview_checks(
        provider,
        provider_admission,
        &status,
        &writer_blockers,
        remote_contract_required,
        &remote_target,
        &remote_contract,
        &registry.admission.check_registry,
    );
    if connector_admission_check_failed(&checks, "retry_policy_bound") {
        blockers.push("retry_policy_bound".to_string());
    }
    blockers.sort();
    blockers.dedup();
    let admissible = blockers.is_empty();
    let route_to = if admissible {
        provider_admission
            .and_then(|admission| admission.route_to.as_deref())
            .unwrap_or("external_issue_surface")
    } else {
        ""
    };
    Ok(serde_json::json!({
        "schema_version": ISSUE_CONNECTOR_ADMISSION_PREVIEW_SCHEMA_VERSION,
        "issue": {
            "id": mirror.issue.id,
            "loop_id": mirror.issue.loop_id,
            "status": mirror.issue.status,
            "title": mirror.issue.title
        },
        "provider": provider.map(compact_connector_provider),
        "provider_admission": provider_admission.map(compact_connector_provider_admission),
        "provider_name": mirror.provider,
        "review_surface": mirror.review_surface,
        "external_key": mirror.external_key,
        "connector": status,
        "adapter": compact_connector_writer_adapter(&mirror.provider, provider),
        "remote_contract": remote_contract,
        "remote_target": remote_target,
        "writer_blockers": writer_blockers,
        "checks": checks,
        "policy": {
            "schema_version": registry.admission.schema_version,
            "gate": registry.admission.gate,
            "route_to": registry.admission.route_to,
            "expected_object_kind": registry.admission.expected_object_kind,
            "check": registry.admission.check,
            "required_receipts": registry.admission.required_receipts,
            "required_checks": registry.admission.required_checks,
            "check_registry": registry.admission.check_registry
        },
        "decision": {
            "admissible": admissible,
            "route_to": if admissible { Some(route_to) } else { None },
            "blockers": blockers
        },
        "commands": {
            "registry": "entrance hive connector registry --compact",
            "status": format!("entrance hive issue mirror-status {issue_id} --compact"),
            "publish": format!("entrance hive issue mirror-publish {issue_id} --compact"),
            "readback": format!("entrance hive issue mirror-readback {issue_id} --record --compact"),
            "admit": format!("entrance hive issue mirror-admit {issue_id} --record --compact"),
            "roundtrip": format!("entrance hive issue mirror-roundtrip {issue_id} --compact")
        }
    }))
}

pub(crate) fn verify_issue_mirror_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let mirror = services.hive.issue_mirror(issue_id)?;
    let registry = services.hive.connector_registry();
    let provider =
        connector_provider_for_surface(&registry, &mirror.provider, &mirror.review_surface);
    let path =
        resolve_issue_mirror_path_for_provider(&services.kernel.root, &mirror, provider, path);
    let receipt_path = mirror_receipt_path(&path);
    let expected_digest = digest_bytes(&mirror_payload(&mirror)?);
    let actual_digest = read_digest(&path)?;
    let receipt = read_receipt(&receipt_path)?;
    Ok(compact_issue_mirror_verify(
        &mirror,
        &path,
        &receipt_path,
        &expected_digest,
        actual_digest.as_ref(),
        receipt.as_ref(),
    ))
}

pub(crate) fn readback_issue_mirror_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
    record: bool,
) -> Result<serde_json::Value> {
    let mirror = services.hive.issue_mirror(issue_id)?;
    let registry = services.hive.connector_registry();
    let provider =
        connector_provider_for_surface(&registry, &mirror.provider, &mirror.review_surface);
    let path =
        resolve_issue_mirror_path_for_provider(&services.kernel.root, &mirror, provider, path);
    let receipt_path = mirror_receipt_path(&path);
    let expected_digest = digest_bytes(&mirror_payload(&mirror)?);
    if let Some(provider) =
        provider.filter(|provider| connector_provider_is_github_remote(provider))
    {
        let receipt = read_receipt(&receipt_path)?;
        let mut readback = compact_github_issue_mirror_readback(
            &mirror,
            &path,
            &receipt_path,
            &expected_digest,
            receipt.as_ref(),
            provider,
        );
        if record {
            let recorded = record_issue_mirror_readback(services, &readback)?;
            if let Some(object) = readback.as_object_mut() {
                object.insert("recorded".to_string(), recorded);
            }
        }
        return Ok(readback);
    }
    if let Some(provider) =
        provider.filter(|provider| connector_provider_is_linear_remote(provider))
    {
        let receipt = read_receipt(&receipt_path)?;
        let mut readback = compact_linear_issue_mirror_readback(
            &mirror,
            &path,
            &receipt_path,
            &expected_digest,
            receipt.as_ref(),
            provider,
        );
        if record {
            let recorded = record_issue_mirror_readback(services, &readback)?;
            if let Some(object) = readback.as_object_mut() {
                object.insert("recorded".to_string(), recorded);
            }
        }
        return Ok(readback);
    }
    let remote_payload = read_payload_with_digest(&path)?;
    let receipt = read_receipt(&receipt_path)?;
    let verify = compact_issue_mirror_verify(
        &mirror,
        &path,
        &receipt_path,
        &expected_digest,
        remote_payload.as_ref().map(|(_, digest)| digest),
        receipt.as_ref(),
    );
    let mut parse_error = None;
    let remote_mirror = match remote_payload.as_ref() {
        Some((bytes, _)) => match serde_json::from_slice::<IssueMirrorReport>(bytes) {
            Ok(report) => Some(report),
            Err(error) => {
                parse_error = Some(error.to_string());
                None
            }
        },
        None => None,
    };

    let mut readback = compact_issue_mirror_readback(
        &mirror,
        &path,
        &receipt_path,
        &expected_digest,
        remote_payload.as_ref().map(|(_, digest)| digest),
        receipt.as_ref(),
        remote_mirror.as_ref(),
        parse_error.as_deref(),
        &verify,
        provider,
    );
    if record {
        let recorded = record_issue_mirror_readback(services, &readback)?;
        if let Some(object) = readback.as_object_mut() {
            object.insert("recorded".to_string(), recorded);
        }
    }
    Ok(readback)
}

pub(crate) fn audit_issue_mirror_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let verify = verify_issue_mirror_file(services, issue_id, path)?;
    Ok(compact_issue_mirror_audit(&verify))
}

pub(crate) fn admit_issue_mirror_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
    record: bool,
) -> Result<serde_json::Value> {
    let audit = audit_issue_mirror_file(services, issue_id, path)?;
    let mut admission = compact_issue_mirror_admission(&audit);
    let provider_preview = issue_connector_admission_preview(services, issue_id, path)?;
    apply_connector_provider_admission_preview(&mut admission, &provider_preview);
    if record {
        let recorded = record_issue_mirror_admission(services, &admission)?;
        if let Some(object) = admission.as_object_mut() {
            object.insert("recorded".to_string(), recorded);
        }
    }
    Ok(admission)
}

pub(crate) fn roundtrip_issue_mirror_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
    record: bool,
) -> Result<serde_json::Value> {
    let initial_publish = publish_issue_mirror_to_file(services, issue_id, path)?;
    let mut stages = vec![issue_mirror_roundtrip_stage(
        "publish_initial",
        "Publish current Hive mirror to the connector surface.",
        report_passed(&initial_publish, "/published"),
        &initial_publish,
    )];
    let initial_published = report_passed(&initial_publish, "/published");
    let mut readback = serde_json::Value::Null;
    let mut publish_after_readback = serde_json::Value::Null;
    let mut admission = serde_json::Value::Null;
    let mut publish_after_admission = serde_json::Value::Null;
    let mut final_readback = serde_json::Value::Null;

    if initial_published {
        readback = readback_issue_mirror_file(services, issue_id, path, record)?;
        let readback_passed = report_passed(&readback, "/passed");
        stages.push(issue_mirror_roundtrip_stage(
            "readback",
            "Read back the connector mirror and optionally record the observation as Hive evidence.",
            readback_passed,
            &readback,
        ));

        if record && issue_report_recorded_publish_required(&readback) {
            publish_after_readback = publish_issue_mirror_to_file(services, issue_id, path)?;
            stages.push(issue_mirror_roundtrip_stage(
                "publish_after_readback",
                "Publish the readback evidence comment back to the connector surface.",
                report_passed(&publish_after_readback, "/published"),
                &publish_after_readback,
            ));
        }

        if readback_passed && publish_stage_allows_next(&publish_after_readback) {
            admission = admit_issue_mirror_file(services, issue_id, path, record)?;
            let admitted = report_passed(&admission, "/admitted");
            stages.push(issue_mirror_roundtrip_stage(
                "admit",
                "Run connector admission gates against the current mirror and optionally record the verdict.",
                admitted,
                &admission,
            ));

            if record && issue_report_recorded_publish_required(&admission) {
                publish_after_admission = publish_issue_mirror_to_file(services, issue_id, path)?;
                stages.push(issue_mirror_roundtrip_stage(
                    "publish_after_admission",
                    "Publish the admission evidence comment back to the connector surface.",
                    report_passed(&publish_after_admission, "/published"),
                    &publish_after_admission,
                ));
            }

            if admitted && publish_stage_allows_next(&publish_after_admission) {
                final_readback = readback_issue_mirror_file(services, issue_id, path, false)?;
                stages.push(issue_mirror_roundtrip_stage(
                    "final_readback",
                    "Verify the connector surface is current after all recorded observations were published.",
                    report_passed(&final_readback, "/passed"),
                    &final_readback,
                ));
            }
        }
    }

    Ok(compact_issue_mirror_roundtrip(
        issue_id,
        record,
        stages,
        initial_publish,
        readback,
        publish_after_readback,
        admission,
        publish_after_admission,
        final_readback,
    ))
}

fn apply_connector_provider_admission_preview(
    admission: &mut serde_json::Value,
    provider_preview: &serde_json::Value,
) {
    let Some(object) = admission.as_object_mut() else {
        return;
    };
    let admissible = provider_preview
        .pointer("/decision/admissible")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let blockers = provider_preview
        .pointer("/decision/blockers")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let failed_count = blockers.as_array().map(Vec::len).unwrap_or_default();
    let route_to = provider_preview
        .pointer("/decision/route_to")
        .and_then(|value| value.as_str());

    object.insert(
        "provider_admission".to_string(),
        provider_preview
            .pointer("/provider_admission")
            .cloned()
            .unwrap_or_else(|| serde_json::json!(null)),
    );
    object.insert(
        "provider_decision".to_string(),
        provider_preview
            .pointer("/decision")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );
    object.insert(
        "provider_policy".to_string(),
        provider_preview
            .pointer("/policy")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );
    object.insert(
        "provider_checks".to_string(),
        provider_preview
            .pointer("/checks")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    );
    object.insert(
        "writer_adapter".to_string(),
        provider_preview
            .pointer("/adapter")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );
    object.insert(
        "remote_contract".to_string(),
        provider_preview
            .pointer("/remote_contract")
            .cloned()
            .unwrap_or_else(|| serde_json::json!(null)),
    );
    object.insert(
        "remote_target".to_string(),
        provider_preview
            .pointer("/remote_target")
            .cloned()
            .unwrap_or_else(|| serde_json::json!(null)),
    );

    if admissible {
        object.insert("admitted".to_string(), serde_json::json!(true));
        object.insert("result".to_string(), serde_json::json!("admitted"));
        object.insert(
            "reason".to_string(),
            serde_json::json!("connector provider admission passed"),
        );
        object.insert("failed_count".to_string(), serde_json::json!(0));
        object.insert("failed_checks".to_string(), serde_json::json!([]));
        if let Some(decision) = object
            .get_mut("decision")
            .and_then(|value| value.as_object_mut())
        {
            decision.insert("route_to".to_string(), serde_json::json!(route_to));
            decision.insert("blockers".to_string(), serde_json::json!([]));
        }
        return;
    }

    object.insert("admitted".to_string(), serde_json::json!(false));
    object.insert("result".to_string(), serde_json::json!("rejected"));
    object.insert(
        "reason".to_string(),
        serde_json::json!("connector provider admission blocked"),
    );
    object.insert("failed_count".to_string(), serde_json::json!(failed_count));
    object.insert("failed_checks".to_string(), blockers.clone());
    if let Some(decision) = object
        .get_mut("decision")
        .and_then(|value| value.as_object_mut())
    {
        decision.insert("route_to".to_string(), serde_json::json!(null));
        decision.insert(
            "human_options".to_string(),
            serde_json::json!(["inspect-provider", "publish", "retry-admission"]),
        );
        decision.insert("blockers".to_string(), blockers);
    }
}

fn connector_admission_preview_checks(
    provider: Option<&ConnectorProviderSpec>,
    provider_admission: Option<&ConnectorProviderAdmissionSpec>,
    connector_status: &serde_json::Value,
    writer_blockers: &[String],
    remote_contract_required: bool,
    remote_target: &serde_json::Value,
    remote_contract: &serde_json::Value,
    check_registry: &[ConnectorAdmissionCheckSpec],
) -> Vec<serde_json::Value> {
    let provider_admission_blockers = provider_admission
        .map(|admission| {
            admission
                .blockers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let connector_failed_checks = connector_status
        .pointer("/failed_checks")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    let mirror_current = connector_status
        .pointer("/current")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let provider_admission_ready = provider_admission
        .map(|admission| admission.status == "ready" && admission.blockers.is_empty())
        .unwrap_or(false);
    let remote_readback_available = provider
        .map(|provider| provider.supports_readback)
        .unwrap_or(false);
    let remote_write_contract_ready =
        !remote_contract_required || (writer_blockers.is_empty() && remote_readback_available);
    let remote_target_valid = !remote_contract_required
        || remote_target
            .pointer("/valid")
            .and_then(|value| value.as_bool())
            == Some(true);

    let checks = vec![
        readback_check(
            "provider_supported",
            "Connector provider is registered for this issue surface.",
            provider.is_some(),
            serde_json::json!({
                "provider": provider.map(|provider| provider.name.as_str()),
                "mode": provider.map(|provider| provider.mode.as_str())
            }),
        ),
        readback_check(
            "provider_admission_ready",
            "Provider admission policy is ready for this connector.",
            provider_admission_ready,
            serde_json::json!({
                "status": provider_admission.map(|admission| admission.status.as_str()),
                "blockers": provider_admission_blockers
            }),
        ),
        readback_check(
            "mirror_current",
            "Connector mirror status is current before admission.",
            mirror_current,
            serde_json::json!({
                "reason": connector_status.pointer("/reason"),
                "publish_required": connector_status.pointer("/publish_required")
            }),
        ),
        readback_check(
            "readback_checks_passed",
            "Issue/status/comment readback checks pass for the connector mirror.",
            connector_failed_checks.is_empty(),
            serde_json::json!({
                "failed_checks": connector_failed_checks
            }),
        ),
        readback_check(
            "remote_write_contract_ready",
            "Remote write/readback contract is ready when the provider needs a remote issue API.",
            remote_write_contract_ready,
            serde_json::json!({
                "required": remote_contract_required,
                "writer_blockers": writer_blockers,
                "supports_readback": remote_readback_available
            }),
        ),
        readback_check(
            "remote_target_valid",
            "Review surface parses as a provider-specific remote issue target.",
            remote_target_valid,
            serde_json::json!({
                "required": remote_contract_required,
                "target": remote_target
            }),
        ),
        connector_retry_policy_admission_check(
            provider,
            connector_status,
            remote_contract_required,
            remote_contract,
        ),
    ];
    checks
        .into_iter()
        .map(|check| connector_admission_check_with_policy(check, check_registry))
        .collect()
}

fn connector_admission_check_with_policy(
    mut check: serde_json::Value,
    check_registry: &[ConnectorAdmissionCheckSpec],
) -> serde_json::Value {
    let Some(name) = check
        .pointer("/name")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
    else {
        return check;
    };
    let Some(spec) = check_registry.iter().find(|spec| spec.name == name) else {
        return check;
    };
    let Some(object) = check.as_object_mut() else {
        return check;
    };
    object.insert(
        "severity".to_string(),
        serde_json::json!(spec.severity.as_str()),
    );
    object.insert("owner".to_string(), serde_json::json!(spec.owner.as_str()));
    object.insert(
        "required_evidence".to_string(),
        serde_json::json!(spec
            .required_evidence
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()),
    );
    object.insert(
        "policy_summary".to_string(),
        serde_json::json!(spec.summary.as_str()),
    );
    check
}

fn connector_admission_check_failed(checks: &[serde_json::Value], name: &str) -> bool {
    checks.iter().any(|check| {
        check.pointer("/name").and_then(|value| value.as_str()) == Some(name)
            && check.pointer("/passed").and_then(|value| value.as_bool()) != Some(true)
    })
}

fn connector_retry_policy_admission_check(
    provider: Option<&ConnectorProviderSpec>,
    connector_status: &serde_json::Value,
    remote_contract_required: bool,
    remote_contract: &serde_json::Value,
) -> serde_json::Value {
    let retry_policy = remote_contract
        .pointer("/retry")
        .filter(|value| value.is_object());
    let mut violations = Vec::new();
    let expected_provider = provider.map(|provider| provider.name.as_str());

    if remote_contract_required && retry_policy.is_none() {
        violations.push(serde_json::json!({
            "code": "retry_policy_missing",
            "field": "remote_contract.retry"
        }));
    }

    let schema_version = retry_policy.and_then(|policy| {
        policy
            .pointer("/schema_version")
            .and_then(|value| value.as_str())
    });
    if remote_contract_required && schema_version != Some(POLICY_SCHEMA_VERSION) {
        violations.push(serde_json::json!({
            "code": "retry_policy_schema_mismatch",
            "field": "schema_version",
            "expected": POLICY_SCHEMA_VERSION,
            "actual": schema_version
        }));
    }

    let policy_provider = retry_policy
        .and_then(|policy| policy.pointer("/provider").and_then(|value| value.as_str()));
    if remote_contract_required
        && expected_provider.is_some()
        && policy_provider != expected_provider
    {
        violations.push(serde_json::json!({
            "code": "retry_policy_provider_mismatch",
            "field": "provider",
            "expected": expected_provider,
            "actual": policy_provider
        }));
    }

    let policy_max_attempts = retry_policy.and_then(|policy| {
        policy
            .pointer("/max_attempts")
            .and_then(|value| value.as_u64())
    });
    let valid_policy_max_attempts = policy_max_attempts.filter(|value| *value >= 1);
    if remote_contract_required && valid_policy_max_attempts.is_none() {
        violations.push(serde_json::json!({
            "code": "retry_policy_max_attempts_invalid",
            "field": "max_attempts",
            "actual": policy_max_attempts
        }));
    }

    let observed_operations = connector_retry_policy_attempt_observations(connector_status);
    if remote_contract_required {
        if let Some(max_attempts) = valid_policy_max_attempts {
            for observation in &observed_operations {
                connector_retry_policy_push_budget_violations(
                    &mut violations,
                    observation,
                    max_attempts,
                );
            }
        }
    }

    readback_check(
        "retry_policy_bound",
        "Remote retry diagnostics stay within the connector retry policy budget.",
        violations.is_empty(),
        serde_json::json!({
            "required": remote_contract_required,
            "provider": expected_provider,
            "policy": retry_policy.map(|policy| serde_json::json!({
                "schema_version": policy.pointer("/schema_version").and_then(|value| value.as_str()),
                "provider": policy.pointer("/provider").and_then(|value| value.as_str()),
                "max_attempts": policy.pointer("/max_attempts").and_then(|value| value.as_u64()),
                "base_backoff_ms": policy.pointer("/base_backoff_ms").and_then(|value| value.as_u64()),
                "backoff_strategy": policy.pointer("/backoff_strategy").and_then(|value| value.as_str())
            })),
            "observed_operations": observed_operations,
            "violations": violations
        }),
    )
}

fn connector_retry_policy_attempt_observations(
    connector_status: &serde_json::Value,
) -> Vec<serde_json::Value> {
    ["write", "readback"]
        .into_iter()
        .filter_map(|stage| {
            let pointer = format!("/remote_diagnostics/{stage}/primary_operation");
            let operation = connector_status.pointer(&pointer)?;
            if !operation.is_object() {
                return None;
            }
            let attempt_count = operation
                .pointer("/attempt_count")
                .and_then(|value| value.as_u64());
            let operation_max_attempts = operation
                .pointer("/max_attempts")
                .and_then(|value| value.as_u64());
            let attempts_len = operation
                .pointer("/attempts")
                .and_then(|value| value.as_array())
                .map(|attempts| attempts.len() as u64);
            if attempt_count.is_none() && operation_max_attempts.is_none() && attempts_len.is_none()
            {
                return None;
            }
            Some(serde_json::json!({
                "stage": stage,
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": operation.pointer("/method").and_then(|value| value.as_str()),
                "graphql_operation": operation.pointer("/graphql_operation").and_then(|value| value.as_str()),
                "attempt_count": attempt_count,
                "operation_max_attempts": operation_max_attempts,
                "attempts_len": attempts_len
            }))
        })
        .collect()
}

fn connector_retry_policy_push_budget_violations(
    violations: &mut Vec<serde_json::Value>,
    observation: &serde_json::Value,
    max_attempts: u64,
) {
    for (field, code) in [
        ("attempt_count", "retry_attempt_budget_exceeded"),
        ("attempts_len", "retry_attempt_budget_exceeded"),
        (
            "operation_max_attempts",
            "retry_operation_budget_exceeds_policy",
        ),
    ] {
        let Some(value) = observation
            .pointer(&format!("/{field}"))
            .and_then(|value| value.as_u64())
        else {
            continue;
        };
        if value <= max_attempts {
            continue;
        }
        violations.push(serde_json::json!({
            "code": code,
            "stage": observation.pointer("/stage").and_then(|value| value.as_str()),
            "field": field,
            "value": value,
            "max_attempts": max_attempts
        }));
    }
}

fn record_issue_mirror_readback(
    services: &AppServices,
    readback: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = readback
        .pointer("/issue_id")
        .and_then(|value| value.as_i64())
        .context("connector readback missing issue_id")?;
    let issue = services
        .kernel
        .store
        .get_hive_issue(issue_id)?
        .with_context(|| format!("unknown hive issue `{issue_id}`"))?;
    let contract = issue
        .loop_id
        .map(|loop_id| services.kernel.store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let passed = readback
        .pointer("/passed")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let result = if passed { "current" } else { "stale" };
    let failed_checks = readback
        .pointer("/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let failed_label = failed_checks
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let body = if passed {
        "Connector readback current: external issue surface matches Hive.".to_string()
    } else if failed_label.is_empty() {
        "Connector readback stale.".to_string()
    } else {
        format!("Connector readback stale: {failed_label}.")
    };
    let loop_id = issue.loop_id;
    let round = contract
        .as_ref()
        .map(|contract| contract.current_round)
        .unwrap_or(1);
    let phase = contract
        .as_ref()
        .map(|contract| contract.active_phase.as_str());
    let comment_id = services
        .kernel
        .store
        .insert_hive_comment(HiveCommentCreate {
            issue_id,
            author: "hive".to_string(),
            body: body.clone(),
            payload: serde_json::json!({
                "schema_version": SYSTEM_COMMENT_SCHEMA_VERSION,
                "source": "hive",
                "loop_id": loop_id,
                "round": round,
                "status": issue.status.as_str(),
                "phase": phase,
                "connector_readback": {
                    "schema_version": readback.pointer("/schema_version"),
                    "result": result,
                    "passed": passed,
                    "failed_checks": failed_checks,
                    "current_comment_count": readback.pointer("/current/comments/count"),
                    "remote_comment_count": readback.pointer("/remote/surface/comments/count"),
                    "path": readback.pointer("/path")
                }
            }),
        })?;

    let evidence_id = if let Some(loop_id) = issue.loop_id {
        Some(services.kernel.store.insert_hive_loop_evidence(
            HiveLoopEvidenceCreate {
                loop_id,
                stage_id: None,
                round,
                kind: "connector_readback".to_string(),
                summary: body.clone(),
                path: readback
                    .pointer("/path")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                payload: serde_json::json!({
                    "schema_version": ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "result": result,
                    "passed": passed,
                    "issue": {
                        "id": issue.id,
                        "status": issue.status.as_str(),
                        "comment_id": comment_id
                    },
                    "loop": {
                        "id": loop_id,
                        "status": contract.as_ref().map(|contract| contract.status.as_str()),
                        "phase": phase,
                        "round": round
                    },
                    "connector": {
                        "provider": readback.pointer("/provider"),
                        "review_surface": readback.pointer("/review_surface"),
                        "external_key": readback.pointer("/external_key")
                    },
                    "current": readback.pointer("/current").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "remote": readback.pointer("/remote").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "receipt": readback.pointer("/receipt").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "failed_checks": readback.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
                    "checks": readback.pointer("/checks").cloned().unwrap_or_else(|| serde_json::json!([]))
                }),
            },
        )?)
    } else {
        None
    };

    Ok(serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_readback_record.v1",
        "comment_id": comment_id,
        "evidence_id": evidence_id,
        "comment_body": body,
        "publish": connector_record_publish_hint(issue_id)
    }))
}

fn record_issue_mirror_admission(
    services: &AppServices,
    admission: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = admission
        .pointer("/issue_id")
        .and_then(|value| value.as_i64())
        .context("connector admission missing issue_id")?;
    let issue = services
        .kernel
        .store
        .get_hive_issue(issue_id)?
        .with_context(|| format!("unknown hive issue `{issue_id}`"))?;
    let contract = issue
        .loop_id
        .map(|loop_id| services.kernel.store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let admitted = admission
        .pointer("/admitted")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let result = admission
        .pointer("/result")
        .and_then(|value| value.as_str())
        .unwrap_or(if admitted { "admitted" } else { "rejected" });
    let route_to = admission
        .pointer("/decision/route_to")
        .and_then(|value| value.as_str())
        .unwrap_or("operator");
    let failed_checks = admission
        .pointer("/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let failed_label = failed_checks
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let provider_checks = admission
        .pointer("/provider_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let provider_check_count = provider_checks.as_array().map(Vec::len).unwrap_or_default();
    let provider_passed_check_count = provider_checks
        .as_array()
        .into_iter()
        .flatten()
        .filter(|check| {
            check
                .pointer("/passed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .count();
    let body = if admitted {
        format!("Connector admission admitted: {route_to}.")
    } else if failed_label.is_empty() {
        "Connector admission rejected.".to_string()
    } else {
        format!("Connector admission rejected: {failed_label}.")
    };
    let loop_id = issue.loop_id;
    let round = contract
        .as_ref()
        .map(|contract| contract.current_round)
        .unwrap_or(1);
    let phase = contract
        .as_ref()
        .map(|contract| contract.active_phase.as_str());
    let comment_id = services
        .kernel
        .store
        .insert_hive_comment(HiveCommentCreate {
            issue_id,
            author: "hive".to_string(),
            body: body.clone(),
            payload: serde_json::json!({
                "schema_version": SYSTEM_COMMENT_SCHEMA_VERSION,
                "source": "hive",
                "loop_id": loop_id,
                "round": round,
                "status": issue.status.as_str(),
                "phase": phase,
                "connector_admission": {
                    "schema_version": admission.pointer("/schema_version"),
                    "object_kind": admission.pointer("/object_kind"),
                    "result": result,
                    "admitted": admitted,
                    "gate": admission.pointer("/policy/gate"),
                    "route_to": route_to,
                    "failed_checks": failed_checks,
                    "provider_check_count": provider_check_count,
                    "provider_passed_check_count": provider_passed_check_count,
                    "writer_adapter": admission.pointer("/writer_adapter"),
                    "remote_contract": admission.pointer("/remote_contract"),
                    "remote_target": admission.pointer("/remote_target"),
                    "dry_run": admission.pointer("/dry_run")
                }
            }),
        })?;

    let evidence_id = if let Some(loop_id) = issue.loop_id {
        Some(services.kernel.store.insert_hive_loop_evidence(
            HiveLoopEvidenceCreate {
                loop_id,
                stage_id: None,
                round,
                kind: "connector_admission".to_string(),
                summary: body.clone(),
                path: admission
                    .pointer("/receipt/path")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                payload: serde_json::json!({
                    "schema_version": ISSUE_MIRROR_ADMISSION_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "result": result,
                    "admitted": admitted,
                    "issue": {
                        "id": issue.id,
                        "status": issue.status.as_str(),
                        "comment_id": comment_id
                    },
                    "loop": {
                        "id": loop_id,
                        "status": contract.as_ref().map(|contract| contract.status.as_str()),
                        "phase": phase,
                        "round": round
                    },
                    "connector": {
                        "provider": admission.pointer("/provider"),
                        "review_surface": admission.pointer("/review_surface"),
                        "external_key": admission.pointer("/external_key")
                    },
                    "decision": admission.pointer("/decision").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "provider_admission": admission.pointer("/provider_admission").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "provider_decision": admission.pointer("/provider_decision").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "provider_checks": provider_checks,
                    "writer_adapter": admission.pointer("/writer_adapter").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "remote_contract": admission.pointer("/remote_contract").cloned().unwrap_or_else(|| serde_json::json!(null)),
                    "remote_target": admission.pointer("/remote_target").cloned().unwrap_or_else(|| serde_json::json!(null)),
                    "policy": admission.pointer("/policy").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "receipt": admission.pointer("/receipt").cloned().unwrap_or_else(|| serde_json::json!({})),
                    "failed_checks": admission.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
                    "audit": {
                        "schema_version": admission.pointer("/audit/schema_version"),
                        "passed": admission.pointer("/audit/passed"),
                        "failed_count": admission.pointer("/audit/failed_count"),
                        "failed_checks": admission.pointer("/audit/failed_checks")
                    }
                }),
            },
        )?)
    } else {
        None
    };

    Ok(serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_admission_record.v1",
        "comment_id": comment_id,
        "evidence_id": evidence_id,
        "comment_body": body,
        "publish": connector_record_publish_hint(issue_id)
    }))
}

fn connector_record_publish_hint(issue_id: i64) -> serde_json::Value {
    serde_json::json!({
        "schema_version": CONNECTOR_PUBLISH_HINT_SCHEMA_VERSION,
        "required": true,
        "reason": "record_created_local_issue_event",
        "summary": "Recording this connector observation added a local Hive comment/evidence row; publish the new ledger event to the connector mirror.",
        "action": "publish",
        "command": format!("entrance hive issue mirror-publish {} --compact", issue_id)
    })
}

fn resolve_issue_mirror_path_for_provider(
    app_root: &Path,
    mirror: &IssueMirrorReport,
    provider: Option<&ConnectorProviderSpec>,
    explicit_path: Option<&str>,
) -> PathBuf {
    explicit_path.map(PathBuf::from).unwrap_or_else(|| {
        default_issue_mirror_path_for_provider(app_root, &mirror.external_key, provider)
    })
}

fn default_issue_mirror_path_for_provider(
    app_root: &Path,
    external_key: &str,
    provider: Option<&ConnectorProviderSpec>,
) -> PathBuf {
    let Some(provider) = provider else {
        return default_issue_mirror_path(app_root, external_key);
    };
    let storage = provider.storage.trim();
    if storage.is_empty()
        || storage == "not-configured"
        || storage == "sqlite"
        || storage.starts_with("sqlite:")
        || storage.starts_with("https://")
        || storage.starts_with("http://")
    {
        return default_issue_mirror_path(app_root, external_key);
    }

    let key = sanitize_mirror_key(external_key);
    let storage_path = if storage.contains("{external_key}") {
        storage.replace("{external_key}", &key)
    } else if storage.contains('*') {
        storage.replacen('*', &key, 1)
    } else if storage.ends_with('/') {
        format!("{storage}{key}.json")
    } else if storage.ends_with(".json") {
        storage.to_string()
    } else {
        format!("{}/{key}.json", storage.trim_end_matches('/'))
    };
    let path = PathBuf::from(storage_path);
    if path.is_absolute() {
        path
    } else {
        app_root.join(path)
    }
}

fn default_issue_mirror_path(app_root: &Path, external_key: &str) -> PathBuf {
    app_root
        .join("connectors")
        .join("issue-mirrors")
        .join(format!("{}.json", sanitize_mirror_key(external_key)))
}

fn mirror_receipt_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("issue-mirror");
    path.with_file_name(format!("{stem}.receipt.json"))
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

fn write_issue_mirror_file(mirror: &IssueMirrorReport, path: &Path) -> Result<MirrorFileDigest> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mirror sink directory {}",
                parent.display()
            )
        })?;
    }
    let payload = mirror_payload(mirror)?;
    fs::write(path, &payload)
        .with_context(|| format!("failed to write issue mirror {}", path.display()))?;
    Ok(digest_bytes(&payload))
}

fn write_issue_mirror_receipt(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
    provider: Option<&ConnectorProviderSpec>,
) -> Result<MirrorFileDigest> {
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mirror receipt directory {}",
                parent.display()
            )
        })?;
    }
    let receipt =
        issue_mirror_sync_receipt_for_provider(mirror, path, receipt_path, digest, provider);
    let payload = serde_json::to_vec_pretty(&receipt)?;
    fs::write(receipt_path, &payload).with_context(|| {
        format!(
            "failed to write issue mirror receipt {}",
            receipt_path.display()
        )
    })?;
    Ok(digest_bytes(&payload))
}

fn write_issue_mirror_receipt_with_remote_execution(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
    provider: &ConnectorProviderSpec,
    remote_write_receipt: &serde_json::Value,
    remote_write_execution: &serde_json::Value,
) -> Result<MirrorFileDigest> {
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mirror receipt directory {}",
                parent.display()
            )
        })?;
    }
    let mut receipt =
        issue_mirror_sync_receipt_for_provider(mirror, path, receipt_path, digest, Some(provider));
    if let Some(object) = receipt.as_object_mut() {
        object.insert(
            "remote_write_receipt".to_string(),
            remote_write_receipt.clone(),
        );
        object.insert(
            "remote_write_execution".to_string(),
            remote_write_execution.clone(),
        );
    }
    let payload = serde_json::to_vec_pretty(&receipt)?;
    fs::write(receipt_path, &payload).with_context(|| {
        format!(
            "failed to write issue mirror receipt {}",
            receipt_path.display()
        )
    })?;
    Ok(digest_bytes(&payload))
}

fn write_issue_mirror_receipt_with_remote_write_failure(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
    provider: &ConnectorProviderSpec,
    remote_write_execution: &serde_json::Value,
) -> Result<MirrorFileDigest> {
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mirror receipt directory {}",
                parent.display()
            )
        })?;
    }
    let mut receipt =
        issue_mirror_sync_receipt_for_provider(mirror, path, receipt_path, digest, Some(provider));
    if let Some(object) = receipt.as_object_mut() {
        object.insert("remote_write_receipt".to_string(), serde_json::Value::Null);
        object.insert(
            "remote_write_execution".to_string(),
            remote_write_execution.clone(),
        );
    }
    let payload = serde_json::to_vec_pretty(&receipt)?;
    fs::write(receipt_path, &payload).with_context(|| {
        format!(
            "failed to write issue mirror receipt {}",
            receipt_path.display()
        )
    })?;
    Ok(digest_bytes(&payload))
}

fn mirror_payload(mirror: &IssueMirrorReport) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(mirror)?)
}

fn read_digest(path: &Path) -> Result<Option<MirrorFileDigest>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(digest_bytes(&bytes))),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn read_payload_with_digest(path: &Path) -> Result<Option<(Vec<u8>, MirrorFileDigest)>> {
    match fs::read(path) {
        Ok(bytes) => {
            let digest = digest_bytes(&bytes);
            Ok(Some((bytes, digest)))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn read_receipt(path: &Path) -> Result<Option<serde_json::Value>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
            format!("failed to parse issue mirror receipt {}", path.display())
        })?)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn digest_bytes(bytes: &[u8]) -> MirrorFileDigest {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    MirrorFileDigest {
        bytes: bytes.len() as u64,
        sha256: encode_hex(&hasher.finalize()),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
fn issue_mirror_sync_receipt(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
) -> serde_json::Value {
    issue_mirror_sync_receipt_for_provider(mirror, path, receipt_path, digest, None)
}

fn issue_mirror_sync_receipt_for_provider(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_SYNC_RECEIPT_SCHEMA_VERSION,
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "mirror_schema_version": mirror.schema_version.as_str(),
        "issue": issue_mirror_issue_binding(mirror),
        "loop": issue_mirror_loop_binding(mirror),
        "mirror": {
            "path": path.display().to_string(),
            "bytes": digest.bytes,
            "sha256": digest.sha256.as_str()
        },
        "receipt": {
            "path": receipt_path.display().to_string()
        },
        "remote_contract": compact_connector_remote_contract(provider),
        "remote_target": connector_remote_target(
            provider,
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_write_receipt": connector_remote_write_receipt(mirror, path, digest, provider),
        "commands": {
            "refresh": format!("entrance hive issue mirror {} --compact", mirror.issue.id),
            "sync": format!("entrance hive issue mirror-sync {}", mirror.issue.id),
            "publish": format!("entrance hive issue mirror-publish {} --compact", mirror.issue.id),
            "verify": format!("entrance hive issue mirror-verify {}", mirror.issue.id),
            "readback": format!("entrance hive issue mirror-readback {} --record --compact", mirror.issue.id)
        }
    })
}

fn issue_mirror_issue_binding(mirror: &IssueMirrorReport) -> serde_json::Value {
    serde_json::json!({
        "id": mirror.issue.id,
        "loop_id": mirror.issue.loop_id,
        "status": mirror.issue.status.as_str(),
        "updated_at": mirror.issue.updated_at.as_str()
    })
}

fn issue_mirror_loop_binding(mirror: &IssueMirrorReport) -> serde_json::Value {
    mirror
        .loop_contract
        .as_ref()
        .map_or(serde_json::Value::Null, |contract| {
            serde_json::json!({
                "id": contract.id,
                "status": contract.status.as_str(),
                "phase": contract.active_phase.as_str(),
                "round": contract.current_round,
                "updated_at": contract.updated_at.as_str()
            })
        })
}

fn compact_issue_mirror_sync(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_SYNC_SCHEMA_VERSION,
        "mirror_schema_version": mirror.schema_version.as_str(),
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": mirror.issue.id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "bytes": digest.bytes,
        "sha256": digest.sha256.as_str(),
        "remote_contract": compact_connector_remote_contract(provider),
        "remote_target": connector_remote_target(
            provider,
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_write_receipt": connector_remote_write_receipt(mirror, path, digest, provider),
        "refresh_command": format!("entrance hive issue mirror {} --compact", mirror.issue.id),
        "sync_command": format!("entrance hive issue mirror-sync {}", mirror.issue.id),
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", mirror.issue.id),
        "verify_command": format!("entrance hive issue mirror-verify {}", mirror.issue.id),
        "readback_command": format!("entrance hive issue mirror-readback {} --record --compact", mirror.issue.id),
        "roundtrip_command": format!("entrance hive issue mirror-roundtrip {} --compact", mirror.issue.id)
    })
}

fn compact_issue_mirror_publish(sync: &serde_json::Value) -> serde_json::Value {
    let issue_id = sync.pointer("/issue_id").and_then(|value| value.as_i64());
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_PUBLISH_SCHEMA_VERSION,
        "published": true,
        "reason": "operator_publish",
        "provider": sync.pointer("/provider"),
        "review_surface": sync.pointer("/review_surface"),
        "external_key": sync.pointer("/external_key"),
        "issue_id": issue_id,
        "issue_status": sync.pointer("/issue_status"),
        "loop_id": sync.pointer("/loop_id"),
        "loop_round": sync.pointer("/loop_round"),
        "path": sync.pointer("/path"),
        "receipt_path": sync.pointer("/receipt_path"),
        "bytes": sync.pointer("/bytes"),
        "sha256": sync.pointer("/sha256"),
        "sync": sync,
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", issue_id.unwrap_or_default()),
        "readback_command": format!("entrance hive issue mirror-readback {} --record --compact", issue_id.unwrap_or_default()),
        "admit_command": format!("entrance hive issue mirror-admit {} --record --compact", issue_id.unwrap_or_default()),
        "roundtrip_command": format!("entrance hive issue mirror-roundtrip {} --compact", issue_id.unwrap_or_default())
    })
}

fn compact_issue_mirror_publish_blocked(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    provider: Option<&ConnectorProviderSpec>,
    blockers: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_PUBLISH_SCHEMA_VERSION,
        "published": false,
        "reason": "connector_writer_blocked",
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": mirror.issue.id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "failed_checks": blockers,
        "adapter": compact_connector_writer_adapter(&mirror.provider, provider),
        "remote_contract": compact_connector_remote_contract(provider),
        "remote_target": connector_remote_target(
            provider,
            &mirror.review_surface,
            &mirror.external_key
        ),
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", mirror.issue.id),
        "registry_command": "entrance hive connector registry --compact",
        "queue_command": format!("entrance hive connector queue --provider {} --compact", mirror.provider)
    })
}

fn compact_issue_mirror_remote_publish_failed(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    provider: &ConnectorProviderSpec,
    write_plan: &serde_json::Value,
    write_execution: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_PUBLISH_SCHEMA_VERSION,
        "published": false,
        "reason": "remote_write_failed",
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": mirror.issue.id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "failed_checks": write_execution.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!(["remote_write_failed"])),
        "adapter": compact_connector_writer_adapter(&mirror.provider, Some(provider)),
        "remote_contract": compact_connector_remote_contract(Some(provider)),
        "remote_target": connector_remote_target(
            Some(provider),
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_write_plan": write_plan,
        "remote_write_execution": write_execution,
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", mirror.issue.id),
        "registry_command": "entrance hive connector registry --compact",
        "queue_command": format!("entrance hive connector queue --provider {} --compact", mirror.provider)
    })
}

fn connector_write_receipt(
    mirror: &IssueMirrorReport,
    sync: &serde_json::Value,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": CONNECTOR_WRITE_RECEIPT_SCHEMA_VERSION,
        "object_kind": "ISSUE_CONNECTOR_WRITE",
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue": issue_mirror_issue_binding(mirror),
        "loop": issue_mirror_loop_binding(mirror),
        "adapter": compact_connector_writer_adapter(&mirror.provider, provider),
        "remote_contract": compact_connector_remote_contract(provider),
        "remote_target": connector_remote_target(
            provider,
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_write_receipt": sync.pointer("/remote_write_receipt").cloned().unwrap_or_else(|| serde_json::json!(null)),
        "status_surface": {
            "status": mirror.issue.status.as_str(),
            "updated_at": mirror.issue.updated_at.as_str()
        },
        "comment_surface": issue_mirror_comment_surface(mirror),
        "mirror": {
            "path": sync.pointer("/path"),
            "receipt_path": sync.pointer("/receipt_path"),
            "bytes": sync.pointer("/bytes"),
            "sha256": sync.pointer("/sha256")
        },
        "readback": {
            "available": provider.map(|provider| provider.supports_readback).unwrap_or(false),
            "command": sync.pointer("/readback_command")
        }
    })
}

fn connector_remote_write_receipt(
    mirror: &IssueMirrorReport,
    path: &Path,
    digest: &MirrorFileDigest,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    let Some(provider) =
        provider.filter(|provider| connector_provider_uses_remote_contract(provider))
    else {
        return serde_json::Value::Null;
    };
    let remote_contract = compact_connector_remote_contract(Some(provider));
    let remote_target =
        connector_remote_target(Some(provider), &mirror.review_surface, &mirror.external_key);
    let idempotency_key = connector_remote_idempotency_key(mirror, digest, provider);
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_WRITE_RECEIPT_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "remote_id": connector_remote_id(mirror, provider, &remote_target),
        "remote_url": connector_remote_url(mirror, provider, path, &remote_target),
        "external_key": mirror.external_key.as_str(),
        "remote_target": remote_target,
        "status": mirror.issue.status.as_str(),
        "comment_count": mirror.comments.len(),
        "idempotency_key": idempotency_key,
        "source_mirror_sha256": digest.sha256.as_str(),
        "source_mirror_bytes": digest.bytes,
        "surface": {
            "review_surface": mirror.review_surface.as_str(),
            "issue": issue_mirror_issue_binding(mirror),
            "comments": issue_mirror_comment_surface(mirror)
        },
        "contract": {
            "schema_version": remote_contract.pointer("/schema_version").and_then(|value| value.as_str()),
            "write_receipt_schema_version": remote_contract.pointer("/write/receipt_schema_version").and_then(|value| value.as_str()),
            "readback_schema_version": remote_contract.pointer("/readback/schema_version").and_then(|value| value.as_str())
        }
    })
}

fn connector_remote_idempotency_key(
    mirror: &IssueMirrorReport,
    digest: &MirrorFileDigest,
    provider: &ConnectorProviderSpec,
) -> String {
    let issue = connector_remote_write_issue_from_mirror(mirror, digest);
    connector_remote_issue_idempotency_key(provider.name.as_str(), &issue)
}

fn connector_remote_issue_idempotency_key(
    provider_name: &str,
    issue: &serde_json::Value,
) -> String {
    let basis = serde_json::json!({
        "surface": "issue_latest_comment",
        "provider": provider_name,
        "external_key": issue.pointer("/connector/external_key").and_then(|value| value.as_str()),
        "issue_id": issue.pointer("/id").and_then(|value| value.as_i64()),
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64())
    });
    let payload = serde_json::to_vec(&basis).unwrap_or_default();
    digest_bytes(&payload).sha256
}

fn connector_remote_id(
    mirror: &IssueMirrorReport,
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
) -> String {
    remote_target
        .pointer("/remote_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{}:{}", provider.name, mirror.external_key))
}

fn connector_remote_url(
    mirror: &IssueMirrorReport,
    provider: &ConnectorProviderSpec,
    path: &Path,
    remote_target: &serde_json::Value,
) -> String {
    if connector_provider_is_remote_fixture(provider) {
        format!("file://{}", path.display())
    } else {
        remote_target
            .pointer("/remote_url")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("{}://{}", provider.name, mirror.external_key))
    }
}

fn compact_issue_mirror_status(readback: &serde_json::Value) -> serde_json::Value {
    let issue_id = readback
        .pointer("/issue_id")
        .and_then(|value| value.as_i64());
    let current = readback
        .pointer("/passed")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let failed_checks = readback
        .pointer("/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let reason = if current {
        "connector_mirror_current".to_string()
    } else {
        connector_status_reason(&failed_checks).to_string()
    };
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_STATUS_SCHEMA_VERSION,
        "current": current,
        "publish_required": !current,
        "reason": reason,
        "failed_checks": failed_checks,
        "provider": readback.pointer("/provider"),
        "review_surface": readback.pointer("/review_surface"),
        "external_key": readback.pointer("/external_key"),
        "issue_id": issue_id,
        "issue_status": readback.pointer("/issue_status"),
        "loop_id": readback.pointer("/loop_id"),
        "loop_round": readback.pointer("/loop_round"),
        "path": readback.pointer("/path"),
        "receipt_path": readback.pointer("/receipt_path"),
        "current_sha256": readback.pointer("/current/digest/sha256"),
        "remote_sha256": readback.pointer("/remote/digest/sha256"),
        "current_comment_count": readback.pointer("/current/comments/count"),
        "remote_comment_count": readback.pointer("/remote/surface/comments/count"),
        "remote_parsed": readback.pointer("/remote/parsed"),
        "receipt_found": readback.pointer("/receipt/found"),
        "checks": readback.pointer("/checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "remote_readback_checks": readback.pointer("/remote_readback/checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "remote_diagnostics": connector_remote_diagnostics(readback),
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", issue_id.unwrap_or_default()),
        "readback_command": format!("entrance hive issue mirror-readback {} --record --compact", issue_id.unwrap_or_default()),
        "admit_command": format!("entrance hive issue mirror-admit {} --record --compact", issue_id.unwrap_or_default()),
        "roundtrip_command": format!("entrance hive issue mirror-roundtrip {} --compact", issue_id.unwrap_or_default())
    })
}

fn connector_remote_diagnostics(readback: &serde_json::Value) -> serde_json::Value {
    let write = connector_remote_execution_diagnostics(
        "write",
        connector_remote_write_execution_from_readback(readback),
    );
    let remote_readback = connector_remote_execution_diagnostics(
        "readback",
        readback.pointer("/remote_readback/execution"),
    );
    let signals = [&write, &remote_readback]
        .iter()
        .filter_map(|diagnostic| diagnostic.pointer("/signal").cloned())
        .filter(|signal| !signal.is_null())
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": "entrance.hive.connector_remote_diagnostics.v1",
        "write": write,
        "readback": remote_readback,
        "signals": signals
    })
}

fn connector_remote_write_execution_from_readback(
    readback: &serde_json::Value,
) -> Option<&serde_json::Value> {
    readback
        .pointer("/receipt/remote_write_receipt/write_execution")
        .or_else(|| readback.pointer("/receipt/remote_write_execution"))
}

fn connector_remote_execution_diagnostics(
    stage: &str,
    execution: Option<&serde_json::Value>,
) -> serde_json::Value {
    let Some(execution) = execution.filter(|value| value.is_object()) else {
        return serde_json::Value::Null;
    };
    let operations = connector_remote_execution_operations(execution);
    let primary_operation = connector_remote_primary_operation(&operations)
        .map(connector_remote_operation_diagnostics)
        .unwrap_or_else(|| serde_json::Value::Null);
    let failed_checks = connector_remote_execution_failed_checks(execution, &primary_operation);
    let success = execution
        .pointer("/success")
        .and_then(|value| value.as_bool())
        .or_else(|| {
            primary_operation
                .pointer("/success")
                .and_then(|value| value.as_bool())
        });
    let operation_count = execution
        .pointer("/operation_count")
        .and_then(|value| value.as_u64())
        .unwrap_or(operations.len() as u64);
    let signal =
        connector_remote_execution_signal(stage, success, &failed_checks, &primary_operation);
    serde_json::json!({
        "schema_version": "entrance.hive.connector_remote_execution_diagnostics.v1",
        "stage": stage,
        "success": success,
        "failed_checks": failed_checks,
        "operation_count": operation_count,
        "primary_operation": primary_operation,
        "signal": signal
    })
}

fn connector_remote_execution_operations(execution: &serde_json::Value) -> Vec<&serde_json::Value> {
    if let Some(operations) = execution
        .pointer("/operations")
        .and_then(|value| value.as_array())
    {
        return operations.iter().collect();
    }
    ["issue", "comments"]
        .iter()
        .filter_map(|key| execution.get(*key))
        .filter(|value| value.is_object())
        .collect()
}

fn connector_remote_primary_operation<'a>(
    operations: &'a [&'a serde_json::Value],
) -> Option<&'a serde_json::Value> {
    operations
        .iter()
        .copied()
        .find(|operation| {
            operation
                .pointer("/success")
                .and_then(|value| value.as_bool())
                == Some(false)
        })
        .or_else(|| {
            operations.iter().copied().find(|operation| {
                operation
                    .pointer("/retry/rate_limited")
                    .and_then(|value| value.as_bool())
                    == Some(true)
                    || operation
                        .pointer("/retry/attempted")
                        .and_then(|value| value.as_bool())
                        == Some(true)
                    || operation
                        .pointer("/retry/scheduled")
                        .and_then(|value| value.as_bool())
                        == Some(true)
            })
        })
        .or_else(|| operations.first().copied())
}

fn connector_remote_execution_failed_checks(
    execution: &serde_json::Value,
    primary_operation: &serde_json::Value,
) -> serde_json::Value {
    if let Some(checks) = execution
        .pointer("/failed_checks")
        .and_then(|value| value.as_array())
    {
        return serde_json::Value::Array(checks.clone());
    }
    primary_operation
        .pointer("/failed_check")
        .and_then(|value| value.as_str())
        .map(|check| serde_json::json!([check]))
        .unwrap_or_else(|| serde_json::json!([]))
}

fn connector_remote_operation_diagnostics(operation: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": operation.pointer("/method").and_then(|value| value.as_str()),
        "graphql_operation": operation.pointer("/graphql/operation").and_then(|value| value.as_str()),
        "success": operation.pointer("/success").and_then(|value| value.as_bool()),
        "failed_check": operation.pointer("/failed_check").and_then(|value| value.as_str()),
        "http_status": operation.pointer("/http_status").and_then(|value| value.as_u64()),
        "attempt_count": operation.pointer("/attempt_count").and_then(|value| value.as_u64()),
        "max_attempts": operation.pointer("/max_attempts").and_then(|value| value.as_u64()),
        "attempts": connector_remote_operation_attempts(operation),
        "retry": connector_remote_retry_diagnostics(operation.pointer("/retry"))
    })
}

fn connector_remote_operation_attempts(operation: &serde_json::Value) -> Vec<serde_json::Value> {
    operation
        .pointer("/attempts")
        .and_then(|value| value.as_array())
        .map(|attempts| {
            attempts
                .iter()
                .map(connector_remote_operation_attempt_diagnostics)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn connector_remote_operation_attempt_diagnostics(
    attempt: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "attempt": attempt.pointer("/attempt").and_then(|value| value.as_u64()),
        "success": attempt.pointer("/success").and_then(|value| value.as_bool()),
        "failed_check": attempt.pointer("/failed_check").and_then(|value| value.as_str()),
        "http_status": attempt.pointer("/http_status").and_then(|value| value.as_u64()),
        "error": attempt.pointer("/error").and_then(|value| value.as_str()),
        "retry": connector_remote_retry_diagnostics(attempt.pointer("/retry"))
    })
}

fn connector_remote_retry_diagnostics(retry: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(retry) = retry.filter(|value| value.is_object()) else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "reason": retry.pointer("/reason").and_then(|value| value.as_str()),
        "retryable": retry.pointer("/retryable").and_then(|value| value.as_bool()),
        "scheduled": retry.pointer("/scheduled").and_then(|value| value.as_bool()),
        "attempted": retry.pointer("/attempted").and_then(|value| value.as_bool()),
        "exhausted": retry.pointer("/exhausted").and_then(|value| value.as_bool()),
        "rate_limited": retry.pointer("/rate_limited").and_then(|value| value.as_bool()),
        "backoff_ms": retry.pointer("/backoff_ms").and_then(|value| value.as_u64()),
        "retry_after_secs": retry.pointer("/retry_after_secs").and_then(|value| value.as_u64()),
        "rate_limit": retry.pointer("/rate_limit").cloned().unwrap_or_else(|| serde_json::json!(null))
    })
}

fn connector_remote_execution_signal(
    stage: &str,
    success: Option<bool>,
    failed_checks: &serde_json::Value,
    primary_operation: &serde_json::Value,
) -> serde_json::Value {
    let retry = primary_operation.pointer("/retry");
    let attempt_count = primary_operation
        .pointer("/attempt_count")
        .and_then(|value| value.as_u64());
    let retry_attempted = retry
        .and_then(|value| value.pointer("/attempted"))
        .and_then(|value| value.as_bool())
        == Some(true);
    let retry_scheduled = retry
        .and_then(|value| value.pointer("/scheduled"))
        .and_then(|value| value.as_bool())
        == Some(true);
    let retry_exhausted = retry
        .and_then(|value| value.pointer("/exhausted"))
        .and_then(|value| value.as_bool())
        == Some(true);
    let rate_limited = retry
        .and_then(|value| value.pointer("/rate_limited"))
        .and_then(|value| value.as_bool())
        == Some(true);
    let retry_after_secs = retry
        .and_then(|value| value.pointer("/retry_after_secs"))
        .and_then(|value| value.as_u64());
    let http_status = primary_operation
        .pointer("/http_status")
        .and_then(|value| value.as_u64());
    let first_failed_check = failed_checks
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|value| value.as_str())
        .or_else(|| {
            primary_operation
                .pointer("/failed_check")
                .and_then(|value| value.as_str())
        });

    if success == Some(false) || rate_limited || retry_exhausted {
        let failed_check = first_failed_check.unwrap_or("remote_failed");
        let mut parts = vec![stage.to_string(), failed_check.to_string()];
        if let Some(status) = http_status {
            parts.push(status.to_string());
        }
        if let Some(attempts) = attempt_count.filter(|attempts| *attempts > 1) {
            parts.push(format!("{attempts} attempts"));
        }
        if let Some(seconds) = retry_after_secs {
            parts.push(format!("retry after {seconds}s"));
        }
        return serde_json::json!({
            "stage": stage,
            "tone": "warn",
            "label": parts.join(" "),
            "failed_check": failed_check,
            "http_status": http_status,
            "attempt_count": attempt_count,
            "retry": primary_operation.pointer("/retry").cloned().unwrap_or_else(|| serde_json::json!(null))
        });
    }

    if success == Some(true)
        && (retry_attempted || retry_scheduled || attempt_count.unwrap_or(0) > 1)
    {
        let attempts = attempt_count.unwrap_or(1);
        return serde_json::json!({
            "stage": stage,
            "tone": "info",
            "label": format!("{stage} retry {attempts} attempts"),
            "failed_check": first_failed_check,
            "http_status": http_status,
            "attempt_count": attempt_count,
            "retry": primary_operation.pointer("/retry").cloned().unwrap_or_else(|| serde_json::json!(null))
        });
    }

    serde_json::Value::Null
}

fn connector_status_reason(failed_checks: &serde_json::Value) -> &'static str {
    let first_check = failed_checks
        .as_array()
        .and_then(|checks| checks.iter().find_map(|value| value.as_str()));
    match first_check {
        Some("remote_file_present") => "mirror_file_missing",
        Some("remote_parse") => "mirror_parse_failed",
        Some("remote_digest_current") => "mirror_stale",
        Some("remote_binding") => "mirror_binding_stale",
        Some("remote_comment_surface") => "mirror_comment_surface_stale",
        Some("receipt_current") => "mirror_receipt_stale",
        Some("provider_readback_ready") => "connector_readback_not_ready",
        Some("remote_auth") => "connector_auth_missing",
        Some("remote_target_valid") => "remote_target_invalid",
        Some("remote_issue_read") => "remote_issue_read_failed",
        Some("remote_comments_read") => "remote_comments_read_failed",
        Some("remote_identity") => "remote_identity_stale",
        Some("remote_status") => "remote_status_stale",
        Some("remote_issue_body") => "remote_issue_body_stale",
        Some("write_receipt_binding") => "remote_write_receipt_stale",
        _ => "connector_mirror_stale",
    }
}

fn compact_issue_mirror_verify(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    expected_digest: &MirrorFileDigest,
    actual_digest: Option<&MirrorFileDigest>,
    receipt: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut failures = Vec::new();
    if actual_digest.is_none() {
        failures.push("mirror_file_missing".to_string());
    }
    if let Some(actual) = actual_digest {
        if actual.sha256 != expected_digest.sha256 || actual.bytes != expected_digest.bytes {
            failures.push("mirror_current_mismatch".to_string());
        }
    }
    let receipt_summary = if let Some(receipt) = receipt {
        let receipt_sha = json_pointer_str(receipt, "/mirror/sha256");
        let receipt_bytes = json_pointer_u64(receipt, "/mirror/bytes");
        let receipt_issue_status = json_pointer_str(receipt, "/issue/status");
        let receipt_issue_updated_at = json_pointer_str(receipt, "/issue/updated_at");
        let receipt_loop_round = json_pointer_i64(receipt, "/loop/round");
        if json_pointer_str(receipt, "/schema_version")
            != Some(ISSUE_MIRROR_SYNC_RECEIPT_SCHEMA_VERSION)
        {
            failures.push("receipt_schema_mismatch".to_string());
        }
        if Some(mirror.issue.id) != json_pointer_i64(receipt, "/issue/id") {
            failures.push("receipt_issue_id_mismatch".to_string());
        }
        if Some(mirror.issue.status.as_str()) != receipt_issue_status {
            failures.push("receipt_issue_status_mismatch".to_string());
        }
        if Some(mirror.issue.updated_at.as_str()) != receipt_issue_updated_at {
            failures.push("receipt_issue_updated_at_mismatch".to_string());
        }
        if mirror
            .loop_contract
            .as_ref()
            .map(|contract| contract.current_round)
            != receipt_loop_round
        {
            failures.push("receipt_loop_round_mismatch".to_string());
        }
        if let Some(actual) = actual_digest {
            if Some(actual.sha256.as_str()) != receipt_sha || Some(actual.bytes) != receipt_bytes {
                failures.push("receipt_file_digest_mismatch".to_string());
            }
        }
        serde_json::json!({
            "found": true,
            "schema_version": json_pointer_str(receipt, "/schema_version"),
            "sha256": receipt_sha,
            "bytes": receipt_bytes,
            "issue_status": receipt_issue_status,
            "issue_updated_at": receipt_issue_updated_at,
            "loop_round": receipt_loop_round
        })
    } else {
        failures.push("receipt_file_missing".to_string());
        serde_json::json!({
            "found": false
        })
    };
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_VERIFY_SCHEMA_VERSION,
        "passed": failures.is_empty(),
        "failures": failures,
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": mirror.issue.id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "current": {
            "bytes": expected_digest.bytes,
            "sha256": expected_digest.sha256.as_str()
        },
        "file": actual_digest.map(|digest| serde_json::json!({
            "bytes": digest.bytes,
            "sha256": digest.sha256.as_str()
        })),
        "receipt": receipt_summary,
        "sync_command": format!("entrance hive issue mirror-sync {}", mirror.issue.id),
        "verify_command": format!("entrance hive issue mirror-verify {}", mirror.issue.id)
    })
}

fn compact_issue_mirror_audit(verify: &serde_json::Value) -> serde_json::Value {
    let failures = verify_failures(verify);
    let issue_id = verify.pointer("/issue_id").and_then(|value| value.as_i64());
    let checks = vec![
        mirror_audit_check(
            "mirror_file_current",
            "Mirror file matches the current Hive issue mirror.",
            &failures,
            &["mirror_file_missing", "mirror_current_mismatch"],
            serde_json::json!({
                "path": verify.pointer("/path"),
                "current": verify.pointer("/current"),
                "file": verify.pointer("/file")
            }),
        ),
        mirror_audit_check(
            "receipt_schema",
            "Mirror receipt exists and uses the expected schema.",
            &failures,
            &["receipt_file_missing", "receipt_schema_mismatch"],
            serde_json::json!({
                "receipt_path": verify.pointer("/receipt_path"),
                "receipt": verify.pointer("/receipt")
            }),
        ),
        mirror_audit_check(
            "receipt_binding",
            "Mirror receipt is bound to the current issue status, update time, and loop round.",
            &failures,
            &[
                "receipt_issue_id_mismatch",
                "receipt_issue_status_mismatch",
                "receipt_issue_updated_at_mismatch",
                "receipt_loop_round_mismatch",
            ],
            serde_json::json!({
                "issue": {
                    "id": verify.pointer("/issue_id"),
                    "status": verify.pointer("/issue_status"),
                    "loop_id": verify.pointer("/loop_id"),
                    "loop_round": verify.pointer("/loop_round")
                },
                "receipt": verify.pointer("/receipt")
            }),
        ),
        mirror_audit_check(
            "receipt_digest",
            "Mirror receipt digest matches the written mirror file.",
            &failures,
            &["receipt_file_digest_mismatch"],
            serde_json::json!({
                "file": verify.pointer("/file"),
                "receipt": verify.pointer("/receipt")
            }),
        ),
    ];
    let failed_checks = checks
        .iter()
        .filter(|check| {
            !check
                .pointer("/passed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .filter_map(|check| {
            check
                .pointer("/name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_AUDIT_SCHEMA_VERSION,
        "passed": failed_checks.is_empty(),
        "failed_count": failed_checks.len(),
        "failed_checks": failed_checks,
        "issue_id": issue_id,
        "provider": verify.pointer("/provider"),
        "review_surface": verify.pointer("/review_surface"),
        "external_key": verify.pointer("/external_key"),
        "gate": {
            "schema_version": POLICY_SCHEMA_VERSION,
            "gate": CONNECTOR_MIRROR_RECEIPT_GATE,
            "description": "Connector mirror receipts must match the current Hive issue mirror before external issue/status/comment surfaces trust them.",
            "expected_object_kind": CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND
        },
        "verify": verify,
        "checks": checks,
        "actions": [
            compact_loop_action(
                "publish",
                "Publish",
                format!("entrance hive issue mirror-publish {} --compact", issue_id.unwrap_or_default())
            ),
            compact_loop_action(
                "verify",
                "Verify",
                format!("entrance hive issue mirror-verify {}", issue_id.unwrap_or_default())
            ),
            compact_loop_action(
                "audit",
                "Audit",
                format!("entrance hive issue mirror-audit {} --compact", issue_id.unwrap_or_default())
            )
        ]
    })
}

fn compact_issue_mirror_audit_summary(report: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_audit.compact.v1",
        "source_schema_version": report.pointer("/schema_version").and_then(|value| value.as_str()),
        "passed": report.pointer("/passed").and_then(|value| value.as_bool()),
        "failed_count": report.pointer("/failed_count").and_then(|value| value.as_u64()),
        "failed_checks": report.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "issue_id": report.pointer("/issue_id").and_then(|value| value.as_i64()),
        "provider": report.pointer("/provider").and_then(|value| value.as_str()),
        "review_surface": report.pointer("/review_surface").and_then(|value| value.as_str()),
        "gate": report.pointer("/gate/gate").and_then(|value| value.as_str()),
        "path": report.pointer("/verify/path").and_then(|value| value.as_str()),
        "receipt_path": report.pointer("/verify/receipt_path").and_then(|value| value.as_str()),
        "sha256": report.pointer("/verify/current/sha256").and_then(|value| value.as_str()),
        "actions": report.pointer("/actions").cloned().unwrap_or_else(|| serde_json::json!([]))
    })
}

fn compact_issue_mirror_readback(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    expected_digest: &MirrorFileDigest,
    actual_digest: Option<&MirrorFileDigest>,
    receipt: Option<&serde_json::Value>,
    remote_mirror: Option<&IssueMirrorReport>,
    remote_parse_error: Option<&str>,
    verify: &serde_json::Value,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    let receipt_failures = verify_failures(verify)
        .into_iter()
        .filter(|failure| failure.starts_with("receipt_"))
        .collect::<Vec<_>>();
    let remote_digest_current = actual_digest
        .map(|actual| {
            actual.sha256 == expected_digest.sha256 && actual.bytes == expected_digest.bytes
        })
        .unwrap_or(false);
    let remote_parse_current =
        actual_digest.is_none() || (remote_parse_error.is_none() && remote_mirror.is_some());
    let remote_binding_current = remote_mirror
        .map(|remote| issue_mirror_binding_current(mirror, remote))
        .unwrap_or(actual_digest.is_none() || remote_parse_error.is_some());
    let remote_comment_surface_current = remote_mirror
        .map(|remote| issue_mirror_comment_surface_current(mirror, remote))
        .unwrap_or(actual_digest.is_none() || remote_parse_error.is_some());

    let checks = vec![
        readback_check(
            "remote_file_present",
            "Connector mirror file exists and can be read back.",
            actual_digest.is_some(),
            serde_json::json!({
                "path": path.display().to_string(),
                "file": actual_digest.map(compact_digest)
            }),
        ),
        readback_check(
            "remote_parse",
            "Connector mirror file parses as a typed issue mirror.",
            remote_parse_current,
            serde_json::json!({
                "schema_version": remote_mirror.map(|remote| remote.schema_version.as_str()),
                "error": remote_parse_error
            }),
        ),
        readback_check(
            "remote_digest_current",
            "Read-back mirror bytes match the current Hive issue mirror digest.",
            remote_digest_current,
            serde_json::json!({
                "current": compact_digest(expected_digest),
                "remote": actual_digest.map(compact_digest)
            }),
        ),
        readback_check(
            "remote_binding",
            "Read-back mirror keeps the current provider, issue, loop, and external key binding.",
            remote_binding_current,
            serde_json::json!({
                "current": issue_mirror_readback_surface(mirror),
                "remote": remote_mirror.map(issue_mirror_readback_surface)
            }),
        ),
        readback_check(
            "remote_comment_surface",
            "Read-back mirror exposes the current issue comment surface.",
            remote_comment_surface_current,
            serde_json::json!({
                "current": issue_mirror_comment_surface(mirror),
                "remote": remote_mirror.map(issue_mirror_comment_surface)
            }),
        ),
        readback_check(
            "receipt_current",
            "Read-back receipt is current for the remote mirror file.",
            receipt_failures.is_empty(),
            serde_json::json!({
                "receipt_path": receipt_path.display().to_string(),
                "receipt": receipt,
                "failures": receipt_failures
            }),
        ),
    ];
    let failed_checks = checks
        .iter()
        .filter(|check| {
            !check
                .pointer("/passed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .filter_map(|check| {
            check
                .pointer("/name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    let issue_id = mirror.issue.id;

    serde_json::json!({
        "schema_version": ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
        "passed": failed_checks.is_empty(),
        "failed_count": failed_checks.len(),
        "failed_checks": failed_checks,
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": issue_id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "current": {
            "digest": compact_digest(expected_digest),
            "surface": issue_mirror_readback_surface(mirror),
            "comments": issue_mirror_comment_surface(mirror)
        },
        "remote": {
            "found": actual_digest.is_some(),
            "parsed": remote_mirror.is_some(),
            "parse_error": remote_parse_error,
            "digest": actual_digest.map(compact_digest),
            "surface": remote_mirror.map(issue_mirror_readback_surface)
        },
        "receipt": {
            "found": receipt.is_some(),
            "schema_version": receipt.and_then(|value| json_pointer_str(value, "/schema_version")),
            "sha256": receipt.and_then(|value| json_pointer_str(value, "/mirror/sha256")),
            "bytes": receipt.and_then(|value| json_pointer_u64(value, "/mirror/bytes")),
            "issue_status": receipt.and_then(|value| json_pointer_str(value, "/issue/status")),
            "loop_round": receipt.and_then(|value| json_pointer_i64(value, "/loop/round")),
            "remote_write_receipt": receipt.and_then(|value| value.pointer("/remote_write_receipt")),
            "remote_write_execution": receipt.and_then(|value| value.pointer("/remote_write_execution"))
        },
        "remote_contract": compact_connector_remote_contract(provider),
        "remote_target": connector_remote_target(
            provider,
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_readback": connector_remote_readback_report(
            mirror,
            path,
            expected_digest,
            actual_digest,
            receipt,
            remote_mirror,
            verify,
            provider
        ),
        "verify": verify,
        "checks": checks,
        "actions": [
            compact_loop_action(
                "publish",
                "Publish",
                format!("entrance hive issue mirror-publish {} --compact", issue_id)
            ),
            compact_loop_action(
                "readback",
                "Readback",
                format!("entrance hive issue mirror-readback {} --record --compact", issue_id)
            ),
            compact_loop_action(
                "audit",
                "Audit",
                format!("entrance hive issue mirror-audit {} --compact", issue_id)
            ),
            compact_loop_action(
                "roundtrip",
                "Roundtrip",
                format!("entrance hive issue mirror-roundtrip {} --compact", issue_id)
            )
        ]
    })
}

fn connector_remote_readback_report(
    mirror: &IssueMirrorReport,
    path: &Path,
    expected_digest: &MirrorFileDigest,
    actual_digest: Option<&MirrorFileDigest>,
    receipt: Option<&serde_json::Value>,
    remote_mirror: Option<&IssueMirrorReport>,
    verify: &serde_json::Value,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    let Some(provider) =
        provider.filter(|provider| connector_provider_uses_remote_contract(provider))
    else {
        return serde_json::Value::Null;
    };
    let remote_identity_current = remote_mirror
        .map(|remote| issue_mirror_identity_current(mirror, remote))
        .unwrap_or(false);
    let remote_status_current = remote_mirror
        .map(|remote| remote.issue.status == mirror.issue.status)
        .unwrap_or(false);
    let remote_comment_surface_current = remote_mirror
        .map(|remote| issue_mirror_comment_surface_current(mirror, remote))
        .unwrap_or(false);
    let remote_write_receipt = receipt.and_then(|value| value.pointer("/remote_write_receipt"));
    let receipt_failures = verify_failures(verify)
        .into_iter()
        .filter(|failure| failure.starts_with("receipt_"))
        .collect::<Vec<_>>();
    let write_receipt_binding_current = remote_write_receipt
        .and_then(|value| json_pointer_str(value, "/source_mirror_sha256"))
        == Some(expected_digest.sha256.as_str())
        && receipt_failures.is_empty();
    let remote_target =
        connector_remote_target(Some(provider), &mirror.review_surface, &mirror.external_key);
    let checks = vec![
        readback_check(
            "remote_identity",
            "Remote issue object keeps the current provider, issue, loop, and external key binding.",
            remote_identity_current,
            serde_json::json!({
                "current": issue_mirror_readback_surface(mirror),
                "remote": remote_mirror.map(issue_mirror_readback_surface)
            }),
        ),
        readback_check(
            "remote_status",
            "Remote issue status matches the current Hive issue status.",
            remote_status_current,
            serde_json::json!({
                "current": mirror.issue.status.as_str(),
                "remote": remote_mirror.map(|remote| remote.issue.status.as_str())
            }),
        ),
        readback_check(
            "remote_comment_surface",
            "Remote issue comment surface matches the current Hive comment ledger.",
            remote_comment_surface_current,
            serde_json::json!({
                "current": issue_mirror_comment_surface(mirror),
                "remote": remote_mirror.map(issue_mirror_comment_surface)
            }),
        ),
        readback_check(
            "write_receipt_binding",
            "Remote readback is bound to the current remote write receipt.",
            write_receipt_binding_current,
            serde_json::json!({
                "receipt_schema_version": remote_write_receipt.and_then(|value| json_pointer_str(value, "/schema_version")),
                "source_mirror_sha256": remote_write_receipt.and_then(|value| json_pointer_str(value, "/source_mirror_sha256")),
                "expected_source_mirror_sha256": expected_digest.sha256.as_str(),
                "receipt_failures": receipt_failures
            }),
        ),
    ];
    let failed_checks = checks
        .iter()
        .filter(|check| {
            !check
                .pointer("/passed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .filter_map(|check| {
            check
                .pointer("/name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
        "passed": failed_checks.is_empty(),
        "failed_count": failed_checks.len(),
        "failed_checks": failed_checks,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "remote_id": connector_remote_id(mirror, provider, &remote_target),
        "remote_url": connector_remote_url(mirror, provider, path, &remote_target),
        "external_key": mirror.external_key.as_str(),
        "remote_target": remote_target,
        "source_mirror_sha256": expected_digest.sha256.as_str(),
        "remote_mirror_sha256": actual_digest.map(|digest| digest.sha256.as_str()),
        "status": mirror.issue.status.as_str(),
        "comment_count": mirror.comments.len(),
        "write_receipt": remote_write_receipt,
        "checks": checks
    })
}

fn compact_github_issue_mirror_readback(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    expected_digest: &MirrorFileDigest,
    receipt: Option<&serde_json::Value>,
    provider: &ConnectorProviderSpec,
) -> serde_json::Value {
    let remote_target =
        connector_remote_target(Some(provider), &mirror.review_surface, &mirror.external_key);
    let execution = execute_github_remote_readback(provider, &remote_target, receipt);
    let remote_readback = connector_github_remote_readback_report(
        mirror,
        expected_digest,
        receipt,
        provider,
        &remote_target,
        &execution,
    );
    let failed_checks = remote_readback
        .pointer("/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(["remote_readback_failed"]));
    let passed = failed_checks
        .as_array()
        .map(|checks| checks.is_empty())
        .unwrap_or(false);
    let issue_summary = execution
        .pointer("/issue/response/summary")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(null));
    let comments_summary = execution
        .pointer("/comments/response/summary")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "count": 0, "items": [] }));
    let response_digest = execution
        .pointer("/response_digest")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(null));
    let checks = remote_readback
        .pointer("/checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let issue_id = mirror.issue.id;

    serde_json::json!({
        "schema_version": ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
        "passed": passed,
        "failed_count": failed_checks.as_array().map(Vec::len).unwrap_or_default(),
        "failed_checks": failed_checks,
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": issue_id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "current": {
            "digest": compact_digest(expected_digest),
            "surface": issue_mirror_readback_surface(mirror),
            "comments": issue_mirror_comment_surface(mirror)
        },
        "remote": {
            "found": execution.pointer("/issue/success").and_then(|value| value.as_bool()).unwrap_or(false),
            "parsed": execution.pointer("/issue/response/parsed").and_then(|value| value.as_bool()).unwrap_or(false),
            "digest": response_digest,
            "surface": {
                "schema_version": "entrance.hive.github_issue_surface.v1",
                "provider": provider.name.as_str(),
                "remote_target": remote_target,
                "issue": issue_summary,
                "comments": comments_summary
            }
        },
        "receipt": {
            "found": receipt.is_some(),
            "schema_version": receipt.and_then(|value| json_pointer_str(value, "/schema_version")),
            "issue_status": receipt.and_then(|value| json_pointer_str(value, "/issue/status")),
            "loop_round": receipt.and_then(|value| json_pointer_i64(value, "/loop/round")),
            "remote_write_receipt": receipt.and_then(|value| value.pointer("/remote_write_receipt")),
            "remote_write_execution": receipt.and_then(|value| value.pointer("/remote_write_execution"))
        },
        "remote_contract": compact_connector_remote_contract(Some(provider)),
        "remote_target": connector_remote_target(
            Some(provider),
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_readback": remote_readback,
        "verify": serde_json::Value::Null,
        "checks": checks,
        "actions": [
            compact_loop_action(
                "publish",
                "Publish",
                format!("entrance hive issue mirror-publish {} --compact", issue_id)
            ),
            compact_loop_action(
                "readback",
                "Readback",
                format!("entrance hive issue mirror-readback {} --record --compact", issue_id)
            ),
            compact_loop_action(
                "admit",
                "Admit",
                format!("entrance hive issue mirror-admit {} --compact", issue_id)
            ),
            compact_loop_action(
                "roundtrip",
                "Roundtrip",
                format!("entrance hive issue mirror-roundtrip {} --compact", issue_id)
            )
        ]
    })
}

fn connector_github_remote_readback_report(
    mirror: &IssueMirrorReport,
    expected_digest: &MirrorFileDigest,
    receipt: Option<&serde_json::Value>,
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    execution: &serde_json::Value,
) -> serde_json::Value {
    let source_issue = connector_remote_write_issue_from_mirror(mirror, expected_digest);
    let expected_title = source_issue
        .pointer("/title")
        .and_then(|value| value.as_str())
        .unwrap_or("Entrance issue");
    let expected_body = connector_remote_issue_body(&source_issue);
    let expected_state = connector_github_issue_state(&mirror.issue.status);
    let expected_state_reason = connector_github_issue_state_reason(&mirror.issue.status);
    let expected_comment = mirror
        .comments
        .last()
        .map(|_| connector_github_remote_comment_body(&source_issue));
    let issue_summary = execution.pointer("/issue/response/summary");
    let comments_summary = execution.pointer("/comments/response/summary");
    let remote_issue_number = issue_summary
        .and_then(|value| value.pointer("/number"))
        .and_then(|value| value.as_u64());
    let expected_issue_number = connector_github_readback_issue_number(remote_target, receipt);
    let remote_state = issue_summary
        .and_then(|value| value.pointer("/state"))
        .and_then(|value| value.as_str());
    let remote_state_reason = issue_summary
        .and_then(|value| value.pointer("/state_reason"))
        .and_then(|value| value.as_str());
    let remote_title = issue_summary
        .and_then(|value| value.pointer("/title"))
        .and_then(|value| value.as_str());
    let remote_body = issue_summary
        .and_then(|value| value.pointer("/body"))
        .and_then(|value| value.as_str());
    let remote_comment_surface_current = expected_comment.as_ref().map_or(true, |expected| {
        github_readback_comments_contain_body(comments_summary, expected)
    });
    let remote_status_current = remote_state == Some(expected_state)
        && expected_state_reason.map_or(true, |reason| remote_state_reason == Some(reason));
    let remote_issue_body_current =
        remote_title == Some(expected_title) && remote_body == Some(expected_body.as_str());
    let remote_identity_current =
        expected_issue_number.is_some() && remote_issue_number == expected_issue_number;
    let remote_write_receipt = receipt.and_then(|value| value.pointer("/remote_write_receipt"));
    let write_receipt_binding_current = remote_write_receipt
        .and_then(|value| json_pointer_str(value, "/source_mirror_sha256"))
        == Some(expected_digest.sha256.as_str());
    let execution_failed_checks = execution
        .pointer("/failed_checks")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    let provider_ready =
        provider.status == "active" && provider.configured && provider.supports_readback;
    let target_valid = remote_target
        .pointer("/valid")
        .and_then(|value| value.as_bool())
        == Some(true);
    let auth_ready = !execution_failed_checks.contains(&"remote_auth_missing");
    let issue_read = execution
        .pointer("/issue/success")
        .and_then(|value| value.as_bool())
        == Some(true);
    let comments_read = execution
        .pointer("/comments/success")
        .and_then(|value| value.as_bool())
        == Some(true);
    let checks = vec![
        readback_check(
            "provider_readback_ready",
            "GitHub provider is active, configured, and declares remote readback support.",
            provider_ready,
            serde_json::json!({
                "status": provider.status.as_str(),
                "configured": provider.configured,
                "supports_readback": provider.supports_readback
            }),
        ),
        readback_check(
            "remote_target_valid",
            "GitHub review surface resolves to an issue target.",
            target_valid,
            serde_json::json!({
                "target": remote_target
            }),
        ),
        readback_check(
            "remote_auth",
            "GitHub token is available before remote readback.",
            auth_ready,
            serde_json::json!({
                "auth_env": provider.auth_env.iter().map(String::as_str).collect::<Vec<_>>()
            }),
        ),
        readback_check(
            "remote_issue_read",
            "GitHub issue can be read after write.",
            issue_read,
            serde_json::json!({
                "request": execution.pointer("/issue/request"),
                "http_status": execution.pointer("/issue/http_status"),
                "failed_check": execution.pointer("/issue/failed_check")
            }),
        ),
        readback_check(
            "remote_comments_read",
            "GitHub issue comments can be read after write.",
            comments_read,
            serde_json::json!({
                "request": execution.pointer("/comments/request"),
                "http_status": execution.pointer("/comments/http_status"),
                "failed_check": execution.pointer("/comments/failed_check")
            }),
        ),
        readback_check(
            "remote_identity",
            "GitHub issue number matches the connector target or write receipt.",
            remote_identity_current,
            serde_json::json!({
                "expected_issue_number": expected_issue_number,
                "remote_issue_number": remote_issue_number,
                "remote_id": remote_write_receipt.and_then(|value| json_pointer_str(value, "/remote_id"))
            }),
        ),
        readback_check(
            "remote_status",
            "GitHub issue state matches the current Hive issue status.",
            remote_status_current,
            serde_json::json!({
                "expected_state": expected_state,
                "expected_state_reason": expected_state_reason,
                "remote_state": remote_state,
                "remote_state_reason": remote_state_reason
            }),
        ),
        readback_check(
            "remote_issue_body",
            "GitHub issue title and body match the current Hive issue mirror.",
            remote_issue_body_current,
            serde_json::json!({
                "expected_title": expected_title,
                "remote_title": remote_title,
                "expected_body_sha256": digest_bytes(expected_body.as_bytes()).sha256,
                "remote_body_sha256": remote_body.map(|body| digest_bytes(body.as_bytes()).sha256)
            }),
        ),
        readback_check(
            "remote_comment_surface",
            "GitHub comments include the latest Hive issue comment body.",
            remote_comment_surface_current,
            serde_json::json!({
                "expected_comment_sha256": expected_comment.as_ref().map(|body| digest_bytes(body.as_bytes()).sha256),
                "remote_comment_count": comments_summary
                    .and_then(|value| value.pointer("/count"))
                    .and_then(|value| value.as_u64())
            }),
        ),
        readback_check(
            "write_receipt_binding",
            "GitHub readback is bound to the current remote write receipt.",
            write_receipt_binding_current,
            serde_json::json!({
                "receipt_found": receipt.is_some(),
                "receipt_schema_version": remote_write_receipt.and_then(|value| json_pointer_str(value, "/schema_version")),
                "source_mirror_sha256": remote_write_receipt.and_then(|value| json_pointer_str(value, "/source_mirror_sha256")),
                "expected_source_mirror_sha256": expected_digest.sha256.as_str()
            }),
        ),
    ];
    let failed_checks = checks
        .iter()
        .filter(|check| {
            !check
                .pointer("/passed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .filter_map(|check| {
            check
                .pointer("/name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
        "passed": failed_checks.is_empty(),
        "failed_count": failed_checks.len(),
        "failed_checks": failed_checks,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "remote_id": connector_remote_id(mirror, provider, remote_target),
        "remote_url": connector_remote_url(mirror, provider, Path::new(""), remote_target),
        "external_key": mirror.external_key.as_str(),
        "remote_target": remote_target,
        "source_mirror_sha256": expected_digest.sha256.as_str(),
        "remote_mirror_sha256": execution.pointer("/response_digest/sha256").and_then(|value| value.as_str()),
        "status": mirror.issue.status.as_str(),
        "comment_count": mirror.comments.len(),
        "write_receipt": remote_write_receipt,
        "execution": execution,
        "checks": checks
    })
}

fn compact_linear_issue_mirror_readback(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    expected_digest: &MirrorFileDigest,
    receipt: Option<&serde_json::Value>,
    provider: &ConnectorProviderSpec,
) -> serde_json::Value {
    let remote_target =
        connector_remote_target(Some(provider), &mirror.review_surface, &mirror.external_key);
    let execution = execute_linear_remote_readback(provider, &remote_target, receipt);
    let remote_readback = connector_linear_remote_readback_report(
        mirror,
        expected_digest,
        receipt,
        provider,
        &remote_target,
        &execution,
    );
    let failed_checks = remote_readback
        .pointer("/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(["remote_readback_failed"]));
    let passed = failed_checks
        .as_array()
        .map(|checks| checks.is_empty())
        .unwrap_or(false);
    let issue_summary = execution
        .pointer("/issue/response/summary")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(null));
    let comments_summary = issue_summary
        .pointer("/comments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "count": 0, "items": [] }));
    let response_digest = execution
        .pointer("/response_digest")
        .cloned()
        .unwrap_or_else(|| serde_json::json!(null));
    let checks = remote_readback
        .pointer("/checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let issue_id = mirror.issue.id;

    serde_json::json!({
        "schema_version": ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
        "passed": passed,
        "failed_count": failed_checks.as_array().map(Vec::len).unwrap_or_default(),
        "failed_checks": failed_checks,
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue_id": issue_id,
        "issue_status": mirror.issue.status.as_str(),
        "loop_id": mirror.issue.loop_id,
        "loop_round": mirror.loop_contract.as_ref().map(|contract| contract.current_round),
        "path": path.display().to_string(),
        "receipt_path": receipt_path.display().to_string(),
        "current": {
            "digest": compact_digest(expected_digest),
            "surface": issue_mirror_readback_surface(mirror),
            "comments": issue_mirror_comment_surface(mirror)
        },
        "remote": {
            "found": execution.pointer("/issue/success").and_then(|value| value.as_bool()).unwrap_or(false),
            "parsed": execution.pointer("/issue/response/parsed").and_then(|value| value.as_bool()).unwrap_or(false),
            "digest": response_digest,
            "surface": {
                "schema_version": "entrance.hive.linear_issue_surface.v1",
                "provider": provider.name.as_str(),
                "remote_target": remote_target,
                "issue": issue_summary,
                "comments": comments_summary
            }
        },
        "receipt": {
            "found": receipt.is_some(),
            "schema_version": receipt.and_then(|value| json_pointer_str(value, "/schema_version")),
            "issue_status": receipt.and_then(|value| json_pointer_str(value, "/issue/status")),
            "loop_round": receipt.and_then(|value| json_pointer_i64(value, "/loop/round")),
            "remote_write_receipt": receipt.and_then(|value| value.pointer("/remote_write_receipt")),
            "remote_write_execution": receipt.and_then(|value| value.pointer("/remote_write_execution"))
        },
        "remote_contract": compact_connector_remote_contract(Some(provider)),
        "remote_target": connector_remote_target(
            Some(provider),
            &mirror.review_surface,
            &mirror.external_key
        ),
        "remote_readback": remote_readback,
        "verify": serde_json::Value::Null,
        "checks": checks,
        "actions": [
            compact_loop_action(
                "publish",
                "Publish",
                format!("entrance hive issue mirror-publish {} --compact", issue_id)
            ),
            compact_loop_action(
                "readback",
                "Readback",
                format!("entrance hive issue mirror-readback {} --record --compact", issue_id)
            ),
            compact_loop_action(
                "admit",
                "Admit",
                format!("entrance hive issue mirror-admit {} --compact", issue_id)
            ),
            compact_loop_action(
                "roundtrip",
                "Roundtrip",
                format!("entrance hive issue mirror-roundtrip {} --compact", issue_id)
            )
        ]
    })
}

fn connector_linear_remote_readback_report(
    mirror: &IssueMirrorReport,
    expected_digest: &MirrorFileDigest,
    receipt: Option<&serde_json::Value>,
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    execution: &serde_json::Value,
) -> serde_json::Value {
    let source_issue = connector_remote_write_issue_from_mirror(mirror, expected_digest);
    let expected_title = source_issue
        .pointer("/title")
        .and_then(|value| value.as_str())
        .unwrap_or("Entrance issue");
    let expected_body = connector_remote_issue_body(&source_issue);
    let expected_comment = mirror
        .comments
        .last()
        .map(|_| connector_linear_remote_comment_body(&source_issue));
    let issue_summary = execution.pointer("/issue/response/summary");
    let comments_summary = issue_summary.and_then(|value| value.pointer("/comments"));
    let expected_issue_key = remote_target
        .pointer("/issue_key")
        .and_then(|value| value.as_str());
    let remote_issue_key = issue_summary
        .and_then(|value| value.pointer("/identifier"))
        .and_then(|value| value.as_str());
    let remote_title = issue_summary
        .and_then(|value| value.pointer("/title"))
        .and_then(|value| value.as_str());
    let remote_body = issue_summary
        .and_then(|value| value.pointer("/description"))
        .and_then(|value| value.as_str());
    let remote_state_name = issue_summary
        .and_then(|value| value.pointer("/state/name"))
        .and_then(|value| value.as_str());
    let remote_comment_surface_current = expected_comment.as_ref().map_or(true, |expected| {
        linear_readback_comments_contain_body(comments_summary, expected)
    });
    let expected_status = mirror.issue.status.as_str();
    let remote_status_current = remote_state_name == Some(expected_status)
        || remote_body
            .map(|body| body.contains(&format!("Status: {expected_status}")))
            .unwrap_or(false);
    let remote_issue_body_current =
        remote_title == Some(expected_title) && remote_body == Some(expected_body.as_str());
    let remote_identity_current =
        expected_issue_key.is_some() && remote_issue_key == expected_issue_key;
    let remote_write_receipt = receipt.and_then(|value| value.pointer("/remote_write_receipt"));
    let write_receipt_binding_current = remote_write_receipt
        .and_then(|value| json_pointer_str(value, "/source_mirror_sha256"))
        == Some(expected_digest.sha256.as_str());
    let execution_failed_checks = execution
        .pointer("/failed_checks")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    let provider_ready =
        provider.status == "active" && provider.configured && provider.supports_readback;
    let target_valid = remote_target
        .pointer("/valid")
        .and_then(|value| value.as_bool())
        == Some(true);
    let auth_ready = !execution_failed_checks.contains(&"remote_auth_missing");
    let issue_read = execution
        .pointer("/issue/success")
        .and_then(|value| value.as_bool())
        == Some(true);
    let comments_read = issue_read && comments_summary.is_some();
    let checks = vec![
        readback_check(
            "provider_readback_ready",
            "Linear provider is active, configured, and declares remote readback support.",
            provider_ready,
            serde_json::json!({
                "status": provider.status.as_str(),
                "configured": provider.configured,
                "supports_readback": provider.supports_readback
            }),
        ),
        readback_check(
            "remote_target_valid",
            "Linear review surface resolves to an issue target.",
            target_valid,
            serde_json::json!({
                "target": remote_target
            }),
        ),
        readback_check(
            "remote_auth",
            "Linear token is available before remote readback.",
            auth_ready,
            serde_json::json!({
                "auth_env": provider.auth_env.iter().map(String::as_str).collect::<Vec<_>>()
            }),
        ),
        readback_check(
            "remote_issue_read",
            "Linear issue can be read after write.",
            issue_read,
            serde_json::json!({
                "request": execution.pointer("/issue/request"),
                "http_status": execution.pointer("/issue/http_status"),
                "failed_check": execution.pointer("/issue/failed_check")
            }),
        ),
        readback_check(
            "remote_comments_read",
            "Linear issue comments can be read after write.",
            comments_read,
            serde_json::json!({
                "request": execution.pointer("/issue/request"),
                "http_status": execution.pointer("/issue/http_status"),
                "failed_check": execution.pointer("/issue/failed_check")
            }),
        ),
        readback_check(
            "remote_identity",
            "Linear issue identifier matches the connector target.",
            remote_identity_current,
            serde_json::json!({
                "expected_issue_key": expected_issue_key,
                "remote_issue_key": remote_issue_key,
                "remote_id": remote_write_receipt.and_then(|value| json_pointer_str(value, "/remote_id"))
            }),
        ),
        readback_check(
            "remote_status",
            "Linear issue state or mirrored description matches the current Hive issue status.",
            remote_status_current,
            serde_json::json!({
                "expected_status": expected_status,
                "remote_state_name": remote_state_name,
                "description_status_marker": remote_body.map(|body| body.contains(&format!("Status: {expected_status}")))
            }),
        ),
        readback_check(
            "remote_issue_body",
            "Linear issue title and description match the current Hive issue mirror.",
            remote_issue_body_current,
            serde_json::json!({
                "expected_title": expected_title,
                "remote_title": remote_title,
                "expected_body_sha256": digest_bytes(expected_body.as_bytes()).sha256,
                "remote_body_sha256": remote_body.map(|body| digest_bytes(body.as_bytes()).sha256)
            }),
        ),
        readback_check(
            "remote_comment_surface",
            "Linear comments include the latest Hive issue comment body.",
            remote_comment_surface_current,
            serde_json::json!({
                "expected_comment_sha256": expected_comment.as_ref().map(|body| digest_bytes(body.as_bytes()).sha256),
                "remote_comment_count": comments_summary
                    .and_then(|value| value.pointer("/count"))
                    .and_then(|value| value.as_u64())
            }),
        ),
        readback_check(
            "write_receipt_binding",
            "Linear readback is bound to the current remote write receipt.",
            write_receipt_binding_current,
            serde_json::json!({
                "receipt_found": receipt.is_some(),
                "receipt_schema_version": remote_write_receipt.and_then(|value| json_pointer_str(value, "/schema_version")),
                "source_mirror_sha256": remote_write_receipt.and_then(|value| json_pointer_str(value, "/source_mirror_sha256")),
                "expected_source_mirror_sha256": expected_digest.sha256.as_str()
            }),
        ),
    ];
    let failed_checks = checks
        .iter()
        .filter(|check| {
            !check
                .pointer("/passed")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .filter_map(|check| {
            check
                .pointer("/name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
        "passed": failed_checks.is_empty(),
        "failed_count": failed_checks.len(),
        "failed_checks": failed_checks,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "remote_id": connector_remote_id(mirror, provider, remote_target),
        "remote_url": connector_remote_url(mirror, provider, Path::new(""), remote_target),
        "external_key": mirror.external_key.as_str(),
        "remote_target": remote_target,
        "source_mirror_sha256": expected_digest.sha256.as_str(),
        "remote_mirror_sha256": execution.pointer("/response_digest/sha256").and_then(|value| value.as_str()),
        "status": mirror.issue.status.as_str(),
        "comment_count": mirror.comments.len(),
        "write_receipt": remote_write_receipt,
        "execution": execution,
        "checks": checks
    })
}

fn execute_linear_remote_readback(
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    receipt: Option<&serde_json::Value>,
) -> serde_json::Value {
    let provider_name = provider.name.clone();
    let provider = provider.clone();
    let remote_target = remote_target.clone();
    let receipt = receipt.cloned();
    std::thread::spawn(move || {
        execute_linear_remote_readback_blocking(&provider, &remote_target, receipt.as_ref())
    })
    .join()
    .unwrap_or_else(|_| {
        serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "provider": provider_name,
            "success": false,
            "failed_checks": ["remote_readback_thread_panic"],
            "issue": serde_json::Value::Null
        })
    })
}

fn execute_linear_remote_readback_blocking(
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    _receipt: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut setup_failed = Vec::new();
    if provider.status != "active" {
        setup_failed.push("provider_not_active".to_string());
    }
    if !provider.configured {
        setup_failed.push("connector_not_configured".to_string());
    }
    if !provider.supports_readback {
        setup_failed.push("readback_not_supported".to_string());
    }
    if remote_target
        .pointer("/valid")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        setup_failed.push("remote_target_invalid".to_string());
    }
    let issue_key = remote_target
        .pointer("/issue_key")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if issue_key.is_empty() {
        setup_failed.push("linear_issue_key_missing".to_string());
    }
    setup_failed.sort();
    setup_failed.dedup();
    if !setup_failed.is_empty() {
        return serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "provider": provider.name.as_str(),
            "success": false,
            "failed_checks": setup_failed,
            "issue": serde_json::Value::Null
        });
    }
    let Some(token) = connector_provider_auth_token(provider) else {
        return serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "provider": provider.name.as_str(),
            "success": false,
            "failed_checks": ["remote_auth_missing"],
            "issue": serde_json::Value::Null
        });
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Entrance-Hive/2.0")
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return serde_json::json!({
                "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
                "provider": provider.name.as_str(),
                "success": false,
                "failed_checks": ["http_client_unavailable"],
                "error": error.to_string(),
                "issue": serde_json::Value::Null
            });
        }
    };
    let issue = execute_linear_graphql_operation(
        &client,
        &token,
        &serde_json::json!({
            "kind": "read_issue",
            "method": "POST",
            "url": remote_target.pointer("/api_url").and_then(|value| value.as_str()).unwrap_or("https://api.linear.app/graphql"),
            "graphql": {
                "operation": "EntranceIssueRead",
                "query": "query EntranceIssueRead($id: String!) { issue(id: $id) { id identifier title description url state { id name type } comments { nodes { id body createdAt updatedAt user { name } } } } }",
                "variables": {
                    "id": issue_key
                }
            }
        }),
    );
    let mut failed_checks = Vec::new();
    if issue.pointer("/success").and_then(|value| value.as_bool()) != Some(true) {
        failed_checks.push(
            issue
                .pointer("/failed_check")
                .and_then(|value| value.as_str())
                .unwrap_or("remote_issue_read")
                .to_string(),
        );
    }
    failed_checks.sort();
    failed_checks.dedup();
    let response_digest = digest_bytes(
        serde_json::to_vec(&serde_json::json!({
            "issue": issue.pointer("/response/sha256").and_then(|value| value.as_str())
        }))
        .unwrap_or_default()
        .as_slice(),
    );
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "success": failed_checks.is_empty(),
        "failed_checks": failed_checks,
        "issue": issue,
        "response_digest": compact_digest(&response_digest)
    })
}

fn execute_github_remote_readback(
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    receipt: Option<&serde_json::Value>,
) -> serde_json::Value {
    let provider_name = provider.name.clone();
    let provider = provider.clone();
    let remote_target = remote_target.clone();
    let receipt = receipt.cloned();
    std::thread::spawn(move || {
        execute_github_remote_readback_blocking(&provider, &remote_target, receipt.as_ref())
    })
    .join()
    .unwrap_or_else(|_| {
        serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "provider": provider_name,
            "success": false,
            "failed_checks": ["remote_readback_thread_panic"],
            "issue": serde_json::Value::Null,
            "comments": serde_json::Value::Null
        })
    })
}

fn execute_github_remote_readback_blocking(
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    receipt: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut setup_failed = Vec::new();
    if provider.status != "active" {
        setup_failed.push("provider_not_active".to_string());
    }
    if !provider.configured {
        setup_failed.push("connector_not_configured".to_string());
    }
    if !provider.supports_readback {
        setup_failed.push("readback_not_supported".to_string());
    }
    if remote_target
        .pointer("/valid")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        setup_failed.push("remote_target_invalid".to_string());
    }
    let issue_number = connector_github_readback_issue_number(remote_target, receipt);
    if issue_number.is_none() {
        setup_failed.push("github_issue_number_missing".to_string());
    }
    setup_failed.sort();
    setup_failed.dedup();
    if !setup_failed.is_empty() {
        return serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "provider": provider.name.as_str(),
            "success": false,
            "failed_checks": setup_failed,
            "issue": serde_json::Value::Null,
            "comments": serde_json::Value::Null
        });
    }
    let Some(token) = connector_provider_auth_token(provider) else {
        return serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "provider": provider.name.as_str(),
            "success": false,
            "failed_checks": ["remote_auth_missing"],
            "issue": serde_json::Value::Null,
            "comments": serde_json::Value::Null
        });
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Entrance-Hive/2.0")
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return serde_json::json!({
                "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
                "provider": provider.name.as_str(),
                "success": false,
                "failed_checks": ["http_client_unavailable"],
                "error": error.to_string(),
                "issue": serde_json::Value::Null,
                "comments": serde_json::Value::Null
            });
        }
    };
    let issue_number = issue_number.unwrap_or_default();
    let (issue_url, comments_url) = connector_github_readback_urls(remote_target, issue_number);
    let issue = execute_github_remote_get_operation(&client, &token, "get_issue", &issue_url);
    let comments =
        execute_github_remote_get_operation(&client, &token, "list_issue_comments", &comments_url);
    let mut failed_checks = Vec::new();
    if issue.pointer("/success").and_then(|value| value.as_bool()) != Some(true) {
        failed_checks.push(
            issue
                .pointer("/failed_check")
                .and_then(|value| value.as_str())
                .unwrap_or("remote_issue_read")
                .to_string(),
        );
    }
    if comments
        .pointer("/success")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        failed_checks.push(
            comments
                .pointer("/failed_check")
                .and_then(|value| value.as_str())
                .unwrap_or("remote_comments_read")
                .to_string(),
        );
    }
    failed_checks.sort();
    failed_checks.dedup();
    let response_digest = digest_bytes(
        serde_json::to_vec(&serde_json::json!({
            "issue": issue.pointer("/response/sha256").and_then(|value| value.as_str()),
            "comments": comments.pointer("/response/sha256").and_then(|value| value.as_str())
        }))
        .unwrap_or_default()
        .as_slice(),
    );
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "success": failed_checks.is_empty(),
        "failed_checks": failed_checks,
        "issue": issue,
        "comments": comments,
        "response_digest": compact_digest(&response_digest)
    })
}

fn execute_github_remote_get_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    kind: &str,
    url: &str,
) -> serde_json::Value {
    if kind == "list_issue_comments" || kind == "lookup_issue_comments_for_idempotency" {
        return execute_github_remote_get_paginated_comments_operation(client, token, kind, url);
    }
    execute_github_remote_get_single_operation(client, token, kind, url)
}

fn execute_github_remote_get_paginated_comments_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    kind: &str,
    url: &str,
) -> serde_json::Value {
    let mut next_url = url.to_string();
    let mut pages = Vec::new();
    let mut comments = Vec::new();
    let mut page_sha256 = Vec::new();
    let mut response_bytes = 0_u64;
    let mut truncated = false;

    for page_index in 0..GITHUB_COMMENTS_MAX_PAGES {
        let page = execute_github_remote_get_single_operation(client, token, kind, &next_url);
        response_bytes += page
            .pointer("/response/bytes")
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        if let Some(sha256) = page
            .pointer("/response/sha256")
            .and_then(|value| value.as_str())
        {
            page_sha256.push(sha256.to_string());
        }
        pages.push(serde_json::json!({
            "index": page_index + 1,
            "url": next_url,
            "success": page.pointer("/success").and_then(|value| value.as_bool()),
            "http_status": page.pointer("/http_status").and_then(|value| value.as_u64()),
            "failed_check": page.pointer("/failed_check").and_then(|value| value.as_str()),
            "attempt_count": page.pointer("/attempt_count").and_then(|value| value.as_u64()),
            "retry": page.pointer("/retry").cloned().unwrap_or_else(|| serde_json::json!({})),
            "item_count": page.pointer("/response/summary/count").and_then(|value| value.as_u64()),
            "next_url": page.pointer("/response/headers/link").and_then(|value| value.as_str()).and_then(github_link_header_next_url)
        }));
        if page.pointer("/success").and_then(|value| value.as_bool()) != Some(true) {
            let summary = compact_github_comments_readback_summary(&serde_json::Value::Array(
                comments.clone(),
            ));
            let digest = digest_bytes(
                serde_json::to_vec(&serde_json::json!({
                    "pages": page_sha256,
                    "comments": summary
                }))
                .unwrap_or_default()
                .as_slice(),
            );
            return serde_json::json!({
                "kind": kind,
                "method": "GET",
                "url": url,
                "success": false,
                "failed_check": page.pointer("/failed_check").and_then(|value| value.as_str()).unwrap_or("remote_comments_read"),
                "http_status": page.pointer("/http_status").and_then(|value| value.as_u64()),
                "attempt_count": page.pointer("/attempt_count").and_then(|value| value.as_u64()),
                "retry": page.pointer("/retry").cloned().unwrap_or_else(|| serde_json::json!({})),
                "attempts": page.pointer("/attempts").cloned().unwrap_or_else(|| serde_json::json!([])),
                "request": {
                    "auth": "Authorization: Bearer <redacted>"
                },
                "pagination": {
                    "page_count": pages.len(),
                    "max_pages": GITHUB_COMMENTS_MAX_PAGES,
                    "truncated": truncated,
                    "pages": pages
                },
                "response": {
                    "bytes": response_bytes,
                    "sha256": digest.sha256,
                    "parsed": true,
                    "summary": summary
                }
            });
        }

        comments.extend(
            page.pointer("/response/summary/items")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .cloned(),
        );
        let next = page
            .pointer("/response/headers/link")
            .and_then(|value| value.as_str())
            .and_then(github_link_header_next_url);
        let Some(next) = next else {
            let mut summary = compact_github_comments_readback_summary(&serde_json::Value::Array(
                comments.clone(),
            ));
            if let Some(object) = summary.as_object_mut() {
                object.insert("page_count".to_string(), serde_json::json!(pages.len()));
                object.insert("truncated".to_string(), serde_json::json!(false));
            }
            let digest = digest_bytes(
                serde_json::to_vec(&serde_json::json!({
                    "pages": page_sha256,
                    "comments": summary
                }))
                .unwrap_or_default()
                .as_slice(),
            );
            return serde_json::json!({
                "kind": kind,
                "method": "GET",
                "url": url,
                "success": true,
                "failed_check": None::<&str>,
                "http_status": page.pointer("/http_status").and_then(|value| value.as_u64()),
                "request": {
                    "auth": "Authorization: Bearer <redacted>"
                },
                "pagination": {
                    "page_count": pages.len(),
                    "max_pages": GITHUB_COMMENTS_MAX_PAGES,
                    "truncated": false,
                    "pages": pages
                },
                "response": {
                    "bytes": response_bytes,
                    "sha256": digest.sha256,
                    "parsed": true,
                    "summary": summary
                }
            });
        };
        next_url = next;
    }

    truncated = true;
    let mut summary =
        compact_github_comments_readback_summary(&serde_json::Value::Array(comments.clone()));
    if let Some(object) = summary.as_object_mut() {
        object.insert("page_count".to_string(), serde_json::json!(pages.len()));
        object.insert("truncated".to_string(), serde_json::json!(truncated));
    }
    let digest = digest_bytes(
        serde_json::to_vec(&serde_json::json!({
            "pages": page_sha256,
            "comments": summary
        }))
        .unwrap_or_default()
        .as_slice(),
    );
    serde_json::json!({
        "kind": kind,
        "method": "GET",
        "url": url,
        "success": false,
        "failed_check": "remote_comments_pagination_limit",
        "request": {
            "auth": "Authorization: Bearer <redacted>"
        },
        "pagination": {
            "page_count": pages.len(),
            "max_pages": GITHUB_COMMENTS_MAX_PAGES,
            "truncated": truncated,
            "pages": pages
        },
        "response": {
            "bytes": response_bytes,
            "sha256": digest.sha256,
            "parsed": true,
            "summary": summary
        }
    })
}

fn execute_github_remote_get_single_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    kind: &str,
    url: &str,
) -> serde_json::Value {
    if url.trim().is_empty() {
        return serde_json::json!({
            "kind": kind,
            "method": "GET",
            "url": url,
            "success": false,
            "failed_check": "remote_url_missing"
        });
    }
    let mut attempts = Vec::new();
    let retry_policy = github_retry_policy();
    let max_attempts = connector_retry_max_attempts(&retry_policy);
    for attempt_number in 1..=max_attempts {
        let mut result = execute_github_remote_get_single_attempt(client, token, kind, url);
        let should_retry =
            github_operation_result_should_retry_now(&result, attempt_number, &retry_policy);
        if should_retry {
            github_mark_retry_scheduled(
                &mut result,
                github_retry_backoff_ms(attempt_number, &retry_policy),
            );
        }
        attempts.push(github_operation_attempt_summary(&result, attempt_number));
        if !should_retry {
            return github_operation_with_attempts(result, attempts, &retry_policy);
        }
        std::thread::sleep(Duration::from_millis(github_retry_backoff_ms(
            attempt_number,
            &retry_policy,
        )));
    }

    serde_json::json!({
        "kind": kind,
        "method": "GET",
        "url": url,
        "success": false,
        "failed_check": "remote_retry_exhausted",
        "attempt_count": attempts.len(),
        "max_attempts": max_attempts,
        "attempts": attempts
    })
}

fn execute_github_remote_get_single_attempt(
    client: &reqwest::blocking::Client,
    token: &str,
    kind: &str,
    url: &str,
) -> serde_json::Value {
    let response = client
        .get(url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .send();
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return serde_json::json!({
                "kind": kind,
                "method": "GET",
                "url": url,
                "success": false,
                "failed_check": "remote_request_failed",
                "retry": github_request_retry_metadata(),
                "error": error.to_string()
            });
        }
    };
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let text = match response.text() {
        Ok(text) => text,
        Err(error) => {
            return serde_json::json!({
                "kind": kind,
                "method": "GET",
                "url": url,
                "success": false,
                "failed_check": "remote_response_read_failed",
                "http_status": status,
                "retry": github_response_read_retry_metadata(),
                "error": error.to_string()
            });
        }
    };
    let digest = digest_bytes(text.as_bytes());
    let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
    let parsed_ok = parsed.is_some();
    let status_success = (200..300).contains(&status);
    let success = status_success && parsed_ok;
    let failed_check = if success {
        None
    } else if !status_success {
        Some(github_http_failed_check(status, &headers))
    } else {
        Some("remote_response_parse_failed")
    };
    serde_json::json!({
        "kind": kind,
        "method": "GET",
        "url": url,
        "success": success,
        "failed_check": failed_check,
        "http_status": status,
        "retry": if status_success {
            github_parse_retry_metadata(parsed_ok)
        } else {
            github_http_retry_metadata(status, &headers)
        },
        "request": {
            "auth": "Authorization: Bearer <redacted>"
        },
        "response": {
            "headers": github_response_headers_summary(&headers),
            "bytes": digest.bytes,
            "sha256": digest.sha256,
            "parsed": parsed_ok,
            "summary": parsed.as_ref().map(|value| {
                if kind == "list_issue_comments" || kind == "lookup_issue_comments_for_idempotency"
                {
                    compact_github_comments_readback_summary(value)
                } else {
                    compact_github_issue_readback_summary(value)
                }
            })
        }
    })
}

fn github_operation_result_should_retry_now(
    result: &serde_json::Value,
    attempt_number: usize,
    retry_policy: &ConnectorRetryPolicySpec,
) -> bool {
    if attempt_number >= connector_retry_max_attempts(retry_policy) {
        return false;
    }
    if result.pointer("/success").and_then(|value| value.as_bool()) == Some(true) {
        return false;
    }
    if result
        .pointer("/retry/rate_limited")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        return false;
    }
    result
        .pointer("/retry/retryable")
        .and_then(|value| value.as_bool())
        == Some(true)
}

fn github_operation_with_attempts(
    mut result: serde_json::Value,
    attempts: Vec<serde_json::Value>,
    retry_policy: &ConnectorRetryPolicySpec,
) -> serde_json::Value {
    let max_attempts = connector_retry_max_attempts(retry_policy);
    let success = result.pointer("/success").and_then(|value| value.as_bool()) == Some(true);
    let rate_limited = result
        .pointer("/retry/rate_limited")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let retryable = result
        .pointer("/retry/retryable")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let attempted = attempts.len() > 1;
    let exhausted = !success && retryable && !rate_limited && attempts.len() >= max_attempts;
    let mut retry = result
        .pointer("/retry")
        .cloned()
        .unwrap_or_else(|| github_terminal_retry_metadata("success"));
    if let Some(object) = retry.as_object_mut() {
        object.insert("attempted".to_string(), serde_json::json!(attempted));
        object.insert("exhausted".to_string(), serde_json::json!(exhausted));
    }
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "attempt_count".to_string(),
            serde_json::json!(attempts.len()),
        );
        object.insert("max_attempts".to_string(), serde_json::json!(max_attempts));
        object.insert("retry".to_string(), retry);
        object.insert("attempts".to_string(), serde_json::Value::Array(attempts));
    }
    result
}

fn github_operation_attempt_summary(
    result: &serde_json::Value,
    attempt_number: usize,
) -> serde_json::Value {
    serde_json::json!({
        "attempt": attempt_number,
        "success": result.pointer("/success").and_then(|value| value.as_bool()),
        "failed_check": result.pointer("/failed_check").and_then(|value| value.as_str()),
        "http_status": result.pointer("/http_status").and_then(|value| value.as_u64()),
        "retry": result.pointer("/retry").cloned().unwrap_or_else(|| serde_json::json!({})),
        "error": result.pointer("/error").and_then(|value| value.as_str()),
        "response_headers": result.pointer("/response/headers").cloned().unwrap_or_else(|| serde_json::json!({}))
    })
}

fn github_mark_retry_scheduled(result: &mut serde_json::Value, backoff_ms: u64) {
    if let Some(object) = result.as_object_mut() {
        let retry = object
            .entry("retry".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !retry.is_object() {
            *retry = serde_json::json!({});
        }
        if let Some(retry_object) = retry.as_object_mut() {
            retry_object.insert("scheduled".to_string(), serde_json::json!(true));
            retry_object.insert("backoff_ms".to_string(), serde_json::json!(backoff_ms));
        }
    }
}

fn github_retry_backoff_ms(attempt_number: usize, retry_policy: &ConnectorRetryPolicySpec) -> u64 {
    connector_retry_backoff_ms(retry_policy, attempt_number)
}

fn github_http_failed_check(status: u16, headers: &reqwest::header::HeaderMap) -> &'static str {
    if github_http_rate_limited(status, headers) {
        "remote_rate_limited"
    } else if github_http_retryable_status(status) {
        "remote_retryable_http_status"
    } else {
        "remote_http_status_failed"
    }
}

fn github_http_retry_metadata(
    status: u16,
    headers: &reqwest::header::HeaderMap,
) -> serde_json::Value {
    let rate_limited = github_http_rate_limited(status, headers);
    let retryable = github_http_retryable_status(status);
    serde_json::json!({
        "retryable": retryable,
        "rate_limited": rate_limited,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": if rate_limited {
            "rate_limited"
        } else if retryable {
            "retryable_http_status"
        } else {
            "http_status_failed"
        },
        "retry_after_secs": github_retry_after_secs(headers),
        "rate_limit": {
            "remaining": github_header_value(headers, "x-ratelimit-remaining"),
            "reset": github_header_value(headers, "x-ratelimit-reset"),
            "resource": github_header_value(headers, "x-ratelimit-resource")
        }
    })
}

fn github_request_retry_metadata() -> serde_json::Value {
    serde_json::json!({
        "retryable": true,
        "rate_limited": false,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": "request_failed"
    })
}

fn github_response_read_retry_metadata() -> serde_json::Value {
    serde_json::json!({
        "retryable": true,
        "rate_limited": false,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": "response_read_failed"
    })
}

fn github_parse_retry_metadata(parsed: bool) -> serde_json::Value {
    github_terminal_retry_metadata(if parsed {
        "success"
    } else {
        "response_parse_failed"
    })
}

fn github_terminal_retry_metadata(reason: &str) -> serde_json::Value {
    serde_json::json!({
        "retryable": false,
        "rate_limited": false,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": reason
    })
}

fn github_http_rate_limited(status: u16, headers: &reqwest::header::HeaderMap) -> bool {
    let retry_policy = github_retry_policy();
    retry_policy.rate_limit_http_statuses.contains(&status)
        && (status == 429
            || (status == 403
                && (github_header_value(headers, "x-ratelimit-remaining").as_deref() == Some("0")
                    || github_header_value(headers, "retry-after").is_some())))
}

fn github_http_retryable_status(status: u16) -> bool {
    github_retry_policy()
        .retryable_http_statuses
        .contains(&status)
}

fn github_response_headers_summary(headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    serde_json::json!({
        "link": github_header_value(headers, "link"),
        "retry_after": github_header_value(headers, "retry-after"),
        "x_ratelimit_remaining": github_header_value(headers, "x-ratelimit-remaining"),
        "x_ratelimit_reset": github_header_value(headers, "x-ratelimit-reset"),
        "x_ratelimit_resource": github_header_value(headers, "x-ratelimit-resource")
    })
}

fn github_retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    github_header_value(headers, "retry-after").and_then(|value| value.trim().parse::<u64>().ok())
}

fn github_header_value(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn github_link_header_next_url(link_header: &str) -> Option<String> {
    link_header
        .split(',')
        .find(|part| part.contains("rel=\"next\""))
        .and_then(|part| {
            let start = part.find('<')?;
            let end = part[start + 1..].find('>')?;
            Some(part[start + 1..start + 1 + end].trim().to_string())
        })
        .filter(|url| !url.is_empty())
}

fn connector_github_readback_urls(
    remote_target: &serde_json::Value,
    issue_number: u64,
) -> (String, String) {
    let owner = remote_target
        .pointer("/owner")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let repo = remote_target
        .pointer("/repo")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let api_base_url = remote_target
        .pointer("/api_base_url")
        .and_then(|value| value.as_str())
        .unwrap_or("https://api.github.com");
    (
        format!("{api_base_url}/repos/{owner}/{repo}/issues/{issue_number}"),
        format!("{api_base_url}/repos/{owner}/{repo}/issues/{issue_number}/comments?per_page=100"),
    )
}

fn connector_github_readback_issue_number(
    remote_target: &serde_json::Value,
    receipt: Option<&serde_json::Value>,
) -> Option<u64> {
    remote_target
        .pointer("/issue_number")
        .and_then(|value| value.as_u64())
        .or_else(|| {
            receipt
                .and_then(|value| {
                    value.pointer(
                        "/remote_write_receipt/write_execution/operations/0/response/summary/number",
                    )
                })
                .and_then(|value| value.as_u64())
        })
        .or_else(|| {
            receipt
                .and_then(|value| value.pointer("/remote_write_receipt/remote_id"))
                .and_then(|value| value.as_str())
                .and_then(|value| value.rsplit_once('#'))
                .and_then(|(_, number)| parse_positive_u64(number))
        })
}

fn compact_github_issue_readback_summary(response: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": response.pointer("/id").and_then(|value| value.as_i64()),
        "node_id": response.pointer("/node_id").and_then(|value| value.as_str()),
        "number": response.pointer("/number").and_then(|value| value.as_u64()),
        "url": response.pointer("/url").and_then(|value| value.as_str()),
        "html_url": response.pointer("/html_url").and_then(|value| value.as_str()),
        "state": response.pointer("/state").and_then(|value| value.as_str()),
        "state_reason": response.pointer("/state_reason").and_then(|value| value.as_str()),
        "title": response.pointer("/title").and_then(|value| value.as_str()),
        "body": response.pointer("/body").and_then(|value| value.as_str()),
        "comments": response.pointer("/comments").and_then(|value| value.as_u64()),
        "created_at": response.pointer("/created_at").and_then(|value| value.as_str()),
        "updated_at": response.pointer("/updated_at").and_then(|value| value.as_str())
    })
}

fn compact_github_comments_readback_summary(response: &serde_json::Value) -> serde_json::Value {
    let comments = response
        .as_array()
        .map(|items| {
            items
                .iter()
                .take(100)
                .map(|comment| {
                    serde_json::json!({
                        "id": comment.pointer("/id").and_then(|value| value.as_i64()),
                        "node_id": comment.pointer("/node_id").and_then(|value| value.as_str()),
                        "url": comment.pointer("/url").and_then(|value| value.as_str()),
                        "html_url": comment.pointer("/html_url").and_then(|value| value.as_str()),
                        "body": comment.pointer("/body").and_then(|value| value.as_str()),
                        "created_at": comment.pointer("/created_at").and_then(|value| value.as_str()),
                        "updated_at": comment.pointer("/updated_at").and_then(|value| value.as_str())
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    serde_json::json!({
        "count": response.as_array().map(Vec::len).unwrap_or_default(),
        "items": comments
    })
}

fn github_readback_comments_contain_body(
    comments_summary: Option<&serde_json::Value>,
    expected_body: &str,
) -> bool {
    comments_summary
        .and_then(|value| value.pointer("/items"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .any(|comment| {
            comment.pointer("/body").and_then(|value| value.as_str()) == Some(expected_body)
        })
}

fn compact_issue_mirror_readback_summary(report: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_readback.compact.v1",
        "source_schema_version": report.pointer("/schema_version").and_then(|value| value.as_str()),
        "passed": report.pointer("/passed").and_then(|value| value.as_bool()),
        "failed_count": report.pointer("/failed_count").and_then(|value| value.as_u64()),
        "failed_checks": report.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "issue_id": report.pointer("/issue_id").and_then(|value| value.as_i64()),
        "provider": report.pointer("/provider").and_then(|value| value.as_str()),
        "review_surface": report.pointer("/review_surface").and_then(|value| value.as_str()),
        "external_key": report.pointer("/external_key").and_then(|value| value.as_str()),
        "path": report.pointer("/path").and_then(|value| value.as_str()),
        "receipt_path": report.pointer("/receipt_path").and_then(|value| value.as_str()),
        "current_sha256": report.pointer("/current/digest/sha256").and_then(|value| value.as_str()),
        "remote_sha256": report.pointer("/remote/digest/sha256").and_then(|value| value.as_str()),
        "current_comment_count": report.pointer("/current/comments/count").and_then(|value| value.as_u64()),
        "remote_comment_count": report.pointer("/remote/surface/comments/count").and_then(|value| value.as_u64()),
        "remote_parsed": report.pointer("/remote/parsed").and_then(|value| value.as_bool()),
        "latest_remote_comment": report.pointer("/remote/surface/comments/latest").cloned(),
        "remote_contract_schema_version": report.pointer("/remote_contract/schema_version").and_then(|value| value.as_str()),
        "remote_readback_schema_version": report.pointer("/remote_readback/schema_version").and_then(|value| value.as_str()),
        "remote_readback_passed": report.pointer("/remote_readback/passed").and_then(|value| value.as_bool()),
        "remote_object_kind": report.pointer("/remote_readback/remote_object_kind").and_then(|value| value.as_str()),
        "remote_write_receipt_schema_version": report.pointer("/remote_readback/write_receipt/schema_version").and_then(|value| value.as_str()),
        "recorded": report.pointer("/recorded").cloned(),
        "recorded_comment_id": report.pointer("/recorded/comment_id").and_then(|value| value.as_i64()),
        "recorded_evidence_id": report.pointer("/recorded/evidence_id").and_then(|value| value.as_i64()),
        "publish_required": report.pointer("/recorded/publish/required").and_then(|value| value.as_bool()),
        "publish_command": report.pointer("/recorded/publish/command").and_then(|value| value.as_str()),
        "publish_reason": report.pointer("/recorded/publish/reason").and_then(|value| value.as_str()),
        "actions": report.pointer("/actions").cloned().unwrap_or_else(|| serde_json::json!([]))
    })
}

fn compact_issue_mirror_admission(audit: &serde_json::Value) -> serde_json::Value {
    let admitted = audit
        .pointer("/passed")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let issue_id = audit.pointer("/issue_id").and_then(|value| value.as_i64());
    let failed_checks = audit
        .pointer("/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let failed_count = audit
        .pointer("/failed_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let route_to = if admitted {
        "external_issue_surface"
    } else {
        "operator"
    };
    let reason = if admitted {
        format!("{CONNECTOR_MIRROR_RECEIPT_GATE} passed")
    } else {
        format!("{CONNECTOR_MIRROR_RECEIPT_GATE} failed with {failed_count} check(s)")
    };
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_ADMISSION_SCHEMA_VERSION,
        "object_kind": ISSUE_CONNECTOR_ADMISSION_OBJECT_KIND,
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "dry_run": true,
        "admitted": admitted,
        "result": if admitted { "admitted" } else { "rejected" },
        "reason": reason,
        "issue_id": issue_id,
        "provider": audit.pointer("/provider"),
        "review_surface": audit.pointer("/review_surface"),
        "external_key": audit.pointer("/external_key"),
        "failed_count": failed_count,
        "failed_checks": failed_checks,
        "operation": {
            "name": "external_issue_surface_write",
            "dry_run": true,
            "destructive": false
        },
        "decision": {
            "route_to": route_to,
            "human_options": if admitted {
                serde_json::json!(["write-external", "inspect"])
            } else {
                serde_json::json!(["publish", "inspect", "retry-admission"])
            }
        },
        "policy": audit.pointer("/gate").cloned().unwrap_or_else(|| serde_json::json!({
            "schema_version": POLICY_SCHEMA_VERSION,
            "gate": CONNECTOR_MIRROR_RECEIPT_GATE,
            "expected_object_kind": CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND
        })),
        "receipt": {
            "object_kind": CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND,
            "schema_version": audit.pointer("/verify/receipt/schema_version"),
            "path": audit.pointer("/verify/path"),
            "receipt_path": audit.pointer("/verify/receipt_path"),
            "sha256": audit.pointer("/verify/current/sha256"),
            "bytes": audit.pointer("/verify/current/bytes")
        },
        "audit": audit,
        "actions": [
            compact_loop_action(
                "publish",
                "Publish",
                format!("entrance hive issue mirror-publish {} --compact", issue_id.unwrap_or_default())
            ),
            compact_loop_action(
                "audit",
                "Audit",
                format!("entrance hive issue mirror-audit {} --compact", issue_id.unwrap_or_default())
            ),
            compact_loop_action(
                "admit",
                "Admit",
                format!("entrance hive issue mirror-admit {} --compact", issue_id.unwrap_or_default())
            ),
            compact_loop_action(
                "roundtrip",
                "Roundtrip",
                format!("entrance hive issue mirror-roundtrip {} --compact", issue_id.unwrap_or_default())
            )
        ]
    })
}

fn compact_issue_mirror_admission_summary(report: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_admission.compact.v1",
        "source_schema_version": report.pointer("/schema_version").and_then(|value| value.as_str()),
        "object_kind": report.pointer("/object_kind").and_then(|value| value.as_str()),
        "dry_run": report.pointer("/dry_run").and_then(|value| value.as_bool()),
        "admitted": report.pointer("/admitted").and_then(|value| value.as_bool()),
        "result": report.pointer("/result").and_then(|value| value.as_str()),
        "reason": report.pointer("/reason").and_then(|value| value.as_str()),
        "issue_id": report.pointer("/issue_id").and_then(|value| value.as_i64()),
        "provider": report.pointer("/provider").and_then(|value| value.as_str()),
        "provider_admission_status": report.pointer("/provider_admission/status").and_then(|value| value.as_str()),
        "provider_admission_blockers": report.pointer("/provider_admission/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "provider_checks": report.pointer("/provider_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "provider_check_count": report.pointer("/provider_checks").and_then(|value| value.as_array()).map(Vec::len),
        "provider_passed_check_count": report.pointer("/provider_checks").and_then(|value| value.as_array()).map(|checks| {
            checks.iter().filter(|check| {
                check.pointer("/passed").and_then(|value| value.as_bool()).unwrap_or(false)
            }).count()
        }),
        "adapter_driver": report.pointer("/writer_adapter/driver").and_then(|value| value.as_str()),
        "adapter_blockers": report.pointer("/writer_adapter/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "remote_contract_schema_version": report.pointer("/remote_contract/schema_version").and_then(|value| value.as_str()),
        "remote_object_kind": report.pointer("/remote_contract/remote_object_kind").and_then(|value| value.as_str()),
        "remote_target": report.pointer("/remote_target").cloned().unwrap_or_else(|| serde_json::json!(null)),
        "remote_target_valid": report.pointer("/remote_target/valid").and_then(|value| value.as_bool()),
        "review_surface": report.pointer("/review_surface").and_then(|value| value.as_str()),
        "external_key": report.pointer("/external_key").and_then(|value| value.as_str()),
        "gate": report.pointer("/policy/gate").and_then(|value| value.as_str()),
        "route_to": report.pointer("/decision/route_to").and_then(|value| value.as_str()),
        "failed_count": report.pointer("/failed_count").and_then(|value| value.as_u64()),
        "failed_checks": report.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "receipt_object_kind": report.pointer("/receipt/object_kind").and_then(|value| value.as_str()),
        "path": report.pointer("/receipt/path").and_then(|value| value.as_str()),
        "receipt_path": report.pointer("/receipt/receipt_path").and_then(|value| value.as_str()),
        "sha256": report.pointer("/receipt/sha256").and_then(|value| value.as_str()),
        "recorded": report.pointer("/recorded").cloned(),
        "recorded_comment_id": report.pointer("/recorded/comment_id").and_then(|value| value.as_i64()),
        "recorded_evidence_id": report.pointer("/recorded/evidence_id").and_then(|value| value.as_i64()),
        "publish_required": report.pointer("/recorded/publish/required").and_then(|value| value.as_bool()),
        "publish_command": report.pointer("/recorded/publish/command").and_then(|value| value.as_str()),
        "publish_reason": report.pointer("/recorded/publish/reason").and_then(|value| value.as_str()),
        "actions": report.pointer("/actions").cloned().unwrap_or_else(|| serde_json::json!([]))
    })
}

fn compact_issue_mirror_roundtrip(
    issue_id: i64,
    record: bool,
    stages: Vec<serde_json::Value>,
    initial_publish: serde_json::Value,
    readback: serde_json::Value,
    publish_after_readback: serde_json::Value,
    admission: serde_json::Value,
    publish_after_admission: serde_json::Value,
    final_readback: serde_json::Value,
) -> serde_json::Value {
    let failed_stages = failed_roundtrip_stages(&stages);
    let completed = failed_stages.is_empty() && report_passed(&final_readback, "/passed");
    let provider = first_report_str(
        &[
            &final_readback,
            &admission,
            &readback,
            &initial_publish,
            &publish_after_readback,
            &publish_after_admission,
        ],
        "/provider",
    );
    let review_surface = first_report_str(
        &[
            &final_readback,
            &admission,
            &readback,
            &initial_publish,
            &publish_after_readback,
            &publish_after_admission,
        ],
        "/review_surface",
    );
    let external_key = first_report_str(
        &[
            &final_readback,
            &admission,
            &readback,
            &initial_publish,
            &publish_after_readback,
            &publish_after_admission,
        ],
        "/external_key",
    );
    serde_json::json!({
        "schema_version": ISSUE_MIRROR_ROUNDTRIP_SCHEMA_VERSION,
        "object_kind": "ISSUE_CONNECTOR_ROUNDTRIP",
        "issue_id": issue_id,
        "provider": provider,
        "review_surface": review_surface,
        "external_key": external_key,
        "record_observations": record,
        "completed": completed,
        "result": if completed { "completed" } else { "blocked" },
        "stage_count": stages.len(),
        "passed_stage_count": stages.iter().filter(|stage| report_passed(stage, "/passed")).count(),
        "failed_stages": failed_stages,
        "recorded_evidence_ids": roundtrip_recorded_evidence_ids(&stages),
        "remote": {
            "object_kind": final_readback.pointer("/remote_readback/remote_object_kind")
                .or_else(|| readback.pointer("/remote_readback/remote_object_kind")),
            "write_receipt_schema_version": final_readback.pointer("/remote_readback/write_receipt/schema_version")
                .or_else(|| readback.pointer("/remote_readback/write_receipt/schema_version")),
            "readback_schema_version": final_readback.pointer("/remote_readback/schema_version")
                .or_else(|| readback.pointer("/remote_readback/schema_version")),
            "final_readback_passed": final_readback.pointer("/passed").and_then(|value| value.as_bool())
        },
        "steps": {
            "publish_initial": initial_publish,
            "readback": readback,
            "publish_after_readback": publish_after_readback,
            "admission": admission,
            "publish_after_admission": publish_after_admission,
            "final_readback": final_readback
        },
        "stages": stages,
        "commands": {
            "roundtrip": format!("entrance hive issue mirror-roundtrip {issue_id} --compact"),
            "publish": format!("entrance hive issue mirror-publish {issue_id} --compact"),
            "readback": format!("entrance hive issue mirror-readback {issue_id} --record --compact"),
            "admit": format!("entrance hive issue mirror-admit {issue_id} --record --compact")
        }
    })
}

fn compact_issue_mirror_roundtrip_summary(report: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_roundtrip.compact.v1",
        "source_schema_version": report.pointer("/schema_version").and_then(|value| value.as_str()),
        "object_kind": report.pointer("/object_kind").and_then(|value| value.as_str()),
        "issue_id": report.pointer("/issue_id").and_then(|value| value.as_i64()),
        "provider": report.pointer("/provider").and_then(|value| value.as_str()),
        "review_surface": report.pointer("/review_surface").and_then(|value| value.as_str()),
        "external_key": report.pointer("/external_key").and_then(|value| value.as_str()),
        "record_observations": report.pointer("/record_observations").and_then(|value| value.as_bool()),
        "completed": report.pointer("/completed").and_then(|value| value.as_bool()),
        "result": report.pointer("/result").and_then(|value| value.as_str()),
        "stage_count": report.pointer("/stage_count").and_then(|value| value.as_u64()),
        "passed_stage_count": report.pointer("/passed_stage_count").and_then(|value| value.as_u64()),
        "failed_stages": report.pointer("/failed_stages").cloned().unwrap_or_else(|| serde_json::json!([])),
        "recorded_evidence_ids": report.pointer("/recorded_evidence_ids").cloned().unwrap_or_else(|| serde_json::json!([])),
        "remote_object_kind": report.pointer("/remote/object_kind").and_then(|value| value.as_str()),
        "remote_write_receipt_schema_version": report.pointer("/remote/write_receipt_schema_version").and_then(|value| value.as_str()),
        "remote_readback_schema_version": report.pointer("/remote/readback_schema_version").and_then(|value| value.as_str()),
        "final_readback_passed": report.pointer("/remote/final_readback_passed").and_then(|value| value.as_bool()),
        "commands": report.pointer("/commands").cloned().unwrap_or_else(|| serde_json::json!({}))
    })
}

fn issue_mirror_roundtrip_stage(
    name: &str,
    summary: &str,
    passed: bool,
    report: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.hive.issue_mirror_roundtrip_stage.v1",
        "name": name,
        "summary": summary,
        "passed": passed,
        "source_schema_version": report.pointer("/schema_version").and_then(|value| value.as_str()),
        "issue_id": report.pointer("/issue_id").and_then(|value| value.as_i64()),
        "provider": report.pointer("/provider").and_then(|value| value.as_str()),
        "review_surface": report.pointer("/review_surface").and_then(|value| value.as_str()),
        "external_key": report.pointer("/external_key").and_then(|value| value.as_str()),
        "failed_checks": report.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "recorded_comment_id": report.pointer("/recorded/comment_id").and_then(|value| value.as_i64()),
        "recorded_evidence_id": report.pointer("/recorded/evidence_id").and_then(|value| value.as_i64()),
        "publish_required": report.pointer("/recorded/publish/required").and_then(|value| value.as_bool()),
        "path": report.pointer("/path")
            .or_else(|| report.pointer("/receipt/path"))
            .and_then(|value| value.as_str()),
        "sha256": report.pointer("/sha256")
            .or_else(|| report.pointer("/receipt/sha256"))
            .or_else(|| report.pointer("/current/digest/sha256"))
            .and_then(|value| value.as_str())
    })
}

fn failed_roundtrip_stages(stages: &[serde_json::Value]) -> Vec<String> {
    stages
        .iter()
        .filter(|stage| !report_passed(stage, "/passed"))
        .filter_map(|stage| {
            stage
                .pointer("/name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn roundtrip_recorded_evidence_ids(stages: &[serde_json::Value]) -> Vec<i64> {
    stages
        .iter()
        .filter_map(|stage| {
            stage
                .pointer("/recorded_evidence_id")
                .and_then(|value| value.as_i64())
        })
        .collect()
}

fn report_passed(report: &serde_json::Value, pointer: &str) -> bool {
    report
        .pointer(pointer)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn issue_report_recorded_publish_required(report: &serde_json::Value) -> bool {
    report_passed(report, "/recorded/publish/required")
}

fn publish_stage_allows_next(report: &serde_json::Value) -> bool {
    report.is_null() || report_passed(report, "/published")
}

fn first_report_str(reports: &[&serde_json::Value], pointer: &str) -> Option<String> {
    reports.iter().find_map(|report| {
        report
            .pointer(pointer)
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
    })
}

fn compact_digest(digest: &MirrorFileDigest) -> serde_json::Value {
    serde_json::json!({
        "bytes": digest.bytes,
        "sha256": digest.sha256.as_str()
    })
}

fn compact_digest_label(value: &str) -> String {
    value.chars().take(12).collect()
}

fn readback_check(
    name: &str,
    summary: &str,
    passed: bool,
    details: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "passed": passed,
        "summary": if passed {
            summary.to_string()
        } else {
            format!("{summary} Failed.")
        },
        "details": details
    })
}

fn issue_mirror_identity_current(current: &IssueMirrorReport, remote: &IssueMirrorReport) -> bool {
    current.schema_version == remote.schema_version
        && current.provider == remote.provider
        && current.review_surface == remote.review_surface
        && current.external_key == remote.external_key
        && current.issue.id == remote.issue.id
        && current.issue.loop_id == remote.issue.loop_id
        && current.loop_contract.as_ref().map(|contract| contract.id)
            == remote.loop_contract.as_ref().map(|contract| contract.id)
}

fn issue_mirror_binding_current(current: &IssueMirrorReport, remote: &IssueMirrorReport) -> bool {
    current.schema_version == remote.schema_version
        && current.provider == remote.provider
        && current.review_surface == remote.review_surface
        && current.external_key == remote.external_key
        && current.issue.id == remote.issue.id
        && current.issue.loop_id == remote.issue.loop_id
        && current.issue.status == remote.issue.status
        && current.issue.updated_at == remote.issue.updated_at
        && current.loop_contract.as_ref().map(|contract| contract.id)
            == remote.loop_contract.as_ref().map(|contract| contract.id)
        && current
            .loop_contract
            .as_ref()
            .map(|contract| contract.current_round)
            == remote
                .loop_contract
                .as_ref()
                .map(|contract| contract.current_round)
}

fn issue_mirror_comment_surface_current(
    current: &IssueMirrorReport,
    remote: &IssueMirrorReport,
) -> bool {
    current.comments.len() == remote.comments.len()
        && match (current.comments.last(), remote.comments.last()) {
            (Some(current), Some(remote)) => {
                current.id == remote.id
                    && current.author == remote.author
                    && current.body == remote.body
                    && current.created_at == remote.created_at
                    && current.payload == remote.payload
            }
            (None, None) => true,
            _ => false,
        }
}

fn issue_mirror_readback_surface(mirror: &IssueMirrorReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": mirror.schema_version.as_str(),
        "provider": mirror.provider.as_str(),
        "review_surface": mirror.review_surface.as_str(),
        "external_key": mirror.external_key.as_str(),
        "issue": {
            "id": mirror.issue.id,
            "loop_id": mirror.issue.loop_id,
            "status": mirror.issue.status.as_str(),
            "updated_at": mirror.issue.updated_at.as_str()
        },
        "loop": mirror.loop_contract.as_ref().map(|contract| serde_json::json!({
            "id": contract.id,
            "status": contract.status.as_str(),
            "phase": contract.active_phase.as_str(),
            "round": contract.current_round,
            "updated_at": contract.updated_at.as_str()
        })),
        "comments": issue_mirror_comment_surface(mirror)
    })
}

fn issue_mirror_comment_surface(mirror: &IssueMirrorReport) -> serde_json::Value {
    serde_json::json!({
        "count": mirror.comments.len(),
        "latest": mirror.comments.last().map(|comment| serde_json::json!({
            "id": comment.id,
            "author": comment.author.as_str(),
            "body": compact_text(&comment.body, 220),
            "created_at": comment.created_at.as_str(),
            "schema_version": comment.payload.get("schema_version").and_then(|value| value.as_str()),
            "source": comment.payload.get("source").and_then(|value| value.as_str()),
            "action": comment.payload.get("action").and_then(|value| value.as_str())
        }))
    })
}

fn verify_failures(verify: &serde_json::Value) -> Vec<String> {
    verify
        .pointer("/failures")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect()
}

fn mirror_audit_check(
    name: &str,
    summary: &str,
    failures: &[String],
    failure_codes: &[&str],
    details: serde_json::Value,
) -> serde_json::Value {
    let matched_failures = failure_codes
        .iter()
        .filter(|code| failures.iter().any(|failure| failure == **code))
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    serde_json::json!({
        "name": name,
        "passed": matched_failures.is_empty(),
        "summary": if matched_failures.is_empty() {
            summary.to_string()
        } else {
            format!("{} Failed: {}.", summary, matched_failures.join(", "))
        },
        "failure_codes": matched_failures,
        "details": details
    })
}

fn json_pointer_str<'a>(value: &'a serde_json::Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(|value| value.as_str())
}

fn json_pointer_i64(value: &serde_json::Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(|value| value.as_i64())
}

fn json_pointer_u64(value: &serde_json::Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(|value| value.as_u64())
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

fn compact_issue_card_with_connector_status(
    services: &AppServices,
    card: &IssueCard,
) -> serde_json::Value {
    let mut issue = compact_issue_card(card);
    if let Some(object) = issue.as_object_mut() {
        object.insert(
            "connector".to_string(),
            compact_connector_status_for_issue(services, card.issue.id),
        );
    }
    issue
}

fn compact_connector_status_for_issue(services: &AppServices, issue_id: i64) -> serde_json::Value {
    issue_mirror_status(services, issue_id, None).unwrap_or_else(|error| {
        serde_json::json!({
            "schema_version": ISSUE_MIRROR_STATUS_SCHEMA_VERSION,
            "current": false,
            "publish_required": null,
            "reason": "connector_status_unavailable",
            "error": error.to_string(),
            "issue_id": issue_id,
            "publish_command": format!("entrance hive issue mirror-publish {issue_id} --compact")
        })
    })
}

pub(crate) fn connector_queue_report(
    services: &AppServices,
    provider_filter: Option<&str>,
) -> Result<serde_json::Value> {
    let cards = services.hive.panel()?;
    let issues = cards
        .iter()
        .map(|card| compact_issue_card_with_connector_status(services, card))
        .collect::<Vec<_>>();
    Ok(compact_connector_queue(
        &services.hive.connector_registry(),
        &issues,
        provider_filter,
    ))
}

pub(crate) fn connector_publish_plan_report(
    services: &AppServices,
    provider_filter: Option<&str>,
) -> Result<serde_json::Value> {
    let queue = connector_queue_report(services, provider_filter)?;
    compact_connector_publish_plan(&queue)
}

pub(crate) fn execute_connector_publish_plan(
    services: &AppServices,
    provider_filter: Option<&str>,
    expected_plan_id: &str,
) -> Result<serde_json::Value> {
    let plan = connector_publish_plan_report(services, provider_filter)?;
    let current_plan_id = plan
        .pointer("/plan_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    if current_plan_id != expected_plan_id {
        return Ok(serde_json::json!({
            "schema_version": CONNECTOR_PUBLISH_EXECUTE_SCHEMA_VERSION,
            "executed": false,
            "reason": "plan_id_mismatch",
            "expected_plan_id": expected_plan_id,
            "current_plan_id": current_plan_id.as_str(),
            "failed_checks": ["connector_publish_plan_current"],
            "current_plan": plan
        }));
    }
    let can_execute = plan
        .pointer("/can_execute")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !can_execute {
        return Ok(serde_json::json!({
            "schema_version": CONNECTOR_PUBLISH_EXECUTE_SCHEMA_VERSION,
            "executed": false,
            "reason": plan.pointer("/reason").and_then(|value| value.as_str()).unwrap_or("plan_not_executable"),
            "plan_id": current_plan_id.as_str(),
            "failed_checks": plan.pointer("/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
            "current_plan": plan
        }));
    }

    let plan_issues = plan
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let issue_ids = plan_issues
        .iter()
        .into_iter()
        .filter_map(|issue| issue.pointer("/id").and_then(|value| value.as_i64()))
        .collect::<Vec<_>>();
    let recorded =
        record_connector_publish_execute_issues(services, &plan, &plan_issues, &current_plan_id)?;
    let mut published = Vec::new();
    for issue_id in &issue_ids {
        published.push(publish_issue_mirror_to_file(services, *issue_id, None)?);
    }
    let after_queue = connector_queue_report(services, provider_filter)?;
    Ok(serde_json::json!({
        "schema_version": CONNECTOR_PUBLISH_EXECUTE_SCHEMA_VERSION,
        "executed": true,
        "reason": "plan_executed",
        "plan_id": current_plan_id.as_str(),
        "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
        "issue_count": issue_ids.len(),
        "issue_ids": issue_ids,
        "recorded": recorded,
        "published": published,
        "after": {
            "publish_required_count": after_queue.pointer("/publish_required_count").and_then(|value| value.as_u64()),
            "current_count": after_queue.pointer("/current_count").and_then(|value| value.as_u64()),
            "queue": after_queue
        },
        "commands": {
            "refresh": "entrance hive connector queue --compact",
            "plan": connector_publish_plan_command(provider_filter),
            "execute": format!("{} --plan-id {} --compact", connector_publish_execute_command_prefix(provider_filter), current_plan_id)
        }
    }))
}

pub(crate) fn connector_roundtrip_plan_report(
    services: &AppServices,
    provider_filter: Option<&str>,
) -> Result<serde_json::Value> {
    let queue = connector_queue_report(services, provider_filter)?;
    compact_connector_roundtrip_plan(&queue)
}

pub(crate) fn execute_connector_roundtrip_plan(
    services: &AppServices,
    provider_filter: Option<&str>,
    expected_plan_id: &str,
) -> Result<serde_json::Value> {
    let plan = connector_roundtrip_plan_report(services, provider_filter)?;
    let current_plan_id = plan
        .pointer("/plan_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    if current_plan_id != expected_plan_id {
        return Ok(serde_json::json!({
            "schema_version": CONNECTOR_ROUNDTRIP_EXECUTE_SCHEMA_VERSION,
            "executed": false,
            "reason": "plan_id_mismatch",
            "expected_plan_id": expected_plan_id,
            "current_plan_id": current_plan_id.as_str(),
            "failed_checks": ["connector_roundtrip_plan_current"],
            "current_plan": plan
        }));
    }
    let can_execute = plan
        .pointer("/can_execute")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !can_execute {
        return Ok(serde_json::json!({
            "schema_version": CONNECTOR_ROUNDTRIP_EXECUTE_SCHEMA_VERSION,
            "executed": false,
            "reason": plan.pointer("/reason").and_then(|value| value.as_str()).unwrap_or("plan_not_executable"),
            "plan_id": current_plan_id.as_str(),
            "failed_checks": plan.pointer("/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
            "current_plan": plan
        }));
    }

    let plan_issues = plan
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let issue_ids = plan_issues
        .iter()
        .filter_map(|issue| issue.pointer("/id").and_then(|value| value.as_i64()))
        .collect::<Vec<_>>();
    let recorded =
        record_connector_roundtrip_execute_issues(services, &plan, &plan_issues, &current_plan_id)?;
    let mut roundtrips = Vec::new();
    for issue_id in &issue_ids {
        roundtrips.push(roundtrip_issue_mirror_file(
            services, *issue_id, None, true,
        )?);
    }
    let completed_count = roundtrips
        .iter()
        .filter(|report| {
            report
                .pointer("/completed")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .count();
    let after_queue = connector_queue_report(services, provider_filter)?;
    Ok(serde_json::json!({
        "schema_version": CONNECTOR_ROUNDTRIP_EXECUTE_SCHEMA_VERSION,
        "executed": true,
        "reason": if completed_count == issue_ids.len() { "plan_executed" } else { "roundtrip_incomplete" },
        "plan_id": current_plan_id.as_str(),
        "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
        "issue_count": issue_ids.len(),
        "completed_count": completed_count,
        "issue_ids": issue_ids,
        "recorded": recorded,
        "roundtrips": roundtrips,
        "after": {
            "publish_required_count": after_queue.pointer("/publish_required_count").and_then(|value| value.as_u64()),
            "current_count": after_queue.pointer("/current_count").and_then(|value| value.as_u64()),
            "queue": after_queue
        },
        "commands": {
            "refresh": "entrance hive connector queue --compact",
            "plan": connector_roundtrip_plan_command(provider_filter),
            "execute": format!("{} --plan-id {} --compact", connector_roundtrip_execute_command_prefix(provider_filter), current_plan_id)
        }
    }))
}

fn record_connector_publish_execute_issues(
    services: &AppServices,
    plan: &serde_json::Value,
    plan_issues: &[serde_json::Value],
    plan_id: &str,
) -> Result<Vec<serde_json::Value>> {
    plan_issues
        .iter()
        .map(|issue_plan| {
            record_connector_publish_execute_issue(services, plan, issue_plan, plan_id)
        })
        .collect()
}

fn record_connector_publish_execute_issue(
    services: &AppServices,
    plan: &serde_json::Value,
    issue_plan: &serde_json::Value,
    plan_id: &str,
) -> Result<serde_json::Value> {
    let issue_id = issue_plan
        .pointer("/id")
        .and_then(|value| value.as_i64())
        .context("connector publish plan issue missing id")?;
    let issue = services
        .kernel
        .store
        .get_hive_issue(issue_id)?
        .with_context(|| format!("unknown hive issue `{issue_id}`"))?;
    let contract = issue
        .loop_id
        .map(|loop_id| services.kernel.store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let loop_id = issue.loop_id;
    let round = contract
        .as_ref()
        .map(|contract| contract.current_round)
        .unwrap_or(1);
    let phase = contract
        .as_ref()
        .map(|contract| contract.active_phase.as_str());
    let short_plan = compact_digest_label(plan_id);
    let issue_count = plan
        .pointer("/issue_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let body = format!("Connector publish plan {short_plan} executed for {issue_count} issue(s).");
    let issue_ids = plan
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .map(|issues| {
            issues
                .iter()
                .filter_map(|issue| issue.pointer("/id").and_then(|value| value.as_i64()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let comment_id = services
        .kernel
        .store
        .insert_hive_comment(HiveCommentCreate {
            issue_id,
            author: "hive".to_string(),
            body: body.clone(),
            payload: serde_json::json!({
                "schema_version": SYSTEM_COMMENT_SCHEMA_VERSION,
                "source": "hive",
                "action": "connector_publish_execute",
                "loop_id": loop_id,
                "round": round,
                "status": issue.status.as_str(),
                "phase": phase,
                "connector_publish": {
                    "schema_version": CONNECTOR_PUBLISH_EXECUTE_SCHEMA_VERSION,
                    "result": "executed",
                    "plan_id": plan_id,
                    "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
                    "issue_count": issue_count,
                    "issue_ids": issue_ids,
                    "issue_plan": issue_plan,
                    "commands": plan.pointer("/commands").cloned().unwrap_or_else(|| serde_json::json!({}))
                }
            }),
        })?;

    let evidence_id = if let Some(loop_id) = issue.loop_id {
        Some(services.kernel.store.insert_hive_loop_evidence(
            HiveLoopEvidenceCreate {
                loop_id,
                stage_id: None,
                round,
                kind: "connector_publish_execute".to_string(),
                summary: body.clone(),
                path: issue_plan
                    .pointer("/path")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                payload: serde_json::json!({
                    "schema_version": CONNECTOR_PUBLISH_EXECUTE_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "result": "executed",
                    "issue": {
                        "id": issue.id,
                        "status": issue.status.as_str(),
                        "comment_id": comment_id
                    },
                    "loop": {
                        "id": loop_id,
                        "status": contract.as_ref().map(|contract| contract.status.as_str()),
                        "phase": phase,
                        "round": round
                    },
                    "operator": {
                        "author": "hive",
                        "action": "connector_publish_execute",
                        "comment_body": body
                    },
                    "connector": {
                        "provider": issue_plan.pointer("/provider").cloned().unwrap_or(serde_json::Value::Null),
                        "provider_status": issue_plan.pointer("/provider_status").cloned().unwrap_or(serde_json::Value::Null),
                        "review_surface": issue_plan.pointer("/review_surface").cloned().unwrap_or(serde_json::Value::Null),
                        "path": issue_plan.pointer("/path").cloned().unwrap_or(serde_json::Value::Null)
                    },
                    "plan": {
                        "schema_version": plan.pointer("/schema_version").cloned().unwrap_or(serde_json::Value::Null),
                        "plan_id": plan_id,
                        "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
                        "issue_count": issue_count,
                        "issue_ids": issue_ids,
                        "issue": issue_plan
                    }
                }),
            },
        )?)
    } else {
        None
    };

    Ok(serde_json::json!({
        "schema_version": "entrance.hive.connector_publish_execute_record.v1",
        "issue_id": issue_id,
        "comment_id": comment_id,
        "evidence_id": evidence_id,
        "comment_body": body,
        "plan_id": plan_id
    }))
}

fn record_connector_roundtrip_execute_issues(
    services: &AppServices,
    plan: &serde_json::Value,
    plan_issues: &[serde_json::Value],
    plan_id: &str,
) -> Result<Vec<serde_json::Value>> {
    plan_issues
        .iter()
        .map(|issue_plan| {
            record_connector_roundtrip_execute_issue(services, plan, issue_plan, plan_id)
        })
        .collect()
}

fn record_connector_roundtrip_execute_issue(
    services: &AppServices,
    plan: &serde_json::Value,
    issue_plan: &serde_json::Value,
    plan_id: &str,
) -> Result<serde_json::Value> {
    let issue_id = issue_plan
        .pointer("/id")
        .and_then(|value| value.as_i64())
        .context("connector roundtrip plan issue missing id")?;
    let issue = services
        .kernel
        .store
        .get_hive_issue(issue_id)?
        .with_context(|| format!("unknown hive issue `{issue_id}`"))?;
    let contract = issue
        .loop_id
        .map(|loop_id| services.kernel.store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let loop_id = issue.loop_id;
    let round = contract
        .as_ref()
        .map(|contract| contract.current_round)
        .unwrap_or(1);
    let phase = contract
        .as_ref()
        .map(|contract| contract.active_phase.as_str());
    let short_plan = compact_digest_label(plan_id);
    let issue_count = plan
        .pointer("/issue_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let body =
        format!("Connector roundtrip plan {short_plan} executed for {issue_count} issue(s).");
    let issue_ids = plan
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .map(|issues| {
            issues
                .iter()
                .filter_map(|issue| issue.pointer("/id").and_then(|value| value.as_i64()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let comment_id = services
        .kernel
        .store
        .insert_hive_comment(HiveCommentCreate {
            issue_id,
            author: "hive".to_string(),
            body: body.clone(),
            payload: serde_json::json!({
                "schema_version": SYSTEM_COMMENT_SCHEMA_VERSION,
                "source": "hive",
                "action": "connector_roundtrip_execute",
                "loop_id": loop_id,
                "round": round,
                "status": issue.status.as_str(),
                "phase": phase,
                "connector_roundtrip": {
                    "schema_version": CONNECTOR_ROUNDTRIP_EXECUTE_SCHEMA_VERSION,
                    "result": "executed",
                    "plan_id": plan_id,
                    "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
                    "issue_count": issue_count,
                    "issue_ids": issue_ids,
                    "issue_plan": issue_plan,
                    "commands": plan.pointer("/commands").cloned().unwrap_or_else(|| serde_json::json!({}))
                }
            }),
        })?;

    let evidence_id = if let Some(loop_id) = issue.loop_id {
        Some(services.kernel.store.insert_hive_loop_evidence(
            HiveLoopEvidenceCreate {
                loop_id,
                stage_id: None,
                round,
                kind: "connector_roundtrip_execute".to_string(),
                summary: body.clone(),
                path: issue_plan
                    .pointer("/path")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                payload: serde_json::json!({
                    "schema_version": CONNECTOR_ROUNDTRIP_EXECUTE_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "result": "executed",
                    "issue": {
                        "id": issue.id,
                        "status": issue.status.as_str(),
                        "comment_id": comment_id
                    },
                    "loop": {
                        "id": loop_id,
                        "status": contract.as_ref().map(|contract| contract.status.as_str()),
                        "phase": phase,
                        "round": round
                    },
                    "operator": {
                        "author": "hive",
                        "action": "connector_roundtrip_execute",
                        "comment_body": body
                    },
                    "connector": {
                        "provider": issue_plan.pointer("/provider").cloned().unwrap_or(serde_json::Value::Null),
                        "provider_status": issue_plan.pointer("/provider_status").cloned().unwrap_or(serde_json::Value::Null),
                        "review_surface": issue_plan.pointer("/review_surface").cloned().unwrap_or(serde_json::Value::Null),
                        "path": issue_plan.pointer("/path").cloned().unwrap_or(serde_json::Value::Null)
                    },
                    "plan": {
                        "schema_version": plan.pointer("/schema_version").cloned().unwrap_or(serde_json::Value::Null),
                        "plan_id": plan_id,
                        "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
                        "issue_count": issue_count,
                        "issue_ids": issue_ids,
                        "issue": issue_plan
                    }
                }),
            },
        )?)
    } else {
        None
    };

    Ok(serde_json::json!({
        "schema_version": "entrance.hive.connector_roundtrip_execute_record.v1",
        "issue_id": issue_id,
        "comment_id": comment_id,
        "evidence_id": evidence_id,
        "comment_body": body,
        "plan_id": plan_id
    }))
}

fn connector_provider_for_surface<'a>(
    registry: &'a ConnectorRegistryReport,
    provider_name: &str,
    review_surface: &str,
) -> Option<&'a ConnectorProviderSpec> {
    registry
        .providers
        .iter()
        .find(|provider| provider.name == provider_name)
        .or_else(|| {
            registry
                .providers
                .iter()
                .find(|provider| connector_provider_matches_surface(provider, review_surface))
        })
}

fn connector_provider_admission_for_surface<'a>(
    registry: &'a ConnectorRegistryReport,
    provider_name: &str,
    review_surface: &str,
) -> Option<&'a ConnectorProviderAdmissionSpec> {
    let provider = connector_provider_for_surface(registry, provider_name, review_surface)?;
    registry
        .provider_admissions
        .iter()
        .find(|admission| admission.provider == provider.name)
}

fn connector_provider_matches_surface(
    provider: &ConnectorProviderSpec,
    review_surface: &str,
) -> bool {
    provider.review_surface_prefixes.iter().any(|prefix| {
        if prefix.ends_with(':') {
            review_surface.starts_with(prefix)
        } else {
            review_surface == prefix
        }
    })
}

fn connector_writer_blockers(provider: Option<&ConnectorProviderSpec>) -> Vec<String> {
    let Some(provider) = provider else {
        return vec!["unsupported_provider".to_string()];
    };
    let mut blockers = Vec::new();
    if provider.status != "active" {
        blockers.push("provider_not_active".to_string());
    }
    if !provider.configured {
        blockers.push("connector_not_configured".to_string());
    }
    if !provider.supports_publish {
        blockers.push("publish_not_supported".to_string());
    }
    blockers
}

fn connector_issue_writer_blockers(
    provider: Option<&ConnectorProviderSpec>,
    review_surface: &str,
    external_key: &str,
) -> Vec<String> {
    let mut blockers = connector_writer_blockers(provider);
    let remote_target = connector_remote_target(provider, review_surface, external_key);
    if remote_target.is_object()
        && remote_target
            .pointer("/valid")
            .and_then(|value| value.as_bool())
            != Some(true)
    {
        blockers.push("remote_target_invalid".to_string());
        blockers.extend(
            remote_target
                .pointer("/blockers")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned),
        );
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

fn compact_connector_writer_adapter(
    provider_name: &str,
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    let blockers = connector_writer_blockers(provider);
    let driver = provider.map_or("unknown", |provider| {
        if !blockers.is_empty() {
            "unavailable"
        } else if provider.name == "file" || provider.mode == "local-json-mirror" {
            "file-mirror"
        } else if connector_provider_is_remote_fixture(provider) {
            "remote-fixture"
        } else if connector_provider_uses_remote_contract(provider) {
            "remote-issue-api"
        } else {
            "custom"
        }
    });
    let remote_write = provider
        .map(|provider| blockers.is_empty() && connector_provider_uses_remote_contract(provider))
        .unwrap_or(false);
    serde_json::json!({
        "schema_version": CONNECTOR_WRITER_ADAPTER_SCHEMA_VERSION,
        "provider": provider.map(|provider| provider.name.as_str()).unwrap_or(provider_name),
        "driver": driver,
        "mode": provider.map(|provider| provider.mode.as_str()),
        "status": provider.map(|provider| provider.status.as_str()),
        "configured": provider.map(|provider| provider.configured),
        "supports_publish": provider.map(|provider| provider.supports_publish),
        "supports_readback": provider.map(|provider| provider.supports_readback),
        "supports_admission": provider.map(|provider| provider.supports_admission),
        "storage": provider.map(|provider| provider.storage.as_str()),
        "remote_write": remote_write,
        "remote_contract": compact_connector_remote_contract(provider),
        "blockers": blockers
    })
}

fn compact_connector_remote_contract(
    provider: Option<&ConnectorProviderSpec>,
) -> serde_json::Value {
    let Some(provider) = provider else {
        return serde_json::Value::Null;
    };
    if !connector_provider_uses_remote_contract(provider) {
        return serde_json::Value::Null;
    }
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_CONTRACT_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "surface": {
            "kind": "issue/status/comment",
            "identity_fields": ["provider", "review_surface", "external_key"],
            "status_source": "hive_issue.status",
            "comment_source": "hive_issue.comments",
            "comment_mode": "upsert_latest_comment_by_issue_stable_marker"
        },
        "target": {
            "schema_version": CONNECTOR_REMOTE_TARGET_SCHEMA_VERSION,
            "review_surface": "provider-specific issue target",
            "required": true,
            "examples": connector_remote_target_examples(&provider.name),
            "required_fields": connector_remote_target_required_fields(&provider.name)
        },
        "write": {
            "operation": "upsert_remote_issue_surface",
            "plan_schema_version": CONNECTOR_REMOTE_WRITE_PLAN_SCHEMA_VERSION,
            "receipt_schema_version": CONNECTOR_REMOTE_WRITE_RECEIPT_SCHEMA_VERSION,
            "required_receipt_fields": [
                "provider",
                "remote_object_kind",
                "remote_id",
                "remote_url",
                "external_key",
                "status",
                "comment_count",
                "idempotency_key",
                "source_mirror_sha256"
            ]
        },
        "readback": {
            "operation": "read_remote_issue_surface",
            "schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION,
            "required_checks": [
                "remote_identity",
                "remote_status",
                "remote_comment_surface",
                "write_receipt_binding"
            ]
        },
        "idempotency": {
            "key_parts": [
                "surface",
                "provider",
                "external_key",
                "issue.id",
                "loop.id"
            ],
            "conflict_policy": "readback-before-write"
        },
        "auth": {
            "required": provider.auth_required,
            "env": provider.auth_env.iter().map(String::as_str).collect::<Vec<_>>()
        },
        "retry": compact_connector_retry_policy(&connector_remote_retry_policy(&provider.name)),
        "admission": {
            "required_before_write": [
                "connector_writer_adapter.blockers_empty",
                "remote_auth_configured",
                "remote_readback_available"
            ],
            "required_after_write": [
                "remote_write_receipt_current",
                "remote_readback_current"
            ]
        }
    })
}

fn connector_remote_target(
    provider: Option<&ConnectorProviderSpec>,
    review_surface: &str,
    external_key: &str,
) -> serde_json::Value {
    let Some(provider) = provider else {
        return serde_json::Value::Null;
    };
    if !connector_provider_uses_remote_contract(provider) {
        return serde_json::Value::Null;
    }
    let raw_target = connector_remote_target_raw(provider, review_surface);
    let target = raw_target.as_deref().unwrap_or_default().trim();
    match provider.name.as_str() {
        "github" => connector_github_remote_target(provider, review_surface, external_key, target),
        "linear" => connector_linear_remote_target(provider, review_surface, external_key, target),
        "remote-fixture" => {
            connector_fixture_remote_target(provider, review_surface, external_key, target)
        }
        _ => connector_generic_remote_target(provider, review_surface, external_key, target),
    }
}

fn connector_remote_target_raw(
    provider: &ConnectorProviderSpec,
    review_surface: &str,
) -> Option<String> {
    let review_surface = review_surface.trim();
    provider.review_surface_prefixes.iter().find_map(|prefix| {
        if prefix.ends_with(':') {
            review_surface
                .strip_prefix(prefix)
                .map(|target| target.trim().to_string())
        } else if review_surface == prefix {
            Some(String::new())
        } else {
            None
        }
    })
}

fn connector_github_remote_target(
    provider: &ConnectorProviderSpec,
    review_surface: &str,
    external_key: &str,
    target: &str,
) -> serde_json::Value {
    let mut blockers = Vec::new();
    let normalized = target
        .strip_prefix("https://github.com/")
        .or_else(|| target.strip_prefix("http://github.com/"))
        .or_else(|| target.strip_prefix("github.com/"))
        .unwrap_or(target)
        .trim_matches('/');
    if normalized.is_empty() {
        blockers.push("github_target_missing".to_string());
    }
    let mut owner = "";
    let mut repo = "";
    let mut issue_number = None;
    let parts = normalized.split('/').collect::<Vec<_>>();
    if parts.len() >= 2 {
        owner = parts[0].trim();
        repo = parts[1].trim();
        if parts.get(2) == Some(&"issues") {
            issue_number = parts.get(3).and_then(|value| parse_positive_u64(value));
            if issue_number.is_none() {
                blockers.push("github_issue_number_invalid".to_string());
            }
        } else if let Some((repo_name, issue)) = repo.split_once('#') {
            repo = repo_name.trim();
            issue_number = parse_positive_u64(issue);
            if issue_number.is_none() {
                blockers.push("github_issue_number_invalid".to_string());
            }
        }
    }
    if owner.is_empty() {
        blockers.push("github_owner_missing".to_string());
    }
    if repo.is_empty() {
        blockers.push("github_repo_missing".to_string());
    }
    if !owner.is_empty() && !connector_target_slug_valid(owner) {
        blockers.push("github_owner_invalid".to_string());
    }
    if !repo.is_empty() && !connector_target_slug_valid(repo) {
        blockers.push("github_repo_invalid".to_string());
    }
    blockers.sort();
    blockers.dedup();
    let valid = blockers.is_empty();
    let remote_id = if valid {
        issue_number.map_or_else(
            || format!("{owner}/{repo}"),
            |number| format!("{owner}/{repo}#{number}"),
        )
    } else {
        format!("{}:{}", provider.name, external_key)
    };
    let remote_url = if valid {
        issue_number.map_or_else(
            || format!("https://github.com/{owner}/{repo}/issues"),
            |number| format!("https://github.com/{owner}/{repo}/issues/{number}"),
        )
    } else {
        String::new()
    };
    let api_base_url = connector_github_api_base_url(provider);
    let api_url = if valid {
        issue_number.map_or_else(
            || format!("{api_base_url}/repos/{owner}/{repo}/issues"),
            |number| format!("{api_base_url}/repos/{owner}/{repo}/issues/{number}"),
        )
    } else {
        String::new()
    };
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_TARGET_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "review_surface": review_surface,
        "external_key": external_key,
        "target": target,
        "valid": valid,
        "blockers": blockers,
        "target_kind": "github.issue",
        "owner": empty_str_none(owner),
        "repo": empty_str_none(repo),
        "issue_number": issue_number,
        "write_mode": if issue_number.is_some() { "update_issue" } else { "create_issue" },
        "remote_id": remote_id,
        "remote_url": empty_string_none(remote_url),
        "api_base_url": api_base_url,
        "api_url": empty_string_none(api_url)
    })
}

fn connector_linear_remote_target(
    provider: &ConnectorProviderSpec,
    review_surface: &str,
    external_key: &str,
    target: &str,
) -> serde_json::Value {
    let normalized = connector_linear_issue_key_candidate(target);
    let mut blockers = Vec::new();
    if normalized.is_empty() {
        blockers.push("linear_issue_key_missing".to_string());
    }
    let (team_key, issue_number) = normalized
        .rsplit_once('-')
        .map(|(team, number)| (team.trim().to_ascii_uppercase(), parse_positive_u64(number)))
        .unwrap_or_else(|| (String::new(), None));
    if team_key.is_empty() {
        blockers.push("linear_team_key_missing".to_string());
    }
    if issue_number.is_none() {
        blockers.push("linear_issue_number_invalid".to_string());
    }
    if !team_key.is_empty() && !team_key.chars().all(|ch| ch.is_ascii_alphabetic()) {
        blockers.push("linear_team_key_invalid".to_string());
    }
    blockers.sort();
    blockers.dedup();
    let valid = blockers.is_empty();
    let issue_key = if valid {
        format!("{}-{}", team_key, issue_number.unwrap_or_default())
    } else {
        normalized
    };
    let api_url = connector_linear_api_url(provider);
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_TARGET_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "review_surface": review_surface,
        "external_key": external_key,
        "target": target,
        "valid": valid,
        "blockers": blockers,
        "target_kind": "linear.issue",
        "team_key": empty_string_none(team_key),
        "issue_key": empty_string_none(issue_key.clone()),
        "issue_number": issue_number,
        "write_mode": "update_issue",
        "remote_id": if valid { issue_key.clone() } else { format!("{}:{}", provider.name, external_key) },
        "remote_url": if valid { Some(format!("linear://{issue_key}")) } else { None },
        "api_url": if valid { Some(api_url) } else { None }
    })
}

fn connector_fixture_remote_target(
    provider: &ConnectorProviderSpec,
    review_surface: &str,
    external_key: &str,
    target: &str,
) -> serde_json::Value {
    let fixture_key = if target.is_empty() {
        external_key
    } else {
        target
    };
    let blockers = if fixture_key.trim().is_empty() {
        vec!["fixture_target_missing".to_string()]
    } else {
        Vec::new()
    };
    let valid = blockers.is_empty();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_TARGET_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "review_surface": review_surface,
        "external_key": external_key,
        "target": fixture_key,
        "valid": valid,
        "blockers": blockers,
        "target_kind": "fixture.issue",
        "fixture_key": fixture_key,
        "write_mode": "upsert_fixture_issue",
        "remote_id": format!("remote-fixture:{fixture_key}"),
        "remote_url": serde_json::Value::Null,
        "api_url": serde_json::Value::Null
    })
}

fn connector_generic_remote_target(
    provider: &ConnectorProviderSpec,
    review_surface: &str,
    external_key: &str,
    target: &str,
) -> serde_json::Value {
    let target = if target.is_empty() {
        external_key
    } else {
        target
    };
    let blockers = if target.trim().is_empty() {
        vec!["remote_target_missing".to_string()]
    } else {
        Vec::new()
    };
    let valid = blockers.is_empty();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_TARGET_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "review_surface": review_surface,
        "external_key": external_key,
        "target": target,
        "valid": valid,
        "blockers": blockers,
        "target_kind": "remote.issue",
        "write_mode": "upsert_remote_issue",
        "remote_id": format!("{}:{}", provider.name, target),
        "remote_url": serde_json::Value::Null,
        "api_url": serde_json::Value::Null
    })
}

fn connector_remote_write_plan(
    provider: Option<&ConnectorProviderSpec>,
    issue: &serde_json::Value,
    remote_target: &serde_json::Value,
    publish_blockers: &[String],
) -> serde_json::Value {
    let Some(provider) = provider else {
        return serde_json::Value::Null;
    };
    if !connector_provider_uses_remote_contract(provider) {
        return serde_json::Value::Null;
    }
    let target_valid = remote_target
        .pointer("/valid")
        .and_then(|value| value.as_bool())
        == Some(true);
    let mut blockers = publish_blockers.to_vec();
    if !target_valid
        && !blockers
            .iter()
            .any(|blocker| blocker == "remote_target_invalid")
    {
        blockers.push("remote_target_invalid".to_string());
    }
    let source = connector_remote_write_source(issue);
    let operations = match provider.name.as_str() {
        "github" => connector_github_remote_write_operations(remote_target, issue),
        "linear" => connector_linear_remote_write_operations(remote_target, issue),
        "remote-fixture" => connector_fixture_remote_write_operations(remote_target, issue),
        _ => connector_generic_remote_write_operations(provider, remote_target, issue),
    };
    blockers.extend(connector_remote_write_operation_blockers(&operations));
    blockers.sort();
    blockers.dedup();
    let executable = blockers.is_empty();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_WRITE_PLAN_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "remote_object_kind": connector_remote_object_kind(&provider.name),
        "operation": "upsert_remote_issue_surface",
        "executable": executable,
        "blocked_by": blockers,
        "auth": connector_remote_write_auth_plan(provider),
        "remote_target": remote_target,
        "source": source,
        "operations": operations,
        "receipt_schema_version": CONNECTOR_REMOTE_WRITE_RECEIPT_SCHEMA_VERSION,
        "readback_schema_version": CONNECTOR_REMOTE_READBACK_SCHEMA_VERSION
    })
}

fn connector_remote_write_operation_blockers(operations: &serde_json::Value) -> Vec<String> {
    operations
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|operation| {
            operation
                .pointer("/blocked_by")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
        })
        .filter_map(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn connector_remote_write_source(issue: &serde_json::Value) -> serde_json::Value {
    let latest_comment = issue
        .pointer("/latest_comment")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "issue_id": issue.pointer("/id").and_then(|value| value.as_i64()),
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        "title": issue.pointer("/title").and_then(|value| value.as_str()),
        "status": issue.pointer("/status").and_then(|value| value.as_str()),
        "summary": issue.pointer("/summary").and_then(|value| value.as_str()),
        "comment_count": issue.pointer("/comment_count").and_then(|value| value.as_u64()),
        "latest_comment": latest_comment,
        "current_sha256": issue.pointer("/connector/current_sha256").and_then(|value| value.as_str()),
        "external_key": issue.pointer("/connector/external_key").and_then(|value| value.as_str())
    })
}

fn connector_remote_write_issue_from_mirror(
    mirror: &IssueMirrorReport,
    digest: &MirrorFileDigest,
) -> serde_json::Value {
    serde_json::json!({
        "id": mirror.issue.id,
        "loop_id": mirror.issue.loop_id,
        "title": mirror.issue.title.as_str(),
        "status": mirror.issue.status.as_str(),
        "summary": mirror.issue.summary.as_deref(),
        "updated_at": mirror.issue.updated_at.as_str(),
        "comment_count": mirror.comments.len(),
        "latest_comment": mirror.comments.last().map(|comment| serde_json::json!({
            "id": comment.id,
            "author": comment.author.as_str(),
            "body": comment.body.as_str(),
            "created_at": comment.created_at.as_str(),
            "payload": comment.payload
        })),
        "connector": {
            "external_key": mirror.external_key.as_str(),
            "current_sha256": digest.sha256.as_str()
        }
    })
}

fn connector_remote_write_auth_plan(provider: &ConnectorProviderSpec) -> serde_json::Value {
    serde_json::json!({
        "required": provider.auth_required,
        "configured": provider.configured,
        "env": provider.auth_env.iter().map(String::as_str).collect::<Vec<_>>(),
        "header": if provider.auth_required { Some("Authorization: Bearer <redacted>") } else { None::<&str> }
    })
}

fn connector_github_remote_write_operations(
    remote_target: &serde_json::Value,
    issue: &serde_json::Value,
) -> serde_json::Value {
    let api_url = remote_target
        .pointer("/api_url")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let issue_number = remote_target
        .pointer("/issue_number")
        .and_then(|value| value.as_u64());
    let status = issue
        .pointer("/status")
        .and_then(|value| value.as_str())
        .unwrap_or("Todo");
    let state = connector_github_issue_state(status);
    let state_reason = connector_github_issue_state_reason(status);
    let method = if issue_number.is_some() {
        "PATCH"
    } else {
        "POST"
    };
    let mut body = serde_json::json!({
        "title": issue.pointer("/title").and_then(|value| value.as_str()).unwrap_or("Entrance issue"),
        "body": connector_remote_issue_body(issue),
        "state": state
    });
    if let Some(reason) = state_reason {
        if let Some(object) = body.as_object_mut() {
            object.insert(
                "state_reason".to_string(),
                serde_json::Value::String(reason.to_string()),
            );
        }
    }
    let comment_operation = issue_number.map(|number| {
        let owner = remote_target
            .pointer("/owner")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let repo = remote_target
            .pointer("/repo")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let api_base_url = remote_target
            .pointer("/api_base_url")
            .and_then(|value| value.as_str())
            .unwrap_or("https://api.github.com");
        let idempotency_key = connector_remote_issue_idempotency_key("github", issue);
        serde_json::json!({
            "kind": "upsert_latest_comment",
            "method": "GET_PATCH_OR_POST",
            "url": format!("{api_base_url}/repos/{owner}/{repo}/issues/{number}/comments"),
            "lookup": {
                "method": "GET",
                "url": format!("{api_base_url}/repos/{owner}/{repo}/issues/{number}/comments?per_page=100"),
                "idempotency_key": idempotency_key.as_str()
            },
            "update": {
                "method": "PATCH",
                "url_template": format!("{api_base_url}/repos/{owner}/{repo}/issues/comments/{{comment_id}}")
            },
            "headers": connector_github_headers(),
            "body": {
                "body": connector_github_remote_comment_body(issue)
            },
            "source": "hive_issue.latest_comment",
            "idempotency_key": idempotency_key
        })
    });
    let mut operations = vec![serde_json::json!({
            "kind": if issue_number.is_some() { "update_issue" } else { "create_issue" },
            "method": method,
            "url": empty_str_none(api_url),
            "headers": connector_github_headers(),
            "body": body,
            "source": "hive_issue.status_title_summary"
    })];
    if let Some(comment_operation) = comment_operation {
        operations.push(comment_operation);
    }
    serde_json::Value::Array(operations)
}

fn connector_linear_remote_write_operations(
    remote_target: &serde_json::Value,
    issue: &serde_json::Value,
) -> serde_json::Value {
    let issue_key = remote_target
        .pointer("/issue_key")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let api_url = remote_target
        .pointer("/api_url")
        .and_then(|value| value.as_str())
        .unwrap_or("https://api.linear.app/graphql");
    let idempotency_key = connector_remote_issue_idempotency_key("linear", issue);
    serde_json::json!([
        {
            "kind": "read_issue_for_write",
            "method": "POST",
            "url": api_url,
            "headers": connector_linear_headers(),
            "graphql": {
                "operation": "EntranceIssueRead",
                "query": "query EntranceIssueRead($id: String!) { issue(id: $id) { id identifier title description url state { id name type } comments { nodes { id body createdAt updatedAt user { name } } } } }",
                "variables": {
                    "id": issue_key
                }
            },
            "source": "remote_issue.identity"
        },
        {
            "kind": "update_issue",
            "method": "POST",
            "url": api_url,
            "headers": connector_linear_headers(),
            "graphql": {
                "operation": "EntranceIssueUpdate",
                "query": "mutation EntranceIssueUpdate($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success issue { id identifier title state { id name } } } }",
                "variables": {
                    "id": "<lookup.issue.id>",
                    "input": {
                        "title": issue.pointer("/title").and_then(|value| value.as_str()).unwrap_or("Entrance issue"),
                        "description": connector_remote_issue_body(issue)
                    }
                }
            },
            "source": "hive_issue.status_title_summary",
            "status_mapping": {
                "strategy": "description_status_marker_until_state_mapping",
                "source_status": issue.pointer("/status").and_then(|value| value.as_str())
            }
        },
        {
            "kind": "upsert_latest_comment",
            "method": "GRAPHQL_READ_UPDATE_OR_CREATE",
            "url": api_url,
            "headers": connector_linear_headers(),
            "create": {
                "operation": "EntranceCommentCreate",
                "query": "mutation EntranceCommentCreate($input: CommentCreateInput!) { commentCreate(input: $input) { success comment { id body } } }",
                "issue_id_variable": "input.issueId"
            },
            "update": {
                "operation": "EntranceCommentUpdate",
                "query": "mutation EntranceCommentUpdate($id: String!, $input: CommentUpdateInput!) { commentUpdate(id: $id, input: $input) { success comment { id body } } }"
            },
            "body": {
                "body": connector_linear_remote_comment_body(issue)
            },
            "source": "hive_issue.latest_comment",
            "idempotency_key": idempotency_key
        }
    ])
}

fn connector_fixture_remote_write_operations(
    remote_target: &serde_json::Value,
    issue: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!([
        {
            "kind": "upsert_fixture_issue",
            "method": "WRITE_FILE",
            "url": remote_target.pointer("/remote_id").and_then(|value| value.as_str()),
            "body": {
                "title": issue.pointer("/title").and_then(|value| value.as_str()),
                "status": issue.pointer("/status").and_then(|value| value.as_str()),
                "summary": issue.pointer("/summary").and_then(|value| value.as_str()),
                "comment_count": issue.pointer("/comment_count").and_then(|value| value.as_u64())
            },
            "source": "hive_issue.mirror"
        }
    ])
}

fn connector_generic_remote_write_operations(
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    issue: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!([
        {
            "kind": "upsert_remote_issue",
            "method": "PROVIDER_ADAPTER",
            "url": remote_target.pointer("/remote_url").and_then(|value| value.as_str()),
            "provider": provider.name.as_str(),
            "body": {
                "title": issue.pointer("/title").and_then(|value| value.as_str()),
                "status": issue.pointer("/status").and_then(|value| value.as_str()),
                "summary": issue.pointer("/summary").and_then(|value| value.as_str())
            },
            "source": "hive_issue"
        }
    ])
}

fn connector_github_headers() -> Vec<&'static str> {
    vec![
        "Accept: application/vnd.github+json",
        "Authorization: Bearer <redacted>",
        "X-GitHub-Api-Version: 2026-03-10",
    ]
}

fn connector_linear_headers() -> Vec<&'static str> {
    vec![
        "Content-Type: application/json",
        "Authorization: Bearer <redacted>",
    ]
}

fn connector_github_issue_state(status: &str) -> &'static str {
    match status {
        "Done" | "Canceled" => "closed",
        _ => "open",
    }
}

fn connector_github_issue_state_reason(status: &str) -> Option<&'static str> {
    match status {
        "Done" => Some("completed"),
        "Canceled" => Some("not_planned"),
        _ => None,
    }
}

fn connector_remote_issue_body(issue: &serde_json::Value) -> String {
    let summary = issue
        .pointer("/summary")
        .and_then(|value| value.as_str())
        .unwrap_or("No summary.");
    let status = issue
        .pointer("/status")
        .and_then(|value| value.as_str())
        .unwrap_or("Unknown");
    let issue_id = issue
        .pointer("/id")
        .and_then(|value| value.as_i64())
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let external_key = issue
        .pointer("/connector/external_key")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    format!("Entrance mirror for issue #{issue_id}.\n\nStatus: {status}\nExternal key: {external_key}\n\n{summary}")
}

fn connector_remote_comment_body(issue: &serde_json::Value) -> String {
    let latest = issue
        .pointer("/latest_comment/body")
        .and_then(|value| value.as_str())
        .unwrap_or("No comment body.");
    let author = issue
        .pointer("/latest_comment/author")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    format!("Entrance comment from {author}:\n\n{latest}")
}

fn connector_github_remote_comment_body(issue: &serde_json::Value) -> String {
    let body = connector_remote_comment_body(issue);
    let idempotency_key = connector_remote_issue_idempotency_key("github", issue);
    if idempotency_key.trim().is_empty() {
        body
    } else {
        format!(
            "{body}\n\n{}{} {}",
            GITHUB_COMMENT_IDEMPOTENCY_PREFIX, idempotency_key, GITHUB_COMMENT_IDEMPOTENCY_SUFFIX
        )
    }
}

fn connector_linear_remote_comment_body(issue: &serde_json::Value) -> String {
    let body = connector_remote_comment_body(issue);
    let idempotency_key = connector_remote_issue_idempotency_key("linear", issue);
    if idempotency_key.trim().is_empty() {
        body
    } else {
        format!(
            "{body}\n\n{}{} {}",
            LINEAR_COMMENT_IDEMPOTENCY_PREFIX, idempotency_key, LINEAR_COMMENT_IDEMPOTENCY_SUFFIX
        )
    }
}

fn execute_linear_remote_write_plan(
    provider: &ConnectorProviderSpec,
    plan: &serde_json::Value,
) -> serde_json::Value {
    let provider_name = provider.name.clone();
    let plan_schema_version = plan
        .pointer("/schema_version")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let provider = provider.clone();
    let plan = plan.clone();
    std::thread::spawn(move || execute_linear_remote_write_plan_blocking(&provider, &plan))
        .join()
        .unwrap_or_else(|_| {
            serde_json::json!({
                "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
                "provider": provider_name,
                "plan_schema_version": plan_schema_version,
                "success": false,
                "failed_checks": ["remote_write_thread_panic"],
                "operations": []
            })
        })
}

fn execute_linear_remote_write_plan_blocking(
    provider: &ConnectorProviderSpec,
    plan: &serde_json::Value,
) -> serde_json::Value {
    let Some(token) = connector_provider_auth_token(provider) else {
        return serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
            "provider": provider.name.as_str(),
            "plan_schema_version": plan.pointer("/schema_version").and_then(|value| value.as_str()),
            "success": false,
            "failed_checks": ["remote_auth_missing"],
            "operations": []
        });
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Entrance-Hive/2.0")
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return serde_json::json!({
                "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
                "provider": provider.name.as_str(),
                "plan_schema_version": plan.pointer("/schema_version").and_then(|value| value.as_str()),
                "success": false,
                "failed_checks": ["http_client_unavailable"],
                "error": error.to_string(),
                "operations": []
            });
        }
    };
    let mut operation_results = Vec::new();
    let mut failed_checks = Vec::new();
    let mut issue_context = serde_json::Value::Null;
    for operation in plan
        .pointer("/operations")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        let blockers = operation
            .pointer("/blocked_by")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !blockers.is_empty() {
            failed_checks.extend(blockers.clone());
            operation_results.push(serde_json::json!({
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": operation.pointer("/method").and_then(|value| value.as_str()),
                "url": operation.pointer("/url").and_then(|value| value.as_str()),
                "success": false,
                "skipped": true,
                "failed_checks": blockers
            }));
            continue;
        }
        let result = match operation.pointer("/kind").and_then(|value| value.as_str()) {
            Some("upsert_latest_comment") => execute_linear_upsert_latest_comment_operation(
                &client,
                &token,
                operation,
                &issue_context,
            ),
            _ => {
                let operation = linear_operation_with_issue_context(operation, &issue_context);
                execute_linear_graphql_operation(&client, &token, &operation)
            }
        };
        let result_kind = result.pointer("/kind").and_then(|value| value.as_str());
        let result_success =
            result.pointer("/success").and_then(|value| value.as_bool()) == Some(true);
        if !result_success {
            failed_checks.push(
                result
                    .pointer("/failed_check")
                    .and_then(|value| value.as_str())
                    .unwrap_or("remote_operation_failed")
                    .to_string(),
            );
        }
        if result_kind == Some("read_issue_for_write") {
            if let Some(summary) = result.pointer("/response/summary") {
                issue_context = summary.clone();
            }
        }
        let stop_after_result = result_kind == Some("read_issue_for_write") && !result_success;
        operation_results.push(result);
        if stop_after_result {
            break;
        }
    }
    failed_checks.sort();
    failed_checks.dedup();
    let success = failed_checks.is_empty();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "plan_schema_version": plan.pointer("/schema_version").and_then(|value| value.as_str()),
        "remote_object_kind": plan.pointer("/remote_object_kind").and_then(|value| value.as_str()),
        "success": success,
        "failed_checks": failed_checks,
        "operation_count": operation_results.len(),
        "operations": operation_results
    })
}

fn linear_operation_with_issue_context(
    operation: &serde_json::Value,
    issue_context: &serde_json::Value,
) -> serde_json::Value {
    let mut operation = operation.clone();
    let Some(issue_id) = issue_context
        .pointer("/id")
        .and_then(|value| value.as_str())
    else {
        return operation;
    };
    if let Some(id) = operation.pointer_mut("/graphql/variables/id") {
        if id.as_str() == Some("<lookup.issue.id>") {
            *id = serde_json::Value::String(issue_id.to_string());
        }
    }
    operation
}

fn execute_linear_upsert_latest_comment_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    operation: &serde_json::Value,
    issue_context: &serde_json::Value,
) -> serde_json::Value {
    let idempotency_key = operation
        .pointer("/idempotency_key")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let Some(issue_id) = issue_context
        .pointer("/id")
        .and_then(|value| value.as_str())
    else {
        return serde_json::json!({
            "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
            "method": operation.pointer("/method").and_then(|value| value.as_str()),
            "url": operation.pointer("/url").and_then(|value| value.as_str()),
            "success": false,
            "failed_check": "linear_issue_uuid_readback_required",
            "idempotency_key": idempotency_key
        });
    };
    let matched_comment = linear_comment_summary_find_idempotency_key(
        issue_context.pointer("/comments"),
        idempotency_key,
    );
    let body = operation
        .pointer("/body/body")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let (mode, write_operation) = if let Some(comment) = matched_comment.as_ref() {
        let comment_id = comment
            .pointer("/id")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        (
            "update_existing_comment",
            serde_json::json!({
                "kind": "update_latest_comment",
                "method": "POST",
                "url": operation.pointer("/url").and_then(|value| value.as_str()).unwrap_or_default(),
                "graphql": {
                    "operation": operation.pointer("/update/operation").and_then(|value| value.as_str()).unwrap_or("EntranceCommentUpdate"),
                    "query": operation.pointer("/update/query").and_then(|value| value.as_str()).unwrap_or("mutation EntranceCommentUpdate($id: String!, $input: CommentUpdateInput!) { commentUpdate(id: $id, input: $input) { success comment { id body } } }"),
                    "variables": {
                        "id": comment_id,
                        "input": {
                            "body": body
                        }
                    }
                },
                "source": operation.pointer("/source").and_then(|value| value.as_str())
            }),
        )
    } else {
        (
            "append_new_comment",
            serde_json::json!({
                "kind": "append_latest_comment",
                "method": "POST",
                "url": operation.pointer("/url").and_then(|value| value.as_str()).unwrap_or_default(),
                "graphql": {
                    "operation": operation.pointer("/create/operation").and_then(|value| value.as_str()).unwrap_or("EntranceCommentCreate"),
                    "query": operation.pointer("/create/query").and_then(|value| value.as_str()).unwrap_or("mutation EntranceCommentCreate($input: CommentCreateInput!) { commentCreate(input: $input) { success comment { id body } } }"),
                    "variables": {
                        "input": {
                            "issueId": issue_id,
                            "body": body
                        }
                    }
                },
                "source": operation.pointer("/source").and_then(|value| value.as_str())
            }),
        )
    };
    let write = execute_linear_graphql_operation(client, token, &write_operation);
    let success = write.pointer("/success").and_then(|value| value.as_bool()) == Some(true);
    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": write.pointer("/method").and_then(|value| value.as_str()),
        "url": write.pointer("/url").and_then(|value| value.as_str()),
        "success": success,
        "failed_check": if success { None::<&str> } else { Some("remote_comment_upsert_failed") },
        "mode": mode,
        "idempotency_key": idempotency_key,
        "matched_comment": matched_comment.map(|comment| serde_json::json!({
            "id": comment.pointer("/id").and_then(|value| value.as_str())
        })),
        "write": write
    })
}

fn execute_linear_graphql_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    operation: &serde_json::Value,
) -> serde_json::Value {
    let method_label = operation
        .pointer("/method")
        .and_then(|value| value.as_str())
        .unwrap_or("POST");
    let url = operation
        .pointer("/url")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if method_label != "POST" {
        return serde_json::json!({
            "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
            "method": method_label,
            "url": url,
            "success": false,
            "failed_check": "http_method_invalid"
        });
    }
    if url.trim().is_empty() {
        return serde_json::json!({
            "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
            "method": method_label,
            "url": url,
            "success": false,
            "failed_check": "remote_url_missing"
        });
    }
    let body = serde_json::json!({
        "operationName": operation.pointer("/graphql/operation").and_then(|value| value.as_str()),
        "query": operation.pointer("/graphql/query").and_then(|value| value.as_str()).unwrap_or_default(),
        "variables": operation.pointer("/graphql/variables").cloned().unwrap_or_else(|| serde_json::json!({}))
    });
    let mut attempts = Vec::new();
    let retry_policy = linear_retry_policy();
    let max_attempts = connector_retry_max_attempts(&retry_policy);
    for attempt_number in 1..=max_attempts {
        let mut result = execute_linear_graphql_operation_attempt(
            client,
            token,
            operation,
            method_label,
            url,
            &body,
        );
        let should_retry =
            linear_operation_result_should_retry_now(&result, attempt_number, &retry_policy);
        if should_retry {
            linear_mark_retry_scheduled(
                &mut result,
                linear_retry_backoff_ms(attempt_number, &retry_policy),
            );
        }
        attempts.push(linear_operation_attempt_summary(&result, attempt_number));
        if !should_retry {
            return linear_operation_with_attempts(result, attempts, &retry_policy);
        }
        std::thread::sleep(Duration::from_millis(linear_retry_backoff_ms(
            attempt_number,
            &retry_policy,
        )));
    }

    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": method_label,
        "url": url,
        "success": false,
        "failed_check": "remote_retry_exhausted",
        "attempt_count": attempts.len(),
        "max_attempts": max_attempts,
        "attempts": attempts
    })
}

fn execute_linear_graphql_operation_attempt(
    client: &reqwest::blocking::Client,
    token: &str,
    operation: &serde_json::Value,
    method_label: &str,
    url: &str,
    body: &serde_json::Value,
) -> serde_json::Value {
    let response = client
        .post(url)
        .bearer_auth(token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send();
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return serde_json::json!({
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": method_label,
                "url": url,
                "success": false,
                "failed_check": "remote_request_failed",
                "retry": linear_request_retry_metadata(),
                "error": error.to_string()
            });
        }
    };
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let text = match response.text() {
        Ok(text) => text,
        Err(error) => {
            return serde_json::json!({
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": method_label,
                "url": url,
                "success": false,
                "failed_check": "remote_response_read_failed",
                "http_status": status,
                "retry": linear_response_read_retry_metadata(),
                "error": error.to_string()
            });
        }
    };
    let digest = digest_bytes(text.as_bytes());
    let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
    let parsed_ok = parsed.is_some();
    let status_success = (200..300).contains(&status);
    let graphql_errors = parsed
        .as_ref()
        .and_then(|value| value.pointer("/errors"))
        .and_then(|value| value.as_array())
        .map(|errors| !errors.is_empty())
        .unwrap_or(false);
    let graphql_rate_limited = linear_graphql_errors_rate_limited(parsed.as_ref());
    let summary = parsed
        .as_ref()
        .map(|value| compact_linear_graphql_response_summary(value, operation));
    let success = status_success
        && parsed_ok
        && !graphql_errors
        && linear_graphql_summary_success(summary.as_ref());
    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": method_label,
        "url": url,
        "success": success,
        "failed_check": if success {
            None::<&str>
        } else if !status_success {
            Some(linear_http_failed_check(status, &headers))
        } else if !parsed_ok {
            Some("remote_response_parse_failed")
        } else if graphql_rate_limited {
            Some("remote_rate_limited")
        } else if graphql_errors {
            Some("remote_graphql_errors")
        } else {
            Some("remote_graphql_result_failed")
        },
        "http_status": status,
        "retry": if success {
            linear_terminal_retry_metadata("success")
        } else if !status_success {
            linear_http_retry_metadata(status, &headers)
        } else if !parsed_ok {
            linear_parse_retry_metadata(parsed_ok)
        } else if graphql_rate_limited {
            linear_graphql_rate_limit_retry_metadata(&headers, parsed.as_ref())
        } else {
            linear_terminal_retry_metadata(if graphql_errors {
                "graphql_errors"
            } else {
                "graphql_result_failed"
            })
        },
        "request": {
            "body_sha256": digest_bytes(serde_json::to_vec(&body).unwrap_or_default().as_slice()).sha256,
            "auth": "Authorization: Bearer <redacted>"
        },
        "response": {
            "headers": linear_response_headers_summary(&headers),
            "bytes": digest.bytes,
            "sha256": digest.sha256,
            "parsed": parsed_ok,
            "summary": summary,
            "errors": parsed.as_ref().and_then(|value| value.pointer("/errors")).cloned().unwrap_or_else(|| serde_json::json!([]))
        }
    })
}

fn linear_operation_result_should_retry_now(
    result: &serde_json::Value,
    attempt_number: usize,
    retry_policy: &ConnectorRetryPolicySpec,
) -> bool {
    if attempt_number >= connector_retry_max_attempts(retry_policy) {
        return false;
    }
    if result.pointer("/success").and_then(|value| value.as_bool()) == Some(true) {
        return false;
    }
    if result
        .pointer("/retry/rate_limited")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        return false;
    }
    result
        .pointer("/retry/retryable")
        .and_then(|value| value.as_bool())
        == Some(true)
}

fn linear_operation_with_attempts(
    mut result: serde_json::Value,
    attempts: Vec<serde_json::Value>,
    retry_policy: &ConnectorRetryPolicySpec,
) -> serde_json::Value {
    let max_attempts = connector_retry_max_attempts(retry_policy);
    let success = result.pointer("/success").and_then(|value| value.as_bool()) == Some(true);
    let rate_limited = result
        .pointer("/retry/rate_limited")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let retryable = result
        .pointer("/retry/retryable")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let attempted = attempts.len() > 1;
    let exhausted = !success && retryable && !rate_limited && attempts.len() >= max_attempts;
    let mut retry = result
        .pointer("/retry")
        .cloned()
        .unwrap_or_else(|| linear_terminal_retry_metadata("success"));
    if let Some(object) = retry.as_object_mut() {
        object.insert("attempted".to_string(), serde_json::json!(attempted));
        object.insert("exhausted".to_string(), serde_json::json!(exhausted));
    }
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "attempt_count".to_string(),
            serde_json::json!(attempts.len()),
        );
        object.insert("max_attempts".to_string(), serde_json::json!(max_attempts));
        object.insert("retry".to_string(), retry);
        object.insert("attempts".to_string(), serde_json::Value::Array(attempts));
    }
    result
}

fn linear_operation_attempt_summary(
    result: &serde_json::Value,
    attempt_number: usize,
) -> serde_json::Value {
    serde_json::json!({
        "attempt": attempt_number,
        "success": result.pointer("/success").and_then(|value| value.as_bool()),
        "failed_check": result.pointer("/failed_check").and_then(|value| value.as_str()),
        "http_status": result.pointer("/http_status").and_then(|value| value.as_u64()),
        "retry": result.pointer("/retry").cloned().unwrap_or_else(|| serde_json::json!({})),
        "error": result.pointer("/error").and_then(|value| value.as_str()),
        "response_headers": result.pointer("/response/headers").cloned().unwrap_or_else(|| serde_json::json!({}))
    })
}

fn linear_mark_retry_scheduled(result: &mut serde_json::Value, backoff_ms: u64) {
    if let Some(object) = result.as_object_mut() {
        let retry = object
            .entry("retry".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !retry.is_object() {
            *retry = serde_json::json!({});
        }
        if let Some(retry_object) = retry.as_object_mut() {
            retry_object.insert("scheduled".to_string(), serde_json::json!(true));
            retry_object.insert("backoff_ms".to_string(), serde_json::json!(backoff_ms));
        }
    }
}

fn linear_retry_backoff_ms(attempt_number: usize, retry_policy: &ConnectorRetryPolicySpec) -> u64 {
    connector_retry_backoff_ms(retry_policy, attempt_number)
}

fn linear_http_failed_check(status: u16, headers: &reqwest::header::HeaderMap) -> &'static str {
    if linear_http_rate_limited(status, headers) {
        "remote_rate_limited"
    } else if linear_http_retryable_status(status) {
        "remote_retryable_http_status"
    } else {
        "remote_http_status_failed"
    }
}

fn linear_http_retry_metadata(
    status: u16,
    headers: &reqwest::header::HeaderMap,
) -> serde_json::Value {
    let rate_limited = linear_http_rate_limited(status, headers);
    let retryable = linear_http_retryable_status(status);
    serde_json::json!({
        "retryable": retryable,
        "rate_limited": rate_limited,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": if rate_limited {
            "rate_limited"
        } else if retryable {
            "retryable_http_status"
        } else {
            "http_status_failed"
        },
        "retry_after_secs": linear_retry_after_secs(headers),
        "rate_limit": linear_rate_limit_summary(headers)
    })
}

fn linear_request_retry_metadata() -> serde_json::Value {
    serde_json::json!({
        "retryable": true,
        "rate_limited": false,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": "request_failed"
    })
}

fn linear_response_read_retry_metadata() -> serde_json::Value {
    serde_json::json!({
        "retryable": true,
        "rate_limited": false,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": "response_read_failed"
    })
}

fn linear_parse_retry_metadata(parsed: bool) -> serde_json::Value {
    linear_terminal_retry_metadata(if parsed {
        "success"
    } else {
        "response_parse_failed"
    })
}

fn linear_graphql_rate_limit_retry_metadata(
    headers: &reqwest::header::HeaderMap,
    parsed: Option<&serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "retryable": false,
        "rate_limited": true,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": "graphql_rate_limited",
        "retry_after_secs": linear_retry_after_secs(headers),
        "rate_limit": linear_rate_limit_summary(headers),
        "graphql_errors": parsed.and_then(|value| value.pointer("/errors")).cloned().unwrap_or_else(|| serde_json::json!([]))
    })
}

fn linear_terminal_retry_metadata(reason: &str) -> serde_json::Value {
    serde_json::json!({
        "retryable": false,
        "rate_limited": false,
        "scheduled": false,
        "backoff_ms": None::<u64>,
        "reason": reason
    })
}

fn linear_http_rate_limited(status: u16, headers: &reqwest::header::HeaderMap) -> bool {
    let retry_policy = linear_retry_policy();
    retry_policy.rate_limit_http_statuses.contains(&status)
        && (status == 429
            || (status == 403
                && (linear_header_value(headers, "x-ratelimit-remaining").as_deref() == Some("0")
                    || linear_header_value(headers, "x-rate-limit-remaining").as_deref()
                        == Some("0")
                    || linear_header_value(headers, "retry-after").is_some())))
}

fn linear_http_retryable_status(status: u16) -> bool {
    linear_retry_policy()
        .retryable_http_statuses
        .contains(&status)
}

fn linear_response_headers_summary(headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    serde_json::json!({
        "retry_after": linear_header_value(headers, "retry-after"),
        "x_ratelimit_remaining": linear_header_value(headers, "x-ratelimit-remaining"),
        "x_ratelimit_reset": linear_header_value(headers, "x-ratelimit-reset"),
        "x_ratelimit_limit": linear_header_value(headers, "x-ratelimit-limit"),
        "x_rate_limit_remaining": linear_header_value(headers, "x-rate-limit-remaining"),
        "x_rate_limit_reset": linear_header_value(headers, "x-rate-limit-reset"),
        "x_rate_limit_limit": linear_header_value(headers, "x-rate-limit-limit")
    })
}

fn linear_rate_limit_summary(headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    serde_json::json!({
        "remaining": linear_header_value(headers, "x-ratelimit-remaining")
            .or_else(|| linear_header_value(headers, "x-rate-limit-remaining")),
        "reset": linear_header_value(headers, "x-ratelimit-reset")
            .or_else(|| linear_header_value(headers, "x-rate-limit-reset")),
        "limit": linear_header_value(headers, "x-ratelimit-limit")
            .or_else(|| linear_header_value(headers, "x-rate-limit-limit"))
    })
}

fn linear_retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    linear_header_value(headers, "retry-after").and_then(|value| value.trim().parse::<u64>().ok())
}

fn linear_header_value(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn linear_graphql_errors_rate_limited(parsed: Option<&serde_json::Value>) -> bool {
    parsed
        .and_then(|value| value.pointer("/errors"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .any(|error| {
            [
                "message",
                "extensions/code",
                "extensions/type",
                "extensions/category",
            ]
            .into_iter()
            .filter_map(|path| {
                let pointer = format!("/{}", path.replace('/', "/"));
                error.pointer(&pointer).and_then(|value| value.as_str())
            })
            .any(linear_graphql_error_value_is_rate_limit)
        })
}

fn linear_graphql_error_value_is_rate_limit(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.contains("ratelimit")
        || normalized.contains("ratelimited")
        || normalized.contains("toomanyrequests")
}

fn compact_linear_graphql_response_summary(
    response: &serde_json::Value,
    operation: &serde_json::Value,
) -> serde_json::Value {
    match operation.pointer("/kind").and_then(|value| value.as_str()) {
        Some("read_issue") | Some("read_issue_for_write") => response
            .pointer("/data/issue")
            .map(compact_linear_issue_summary)
            .unwrap_or_else(|| serde_json::json!(null)),
        Some("update_issue") => response
            .pointer("/data/issueUpdate/issue")
            .map(compact_linear_issue_summary)
            .unwrap_or_else(|| serde_json::json!(null)),
        Some("append_latest_comment") => response
            .pointer("/data/commentCreate/comment")
            .map(compact_linear_comment_summary)
            .unwrap_or_else(|| serde_json::json!(null)),
        Some("update_latest_comment") => response
            .pointer("/data/commentUpdate/comment")
            .map(compact_linear_comment_summary)
            .unwrap_or_else(|| serde_json::json!(null)),
        _ => serde_json::json!(null),
    }
}

fn linear_graphql_summary_success(summary: Option<&serde_json::Value>) -> bool {
    summary
        .and_then(|value| value.as_object())
        .map(|object| !object.is_empty())
        .unwrap_or(false)
}

fn compact_linear_issue_summary(issue: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": issue.pointer("/id").and_then(|value| value.as_str()),
        "identifier": issue.pointer("/identifier").and_then(|value| value.as_str()),
        "title": issue.pointer("/title").and_then(|value| value.as_str()),
        "description": issue.pointer("/description").and_then(|value| value.as_str()),
        "url": issue.pointer("/url").and_then(|value| value.as_str()),
        "state": {
            "id": issue.pointer("/state/id").and_then(|value| value.as_str()),
            "name": issue.pointer("/state/name").and_then(|value| value.as_str()),
            "type": issue.pointer("/state/type").and_then(|value| value.as_str())
        },
        "comments": compact_linear_comments_readback_summary(
            issue.pointer("/comments/nodes").unwrap_or(&serde_json::Value::Null)
        )
    })
}

fn compact_linear_comment_summary(comment: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": comment.pointer("/id").and_then(|value| value.as_str()),
        "body": comment.pointer("/body").and_then(|value| value.as_str()),
        "created_at": comment.pointer("/createdAt").and_then(|value| value.as_str())
            .or_else(|| comment.pointer("/created_at").and_then(|value| value.as_str())),
        "updated_at": comment.pointer("/updatedAt").and_then(|value| value.as_str())
            .or_else(|| comment.pointer("/updated_at").and_then(|value| value.as_str())),
        "author": comment.pointer("/user/name").and_then(|value| value.as_str())
    })
}

fn compact_linear_comments_readback_summary(response: &serde_json::Value) -> serde_json::Value {
    let comments = response
        .as_array()
        .map(|items| {
            items
                .iter()
                .take(100)
                .map(compact_linear_comment_summary)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    serde_json::json!({
        "count": response.as_array().map(Vec::len).unwrap_or_default(),
        "items": comments
    })
}

fn linear_readback_comments_contain_body(
    comments_summary: Option<&serde_json::Value>,
    expected_body: &str,
) -> bool {
    comments_summary
        .and_then(|value| value.pointer("/items"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .any(|comment| {
            comment.pointer("/body").and_then(|value| value.as_str()) == Some(expected_body)
        })
}

fn linear_comment_summary_find_idempotency_key(
    comments_summary: Option<&serde_json::Value>,
    idempotency_key: &str,
) -> Option<serde_json::Value> {
    if idempotency_key.trim().is_empty() {
        return None;
    }
    comments_summary
        .and_then(|value| value.pointer("/items"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .find(|comment| {
            comment
                .pointer("/body")
                .and_then(|value| value.as_str())
                .map(|body| linear_comment_body_has_idempotency_key(body, idempotency_key))
                == Some(true)
        })
        .cloned()
}

fn linear_comment_body_has_idempotency_key(body: &str, idempotency_key: &str) -> bool {
    body.contains(&format!(
        "{}{}",
        LINEAR_COMMENT_IDEMPOTENCY_PREFIX, idempotency_key
    ))
}

fn execute_github_remote_write_plan(
    provider: &ConnectorProviderSpec,
    plan: &serde_json::Value,
) -> serde_json::Value {
    let provider_name = provider.name.clone();
    let plan_schema_version = plan
        .pointer("/schema_version")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let provider = provider.clone();
    let plan = plan.clone();
    std::thread::spawn(move || execute_github_remote_write_plan_blocking(&provider, &plan))
        .join()
        .unwrap_or_else(|_| {
            serde_json::json!({
                "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
                "provider": provider_name,
                "plan_schema_version": plan_schema_version,
                "success": false,
                "failed_checks": ["remote_write_thread_panic"],
                "operations": []
            })
        })
}

fn execute_github_remote_write_plan_blocking(
    provider: &ConnectorProviderSpec,
    plan: &serde_json::Value,
) -> serde_json::Value {
    let Some(token) = connector_provider_auth_token(provider) else {
        return serde_json::json!({
            "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
            "provider": provider.name.as_str(),
            "plan_schema_version": plan.pointer("/schema_version").and_then(|value| value.as_str()),
            "success": false,
            "failed_checks": ["remote_auth_missing"],
            "operations": []
        });
    };
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Entrance-Hive/2.0")
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return serde_json::json!({
                "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
                "provider": provider.name.as_str(),
                "plan_schema_version": plan.pointer("/schema_version").and_then(|value| value.as_str()),
                "success": false,
                "failed_checks": ["http_client_unavailable"],
                "error": error.to_string(),
                "operations": []
            });
        }
    };
    let mut operation_results = Vec::new();
    let mut failed_checks = Vec::new();
    for operation in plan
        .pointer("/operations")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        let blockers = operation
            .pointer("/blocked_by")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !blockers.is_empty() {
            failed_checks.extend(blockers.clone());
            operation_results.push(serde_json::json!({
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": operation.pointer("/method").and_then(|value| value.as_str()),
                "url": operation.pointer("/url").and_then(|value| value.as_str()),
                "success": false,
                "skipped": true,
                "failed_checks": blockers
            }));
            continue;
        }
        let result = execute_github_remote_write_operation(&client, &token, operation);
        if result.pointer("/success").and_then(|value| value.as_bool()) != Some(true) {
            failed_checks.push(
                result
                    .pointer("/failed_check")
                    .and_then(|value| value.as_str())
                    .unwrap_or("remote_operation_failed")
                    .to_string(),
            );
        }
        operation_results.push(result);
    }
    failed_checks.sort();
    failed_checks.dedup();
    let success = failed_checks.is_empty();
    serde_json::json!({
        "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
        "provider": provider.name.as_str(),
        "plan_schema_version": plan.pointer("/schema_version").and_then(|value| value.as_str()),
        "remote_object_kind": plan.pointer("/remote_object_kind").and_then(|value| value.as_str()),
        "success": success,
        "failed_checks": failed_checks,
        "operation_count": operation_results.len(),
        "operations": operation_results
    })
}

fn execute_github_remote_write_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    operation: &serde_json::Value,
) -> serde_json::Value {
    if operation.pointer("/kind").and_then(|value| value.as_str()) == Some("upsert_latest_comment")
    {
        return execute_github_upsert_latest_comment_operation(client, token, operation);
    }
    let method_label = operation
        .pointer("/method")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let url = operation
        .pointer("/url")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let Ok(method) = Method::from_bytes(method_label.as_bytes()) else {
        return serde_json::json!({
            "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
            "method": method_label,
            "url": url,
            "success": false,
            "failed_check": "http_method_invalid"
        });
    };
    if url.trim().is_empty() {
        return serde_json::json!({
            "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
            "method": method_label,
            "url": url,
            "success": false,
            "failed_check": "remote_url_missing"
        });
    }

    let body = operation
        .pointer("/body")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let mut attempts = Vec::new();
    let retry_policy = github_retry_policy();
    let max_attempts = connector_retry_max_attempts(&retry_policy);
    for attempt_number in 1..=max_attempts {
        let mut result = execute_github_remote_write_attempt(
            client,
            token,
            operation,
            method.clone(),
            method_label,
            url,
            &body,
        );
        let should_retry =
            github_operation_result_should_retry_now(&result, attempt_number, &retry_policy);
        if should_retry {
            github_mark_retry_scheduled(
                &mut result,
                github_retry_backoff_ms(attempt_number, &retry_policy),
            );
        }
        attempts.push(github_operation_attempt_summary(&result, attempt_number));
        if !should_retry {
            return github_operation_with_attempts(result, attempts, &retry_policy);
        }
        std::thread::sleep(Duration::from_millis(github_retry_backoff_ms(
            attempt_number,
            &retry_policy,
        )));
    }

    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": method_label,
        "url": url,
        "success": false,
        "failed_check": "remote_retry_exhausted",
        "attempt_count": attempts.len(),
        "max_attempts": max_attempts,
        "attempts": attempts
    })
}

fn execute_github_remote_write_attempt(
    client: &reqwest::blocking::Client,
    token: &str,
    operation: &serde_json::Value,
    method: Method,
    method_label: &str,
    url: &str,
    body: &serde_json::Value,
) -> serde_json::Value {
    let response = client
        .request(method, url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2026-03-10")
        .json(&body)
        .send();
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return serde_json::json!({
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": method_label,
                "url": url,
                "success": false,
                "failed_check": "remote_request_failed",
                "retry": github_request_retry_metadata(),
                "error": error.to_string()
            });
        }
    };
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let text = match response.text() {
        Ok(text) => text,
        Err(error) => {
            return serde_json::json!({
                "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
                "method": method_label,
                "url": url,
                "success": false,
                "failed_check": "remote_response_read_failed",
                "http_status": status,
                "retry": github_response_read_retry_metadata(),
                "error": error.to_string()
            });
        }
    };
    let digest = digest_bytes(text.as_bytes());
    let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
    let success = (200..300).contains(&status);
    let failed_check = if success {
        None
    } else {
        Some(github_http_failed_check(status, &headers))
    };
    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": method_label,
        "url": url,
        "success": success,
        "failed_check": failed_check,
        "http_status": status,
        "retry": if success {
            github_terminal_retry_metadata("success")
        } else {
            github_http_retry_metadata(status, &headers)
        },
        "request": {
            "body_sha256": digest_bytes(serde_json::to_vec(&body).unwrap_or_default().as_slice()).sha256,
            "auth": "Authorization: Bearer <redacted>"
        },
        "response": {
            "headers": github_response_headers_summary(&headers),
            "bytes": digest.bytes,
            "sha256": digest.sha256,
            "summary": parsed.as_ref().map(compact_github_response_summary)
        }
    })
}

fn execute_github_upsert_latest_comment_operation(
    client: &reqwest::blocking::Client,
    token: &str,
    operation: &serde_json::Value,
) -> serde_json::Value {
    let lookup_url = operation
        .pointer("/lookup/url")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let idempotency_key = operation
        .pointer("/idempotency_key")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let lookup = execute_github_remote_get_operation(
        client,
        token,
        "lookup_issue_comments_for_idempotency",
        lookup_url,
    );
    if lookup.pointer("/success").and_then(|value| value.as_bool()) != Some(true) {
        return serde_json::json!({
            "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
            "method": operation.pointer("/method").and_then(|value| value.as_str()),
            "url": operation.pointer("/url").and_then(|value| value.as_str()),
            "success": false,
            "failed_check": "remote_comments_lookup_failed",
            "idempotency_key": idempotency_key,
            "lookup": lookup
        });
    }

    let matched_comment = github_comment_summary_find_idempotency_key(
        lookup.pointer("/response/summary"),
        idempotency_key,
    );
    let body = operation
        .pointer("/body")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let (mode, write_operation) = if let Some(comment) = matched_comment.as_ref() {
        let comment_id = comment
            .pointer("/id")
            .and_then(|value| value.as_i64())
            .map(|value| value.to_string());
        let update_url = comment
            .pointer("/url")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .or_else(|| {
                comment_id.as_ref().and_then(|id| {
                    operation
                        .pointer("/update/url_template")
                        .and_then(|value| value.as_str())
                        .map(|template| template.replace("{comment_id}", id))
                })
            })
            .unwrap_or_default();
        (
            "update_existing_comment",
            serde_json::json!({
                "kind": "update_latest_comment",
                "method": operation.pointer("/update/method").and_then(|value| value.as_str()).unwrap_or("PATCH"),
                "url": update_url,
                "body": body,
                "source": operation.pointer("/source").and_then(|value| value.as_str())
            }),
        )
    } else {
        (
            "append_new_comment",
            serde_json::json!({
                "kind": "append_latest_comment",
                "method": "POST",
                "url": operation.pointer("/url").and_then(|value| value.as_str()).unwrap_or_default(),
                "body": body,
                "source": operation.pointer("/source").and_then(|value| value.as_str())
            }),
        )
    };
    let write = execute_github_remote_write_operation(client, token, &write_operation);
    let success = write.pointer("/success").and_then(|value| value.as_bool()) == Some(true);
    serde_json::json!({
        "kind": operation.pointer("/kind").and_then(|value| value.as_str()),
        "method": write.pointer("/method").and_then(|value| value.as_str()),
        "url": write.pointer("/url").and_then(|value| value.as_str()),
        "success": success,
        "failed_check": if success { None::<&str> } else { Some("remote_comment_upsert_failed") },
        "mode": mode,
        "idempotency_key": idempotency_key,
        "matched_comment": matched_comment.map(|comment| serde_json::json!({
            "id": comment.pointer("/id").and_then(|value| value.as_i64()),
            "url": comment.pointer("/url").and_then(|value| value.as_str())
        })),
        "lookup": lookup,
        "write": write
    })
}

fn github_comment_summary_find_idempotency_key(
    comments_summary: Option<&serde_json::Value>,
    idempotency_key: &str,
) -> Option<serde_json::Value> {
    if idempotency_key.trim().is_empty() {
        return None;
    }
    comments_summary
        .and_then(|value| value.pointer("/items"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .find(|comment| {
            comment
                .pointer("/body")
                .and_then(|value| value.as_str())
                .map(|body| github_comment_body_has_idempotency_key(body, idempotency_key))
                == Some(true)
        })
        .cloned()
}

fn github_comment_body_has_idempotency_key(body: &str, idempotency_key: &str) -> bool {
    body.contains(&format!(
        "{}{}",
        GITHUB_COMMENT_IDEMPOTENCY_PREFIX, idempotency_key
    ))
}

fn compact_github_response_summary(response: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": response.pointer("/id").and_then(|value| value.as_i64()),
        "node_id": response.pointer("/node_id").and_then(|value| value.as_str()),
        "number": response.pointer("/number").and_then(|value| value.as_i64()),
        "url": response.pointer("/url").and_then(|value| value.as_str()),
        "html_url": response.pointer("/html_url").and_then(|value| value.as_str()),
        "issue_url": response.pointer("/issue_url").and_then(|value| value.as_str()),
        "state": response.pointer("/state").and_then(|value| value.as_str()),
        "state_reason": response.pointer("/state_reason").and_then(|value| value.as_str()),
        "comments": response.pointer("/comments").and_then(|value| value.as_u64()),
        "created_at": response.pointer("/created_at").and_then(|value| value.as_str()),
        "updated_at": response.pointer("/updated_at").and_then(|value| value.as_str())
    })
}

fn connector_provider_auth_token(provider: &ConnectorProviderSpec) -> Option<String> {
    provider.auth_env.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn connector_remote_write_receipt_from_execution(
    mirror: &IssueMirrorReport,
    path: &Path,
    digest: &MirrorFileDigest,
    provider: &ConnectorProviderSpec,
    remote_target: &serde_json::Value,
    execution: &serde_json::Value,
) -> serde_json::Value {
    let default_receipt = connector_remote_write_receipt(mirror, path, digest, Some(provider));
    let primary_summary = connector_remote_write_primary_summary(execution);
    let remote_url = primary_summary
        .and_then(|value| value.pointer("/html_url"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            primary_summary
                .and_then(|value| value.pointer("/url"))
                .and_then(|value| value.as_str())
        })
        .or_else(|| {
            remote_target
                .pointer("/remote_url")
                .and_then(|value| value.as_str())
        })
        .unwrap_or_default();
    let remote_id = primary_summary
        .and_then(|value| value.pointer("/number"))
        .and_then(|value| value.as_i64())
        .map(|number| {
            remote_target
                .pointer("/owner")
                .and_then(|value| value.as_str())
                .zip(
                    remote_target
                        .pointer("/repo")
                        .and_then(|value| value.as_str()),
                )
                .map(|(owner, repo)| format!("{owner}/{repo}#{number}"))
                .unwrap_or_else(|| number.to_string())
        })
        .or_else(|| {
            primary_summary
                .and_then(|value| value.pointer("/identifier"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| connector_remote_id(mirror, provider, remote_target));
    let mut receipt = default_receipt;
    if let Some(object) = receipt.as_object_mut() {
        object.insert(
            "remote_id".to_string(),
            serde_json::Value::String(remote_id),
        );
        if !remote_url.is_empty() {
            object.insert(
                "remote_url".to_string(),
                serde_json::Value::String(remote_url.to_string()),
            );
        }
        object.insert(
            "write_execution".to_string(),
            serde_json::json!({
                "schema_version": execution.pointer("/schema_version").and_then(|value| value.as_str()),
                "success": execution.pointer("/success").and_then(|value| value.as_bool()),
                "operation_count": execution.pointer("/operation_count").and_then(|value| value.as_u64()),
                "failed_checks": execution.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
                "operations": execution.pointer("/operations").cloned().unwrap_or_else(|| serde_json::json!([]))
            }),
        );
    }
    receipt
}

fn connector_remote_write_primary_summary(
    execution: &serde_json::Value,
) -> Option<&serde_json::Value> {
    execution
        .pointer("/operations")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter(|operation| {
            !matches!(
                operation.pointer("/kind").and_then(|value| value.as_str()),
                Some("upsert_latest_comment" | "append_latest_comment" | "update_latest_comment")
            )
        })
        .find_map(|operation| operation.pointer("/response/summary"))
}

fn connector_remote_target_examples(provider_name: &str) -> Vec<&'static str> {
    match provider_name {
        "github" => vec![
            "github:owner/repo",
            "github:owner/repo#123",
            "github:https://github.com/owner/repo/issues/123",
        ],
        "linear" => vec![
            "linear:TEAM-123",
            "linear:https://linear.app/workspace/issue/TEAM-123/title",
        ],
        "remote-fixture" => vec!["remote-fixture:ENT-1"],
        _ => vec!["provider:external-id"],
    }
}

fn connector_remote_target_required_fields(provider_name: &str) -> Vec<&'static str> {
    match provider_name {
        "github" => vec!["owner", "repo"],
        "linear" => vec!["issue_key"],
        "remote-fixture" => vec!["fixture_key"],
        _ => vec!["target"],
    }
}

fn connector_linear_issue_key_candidate(target: &str) -> String {
    let trimmed = target.trim();
    if let Some(rest) = trimmed
        .strip_prefix("https://linear.app/")
        .or_else(|| trimmed.strip_prefix("http://linear.app/"))
    {
        let parts = rest.split('/').collect::<Vec<_>>();
        if let Some(index) = parts.iter().position(|part| *part == "issue") {
            if let Some(key) = parts.get(index + 1) {
                return key.trim().to_ascii_uppercase();
            }
        }
    }
    trimmed
        .split('/')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase()
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    let value = value.trim().trim_start_matches('#');
    let parsed = value.parse::<u64>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn connector_target_slug_valid(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn empty_str_none(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn empty_string_none(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn connector_remote_object_kind(provider_name: &str) -> &'static str {
    match provider_name {
        "linear" => "linear.issue",
        "github" => "github.issue",
        "remote-fixture" => "fixture.issue",
        _ => "remote.issue",
    }
}

fn connector_provider_uses_remote_contract(provider: &ConnectorProviderSpec) -> bool {
    matches!(
        provider.mode.as_str(),
        "remote-issue-api" | "remote-issue-api-fixture"
    )
}

fn connector_provider_is_remote_fixture(provider: &ConnectorProviderSpec) -> bool {
    provider.name == "remote-fixture" || provider.mode == "remote-issue-api-fixture"
}

fn connector_provider_is_github_remote(provider: &ConnectorProviderSpec) -> bool {
    provider.name == "github" && provider.mode == "remote-issue-api"
}

fn connector_provider_is_linear_remote(provider: &ConnectorProviderSpec) -> bool {
    provider.name == "linear" && provider.mode == "remote-issue-api"
}

fn connector_github_api_base_url(provider: &ConnectorProviderSpec) -> String {
    let storage = provider.storage.trim().trim_end_matches('/');
    if storage.starts_with("https://") || storage.starts_with("http://") {
        storage.to_string()
    } else {
        "https://api.github.com".to_string()
    }
}

fn connector_linear_api_url(provider: &ConnectorProviderSpec) -> String {
    let storage = provider.storage.trim().trim_end_matches('/');
    if storage.starts_with("https://") || storage.starts_with("http://") {
        storage.to_string()
    } else {
        "https://api.linear.app/graphql".to_string()
    }
}

fn connector_writer_target_label(provider: Option<&ConnectorProviderSpec>) -> &'static str {
    if !connector_writer_blockers(provider).is_empty() {
        return "blocked provider adapter";
    }
    if provider
        .map(connector_provider_is_remote_fixture)
        .unwrap_or(false)
    {
        "file-backed remote issue/status/comment fixture"
    } else if provider
        .map(connector_provider_uses_remote_contract)
        .unwrap_or(false)
    {
        "remote issue/status/comment surface"
    } else {
        "local connector mirror file"
    }
}

fn compact_connector_queue(
    registry: &ConnectorRegistryReport,
    issues: &[serde_json::Value],
    provider_filter: Option<&str>,
) -> serde_json::Value {
    let provider_filter = normalized_provider_filter(provider_filter);
    let provider_known = provider_filter.as_ref().map_or(true, |filter| {
        registry
            .providers
            .iter()
            .any(|provider| provider.name == *filter)
    });
    let filtered_issues = issues
        .iter()
        .filter(|issue| {
            let provider = connector_provider_name_for_issue(registry, issue);
            provider_filter
                .as_ref()
                .map_or(true, |filter| provider == *filter)
        })
        .collect::<Vec<_>>();
    let current_count = filtered_issues
        .iter()
        .filter(|issue| {
            issue
                .pointer("/connector/current")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .count();
    let publish_required = filtered_issues
        .iter()
        .filter(|issue| {
            issue
                .pointer("/connector/publish_required")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .map(|issue| compact_connector_queue_issue(registry, issue))
        .collect::<Vec<_>>();
    let providers = registry
        .providers
        .iter()
        .filter(|provider| {
            provider_filter
                .as_ref()
                .map_or(true, |filter| provider.name == *filter)
        })
        .map(|provider| compact_connector_queue_provider(registry, provider, issues))
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": "entrance.hive.connector_queue.v1",
        "provider_filter": provider_filter,
        "provider_known": provider_known,
        "total": filtered_issues.len(),
        "current_count": current_count,
        "publish_required_count": publish_required.len(),
        "providers": providers,
        "issues": publish_required,
        "commands": {
            "refresh": "entrance hive connector queue --compact",
            "provider": "entrance hive connector queue --provider <name> --compact",
            "publish_plan": connector_publish_plan_command(provider_filter.as_deref()),
            "roundtrip_plan": connector_roundtrip_plan_command(provider_filter.as_deref())
        }
    })
}

fn compact_connector_publish_plan(queue: &serde_json::Value) -> Result<serde_json::Value> {
    let provider_filter = queue
        .pointer("/provider_filter")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let provider_known = queue
        .pointer("/provider_known")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let issues = queue
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(compact_connector_publish_plan_issue)
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    if !provider_known {
        blockers.push("provider_unknown".to_string());
    }
    for issue in &issues {
        if issue
            .pointer("/id")
            .and_then(|value| value.as_i64())
            .is_none()
        {
            blockers.push("issue_id_missing".to_string());
        }
        if issue
            .pointer("/commands/publish")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .is_none()
        {
            blockers.push("publish_command_missing".to_string());
        }
        if issue
            .pointer("/current_sha256")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .is_none()
        {
            blockers.push("current_digest_missing".to_string());
        }
        blockers.extend(
            issue
                .pointer("/publish_blockers")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned),
        );
    }
    blockers.sort();
    blockers.dedup();
    let basis = serde_json::json!({
        "schema_version": CONNECTOR_PUBLISH_PLAN_SCHEMA_VERSION,
        "provider_filter": provider_filter,
        "issues": issues.clone()
    });
    let plan_id = json_sha256(&basis)?;
    let can_execute = blockers.is_empty() && !issues.is_empty();
    let reason = if !provider_known {
        "provider_unknown"
    } else if issues.is_empty() {
        "queue_empty"
    } else if blockers.is_empty() {
        "ready"
    } else {
        "plan_blocked"
    };
    Ok(serde_json::json!({
        "schema_version": CONNECTOR_PUBLISH_PLAN_SCHEMA_VERSION,
        "plan_id": plan_id,
        "provider_filter": basis.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
        "provider_known": provider_known,
        "issue_count": issues.len(),
        "can_execute": can_execute,
        "reason": reason,
        "blockers": blockers,
        "issues": issues,
        "basis": basis,
        "commands": {
            "refresh": "entrance hive connector queue --compact",
            "plan": connector_publish_plan_command(
                queue.pointer("/provider_filter").and_then(|value| value.as_str())
            ),
            "execute": if can_execute {
                Some(format!(
                    "{} --plan-id {} --compact",
                    connector_publish_execute_command_prefix(
                        queue.pointer("/provider_filter").and_then(|value| value.as_str())
                    ),
                    plan_id
                ))
            } else {
                None
            }
        }
    }))
}

fn compact_connector_roundtrip_plan(queue: &serde_json::Value) -> Result<serde_json::Value> {
    let provider_filter = queue
        .pointer("/provider_filter")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let provider_known = queue
        .pointer("/provider_known")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let issues = queue
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(compact_connector_roundtrip_plan_issue)
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    if !provider_known {
        blockers.push("provider_unknown".to_string());
    }
    for issue in &issues {
        if issue
            .pointer("/id")
            .and_then(|value| value.as_i64())
            .is_none()
        {
            blockers.push("issue_id_missing".to_string());
        }
        if issue
            .pointer("/commands/roundtrip")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .is_none()
        {
            blockers.push("roundtrip_command_missing".to_string());
        }
        if issue
            .pointer("/current_sha256")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .is_none()
        {
            blockers.push("current_digest_missing".to_string());
        }
        if issue
            .pointer("/supports_readback")
            .and_then(|value| value.as_bool())
            != Some(true)
        {
            blockers.push("readback_not_supported".to_string());
        }
        if issue
            .pointer("/supports_admission")
            .and_then(|value| value.as_bool())
            != Some(true)
        {
            blockers.push("admission_not_supported".to_string());
        }
        blockers.extend(
            issue
                .pointer("/publish_blockers")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned),
        );
    }
    blockers.sort();
    blockers.dedup();
    let basis = serde_json::json!({
        "schema_version": CONNECTOR_ROUNDTRIP_PLAN_SCHEMA_VERSION,
        "provider_filter": provider_filter,
        "issues": issues.clone()
    });
    let plan_id = json_sha256(&basis)?;
    let can_execute = blockers.is_empty() && !issues.is_empty();
    let reason = if !provider_known {
        "provider_unknown"
    } else if issues.is_empty() {
        "queue_empty"
    } else if blockers.is_empty() {
        "ready"
    } else {
        "plan_blocked"
    };
    Ok(serde_json::json!({
        "schema_version": CONNECTOR_ROUNDTRIP_PLAN_SCHEMA_VERSION,
        "plan_id": plan_id,
        "provider_filter": basis.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
        "provider_known": provider_known,
        "issue_count": issues.len(),
        "can_execute": can_execute,
        "reason": reason,
        "blockers": blockers,
        "issues": issues,
        "basis": basis,
        "commands": {
            "refresh": "entrance hive connector queue --compact",
            "plan": connector_roundtrip_plan_command(
                queue.pointer("/provider_filter").and_then(|value| value.as_str())
            ),
            "execute": if can_execute {
                Some(format!(
                    "{} --plan-id {} --compact",
                    connector_roundtrip_execute_command_prefix(
                        queue.pointer("/provider_filter").and_then(|value| value.as_str())
                    ),
                    plan_id
                ))
            } else {
                None
            }
        }
    }))
}

fn compact_connector_roundtrip_plan_summary(plan: &serde_json::Value) -> serde_json::Value {
    let issues = plan
        .pointer("/issues")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|issue| {
            let current_sha256 = issue
                .pointer("/current_sha256")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            serde_json::json!({
                "id": issue.pointer("/id").and_then(|value| value.as_i64()),
                "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
                "provider": issue.pointer("/provider").and_then(|value| value.as_str()),
                "provider_status": issue.pointer("/provider_status").and_then(|value| value.as_str()),
                "status": issue.pointer("/status").and_then(|value| value.as_str()),
                "review_surface": issue.pointer("/review_surface").and_then(|value| value.as_str()),
                "reason": issue.pointer("/reason").and_then(|value| value.as_str()),
                "current_sha256": current_sha256,
                "current_digest": compact_digest_label(current_sha256),
                "remote_target": issue.pointer("/remote_target").cloned().unwrap_or(serde_json::Value::Null),
                "remote_write_plan": issue.pointer("/remote_write_plan").cloned().unwrap_or(serde_json::Value::Null),
                "commands": {
                    "roundtrip": issue.pointer("/commands/roundtrip").and_then(|value| value.as_str())
                }
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": "entrance.hive.connector_roundtrip_plan.compact.v1",
        "plan_id": plan.pointer("/plan_id").and_then(|value| value.as_str()),
        "provider_filter": plan.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
        "provider_known": plan.pointer("/provider_known").and_then(|value| value.as_bool()),
        "issue_count": plan.pointer("/issue_count").and_then(|value| value.as_u64()),
        "can_execute": plan.pointer("/can_execute").and_then(|value| value.as_bool()),
        "reason": plan.pointer("/reason").and_then(|value| value.as_str()),
        "blockers": plan.pointer("/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "issues": issues,
        "commands": plan.pointer("/commands").cloned().unwrap_or_else(|| serde_json::json!({}))
    })
}

fn compact_connector_roundtrip_execute_summary(report: &serde_json::Value) -> serde_json::Value {
    let recorded = report
        .pointer("/recorded")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|record| {
            serde_json::json!({
                "issue_id": record.pointer("/issue_id").and_then(|value| value.as_i64()),
                "comment_id": record.pointer("/comment_id").and_then(|value| value.as_i64()),
                "evidence_id": record.pointer("/evidence_id").and_then(|value| value.as_i64())
            })
        })
        .collect::<Vec<_>>();
    let roundtrips = report
        .pointer("/roundtrips")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|roundtrip| {
            serde_json::json!({
                "issue_id": roundtrip.pointer("/issue_id").and_then(|value| value.as_i64()),
                "provider": roundtrip.pointer("/provider").and_then(|value| value.as_str()),
                "review_surface": roundtrip.pointer("/review_surface").and_then(|value| value.as_str()),
                "completed": roundtrip.pointer("/completed").and_then(|value| value.as_bool()),
                "stage_count": roundtrip.pointer("/stage_count").and_then(|value| value.as_u64()),
                "passed_stage_count": roundtrip.pointer("/passed_stage_count").and_then(|value| value.as_u64()),
                "recorded_evidence_ids": roundtrip
                    .pointer("/recorded_evidence_ids")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([])),
                "remote": roundtrip.pointer("/remote").cloned().unwrap_or(serde_json::Value::Null),
                "failed_stages": roundtrip
                    .pointer("/failed_stages")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([]))
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": "entrance.hive.connector_roundtrip_execute.compact.v1",
        "executed": report.pointer("/executed").and_then(|value| value.as_bool()),
        "reason": report.pointer("/reason").and_then(|value| value.as_str()),
        "plan_id": report.pointer("/plan_id").and_then(|value| value.as_str()),
        "current_plan_id": report.pointer("/current_plan_id").and_then(|value| value.as_str()),
        "provider_filter": report.pointer("/provider_filter").cloned().unwrap_or(serde_json::Value::Null),
        "issue_count": report.pointer("/issue_count").and_then(|value| value.as_u64()),
        "completed_count": report.pointer("/completed_count").and_then(|value| value.as_u64()),
        "issue_ids": report.pointer("/issue_ids").cloned().unwrap_or_else(|| serde_json::json!([])),
        "failed_checks": report.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "recorded": recorded,
        "roundtrips": roundtrips,
        "after": {
            "publish_required_count": report.pointer("/after/publish_required_count").and_then(|value| value.as_u64()),
            "current_count": report.pointer("/after/current_count").and_then(|value| value.as_u64()),
            "total": report.pointer("/after/queue/total").and_then(|value| value.as_u64())
        },
        "commands": report.pointer("/commands").cloned().unwrap_or_else(|| serde_json::json!({}))
    })
}

fn compact_connector_roundtrip_plan_issue(issue: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": issue.pointer("/id").and_then(|value| value.as_i64()),
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        "provider": issue.pointer("/provider").and_then(|value| value.as_str()),
        "provider_status": issue.pointer("/provider_status").and_then(|value| value.as_str()),
        "configured": issue.pointer("/configured").and_then(|value| value.as_bool()),
        "supports_publish": issue.pointer("/supports_publish").and_then(|value| value.as_bool()),
        "supports_readback": issue.pointer("/supports_readback").and_then(|value| value.as_bool()),
        "supports_admission": issue.pointer("/supports_admission").and_then(|value| value.as_bool()),
        "mode": issue.pointer("/mode").and_then(|value| value.as_str()),
        "storage": issue.pointer("/storage").and_then(|value| value.as_str()),
        "review_surface": issue.pointer("/review_surface").and_then(|value| value.as_str()),
        "status": issue.pointer("/status").and_then(|value| value.as_str()),
        "reason": issue.pointer("/reason").and_then(|value| value.as_str()),
        "path": issue.pointer("/path").and_then(|value| value.as_str()),
        "current_sha256": issue.pointer("/current_sha256").and_then(|value| value.as_str()),
        "remote_sha256": issue.pointer("/remote_sha256").and_then(|value| value.as_str()),
        "current_comment_count": issue.pointer("/current_comment_count").and_then(|value| value.as_u64()),
        "remote_comment_count": issue.pointer("/remote_comment_count").and_then(|value| value.as_u64()),
        "failed_checks": issue.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "admission_status": issue.pointer("/admission_status").and_then(|value| value.as_str()),
        "admission_blockers": issue.pointer("/admission_blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "can_publish": issue.pointer("/can_publish").and_then(|value| value.as_bool()).unwrap_or(false),
        "publish_blockers": issue.pointer("/publish_blockers").cloned().unwrap_or_else(|| serde_json::json!(["connector_writer_unknown"])),
        "adapter": issue.pointer("/adapter").cloned().unwrap_or_else(|| serde_json::json!({})),
        "remote_target": issue.pointer("/remote_target").cloned().unwrap_or(serde_json::Value::Null),
        "remote_write_plan": issue.pointer("/remote_write_plan").cloned().unwrap_or(serde_json::Value::Null),
        "commands": {
            "publish": issue.pointer("/commands/publish").and_then(|value| value.as_str()),
            "readback": issue.pointer("/commands/readback").and_then(|value| value.as_str()),
            "admit": issue.pointer("/commands/admit").and_then(|value| value.as_str()),
            "roundtrip": issue.pointer("/commands/roundtrip").and_then(|value| value.as_str())
        },
        "dry_run_action": {
            "schema_version": "entrance.hive.connector_roundtrip_plan_item.v1",
            "action": "roundtrip",
            "remote_write": issue.pointer("/dry_run_action/remote_write").and_then(|value| value.as_bool()).unwrap_or(false),
            "would_write": issue
                .pointer("/dry_run_action/would_write")
                .and_then(|value| value.as_str())
                .unwrap_or("blocked provider adapter")
        }
    })
}

fn compact_connector_publish_plan_issue(issue: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": issue.pointer("/id").and_then(|value| value.as_i64()),
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        "provider": issue.pointer("/provider").and_then(|value| value.as_str()),
        "provider_status": issue.pointer("/provider_status").and_then(|value| value.as_str()),
        "configured": issue.pointer("/configured").and_then(|value| value.as_bool()),
        "supports_publish": issue.pointer("/supports_publish").and_then(|value| value.as_bool()),
        "supports_readback": issue.pointer("/supports_readback").and_then(|value| value.as_bool()),
        "mode": issue.pointer("/mode").and_then(|value| value.as_str()),
        "storage": issue.pointer("/storage").and_then(|value| value.as_str()),
        "review_surface": issue.pointer("/review_surface").and_then(|value| value.as_str()),
        "status": issue.pointer("/status").and_then(|value| value.as_str()),
        "reason": issue.pointer("/reason").and_then(|value| value.as_str()),
        "path": issue.pointer("/path").and_then(|value| value.as_str()),
        "current_sha256": issue.pointer("/current_sha256").and_then(|value| value.as_str()),
        "remote_sha256": issue.pointer("/remote_sha256").and_then(|value| value.as_str()),
        "current_comment_count": issue.pointer("/current_comment_count").and_then(|value| value.as_u64()),
        "remote_comment_count": issue.pointer("/remote_comment_count").and_then(|value| value.as_u64()),
        "failed_checks": issue.pointer("/failed_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "admission_status": issue.pointer("/admission_status").and_then(|value| value.as_str()),
        "admission_blockers": issue.pointer("/admission_blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "can_publish": issue.pointer("/can_publish").and_then(|value| value.as_bool()).unwrap_or(false),
        "publish_blockers": issue.pointer("/publish_blockers").cloned().unwrap_or_else(|| serde_json::json!(["connector_writer_unknown"])),
        "adapter": issue.pointer("/adapter").cloned().unwrap_or_else(|| serde_json::json!({})),
        "remote_target": issue.pointer("/remote_target").cloned().unwrap_or(serde_json::Value::Null),
        "remote_write_plan": issue.pointer("/remote_write_plan").cloned().unwrap_or(serde_json::Value::Null),
        "commands": {
            "publish": issue.pointer("/commands/publish").and_then(|value| value.as_str()),
            "readback": issue.pointer("/commands/readback").and_then(|value| value.as_str()),
            "admit": issue.pointer("/commands/admit").and_then(|value| value.as_str()),
            "roundtrip": issue.pointer("/commands/roundtrip").and_then(|value| value.as_str())
        },
        "dry_run_action": {
            "schema_version": "entrance.hive.connector_publish_plan_item.v1",
            "action": "publish",
            "remote_write": issue.pointer("/dry_run_action/remote_write").and_then(|value| value.as_bool()).unwrap_or(false),
            "would_write": issue
                .pointer("/dry_run_action/would_write")
                .and_then(|value| value.as_str())
                .unwrap_or("blocked provider adapter")
        }
    })
}

fn connector_publish_plan_command(provider_filter: Option<&str>) -> String {
    format!(
        "{} --compact",
        connector_command_with_provider("entrance hive connector publish-plan", provider_filter)
    )
}

fn connector_publish_execute_command_prefix(provider_filter: Option<&str>) -> String {
    connector_command_with_provider("entrance hive connector publish-execute", provider_filter)
}

fn connector_roundtrip_plan_command(provider_filter: Option<&str>) -> String {
    format!(
        "{} --compact",
        connector_command_with_provider("entrance hive connector roundtrip-plan", provider_filter)
    )
}

fn connector_roundtrip_execute_command_prefix(provider_filter: Option<&str>) -> String {
    connector_command_with_provider("entrance hive connector roundtrip-execute", provider_filter)
}

fn connector_command_with_provider(base: &str, provider_filter: Option<&str>) -> String {
    match normalized_provider_filter(provider_filter) {
        Some(provider) => format!("{base} --provider {provider}"),
        None => base.to_string(),
    }
}

fn json_sha256(value: &serde_json::Value) -> Result<String> {
    let payload = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(payload);
    Ok(encode_hex(&hasher.finalize()))
}

fn compact_connector_queue_provider(
    registry: &ConnectorRegistryReport,
    provider: &ConnectorProviderSpec,
    issues: &[serde_json::Value],
) -> serde_json::Value {
    let admission = registry
        .provider_admissions
        .iter()
        .find(|admission| admission.provider == provider.name);
    let provider_issues = issues
        .iter()
        .filter(|issue| connector_provider_name_for_issue(registry, issue) == provider.name)
        .collect::<Vec<_>>();
    let publish_required_count = provider_issues
        .iter()
        .filter(|issue| {
            issue
                .pointer("/connector/publish_required")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .count();
    let current_count = provider_issues
        .iter()
        .filter(|issue| {
            issue
                .pointer("/connector/current")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .count();
    serde_json::json!({
        "name": provider.name.as_str(),
        "display_name": provider.display_name.as_str(),
        "status": provider.status.as_str(),
        "configured": provider.configured,
        "supports_publish": provider.supports_publish,
        "supports_admission": provider.supports_admission,
        "admission_status": admission.map(|admission| admission.status.as_str()),
        "admission_blockers": admission
            .map(|admission| admission.blockers.iter().map(String::as_str).collect::<Vec<_>>())
            .unwrap_or_default(),
        "mode": provider.mode.as_str(),
        "storage": provider.storage.as_str(),
        "adapter": compact_connector_writer_adapter(&provider.name, Some(provider)),
        "issue_count": provider_issues.len(),
        "current_count": current_count,
        "publish_required_count": publish_required_count,
        "queue_command": format!("entrance hive connector queue --provider {} --compact", provider.name),
        "dry_run_action": {
            "schema_version": "entrance.hive.connector_publish_dry_run.v1",
            "action": "publish",
            "provider": provider.name.as_str(),
            "provider_status": provider.status.as_str(),
            "provider_configured": provider.configured,
            "supports_publish": provider.supports_publish,
            "adapter": compact_connector_writer_adapter(&provider.name, Some(provider)),
            "would_write": connector_writer_target_label(Some(provider)),
            "remote_write": connector_provider_uses_remote_contract(provider) && provider.supports_publish
        }
    })
}

fn compact_connector_queue_issue(
    registry: &ConnectorRegistryReport,
    issue: &serde_json::Value,
) -> serde_json::Value {
    let provider_name = connector_provider_name_for_issue(registry, issue);
    let provider = registry
        .providers
        .iter()
        .find(|provider| provider.name == provider_name);
    let admission = registry
        .provider_admissions
        .iter()
        .find(|admission| admission.provider == provider_name);
    let issue_id = issue.pointer("/id").and_then(|value| value.as_i64());
    let failed_checks = issue
        .pointer("/connector/failed_checks")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let failed_check_count = failed_checks.as_array().map(Vec::len).unwrap_or_default();
    let publish_command = issue
        .pointer("/connector/publish_command")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            format!(
                "entrance hive issue mirror-publish {} --compact",
                issue_id.unwrap_or_default()
            )
        });
    let review_surface = issue
        .pointer("/connector/review_surface")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let external_key = issue
        .pointer("/connector/external_key")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let publish_blockers = connector_issue_writer_blockers(provider, review_surface, external_key);
    let can_publish = publish_blockers.is_empty();
    let adapter = compact_connector_writer_adapter(&provider_name, provider);
    let remote_target = connector_remote_target(provider, review_surface, external_key);
    let remote_write_plan =
        connector_remote_write_plan(provider, issue, &remote_target, &publish_blockers);
    let connector_status = issue
        .pointer("/connector")
        .unwrap_or(&serde_json::Value::Null);
    let remote_contract = compact_connector_remote_contract(provider);
    let admission_checks = connector_admission_preview_checks(
        provider,
        admission,
        connector_status,
        &publish_blockers,
        !remote_contract.is_null(),
        &remote_target,
        &remote_contract,
        &registry.admission.check_registry,
    );
    let admission_blockers = admission
        .map(|admission| {
            admission
                .blockers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let commands = serde_json::json!({
        "publish": publish_command.as_str(),
        "readback": issue
            .pointer("/connector/readback_command")
            .and_then(|value| value.as_str()),
        "admit": issue
            .pointer("/connector/admit_command")
            .and_then(|value| value.as_str()),
        "roundtrip": issue
            .pointer("/connector/roundtrip_command")
            .and_then(|value| value.as_str())
    });
    let dry_run_action = serde_json::json!({
        "schema_version": "entrance.hive.connector_publish_dry_run.v1",
        "action": "publish",
        "provider": provider_name.as_str(),
        "provider_status": provider.map(|provider| provider.status.as_str()),
        "provider_configured": provider.map(|provider| provider.configured),
        "supports_publish": provider.map(|provider| provider.supports_publish),
        "adapter": adapter,
        "remote_target": remote_target,
        "remote_write_plan": remote_write_plan,
        "would_write": connector_writer_target_label(provider),
        "remote_write": provider
            .map(|provider| connector_provider_uses_remote_contract(provider) && provider.supports_publish)
            .unwrap_or(false),
        "command": publish_command
    });
    serde_json::json!({
        "id": issue_id,
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        "title": issue.pointer("/title").and_then(|value| value.as_str()),
        "status": issue.pointer("/status").and_then(|value| value.as_str()),
        "provider": provider_name.as_str(),
        "provider_status": provider.map(|provider| provider.status.as_str()),
        "configured": provider.map(|provider| provider.configured),
        "supports_publish": provider.map(|provider| provider.supports_publish),
        "supports_readback": provider.map(|provider| provider.supports_readback),
        "supports_admission": provider.map(|provider| provider.supports_admission),
        "mode": provider.map(|provider| provider.mode.as_str()),
        "storage": provider.map(|provider| provider.storage.as_str()),
        "can_publish": can_publish,
        "publish_blockers": publish_blockers,
        "adapter": adapter,
        "remote_target": remote_target,
        "remote_write_plan": remote_write_plan,
        "admission_status": admission.map(|admission| admission.status.as_str()),
        "admission_blockers": admission_blockers,
        "admission_checks": admission_checks,
        "review_surface": review_surface,
        "external_key": external_key,
        "publish_required": true,
        "current": issue.pointer("/connector/current").and_then(|value| value.as_bool()),
        "reason": issue.pointer("/connector/reason").and_then(|value| value.as_str()),
        "path": issue.pointer("/connector/path").and_then(|value| value.as_str()),
        "current_sha256": issue.pointer("/connector/current_sha256").and_then(|value| value.as_str()),
        "remote_sha256": issue.pointer("/connector/remote_sha256").and_then(|value| value.as_str()),
        "current_comment_count": issue.pointer("/connector/current_comment_count").and_then(|value| value.as_u64()),
        "remote_comment_count": issue.pointer("/connector/remote_comment_count").and_then(|value| value.as_u64()),
        "failed_checks": failed_checks,
        "failed_check_count": failed_check_count,
        "checks": issue.pointer("/connector/checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "remote_readback_checks": issue.pointer("/connector/remote_readback_checks").cloned().unwrap_or_else(|| serde_json::json!([])),
        "remote_diagnostics": issue.pointer("/connector/remote_diagnostics").cloned().unwrap_or_else(|| serde_json::json!(null)),
        "commands": commands,
        "dry_run_action": dry_run_action
    })
}

fn connector_provider_name_for_issue(
    registry: &ConnectorRegistryReport,
    issue: &serde_json::Value,
) -> String {
    if let Some(provider) = issue
        .pointer("/connector/provider")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return provider.trim().to_string();
    }
    if let Some(review_surface) = issue
        .pointer("/connector/review_surface")
        .and_then(|value| value.as_str())
    {
        if let Some(provider) = registry
            .providers
            .iter()
            .find(|provider| connector_provider_matches_surface(provider, review_surface))
        {
            return provider.name.clone();
        }
    }
    "unknown".to_string()
}

fn normalized_provider_filter(provider_filter: Option<&str>) -> Option<String> {
    provider_filter
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(ToOwned::to_owned)
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
            "rounds": trace.rounds.iter().map(|round| serde_json::json!({
                "round": round.round,
                "status": round.status,
                "decision": round.decision,
                "evidence_count": round.evidence_count,
                "rejected_count": round.rejected_count,
                "receipt_required": round.receipt_required_count,
                "receipt_missing": round.receipt_missing_count,
                "worker_ok": round.worker_ok_count,
                "workers": round.worker_count,
                "timeouts": round.worker_timeout_count,
                "retry_exhausted": round.worker_retry_exhausted_count
            })).collect::<Vec<_>>(),
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
                "schema_version": row.schema_version,
                "admission": row.admission_result,
                "operator_author": row.operator_author,
                "operator_action": row.operator_action,
                "worker": row.worker_kind.as_ref().map(|kind| serde_json::json!({
                    "kind": kind,
                    "command": row.worker_command.as_ref().map(|command| compact_text(command, 220)),
                    "cwd": row.worker_cwd.as_ref().map(|cwd| compact_text(cwd, 180)),
                    "ok": row.worker_ok,
                    "receipt_ok": row.worker_receipt_ok,
                    "duration_ms": row.worker_duration_ms,
                    "timeout_secs": row.worker_timeout_secs,
                    "attempt_count": row.worker_attempt_count,
                    "max_attempts": row.worker_max_attempts,
                    "timed_out": row.worker_timed_out,
                    "retry_exhausted": row.worker_retry_exhausted,
                    "receipt_errors": row.worker_receipt_errors,
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
    use std::{
        io::{Read, Write},
        net::TcpListener,
        path::Path,
        sync::{Arc, Mutex},
        thread,
    };

    use super::{
        compact_connector_publish_plan, compact_connector_queue, compact_connector_registry,
        compact_connector_remote_contract, compact_connector_roundtrip_plan,
        compact_github_issue_mirror_readback, compact_issue_board,
        compact_issue_connector_admission_preview, compact_issue_detail, compact_issue_mirror,
        compact_issue_mirror_admission, compact_issue_mirror_admission_summary,
        compact_issue_mirror_audit, compact_issue_mirror_audit_summary,
        compact_issue_mirror_publish, compact_issue_mirror_readback,
        compact_issue_mirror_readback_summary, compact_issue_mirror_roundtrip,
        compact_issue_mirror_roundtrip_summary, compact_issue_mirror_status,
        compact_issue_mirror_sync, compact_issue_mirror_verify,
        compact_linear_issue_mirror_readback, compact_loop_audit, compact_loop_start_summary,
        connector_admission_check_failed, connector_admission_preview_checks,
        connector_github_remote_comment_body, connector_issue_writer_blockers,
        connector_linear_remote_comment_body, connector_remote_issue_body,
        connector_remote_issue_idempotency_key, connector_remote_target,
        connector_remote_write_issue_from_mirror, connector_remote_write_plan,
        connector_remote_write_receipt_from_execution, connector_write_receipt,
        connector_writer_blockers, default_issue_mirror_path,
        default_issue_mirror_path_for_provider, digest_bytes, execute_github_remote_readback,
        execute_github_remote_write_plan, execute_linear_remote_write_plan, flag_present,
        flag_value, issue_mirror_roundtrip_stage, issue_mirror_sync_receipt,
        issue_mirror_sync_receipt_for_provider, loop_demo_request_from_flags, mirror_payload,
        mirror_receipt_path, MirrorFileDigest, CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
        CONNECTOR_REMOTE_WRITE_RECEIPT_SCHEMA_VERSION, ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
    };
    use entrance_core::{HiveComment, HiveIssue, HiveLoopContract};
    use entrance_hive::{
        ConnectorAdmissionCheckSpec, ConnectorAdmissionPolicySpec, ConnectorProviderAdmissionSpec,
        ConnectorProviderSpec, ConnectorRegistryReport, HiveLoopAuditCheck, HiveLoopAuditReport,
        HiveLoopDoctorCounts, IssueAction, IssueCard, IssueDoctorSummary, IssueMirrorReport,
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

    fn test_connector_registry() -> ConnectorRegistryReport {
        let providers = vec![
            test_connector_provider("file", "File Mirror", "active", true, true, vec!["file:"]),
            test_connector_provider("linear", "Linear", "planned", true, false, vec!["linear:"]),
        ];
        ConnectorRegistryReport {
            schema_version: "entrance.hive.connector_registry.v1".to_string(),
            provider_admissions: providers
                .iter()
                .map(|provider| test_provider_admission(provider))
                .collect(),
            providers,
            admission: ConnectorAdmissionPolicySpec {
                schema_version: "entrance.hive.policy_registry.v1".to_string(),
                gate: "connector_mirror_receipt_current".to_string(),
                route_to: "external_issue_surface".to_string(),
                expected_object_kind: "ISSUE_CONNECTOR_MIRROR".to_string(),
                check: "external_receipt_current".to_string(),
                required_receipts: vec!["mirror_file_current".to_string()],
                required_checks: test_connector_admission_required_checks(),
                check_registry: test_connector_admission_check_registry(),
                dry_run_command: "entrance hive issue connector-admission <id> --compact"
                    .to_string(),
            },
        }
    }

    fn test_connector_admission_required_checks() -> Vec<String> {
        [
            "provider_supported",
            "provider_admission_ready",
            "mirror_current",
            "readback_checks_passed",
            "remote_write_contract_ready",
            "remote_target_valid",
            "retry_policy_bound",
        ]
        .iter()
        .map(|check| (*check).to_string())
        .collect()
    }

    fn test_connector_admission_check_registry() -> Vec<ConnectorAdmissionCheckSpec> {
        test_connector_admission_required_checks()
            .into_iter()
            .map(|name| ConnectorAdmissionCheckSpec {
                owner: if name == "retry_policy_bound" {
                    "retry-policy"
                } else {
                    "test-owner"
                }
                .to_string(),
                severity: "blocker".to_string(),
                required_evidence: if name == "retry_policy_bound" {
                    vec![
                        "connector_remote_contract.retry".to_string(),
                        "connector_remote_diagnostics".to_string(),
                    ]
                } else {
                    vec![format!("{name}.evidence")]
                },
                summary: format!("{name} summary"),
                name,
            })
            .collect()
    }

    #[test]
    fn compact_connector_registry_exposes_admission_check_contract() {
        let registry = test_connector_registry();
        let compact = compact_connector_registry(&registry);

        assert_eq!(
            compact
                .pointer("/admission/required_checks/6")
                .and_then(|value| value.as_str()),
            Some("retry_policy_bound")
        );
        assert_eq!(
            compact
                .pointer("/provider_admissions/0/required_checks/6")
                .and_then(|value| value.as_str()),
            Some("retry_policy_bound")
        );
        assert_eq!(
            compact
                .pointer("/admission/check_registry/6/owner")
                .and_then(|value| value.as_str()),
            Some("retry-policy")
        );
        assert_eq!(
            compact
                .pointer("/provider_admissions/0/check_registry/6/required_evidence/0")
                .and_then(|value| value.as_str()),
            Some("connector_remote_contract.retry")
        );
    }

    fn test_provider_admission(provider: &ConnectorProviderSpec) -> ConnectorProviderAdmissionSpec {
        let mut blockers = Vec::new();
        if provider.status != "active" {
            blockers.push("provider_not_active".to_string());
        }
        if !provider.supports_admission {
            blockers.push("admission_not_supported".to_string());
        }
        ConnectorProviderAdmissionSpec {
            schema_version: "entrance.hive.policy_registry.v1".to_string(),
            provider: provider.name.clone(),
            status: if blockers.is_empty() {
                "ready"
            } else {
                "blocked"
            }
            .to_string(),
            gate: "connector_mirror_receipt_current".to_string(),
            route_to: blockers
                .is_empty()
                .then(|| "external_issue_surface".to_string()),
            expected_object_kind: "ISSUE_CONNECTOR_MIRROR".to_string(),
            check: "external_receipt_current".to_string(),
            required_receipts: vec!["mirror_file_current".to_string()],
            required_checks: test_connector_admission_required_checks(),
            check_registry: test_connector_admission_check_registry(),
            blockers,
            dry_run_command: "entrance hive issue connector-admission <id> --compact".to_string(),
        }
    }

    fn test_connector_provider(
        name: &str,
        display_name: &str,
        status: &str,
        configured: bool,
        supports_publish: bool,
        prefixes: Vec<&str>,
    ) -> ConnectorProviderSpec {
        ConnectorProviderSpec {
            name: name.to_string(),
            display_name: display_name.to_string(),
            status: status.to_string(),
            mode: match name {
                "linear" | "github" => "remote-issue-api",
                "remote-fixture" => "remote-issue-api-fixture",
                _ => "test",
            }
            .to_string(),
            review_surface_prefixes: prefixes.into_iter().map(ToOwned::to_owned).collect(),
            auth_required: false,
            auth_env: Vec::new(),
            configured,
            supports_status: supports_publish,
            supports_publish,
            supports_readback: supports_publish,
            supports_admission: supports_publish,
            storage: "test".to_string(),
            notes: "test provider".to_string(),
        }
    }

    fn spawn_github_readback_server(
        issue_body: String,
        comment_body: String,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server_base_url = base_url.clone();
        let handle = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("test request should connect");
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or_default();
                let request = String::from_utf8_lossy(&buffer[..read]);
                let body = if request
                    .starts_with("GET /repos/owner/repo/issues/77/comments?per_page=100&page=2 ")
                {
                    serde_json::json!([
                        {
                            "id": 9001,
                            "node_id": "IC_kwDO-test",
                            "url": "http://localhost/repos/owner/repo/issues/comments/9001",
                            "html_url": "http://localhost/owner/repo/issues/77#issuecomment-9001",
                            "body": comment_body.as_str(),
                            "created_at": "2026-01-01T00:02:00Z",
                            "updated_at": "2026-01-01T00:02:00Z"
                        }
                    ])
                    .to_string()
                } else if request
                    .starts_with("GET /repos/owner/repo/issues/77/comments?per_page=100 ")
                {
                    serde_json::json!([]).to_string()
                } else if request.starts_with("GET /repos/owner/repo/issues/77 ") {
                    serde_json::json!({
                        "id": 77,
                        "node_id": "I_kwDO-test",
                        "number": 77,
                        "url": "http://localhost/repos/owner/repo/issues/77",
                        "html_url": "http://localhost/owner/repo/issues/77",
                        "state": "closed",
                        "state_reason": "completed",
                        "title": "Loop #9: GitHub readback",
                        "body": issue_body.as_str(),
                        "comments": 1,
                        "created_at": "2026-01-01T00:00:00Z",
                        "updated_at": "2026-01-01T00:03:00Z"
                    })
                    .to_string()
                } else {
                    serde_json::json!({"message": "not found"}).to_string()
                };
                let status = if body.contains("not found") {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let link = if request
                    .starts_with("GET /repos/owner/repo/issues/77/comments?per_page=100 ")
                {
                    format!(
                        "Link: <{server_base_url}/repos/owner/repo/issues/77/comments?per_page=100&page=2>; rel=\"next\"\r\n"
                    )
                } else {
                    String::new()
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{link}Connection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("test response should write");
            }
        });
        (base_url, handle)
    }

    fn spawn_github_comment_upsert_server(
        existing_comment_body: String,
    ) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server_base_url = base_url.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().expect("test request should connect");
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or_default();
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                request_log
                    .lock()
                    .expect("request log should lock")
                    .push(request.clone());
                let body = if request.starts_with("PATCH /repos/owner/repo/issues/88 ") {
                    serde_json::json!({
                        "id": 88,
                        "node_id": "I_kwDO-upsert",
                        "number": 88,
                        "url": "http://localhost/repos/owner/repo/issues/88",
                        "html_url": "http://localhost/owner/repo/issues/88",
                        "state": "closed",
                        "state_reason": "completed",
                        "title": "Loop #44: GitHub upsert",
                        "comments": 1,
                        "created_at": "2026-01-01T00:00:00Z",
                        "updated_at": "2026-01-01T00:04:00Z"
                    })
                    .to_string()
                } else if request
                    .starts_with("GET /repos/owner/repo/issues/88/comments?per_page=100&page=2 ")
                {
                    serde_json::json!([
                        {
                            "id": 9001,
                            "node_id": "IC_kwDO-upsert",
                            "url": format!("{server_base_url}/repos/owner/repo/issues/comments/9001"),
                            "html_url": "http://localhost/owner/repo/issues/88#issuecomment-9001",
                            "body": existing_comment_body.as_str(),
                            "created_at": "2026-01-01T00:02:00Z",
                            "updated_at": "2026-01-01T00:02:00Z"
                        }
                    ])
                    .to_string()
                } else if request
                    .starts_with("GET /repos/owner/repo/issues/88/comments?per_page=100 ")
                {
                    serde_json::json!([]).to_string()
                } else if request.starts_with("PATCH /repos/owner/repo/issues/comments/9001 ") {
                    serde_json::json!({
                        "id": 9001,
                        "node_id": "IC_kwDO-upsert",
                        "url": format!("{server_base_url}/repos/owner/repo/issues/comments/9001"),
                        "html_url": "http://localhost/owner/repo/issues/88#issuecomment-9001",
                        "body": "updated",
                        "created_at": "2026-01-01T00:02:00Z",
                        "updated_at": "2026-01-01T00:04:00Z"
                    })
                    .to_string()
                } else {
                    serde_json::json!({"message": "not found"}).to_string()
                };
                let status = if body.contains("not found") {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let link = if request
                    .starts_with("GET /repos/owner/repo/issues/88/comments?per_page=100 ")
                {
                    format!(
                        "Link: <{server_base_url}/repos/owner/repo/issues/88/comments?per_page=100&page=2>; rel=\"next\"\r\n"
                    )
                } else {
                    String::new()
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{link}Connection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("test response should write");
            }
        });
        (base_url, requests, handle)
    }

    fn spawn_github_transient_write_server(
    ) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().expect("test request should connect");
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or_default();
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                request_log
                    .lock()
                    .expect("request log should lock")
                    .push(request.clone());
                let (status, body) = if index == 0 {
                    (
                        "503 Service Unavailable",
                        serde_json::json!({"message": "temporary unavailable"}).to_string(),
                    )
                } else if request.starts_with("PATCH /repos/owner/repo/issues/46 ") {
                    (
                        "200 OK",
                        serde_json::json!({
                            "id": 46,
                            "node_id": "I_kwDO-retry",
                            "number": 46,
                            "url": "http://localhost/repos/owner/repo/issues/46",
                            "html_url": "http://localhost/owner/repo/issues/46",
                            "state": "closed",
                            "state_reason": "completed",
                            "title": "Loop #46: GitHub retry",
                            "comments": 0,
                            "created_at": "2026-01-01T00:00:00Z",
                            "updated_at": "2026-01-01T00:04:00Z"
                        })
                        .to_string(),
                    )
                } else {
                    (
                        "404 Not Found",
                        serde_json::json!({"message": "not found"}).to_string(),
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("test response should write");
            }
        });
        (base_url, requests, handle)
    }

    fn spawn_github_rate_limit_write_server(
    ) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test request should connect");
            let mut buffer = [0_u8; 8192];
            let read = stream.read(&mut buffer).unwrap_or_default();
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            request_log
                .lock()
                .expect("request log should lock")
                .push(request);
            let body = serde_json::json!({"message": "API rate limit exceeded"}).to_string();
            let response = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nRetry-After: 60\r\nX-RateLimit-Remaining: 0\r\nX-RateLimit-Reset: 1780000000\r\nX-RateLimit-Resource: core\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("test response should write");
        });
        (base_url, requests, handle)
    }

    fn spawn_linear_transient_graphql_server(
    ) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().expect("test request should connect");
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or_default();
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                request_log
                    .lock()
                    .expect("request log should lock")
                    .push(request);
                let (status, body) = if index == 0 {
                    (
                        "503 Service Unavailable",
                        serde_json::json!({"errors": [{"message": "temporary unavailable"}]})
                            .to_string(),
                    )
                } else {
                    (
                        "200 OK",
                        serde_json::json!({
                            "data": {
                                "issueUpdate": {
                                    "success": true,
                                    "issue": {
                                        "id": "lin-uuid-49",
                                        "identifier": "ENT-49",
                                        "title": "Loop #49: Linear retry",
                                        "description": "Retry succeeded.",
                                        "url": "https://linear.app/acme/issue/ENT-49/linear-retry",
                                        "state": {
                                            "id": "state-todo",
                                            "name": "Todo",
                                            "type": "unstarted"
                                        },
                                        "comments": {
                                            "nodes": []
                                        }
                                    }
                                }
                            }
                        })
                        .to_string(),
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("test response should write");
            }
        });
        (base_url, requests, handle)
    }

    fn spawn_linear_rate_limit_graphql_server(
    ) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = Arc::clone(&requests);
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test request should connect");
            let mut buffer = [0_u8; 8192];
            let read = stream.read(&mut buffer).unwrap_or_default();
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            request_log
                .lock()
                .expect("request log should lock")
                .push(request);
            let body = serde_json::json!({
                "errors": [
                    {
                        "message": "Too many requests",
                        "extensions": {
                            "code": "RATE_LIMITED"
                        }
                    }
                ]
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nRetry-After: 60\r\nX-RateLimit-Remaining: 0\r\nX-RateLimit-Reset: 1780000000\r\nX-RateLimit-Limit: 1500\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("test response should write");
        });
        (base_url, requests, handle)
    }

    #[derive(Debug)]
    struct LinearFixtureState {
        title: String,
        description: String,
        comment_body: Option<String>,
    }

    fn spawn_linear_graphql_server() -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        spawn_linear_graphql_server_with_comment(None, 4)
    }

    fn spawn_linear_graphql_server_with_comment(
        initial_comment_body: Option<String>,
        request_count: usize,
    ) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = Arc::clone(&requests);
        let state = Arc::new(Mutex::new(LinearFixtureState {
            title: "Loop #47: Linear connector".to_string(),
            description: "Old Linear description".to_string(),
            comment_body: initial_comment_body,
        }));
        let fixture_state = Arc::clone(&state);
        let handle = thread::spawn(move || {
            for _ in 0..request_count {
                let (mut stream, _) = listener.accept().expect("test request should connect");
                let mut buffer = [0_u8; 16384];
                let read = stream.read(&mut buffer).unwrap_or_default();
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                request_log
                    .lock()
                    .expect("request log should lock")
                    .push(request.clone());
                let payload = request
                    .split_once("\r\n\r\n")
                    .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok())
                    .unwrap_or_else(|| serde_json::json!({}));
                let operation = payload
                    .pointer("/operationName")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                let body = match operation {
                    "EntranceIssueUpdate" => {
                        let mut state = fixture_state.lock().expect("fixture state should lock");
                        if let Some(title) = payload
                            .pointer("/variables/input/title")
                            .and_then(|value| value.as_str())
                        {
                            state.title = title.to_string();
                        }
                        if let Some(description) = payload
                            .pointer("/variables/input/description")
                            .and_then(|value| value.as_str())
                        {
                            state.description = description.to_string();
                        }
                        serde_json::json!({
                            "data": {
                                "issueUpdate": {
                                    "success": true,
                                    "issue": linear_fixture_issue_json(&state)
                                }
                            }
                        })
                    }
                    "EntranceCommentCreate" => {
                        let mut state = fixture_state.lock().expect("fixture state should lock");
                        state.comment_body = payload
                            .pointer("/variables/input/body")
                            .and_then(|value| value.as_str())
                            .map(ToOwned::to_owned);
                        serde_json::json!({
                            "data": {
                                "commentCreate": {
                                    "success": true,
                                    "comment": linear_fixture_comment_json(
                                        state.comment_body.as_deref().unwrap_or_default()
                                    )
                                }
                            }
                        })
                    }
                    "EntranceCommentUpdate" => {
                        let mut state = fixture_state.lock().expect("fixture state should lock");
                        state.comment_body = payload
                            .pointer("/variables/input/body")
                            .and_then(|value| value.as_str())
                            .map(ToOwned::to_owned);
                        serde_json::json!({
                            "data": {
                                "commentUpdate": {
                                    "success": true,
                                    "comment": linear_fixture_comment_json(
                                        state.comment_body.as_deref().unwrap_or_default()
                                    )
                                }
                            }
                        })
                    }
                    _ => {
                        let state = fixture_state.lock().expect("fixture state should lock");
                        serde_json::json!({
                            "data": {
                                "issue": linear_fixture_issue_json(&state)
                            }
                        })
                    }
                }
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("test response should write");
            }
        });
        (base_url, requests, handle)
    }

    fn linear_fixture_issue_json(state: &LinearFixtureState) -> serde_json::Value {
        let comments = state
            .comment_body
            .as_deref()
            .map(|body| vec![linear_fixture_comment_json(body)])
            .unwrap_or_default();
        serde_json::json!({
            "id": "lin-uuid-47",
            "identifier": "ENT-47",
            "title": state.title,
            "description": state.description,
            "url": "https://linear.app/acme/issue/ENT-47/linear-connector",
            "state": {
                "id": "state-todo",
                "name": "Todo",
                "type": "unstarted"
            },
            "comments": {
                "nodes": comments
            }
        })
    }

    fn linear_fixture_comment_json(body: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "comment-47",
            "body": body,
            "createdAt": "2026-01-01T00:02:00Z",
            "updatedAt": "2026-01-01T00:04:00Z",
            "user": {
                "name": "Entrance"
            }
        })
    }

    fn linear_test_mirror(issue_id: i64, comment_id: i64, comment_body: &str) -> IssueMirrorReport {
        IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "linear".to_string(),
            review_surface: "linear:ENT-47".to_string(),
            external_key: format!("hive-loop-{issue_id}-issue-{issue_id}"),
            issue: HiveIssue {
                id: issue_id,
                loop_id: Some(issue_id),
                title: format!("Loop #{issue_id}: Linear connector"),
                status: "Todo".to_string(),
                summary: Some("Validate Linear GraphQL mirror.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:04:00Z".to_string(),
            },
            loop_contract: Some(HiveLoopContract {
                id: issue_id,
                title: "Linear connector".to_string(),
                goal: "Verify Linear issue/status/comment surface.".to_string(),
                boundary: "Use a local GraphQL fixture.".to_string(),
                approach_space: vec!["GraphQL write".to_string(), "GraphQL readback".to_string()],
                eval_space: vec!["readback passes".to_string()],
                review_surface: "linear:ENT-47".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "local".to_string(),
                status: "todo".to_string(),
                active_phase: "explorer".to_string(),
                current_round: 1,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:04:00Z".to_string(),
            }),
            comments: vec![HiveComment {
                id: comment_id,
                issue_id,
                author: "hive".to_string(),
                body: comment_body.to_string(),
                payload: serde_json::json!({
                    "schema_version": "entrance.hive.system_comment.v1",
                    "source": "hive"
                }),
                created_at: "2026-01-01T00:02:00Z".to_string(),
            }],
            actions: vec![],
            trace: None,
            doctor: None,
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
    fn loop_demo_request_defaults_to_codex_panel_contract() {
        let request = loop_demo_request_from_flags(&[]);

        assert_eq!(request.title, "Entrance MVP demo");
        assert_eq!(request.runtime, "codex");
        assert_eq!(request.review_surface, "local-hive-panel");
        assert!(request.goal.contains("Explorer -> Doer -> Evaluator"));
        assert_eq!(request.approach_space.len(), 3);
        assert_eq!(request.eval_space.len(), 3);
    }

    #[test]
    fn loop_demo_request_accepts_runtime_and_eval_overrides() {
        let args = vec![
            "--runtime".to_string(),
            "local".to_string(),
            "--eval".to_string(),
            "cli ok,panel ok".to_string(),
        ];
        let request = loop_demo_request_from_flags(&args);

        assert_eq!(request.runtime, "local");
        assert_eq!(request.eval_space, vec!["cli ok", "panel ok"]);
    }

    #[test]
    fn compact_loop_start_summary_exposes_one_command_outcome() {
        let summary = compact_loop_start_summary(&serde_json::json!({
            "schema_version": "entrance.hive.issue.compact.v1",
            "issue": {
                "id": 9,
                "loop_id": 4,
                "title": "Loop #4: start",
                "status": "Done",
                "doctor": {
                    "health": "ok",
                    "runtime": "codex",
                    "counts": {
                        "workers": 3,
                        "worker_ok": 3,
                        "worker_duration_ms": 2100,
                        "receipt_required": 11,
                        "receipt_missing": 0,
                        "audit_failed": 0
                    },
                    "next_actions": ["entrance hive loop doctor 4"]
                },
                "trace": {
                    "decision": "keep",
                    "reason_code": "all_gates_passed"
                }
            },
            "recent_comments": [{
                "author": "evaluator",
                "body": "Evaluator kept the loop."
            }],
            "recent_evidence": [{
                "role": "evaluator",
                "kind": "verdict"
            }],
            "stages": [{
                "role": "evaluator",
                "status": "complete"
            }],
            "connector": {
                "provider": "local-hive-panel",
                "current": false,
                "publish_required": true,
                "reason": "mirror_file_missing",
                "failed_checks": ["remote_file_present"],
                "publish_command": "entrance hive issue mirror-publish 9 --compact"
            }
        }));

        assert_eq!(
            summary
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.loop_start.compact.v1")
        );
        assert_eq!(
            summary
                .pointer("/complete")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            summary.pointer("/runtime").and_then(|value| value.as_str()),
            Some("codex")
        );
        assert_eq!(
            summary
                .pointer("/counts/worker_ok")
                .and_then(|value| value.as_u64()),
            Some(3)
        );
        assert_eq!(
            summary
                .pointer("/commands/show")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue show 9 --compact")
        );
        assert_eq!(
            summary
                .pointer("/commands/doctor")
                .and_then(|value| value.as_str()),
            Some("entrance hive loop doctor 4")
        );
        assert_eq!(
            summary
                .pointer("/commands/retry")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue retry-run 9 --body <note> --runtime codex --worker-attempts 2 --compact")
        );
        assert_eq!(
            summary
                .pointer("/recent_comments/0/body")
                .and_then(|value| value.as_str()),
            Some("Evaluator kept the loop.")
        );
        assert_eq!(
            summary
                .pointer("/connector/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("remote_file_present")
        );
        assert_eq!(
            summary
                .pointer("/issue/title")
                .and_then(|value| value.as_str()),
            Some("Loop #4: start")
        );
        assert_eq!(
            summary
                .pointer("/recovery/required")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            summary
                .pointer("/recovery/retry_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue retry-run 9 --body <note> --runtime codex --worker-attempts 2 --compact")
        );
    }

    #[test]
    fn compact_loop_start_summary_exposes_recovery_for_worker_failure() {
        let summary = compact_loop_start_summary(&serde_json::json!({
            "schema_version": "entrance.hive.issue.compact.v1",
            "issue": {
                "id": 7,
                "loop_id": 3,
                "title": "Loop #3: timeout",
                "status": "Blocked",
                "summary": "Explorer worker timed out.",
                "doctor": {
                    "health": "worker_failed",
                    "runtime": "codex",
                    "counts": {
                        "workers": 1,
                        "worker_ok": 0,
                        "worker_duration_ms": 1000,
                        "receipt_required": 3,
                        "receipt_missing": 1,
                        "audit_failed": 1
                    },
                    "failed_checks": ["worker_receipts"],
                    "missing_receipts": ["role_worker"],
                    "worker_failures": [
                        "explorer:exploration_packet worker=codex ok=false receipt=false timeout"
                    ],
                    "next_actions": [
                        "entrance hive loop evidence 3",
                        "entrance hive loop doctor 3",
                        "entrance hive issue retry-run 7 --body <note> --runtime codex --worker-attempts 2 --compact"
                    ]
                },
                "trace": {
                    "decision": "blocked",
                    "reason_code": "worker_receipt_failed",
                    "round": 1,
                    "rounds": [{
                        "round": 1,
                        "status": "blocked",
                        "decision": "blocked",
                        "evidence_count": 1,
                        "rejected_count": 1,
                        "receipt_required": 3,
                        "receipt_missing": 1,
                        "workers": 1,
                        "worker_ok": 0,
                        "timeouts": 1,
                        "retry_exhausted": 1
                    }]
                }
            },
            "recent_evidence": [{
                "round": 1,
                "role": "explorer",
                "kind": "exploration_packet",
                "worker": {
                    "kind": "codex",
                    "ok": false,
                    "receipt_ok": false,
                    "timed_out": true,
                    "retry_exhausted": true,
                    "attempt_count": 1,
                    "max_attempts": 1,
                    "duration_ms": 1000,
                    "receipt_errors": ["role_worker"]
                }
            }]
        }));

        assert_eq!(
            summary
                .pointer("/complete")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            summary
                .pointer("/recovery/required")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            summary
                .pointer("/recovery/retry_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue retry-run 7 --body <note> --runtime codex --worker-attempts 2 --compact")
        );
        assert_eq!(
            summary
                .pointer("/recovery/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("worker_receipts")
        );
        assert_eq!(
            summary
                .pointer("/recovery/missing_receipts/0")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );
        assert_eq!(
            summary
                .pointer("/recovery/failed_workers/0/role")
                .and_then(|value| value.as_str()),
            Some("explorer")
        );
        assert_eq!(
            summary
                .pointer("/recovery/failed_workers/0/timed_out")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            summary
                .pointer("/recovery/failed_workers/0/attempt_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            summary
                .pointer("/rounds/current")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(
            summary
                .pointer("/rounds/failed_rounds/0")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
    }

    #[test]
    fn compact_loop_start_summary_exposes_recovered_rounds() {
        let summary = compact_loop_start_summary(&serde_json::json!({
            "schema_version": "entrance.hive.issue.compact.v1",
            "issue": {
                "id": 7,
                "loop_id": 3,
                "title": "Loop #3: recovered",
                "status": "Done",
                "doctor": {
                    "health": "ok",
                    "runtime": "codex",
                    "counts": {
                        "workers": 3,
                        "worker_ok": 3,
                        "worker_duration_ms": 32000,
                        "receipt_required": 11,
                        "receipt_missing": 0,
                        "audit_failed": 0
                    },
                    "failed_checks": [],
                    "missing_receipts": [],
                    "worker_failures": [],
                    "next_actions": []
                },
                "trace": {
                    "decision": "keep",
                    "reason_code": "all_gates_passed",
                    "round": 2,
                    "rounds": [
                        {
                            "round": 1,
                            "status": "blocked",
                            "decision": "blocked",
                            "evidence_count": 1,
                            "rejected_count": 1,
                            "receipt_required": 3,
                            "receipt_missing": 1,
                            "workers": 1,
                            "worker_ok": 0,
                            "timeouts": 1,
                            "retry_exhausted": 1
                        },
                        {
                            "round": 2,
                            "status": "kept",
                            "decision": "keep",
                            "evidence_count": 4,
                            "rejected_count": 0,
                            "receipt_required": 11,
                            "receipt_missing": 0,
                            "workers": 3,
                            "worker_ok": 3,
                            "timeouts": 0,
                            "retry_exhausted": 0
                        }
                    ]
                }
            },
            "recent_evidence": [{
                "round": 2,
                "role": "evaluator",
                "kind": "verdict_packet"
            }]
        }));

        assert_eq!(
            summary
                .pointer("/complete")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            summary
                .pointer("/rounds/current")
                .and_then(|value| value.as_i64()),
            Some(2)
        );
        assert_eq!(
            summary
                .pointer("/rounds/failed_rounds/0")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(
            summary
                .pointer("/rounds/recovered_from_rounds/0")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(
            summary
                .pointer("/recovery/required")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
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
    fn compact_connector_queue_collects_publish_required_issues() {
        let registry = test_connector_registry();
        let issues = vec![
            serde_json::json!({
                "id": 7,
                "loop_id": 3,
                "title": "Loop #3: connector stale",
                "status": "Done",
                "connector": {
                    "publish_required": true,
                    "current": false,
                    "provider": "file",
                    "review_surface": "file:local-board",
                    "reason": "mirror_stale",
                    "path": "/tmp/issue.json",
                    "current_sha256": "sha-file-current",
                    "remote_sha256": "sha-file-remote",
                    "current_comment_count": 4,
                    "remote_comment_count": 3,
                    "failed_checks": ["remote_digest_current"],
                    "publish_command": "entrance hive issue mirror-publish 7 --compact",
                    "readback_command": "entrance hive issue mirror-readback 7 --record --compact",
                    "roundtrip_command": "entrance hive issue mirror-roundtrip 7 --compact"
                }
            }),
            serde_json::json!({
                "id": 8,
                "loop_id": 4,
                "title": "Loop #4: connector current",
                "status": "Done",
                "connector": {
                    "provider": "file",
                    "review_surface": "file:local-board",
                    "current": true,
                    "publish_required": false,
                    "reason": "connector_mirror_current"
                }
            }),
            serde_json::json!({
                "id": 9,
                "loop_id": 5,
                "title": "Loop #5: connector linear planned",
                "status": "Blocked",
                "connector": {
                    "publish_required": true,
                    "current": false,
                    "provider": "linear",
                    "review_surface": "linear:ENT-9",
                    "reason": "mirror_file_missing",
                    "path": "/tmp/linear.json",
                    "current_sha256": "sha-linear-current",
                    "current_comment_count": 2,
                    "failed_checks": ["remote_file_present"],
                    "publish_command": "entrance hive issue mirror-publish 9 --compact",
                    "readback_command": "entrance hive issue mirror-readback 9 --record --compact",
                    "admit_command": "entrance hive issue mirror-admit 9 --record --compact",
                    "roundtrip_command": "entrance hive issue mirror-roundtrip 9 --compact"
                }
            }),
        ];

        let queue = compact_connector_queue(&registry, &issues, None);

        assert_eq!(
            queue
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_queue.v1")
        );
        assert_eq!(
            queue
                .pointer("/publish_required_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            queue
                .pointer("/issues/0/id")
                .and_then(|value| value.as_i64()),
            Some(7)
        );
        assert_eq!(
            queue
                .pointer("/issues/0/reason")
                .and_then(|value| value.as_str()),
            Some("mirror_stale")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/failed_check_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            queue
                .pointer("/issues/0/provider")
                .and_then(|value| value.as_str()),
            Some("file")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/commands/publish")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-publish 7 --compact")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/current_sha256")
                .and_then(|value| value.as_str()),
            Some("sha-file-current")
        );
        assert_eq!(
            queue
                .pointer("/commands/publish_plan")
                .and_then(|value| value.as_str()),
            Some("entrance hive connector publish-plan --compact")
        );
        assert_eq!(
            queue
                .pointer("/commands/roundtrip_plan")
                .and_then(|value| value.as_str()),
            Some("entrance hive connector roundtrip-plan --compact")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/commands/roundtrip")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-roundtrip 7 --compact")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/dry_run_action/would_write")
                .and_then(|value| value.as_str()),
            Some("local connector mirror file")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/can_publish")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            queue
                .pointer("/issues/0/adapter/driver")
                .and_then(|value| value.as_str()),
            Some("file-mirror")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/admission_checks")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(7)
        );
        assert_eq!(
            queue
                .pointer("/issues/0/admission_checks/2/name")
                .and_then(|value| value.as_str()),
            Some("mirror_current")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/admission_checks/2/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            queue
                .pointer("/issues/0/admission_checks/2/owner")
                .and_then(|value| value.as_str()),
            Some("test-owner")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/admission_checks/2/severity")
                .and_then(|value| value.as_str()),
            Some("blocker")
        );
        assert_eq!(
            queue
                .pointer("/providers/0/publish_required_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            queue
                .pointer("/providers/0/admission_status")
                .and_then(|value| value.as_str()),
            Some("ready")
        );

        let linear_queue = compact_connector_queue(&registry, &issues, Some("linear"));
        assert_eq!(
            linear_queue
                .pointer("/provider_filter")
                .and_then(|value| value.as_str()),
            Some("linear")
        );
        assert_eq!(
            linear_queue
                .pointer("/publish_required_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            linear_queue
                .pointer("/providers/0/name")
                .and_then(|value| value.as_str()),
            Some("linear")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/provider_status")
                .and_then(|value| value.as_str()),
            Some("planned")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/admission_status")
                .and_then(|value| value.as_str()),
            Some("blocked")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/admission_blockers/0")
                .and_then(|value| value.as_str()),
            Some("provider_not_active")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/can_publish")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/publish_blockers/0")
                .and_then(|value| value.as_str()),
            Some("provider_not_active")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/publish_blockers/1")
                .and_then(|value| value.as_str()),
            Some("publish_not_supported")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_contract.v1")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/remote_object_kind")
                .and_then(|value| value.as_str()),
            Some("linear.issue")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/write/receipt_schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_write_receipt.v1")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/readback/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_readback.v1")
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/retry/max_attempts")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/retry/base_backoff_ms")
                .and_then(|value| value.as_u64()),
            Some(100)
        );
        assert_eq!(
            linear_queue
                .pointer("/issues/0/adapter/remote_contract/retry/retryable_http_statuses/2")
                .and_then(|value| value.as_u64()),
            Some(503)
        );
        assert_eq!(
            linear_queue
                .pointer("/commands/publish_plan")
                .and_then(|value| value.as_str()),
            Some("entrance hive connector publish-plan --provider linear --compact")
        );
        assert_eq!(
            linear_queue
                .pointer("/commands/roundtrip_plan")
                .and_then(|value| value.as_str()),
            Some("entrance hive connector roundtrip-plan --provider linear --compact")
        );

        let linear_plan =
            compact_connector_publish_plan(&linear_queue).expect("linear plan should render");
        assert_eq!(
            linear_plan
                .pointer("/can_execute")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            linear_plan
                .pointer("/reason")
                .and_then(|value| value.as_str()),
            Some("plan_blocked")
        );
        assert_eq!(
            linear_plan
                .pointer("/commands/execute")
                .and_then(|value| value.as_str()),
            None
        );
        let blockers = linear_plan
            .pointer("/blockers")
            .and_then(|value| value.as_array())
            .expect("linear plan blockers should be present")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(blockers.contains(&"provider_not_active"));
        assert!(blockers.contains(&"publish_not_supported"));
    }

    #[test]
    fn connector_status_and_queue_expose_remote_retry_diagnostics() {
        let mut readback = serde_json::json!({
            "schema_version": ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
            "passed": false,
            "failed_checks": ["remote_issue_read"],
            "provider": "linear",
            "review_surface": "linear:ENT-50",
            "external_key": "ENT-50",
            "issue_id": 50,
            "issue_status": "Todo",
            "path": "/tmp/linear-ent-50.json",
            "receipt": {
                "found": true,
                "remote_write_receipt": {
                    "schema_version": CONNECTOR_REMOTE_WRITE_RECEIPT_SCHEMA_VERSION,
                    "write_execution": {
                        "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
                        "success": true,
                        "failed_checks": [],
                        "operation_count": 1,
                        "operations": [{
                            "kind": "read_issue_for_write",
                            "success": true,
                            "http_status": 200,
                            "attempt_count": 2,
                            "max_attempts": 2,
                            "retry": {
                                "reason": "success",
                                "retryable": false,
                                "scheduled": false,
                                "attempted": true,
                                "exhausted": false,
                                "rate_limited": false
                            }
                        }]
                    }
                }
            },
            "remote_readback": {
                "execution": {
                    "success": false,
                    "failed_checks": ["remote_rate_limited"],
                    "issue": {
                        "kind": "read_issue",
                        "success": false,
                        "failed_check": "remote_rate_limited",
                        "http_status": 429,
                        "attempt_count": 1,
                        "max_attempts": 2,
                        "retry": {
                            "reason": "rate_limited",
                            "retryable": false,
                            "scheduled": false,
                            "attempted": false,
                            "exhausted": false,
                            "rate_limited": true,
                            "retry_after_secs": 60
                        }
                    }
                }
            },
            "checks": [],
            "current": {
                "digest": {"sha256": "sha-current"},
                "comments": {"count": 1}
            },
            "remote": {
                "digest": {"sha256": "sha-remote"},
                "surface": {"comments": {"count": 0}}
            }
        });
        readback
            .pointer_mut("/receipt/remote_write_receipt/write_execution/operations/0")
            .and_then(|value| value.as_object_mut())
            .expect("write operation should be present")
            .insert(
                "attempts".to_string(),
                serde_json::json!([
                    {
                        "attempt": 1,
                        "success": false,
                        "failed_check": "remote_retryable_http_status",
                        "http_status": 503,
                        "retry": {
                            "reason": "retryable_http_status",
                            "retryable": true,
                            "scheduled": true,
                            "attempted": false,
                            "exhausted": false,
                            "rate_limited": false,
                            "backoff_ms": 100
                        }
                    },
                    {
                        "attempt": 2,
                        "success": true,
                        "failed_check": null,
                        "http_status": 200,
                        "retry": {
                            "reason": "success",
                            "retryable": false,
                            "scheduled": false,
                            "attempted": false,
                            "exhausted": false,
                            "rate_limited": false
                        }
                    }
                ]),
            );
        readback
            .pointer_mut("/remote_readback/execution/issue")
            .and_then(|value| value.as_object_mut())
            .expect("readback issue operation should be present")
            .insert(
                "attempts".to_string(),
                serde_json::json!([{
                    "attempt": 1,
                    "success": false,
                    "failed_check": "remote_rate_limited",
                    "http_status": 429,
                    "retry": {
                        "reason": "rate_limited",
                        "retryable": false,
                        "scheduled": false,
                        "attempted": false,
                        "exhausted": false,
                        "rate_limited": true,
                        "retry_after_secs": 60
                    }
                }]),
            );
        let status = compact_issue_mirror_status(&readback);

        assert_eq!(
            status
                .pointer("/remote_diagnostics/write/signal/label")
                .and_then(|value| value.as_str()),
            Some("write retry 2 attempts")
        );
        assert_eq!(
            status
                .pointer("/remote_diagnostics/readback/signal/label")
                .and_then(|value| value.as_str()),
            Some("readback remote_rate_limited 429 retry after 60s")
        );
        assert_eq!(
            status
                .pointer("/remote_diagnostics/signals/1/retry/rate_limited")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            status
                .pointer("/remote_diagnostics/write/primary_operation/attempts/0/failed_check")
                .and_then(|value| value.as_str()),
            Some("remote_retryable_http_status")
        );
        assert_eq!(
            status
                .pointer("/remote_diagnostics/write/primary_operation/attempts/0/retry/backoff_ms")
                .and_then(|value| value.as_u64()),
            Some(100)
        );
        assert_eq!(
            status
                .pointer(
                    "/remote_diagnostics/readback/primary_operation/attempts/0/retry/rate_limited"
                )
                .and_then(|value| value.as_bool()),
            Some(true)
        );

        let registry = test_connector_registry();
        let queue = compact_connector_queue(
            &registry,
            &[serde_json::json!({
                "id": 50,
                "loop_id": 7,
                "title": "Loop #7: remote diagnostics",
                "status": "Todo",
                "connector": status
            })],
            Some("linear"),
        );

        assert_eq!(
            queue
                .pointer("/issues/0/remote_diagnostics/signals/0/label")
                .and_then(|value| value.as_str()),
            Some("write retry 2 attempts")
        );
        assert_eq!(
            queue
                .pointer("/issues/0/remote_diagnostics/signals/1/failed_check")
                .and_then(|value| value.as_str()),
            Some("remote_rate_limited")
        );
        assert_eq!(
            queue
                .pointer(
                    "/issues/0/remote_diagnostics/write/primary_operation/attempts/0/http_status"
                )
                .and_then(|value| value.as_u64()),
            Some(503)
        );

        let failed_write_status = compact_issue_mirror_status(&serde_json::json!({
            "schema_version": ISSUE_MIRROR_READBACK_SCHEMA_VERSION,
            "passed": false,
            "failed_checks": ["write_receipt_binding"],
            "provider": "linear",
            "review_surface": "linear:ENT-52",
            "external_key": "ENT-52",
            "issue_id": 52,
            "issue_status": "Todo",
            "receipt": {
                "found": true,
                "remote_write_receipt": null,
                "remote_write_execution": {
                    "schema_version": CONNECTOR_REMOTE_WRITE_EXECUTE_SCHEMA_VERSION,
                    "success": false,
                    "failed_checks": ["remote_rate_limited"],
                    "operation_count": 1,
                    "operations": [{
                        "kind": "read_issue_for_write",
                        "success": false,
                        "failed_check": "remote_rate_limited",
                        "http_status": 429,
                        "attempt_count": 1,
                        "max_attempts": 2,
                        "retry": {
                            "reason": "rate_limited",
                            "retryable": false,
                            "scheduled": false,
                            "attempted": false,
                            "exhausted": false,
                            "rate_limited": true,
                            "retry_after_secs": 60
                        }
                    }]
                }
            },
            "remote_readback": {
                "execution": null
            },
            "checks": []
        }));
        assert_eq!(
            failed_write_status
                .pointer("/remote_diagnostics/write/signal/label")
                .and_then(|value| value.as_str()),
            Some("write remote_rate_limited 429 retry after 60s")
        );
    }

    #[test]
    fn connector_admission_preview_checks_explain_remote_contract_blockers() {
        let provider =
            test_connector_provider("linear", "Linear", "planned", false, false, vec!["linear:"]);
        let admission = test_provider_admission(&provider);
        let status = serde_json::json!({
            "current": false,
            "publish_required": true,
            "reason": "mirror_file_missing",
            "failed_checks": ["remote_file_present"]
        });
        let writer_blockers = connector_writer_blockers(Some(&provider));
        let remote_target =
            connector_remote_target(Some(&provider), "linear:ENT-42", "hive-loop-1-issue-1");
        let remote_contract = compact_connector_remote_contract(Some(&provider));

        let checks = connector_admission_preview_checks(
            Some(&provider),
            Some(&admission),
            &status,
            &writer_blockers,
            true,
            &remote_target,
            &remote_contract,
            &admission.check_registry,
        );

        assert_eq!(checks.len(), 7);
        let names = checks
            .iter()
            .filter_map(|check| check.pointer("/name").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert!(names.contains(&"provider_admission_ready"));
        assert!(names.contains(&"remote_write_contract_ready"));
        assert!(names.contains(&"retry_policy_bound"));
        let remote_contract_check = checks
            .iter()
            .find(|check| {
                check.pointer("/name").and_then(|value| value.as_str())
                    == Some("remote_write_contract_ready")
            })
            .expect("remote contract check should be present");
        assert_eq!(
            remote_contract_check
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            remote_contract_check
                .pointer("/details/writer_blockers/2")
                .and_then(|value| value.as_str()),
            Some("publish_not_supported")
        );
        let target_check = checks
            .iter()
            .find(|check| {
                check.pointer("/name").and_then(|value| value.as_str())
                    == Some("remote_target_valid")
            })
            .expect("remote target check should be present");
        assert_eq!(
            target_check
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        let retry_policy_check = checks
            .iter()
            .find(|check| {
                check.pointer("/name").and_then(|value| value.as_str())
                    == Some("retry_policy_bound")
            })
            .expect("retry policy check should be present");
        assert_eq!(
            retry_policy_check
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            retry_policy_check
                .pointer("/details/policy/max_attempts")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            retry_policy_check
                .pointer("/owner")
                .and_then(|value| value.as_str()),
            Some("retry-policy")
        );
        assert_eq!(
            retry_policy_check
                .pointer("/severity")
                .and_then(|value| value.as_str()),
            Some("blocker")
        );
        assert_eq!(
            retry_policy_check
                .pointer("/required_evidence/0")
                .and_then(|value| value.as_str()),
            Some("connector_remote_contract.retry")
        );
        assert_eq!(
            retry_policy_check
                .pointer("/policy_summary")
                .and_then(|value| value.as_str()),
            Some("retry_policy_bound summary")
        );

        let over_budget_status = serde_json::json!({
            "current": true,
            "publish_required": false,
            "failed_checks": [],
            "remote_diagnostics": {
                "write": {
                    "primary_operation": {
                        "kind": "read_issue_for_write",
                        "attempt_count": 3,
                        "max_attempts": 3,
                        "attempts": [
                            {"attempt": 1, "success": false},
                            {"attempt": 2, "success": false},
                            {"attempt": 3, "success": true}
                        ]
                    }
                },
                "readback": null
            }
        });
        let over_budget_checks = connector_admission_preview_checks(
            Some(&provider),
            Some(&admission),
            &over_budget_status,
            &[],
            true,
            &remote_target,
            &remote_contract,
            &admission.check_registry,
        );
        let over_budget_retry_check = over_budget_checks
            .iter()
            .find(|check| {
                check.pointer("/name").and_then(|value| value.as_str())
                    == Some("retry_policy_bound")
            })
            .expect("retry policy check should be present");
        assert_eq!(
            over_budget_retry_check
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            over_budget_retry_check
                .pointer("/details/violations/0/code")
                .and_then(|value| value.as_str()),
            Some("retry_attempt_budget_exceeded")
        );
        assert_eq!(
            connector_admission_check_failed(&over_budget_checks, "retry_policy_bound"),
            true
        );
    }

    #[test]
    fn remote_issue_targets_are_provider_specific_and_gate_publish() {
        let mut github = test_connector_provider(
            "github",
            "GitHub",
            "active",
            true,
            true,
            vec!["github:", "gh:"],
        );
        github.mode = "remote-issue-api".to_string();
        let target =
            connector_remote_target(Some(&github), "github:owner/repo#42", "hive-loop-1-issue-1");
        assert_eq!(
            target
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_target.v1")
        );
        assert_eq!(
            target.pointer("/valid").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            target.pointer("/owner").and_then(|value| value.as_str()),
            Some("owner")
        );
        assert_eq!(
            target.pointer("/repo").and_then(|value| value.as_str()),
            Some("repo")
        );
        assert_eq!(
            target
                .pointer("/issue_number")
                .and_then(|value| value.as_u64()),
            Some(42)
        );
        assert_eq!(
            target.pointer("/api_url").and_then(|value| value.as_str()),
            Some("https://api.github.com/repos/owner/repo/issues/42")
        );

        let invalid =
            connector_remote_target(Some(&github), "github:not-a-target", "hive-loop-1-issue-1");
        assert_eq!(
            invalid.pointer("/valid").and_then(|value| value.as_bool()),
            Some(false)
        );
        let blockers =
            connector_issue_writer_blockers(Some(&github), "github:not-a-target", "hive-loop-1");
        assert!(blockers.contains(&"remote_target_invalid".to_string()));
        assert!(blockers.contains(&"github_repo_missing".to_string()));

        let linear =
            test_connector_provider("linear", "Linear", "active", true, true, vec!["linear:"]);
        let linear_target = connector_remote_target(
            Some(&linear),
            "linear:https://linear.app/acme/issue/ENT-37/title",
            "hive-loop-2-issue-2",
        );
        assert_eq!(
            linear_target
                .pointer("/issue_key")
                .and_then(|value| value.as_str()),
            Some("ENT-37")
        );
        assert_eq!(
            linear_target
                .pointer("/issue_number")
                .and_then(|value| value.as_u64()),
            Some(37)
        );
    }

    #[test]
    fn remote_write_plan_exposes_provider_request_envelopes() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        let issue = serde_json::json!({
            "id": 42,
            "loop_id": 7,
            "title": "Loop #7: remote writer",
            "status": "Done",
            "summary": "Ship a typed remote writer plan.",
            "comment_count": 2,
            "latest_comment": {
                "author": "hive",
                "body": "Evaluator kept the candidate."
            },
            "connector": {
                "external_key": "hive-loop-7-issue-42",
                "current_sha256": "sha-remote"
            }
        });
        let target = connector_remote_target(
            Some(&github),
            "github:owner/repo#42",
            "hive-loop-7-issue-42",
        );
        let plan = connector_remote_write_plan(Some(&github), &issue, &target, &[]);
        assert_eq!(
            plan.pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_write_plan.v1")
        );
        assert_eq!(
            plan.pointer("/executable")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            plan.pointer("/operations/0/method")
                .and_then(|value| value.as_str()),
            Some("PATCH")
        );
        assert_eq!(
            plan.pointer("/operations/0/url")
                .and_then(|value| value.as_str()),
            Some("https://api.github.com/repos/owner/repo/issues/42")
        );
        assert_eq!(
            plan.pointer("/operations/0/body/state")
                .and_then(|value| value.as_str()),
            Some("closed")
        );
        assert_eq!(
            plan.pointer("/operations/0/body/state_reason")
                .and_then(|value| value.as_str()),
            Some("completed")
        );
        assert_eq!(
            plan.pointer("/operations/1/kind")
                .and_then(|value| value.as_str()),
            Some("upsert_latest_comment")
        );
        assert_eq!(
            plan.pointer("/operations/1/method")
                .and_then(|value| value.as_str()),
            Some("GET_PATCH_OR_POST")
        );
        assert_eq!(
            plan.pointer("/operations/1/url")
                .and_then(|value| value.as_str()),
            Some("https://api.github.com/repos/owner/repo/issues/42/comments")
        );
        assert_eq!(
            plan.pointer("/operations/1/lookup/url")
                .and_then(|value| value.as_str()),
            Some("https://api.github.com/repos/owner/repo/issues/42/comments?per_page=100")
        );
        assert_eq!(
            plan.pointer("/operations/1/update/url_template")
                .and_then(|value| value.as_str()),
            Some("https://api.github.com/repos/owner/repo/issues/comments/{comment_id}")
        );
        assert!(plan
            .pointer("/operations/1/idempotency_key")
            .and_then(|value| value.as_str())
            .is_some());
        assert!(plan
            .pointer("/operations/1/body/body")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .contains("<!-- entrance-idempotency-key:"));

        let invalid_target =
            connector_remote_target(Some(&github), "github:not-a-target", "hive-loop-7-issue-42");
        let invalid_plan = connector_remote_write_plan(
            Some(&github),
            &issue,
            &invalid_target,
            &["remote_target_invalid".to_string()],
        );
        assert_eq!(
            invalid_plan
                .pointer("/executable")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            invalid_plan
                .pointer("/blocked_by/0")
                .and_then(|value| value.as_str()),
            Some("remote_target_invalid")
        );

        let mut linear =
            test_connector_provider("linear", "Linear", "active", true, true, vec!["linear:"]);
        linear.mode = "remote-issue-api".to_string();
        let linear_target =
            connector_remote_target(Some(&linear), "linear:ENT-37", "hive-loop-8-issue-8");
        let linear_plan = connector_remote_write_plan(Some(&linear), &issue, &linear_target, &[]);
        assert_eq!(
            linear_plan
                .pointer("/operations/0/graphql/operation")
                .and_then(|value| value.as_str()),
            Some("EntranceIssueRead")
        );
        assert_eq!(
            linear_plan
                .pointer("/operations/1/graphql/operation")
                .and_then(|value| value.as_str()),
            Some("EntranceIssueUpdate")
        );
        assert_eq!(
            linear_plan
                .pointer("/operations/1/graphql/variables/id")
                .and_then(|value| value.as_str()),
            Some("<lookup.issue.id>")
        );
        assert_eq!(
            linear_plan
                .pointer("/operations/2/kind")
                .and_then(|value| value.as_str()),
            Some("upsert_latest_comment")
        );
        assert_eq!(
            linear_plan
                .pointer("/executable")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn linear_remote_write_and_readback_accepts_graphql_issue_surface() {
        let mut linear =
            test_connector_provider("linear", "Linear", "active", true, true, vec!["linear:"]);
        linear.mode = "remote-issue-api".to_string();
        linear.auth_required = true;
        linear.auth_env = vec!["ENTRANCE_TEST_LINEAR_GRAPHQL_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_LINEAR_GRAPHQL_TOKEN", "test-token");
        let (base_url, requests, server) = spawn_linear_graphql_server();
        linear.storage = base_url;
        let mirror = IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "linear".to_string(),
            review_surface: "linear:ENT-47".to_string(),
            external_key: "hive-loop-47-issue-47".to_string(),
            issue: HiveIssue {
                id: 47,
                loop_id: Some(47),
                title: "Loop #47: Linear connector".to_string(),
                status: "Todo".to_string(),
                summary: Some("Validate Linear GraphQL mirror.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:04:00Z".to_string(),
            },
            loop_contract: Some(HiveLoopContract {
                id: 47,
                title: "Linear connector".to_string(),
                goal: "Verify Linear issue/status/comment surface.".to_string(),
                boundary: "Use a local GraphQL fixture.".to_string(),
                approach_space: vec!["GraphQL write".to_string(), "GraphQL readback".to_string()],
                eval_space: vec!["readback passes".to_string()],
                review_surface: "linear:ENT-47".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "local".to_string(),
                status: "todo".to_string(),
                active_phase: "explorer".to_string(),
                current_round: 1,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:04:00Z".to_string(),
            }),
            comments: vec![HiveComment {
                id: 4701,
                issue_id: 47,
                author: "hive".to_string(),
                body: "Evaluator kept the Linear connector candidate.".to_string(),
                payload: serde_json::json!({
                    "schema_version": "entrance.hive.system_comment.v1",
                    "source": "hive"
                }),
                created_at: "2026-01-01T00:02:00Z".to_string(),
            }],
            actions: vec![],
            trace: None,
            doctor: None,
        };
        let digest = digest_bytes(&mirror_payload(&mirror).expect("mirror should serialize"));
        let source_issue = connector_remote_write_issue_from_mirror(&mirror, &digest);
        let target = connector_remote_target(Some(&linear), "linear:ENT-47", &mirror.external_key);
        let plan = connector_remote_write_plan(Some(&linear), &source_issue, &target, &[]);

        let execution = execute_linear_remote_write_plan(&linear, &plan);

        assert_eq!(
            plan.pointer("/executable")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/response/summary/id")
                .and_then(|value| value.as_str()),
            Some("lin-uuid-47")
        );
        assert_eq!(
            execution
                .pointer("/operations/2/mode")
                .and_then(|value| value.as_str()),
            Some("append_new_comment")
        );
        let path = Path::new("/tmp/root/connectors/linear/hive-loop-47-issue-47.json");
        let receipt_path = mirror_receipt_path(path);
        let remote_write_receipt = connector_remote_write_receipt_from_execution(
            &mirror, path, &digest, &linear, &target, &execution,
        );
        let mut receipt = issue_mirror_sync_receipt_for_provider(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&linear),
        );
        if let Some(object) = receipt.as_object_mut() {
            object.insert("remote_write_receipt".to_string(), remote_write_receipt);
            object.insert("remote_write_execution".to_string(), execution);
        }

        let readback = compact_linear_issue_mirror_readback(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&receipt),
            &linear,
        );
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_LINEAR_GRAPHQL_TOKEN");

        assert_eq!(
            readback
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            readback
                .pointer("/remote/surface/issue/identifier")
                .and_then(|value| value.as_str()),
            Some("ENT-47")
        );
        assert_eq!(
            readback
                .pointer("/remote/surface/comments/count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/checks/9/name")
                .and_then(|value| value.as_str()),
            Some("write_receipt_binding")
        );
        let request_log = requests
            .lock()
            .expect("request log should lock")
            .join("\n---\n");
        assert!(request_log.contains("\"operationName\":\"EntranceIssueRead\""));
        assert!(request_log.contains("\"operationName\":\"EntranceIssueUpdate\""));
        assert!(request_log.contains("\"operationName\":\"EntranceCommentCreate\""));
        assert!(connector_linear_remote_comment_body(&source_issue)
            .contains("<!-- entrance-idempotency-key:"));
    }

    #[test]
    fn linear_remote_write_updates_existing_idempotent_comment_after_local_comment_changes() {
        let mut linear =
            test_connector_provider("linear", "Linear", "active", true, true, vec!["linear:"]);
        linear.mode = "remote-issue-api".to_string();
        linear.auth_required = true;
        linear.auth_env = vec!["ENTRANCE_TEST_LINEAR_GRAPHQL_UPSERT_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_LINEAR_GRAPHQL_UPSERT_TOKEN", "test-token");
        let mirror = linear_test_mirror(48, 4802, "Current local connector observation.");
        let digest = digest_bytes(&mirror_payload(&mirror).expect("mirror should serialize"));
        let source_issue = connector_remote_write_issue_from_mirror(&mirror, &digest);
        let mut old_source_issue = source_issue.clone();
        if let Some(value) = old_source_issue.pointer_mut("/latest_comment/id") {
            *value = serde_json::json!(4801);
        }
        if let Some(value) = old_source_issue.pointer_mut("/latest_comment/body") {
            *value = serde_json::json!("Old remote connector observation.");
        }
        if let Some(value) = old_source_issue.pointer_mut("/connector/current_sha256") {
            *value = serde_json::json!("old-linear-sha");
        }
        let existing_comment_body = connector_linear_remote_comment_body(&old_source_issue);
        let (base_url, requests, server) =
            spawn_linear_graphql_server_with_comment(Some(existing_comment_body), 3);
        linear.storage = base_url;
        let target = connector_remote_target(Some(&linear), "linear:ENT-47", &mirror.external_key);
        let plan = connector_remote_write_plan(Some(&linear), &source_issue, &target, &[]);
        let expected_comment_body = connector_linear_remote_comment_body(&source_issue);

        let execution = execute_linear_remote_write_plan(&linear, &plan);
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_LINEAR_GRAPHQL_UPSERT_TOKEN");

        assert_eq!(
            connector_remote_issue_idempotency_key("linear", &old_source_issue),
            connector_remote_issue_idempotency_key("linear", &source_issue)
        );
        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/2/mode")
                .and_then(|value| value.as_str()),
            Some("update_existing_comment")
        );
        assert_eq!(
            execution
                .pointer("/operations/2/matched_comment/id")
                .and_then(|value| value.as_str()),
            Some("comment-47")
        );
        assert_eq!(
            execution
                .pointer("/operations/2/write/response/summary/body")
                .and_then(|value| value.as_str()),
            Some(expected_comment_body.as_str())
        );
        let request_log = requests
            .lock()
            .expect("request log should lock")
            .join("\n---\n");
        assert!(request_log.contains("\"operationName\":\"EntranceIssueRead\""));
        assert!(request_log.contains("\"operationName\":\"EntranceIssueUpdate\""));
        assert!(request_log.contains("\"operationName\":\"EntranceCommentUpdate\""));
        assert!(!request_log.contains("\"operationName\":\"EntranceCommentCreate\""));
    }

    #[test]
    fn linear_remote_write_retries_retryable_http_status() {
        let mut linear =
            test_connector_provider("linear", "Linear", "active", true, true, vec!["linear:"]);
        linear.mode = "remote-issue-api".to_string();
        linear.auth_required = true;
        linear.auth_env = vec!["ENTRANCE_TEST_LINEAR_RETRY_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_LINEAR_RETRY_TOKEN", "test-token");
        let (base_url, requests, server) = spawn_linear_transient_graphql_server();
        let plan = serde_json::json!({
            "schema_version": "entrance.hive.connector_remote_write_plan.v1",
            "remote_object_kind": "linear.issue",
            "operations": [
                {
                    "kind": "update_issue",
                    "method": "POST",
                    "url": base_url,
                    "graphql": {
                        "operation": "EntranceIssueUpdate",
                        "query": "mutation EntranceIssueUpdate($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success issue { id identifier title description url state { id name type } comments { nodes { id body createdAt updatedAt user { name } } } } } }",
                        "variables": {
                            "id": "lin-uuid-49",
                            "input": {
                                "title": "Loop #49: Linear retry"
                            }
                        }
                    }
                }
            ]
        });

        let execution = execute_linear_remote_write_plan(&linear, &plan);
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_LINEAR_RETRY_TOKEN");

        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempt_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/max_attempts")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempts/0/failed_check")
                .and_then(|value| value.as_str()),
            Some("remote_retryable_http_status")
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempts/0/retry/scheduled")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempts/0/retry/backoff_ms")
                .and_then(|value| value.as_u64()),
            Some(100)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/retry/attempted")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/http_status")
                .and_then(|value| value.as_u64()),
            Some(200)
        );
        assert_eq!(requests.lock().expect("request log should lock").len(), 2);
    }

    #[test]
    fn linear_remote_write_reports_rate_limit_without_immediate_retry() {
        let mut linear =
            test_connector_provider("linear", "Linear", "active", true, true, vec!["linear:"]);
        linear.mode = "remote-issue-api".to_string();
        linear.auth_required = true;
        linear.auth_env = vec!["ENTRANCE_TEST_LINEAR_RATE_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_LINEAR_RATE_TOKEN", "test-token");
        let (base_url, requests, server) = spawn_linear_rate_limit_graphql_server();
        let plan = serde_json::json!({
            "schema_version": "entrance.hive.connector_remote_write_plan.v1",
            "remote_object_kind": "linear.issue",
            "operations": [
                {
                    "kind": "update_issue",
                    "method": "POST",
                    "url": base_url,
                    "graphql": {
                        "operation": "EntranceIssueUpdate",
                        "query": "mutation EntranceIssueUpdate($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success issue { id identifier title } } }",
                        "variables": {
                            "id": "lin-uuid-49",
                            "input": {
                                "title": "Loop #49: Linear rate limit"
                            }
                        }
                    }
                }
            ]
        });

        let execution = execute_linear_remote_write_plan(&linear, &plan);
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_LINEAR_RATE_TOKEN");

        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            execution
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("remote_rate_limited")
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempt_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/retry/rate_limited")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/retry/retry_after_secs")
                .and_then(|value| value.as_u64()),
            Some(60)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/response/headers/x_ratelimit_remaining")
                .and_then(|value| value.as_str()),
            Some("0")
        );
        assert_eq!(requests.lock().expect("request log should lock").len(), 1);
    }

    #[test]
    fn github_remote_write_execute_requires_auth_before_network() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        github.auth_required = true;
        github.auth_env = vec!["ENTRANCE_TEST_GITHUB_TOKEN_MISSING".to_string()];
        std::env::remove_var("ENTRANCE_TEST_GITHUB_TOKEN_MISSING");
        let issue = serde_json::json!({
            "id": 43,
            "loop_id": 7,
            "title": "Loop #7: auth gate",
            "status": "Todo",
            "summary": "Do not touch the network without auth.",
            "comment_count": 1,
            "latest_comment": {
                "author": "hive",
                "body": "Need auth first."
            },
            "connector": {
                "external_key": "hive-loop-7-issue-43",
                "current_sha256": "sha-auth"
            }
        });
        let target = connector_remote_target(
            Some(&github),
            "github:owner/repo#43",
            "hive-loop-7-issue-43",
        );
        let plan = connector_remote_write_plan(Some(&github), &issue, &target, &[]);

        let execution = execute_github_remote_write_plan(&github, &plan);

        assert_eq!(
            execution
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_write_execute.v1")
        );
        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            execution
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("remote_auth_missing")
        );
        assert_eq!(
            execution
                .pointer("/operations")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn github_remote_write_retries_retryable_http_status() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        github.auth_required = true;
        github.auth_env = vec!["ENTRANCE_TEST_GITHUB_RETRY_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_GITHUB_RETRY_TOKEN", "test-token");
        let (base_url, requests, server) = spawn_github_transient_write_server();
        let plan = serde_json::json!({
            "schema_version": "entrance.hive.connector_remote_write_plan.v1",
            "remote_object_kind": "github.issue",
            "operations": [
                {
                    "kind": "update_issue",
                    "method": "PATCH",
                    "url": format!("{base_url}/repos/owner/repo/issues/46"),
                    "body": {
                        "title": "Loop #46: GitHub retry",
                        "state": "closed",
                        "state_reason": "completed"
                    }
                }
            ]
        });

        let execution = execute_github_remote_write_plan(&github, &plan);
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_GITHUB_RETRY_TOKEN");

        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempt_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/max_attempts")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempts/0/failed_check")
                .and_then(|value| value.as_str()),
            Some("remote_retryable_http_status")
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempts/0/retry/scheduled")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempts/0/retry/backoff_ms")
                .and_then(|value| value.as_u64()),
            Some(100)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/retry/attempted")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/http_status")
                .and_then(|value| value.as_u64()),
            Some(200)
        );
        assert_eq!(requests.lock().expect("request log should lock").len(), 2);
    }

    #[test]
    fn github_remote_write_reports_rate_limit_without_immediate_retry() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        github.auth_required = true;
        github.auth_env = vec!["ENTRANCE_TEST_GITHUB_RATE_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_GITHUB_RATE_TOKEN", "test-token");
        let (base_url, requests, server) = spawn_github_rate_limit_write_server();
        let plan = serde_json::json!({
            "schema_version": "entrance.hive.connector_remote_write_plan.v1",
            "remote_object_kind": "github.issue",
            "operations": [
                {
                    "kind": "update_issue",
                    "method": "PATCH",
                    "url": format!("{base_url}/repos/owner/repo/issues/46"),
                    "body": {
                        "title": "Loop #46: GitHub rate limit"
                    }
                }
            ]
        });

        let execution = execute_github_remote_write_plan(&github, &plan);
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_GITHUB_RATE_TOKEN");

        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            execution
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("remote_rate_limited")
        );
        assert_eq!(
            execution
                .pointer("/operations/0/attempt_count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/retry/rate_limited")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/retry/retry_after_secs")
                .and_then(|value| value.as_u64()),
            Some(60)
        );
        assert_eq!(
            execution
                .pointer("/operations/0/response/headers/x_ratelimit_remaining")
                .and_then(|value| value.as_str()),
            Some("0")
        );
        assert_eq!(requests.lock().expect("request log should lock").len(), 1);
    }

    #[test]
    fn github_remote_write_upserts_existing_idempotent_comment() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        github.auth_required = true;
        github.auth_env = vec!["ENTRANCE_TEST_GITHUB_UPSERT_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_GITHUB_UPSERT_TOKEN", "test-token");
        let issue = serde_json::json!({
            "id": 44,
            "loop_id": 44,
            "title": "Loop #44: GitHub upsert",
            "status": "Done",
            "summary": "Avoid duplicate connector comments.",
            "updated_at": "2026-01-01T00:04:00Z",
            "comment_count": 1,
            "latest_comment": {
                "id": 101,
                "author": "hive",
                "body": "Need an idempotent comment.",
                "created_at": "2026-01-01T00:03:00Z"
            },
            "connector": {
                "external_key": "hive-loop-44-issue-44",
                "current_sha256": "sha-upsert"
            }
        });
        let existing_comment_body = connector_github_remote_comment_body(&issue)
            .replace("Need an idempotent comment.", "Old copy.");
        let (base_url, requests, server) =
            spawn_github_comment_upsert_server(existing_comment_body);
        github.storage = base_url;
        let target = connector_remote_target(
            Some(&github),
            "github:owner/repo#88",
            "hive-loop-44-issue-44",
        );
        let plan = connector_remote_write_plan(Some(&github), &issue, &target, &[]);

        let execution = execute_github_remote_write_plan(&github, &plan);
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_GITHUB_UPSERT_TOKEN");

        assert_eq!(
            plan.pointer("/operations/1/kind")
                .and_then(|value| value.as_str()),
            Some("upsert_latest_comment")
        );
        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/1/mode")
                .and_then(|value| value.as_str()),
            Some("update_existing_comment")
        );
        assert_eq!(
            execution
                .pointer("/operations/1/method")
                .and_then(|value| value.as_str()),
            Some("PATCH")
        );
        assert_eq!(
            execution
                .pointer("/operations/1/matched_comment/id")
                .and_then(|value| value.as_i64()),
            Some(9001)
        );
        assert_eq!(
            execution
                .pointer("/operations/1/lookup/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            execution
                .pointer("/operations/1/write/success")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        let request_log = requests
            .lock()
            .expect("request log should lock")
            .join("\n---\n");
        assert!(request_log.contains("PATCH /repos/owner/repo/issues/88 "));
        assert!(request_log.contains("GET /repos/owner/repo/issues/88/comments?per_page=100 "));
        assert!(
            request_log.contains("GET /repos/owner/repo/issues/88/comments?per_page=100&page=2 ")
        );
        assert!(request_log.contains("PATCH /repos/owner/repo/issues/comments/9001 "));
        assert!(!request_log.contains("POST /repos/owner/repo/issues/88/comments "));
    }

    #[test]
    fn github_remote_readback_requires_auth_before_network() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        github.auth_required = true;
        github.auth_env = vec!["ENTRANCE_TEST_GITHUB_READBACK_MISSING".to_string()];
        std::env::remove_var("ENTRANCE_TEST_GITHUB_READBACK_MISSING");
        let target =
            connector_remote_target(Some(&github), "github:owner/repo#77", "hive-loop-9-issue-9");

        let execution = execute_github_remote_readback(&github, &target, None);

        assert_eq!(
            execution
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_readback.v1")
        );
        assert_eq!(
            execution
                .pointer("/success")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            execution
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("remote_auth_missing")
        );
        assert_eq!(execution.pointer("/issue"), Some(&serde_json::Value::Null));
    }

    #[test]
    fn github_remote_readback_accepts_issue_and_comment_from_api() {
        let mut github =
            test_connector_provider("github", "GitHub", "active", true, true, vec!["github:"]);
        github.mode = "remote-issue-api".to_string();
        github.auth_required = true;
        github.auth_env = vec!["ENTRANCE_TEST_GITHUB_READBACK_TOKEN".to_string()];
        std::env::set_var("ENTRANCE_TEST_GITHUB_READBACK_TOKEN", "test-token");
        let mirror = IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "github".to_string(),
            review_surface: "github:owner/repo#77".to_string(),
            external_key: "hive-loop-9-issue-9".to_string(),
            issue: HiveIssue {
                id: 9,
                loop_id: Some(9),
                title: "Loop #9: GitHub readback".to_string(),
                status: "Done".to_string(),
                summary: Some("Read back a real GitHub issue surface.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:03:00Z".to_string(),
            },
            loop_contract: Some(HiveLoopContract {
                id: 9,
                title: "GitHub readback".to_string(),
                goal: "Verify GitHub read-after-write.".to_string(),
                boundary: "Use a local HTTP fixture for unit coverage.".to_string(),
                approach_space: vec!["GET issue".to_string(), "GET comments".to_string()],
                eval_space: vec!["readback passes".to_string()],
                review_surface: "github:owner/repo#77".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "codex".to_string(),
                status: "kept".to_string(),
                active_phase: "complete".to_string(),
                current_round: 1,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:03:00Z".to_string(),
            }),
            comments: vec![HiveComment {
                id: 31,
                issue_id: 9,
                author: "hive".to_string(),
                body: "Evaluator kept the GitHub readback candidate.".to_string(),
                payload: serde_json::json!({
                    "schema_version": "entrance.hive.system_comment.v1",
                    "source": "hive"
                }),
                created_at: "2026-01-01T00:02:00Z".to_string(),
            }],
            actions: vec![],
            trace: None,
            doctor: None,
        };
        let digest = digest_bytes(&mirror_payload(&mirror).expect("mirror should serialize"));
        let source_issue = connector_remote_write_issue_from_mirror(&mirror, &digest);
        let issue_body = connector_remote_issue_body(&source_issue);
        let comment_body = connector_github_remote_comment_body(&source_issue);
        let (base_url, server) = spawn_github_readback_server(issue_body, comment_body);
        github.storage = base_url.clone();
        let target =
            connector_remote_target(Some(&github), "github:owner/repo#77", "hive-loop-9-issue-9");
        let expected_api_url = format!("{base_url}/repos/owner/repo/issues/77");
        assert_eq!(
            target.pointer("/api_url").and_then(|value| value.as_str()),
            Some(expected_api_url.as_str())
        );
        let path = Path::new("/tmp/root/connectors/github/hive-loop-9-issue-9.json");
        let receipt_path = mirror_receipt_path(path);
        let receipt = issue_mirror_sync_receipt_for_provider(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&github),
        );

        let readback = compact_github_issue_mirror_readback(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&receipt),
            &github,
        );
        server.join().expect("test server should finish");
        std::env::remove_var("ENTRANCE_TEST_GITHUB_READBACK_TOKEN");

        assert_eq!(
            readback
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            readback
                .pointer("/remote/surface/issue/number")
                .and_then(|value| value.as_u64()),
            Some(77)
        );
        assert_eq!(
            readback
                .pointer("/remote/surface/comments/count")
                .and_then(|value| value.as_u64()),
            Some(1)
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/execution/comments/pagination/page_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/execution/comments/response/summary/page_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/checks/9/name")
                .and_then(|value| value.as_str()),
            Some("write_receipt_binding")
        );
        let status = compact_issue_mirror_status(&readback);
        assert_eq!(
            status.pointer("/current").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            status
                .pointer("/publish_required")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn compact_connector_admission_preview_exposes_checks_and_contract() {
        let report = serde_json::json!({
            "schema_version": "entrance.hive.issue_connector_admission_preview.v1",
            "issue": {"id": 9, "loop_id": 4},
            "provider_name": "linear",
            "provider": {
                "name": "linear",
                "status": "planned",
                "configured": false
            },
            "provider_admission": {
                "status": "blocked",
                "blockers": ["provider_not_active"]
            },
            "review_surface": "linear:ENT-9",
            "connector": {
                "current": false,
                "publish_required": true,
                "reason": "mirror_file_missing"
            },
            "adapter": {
                "driver": "unavailable",
                "blockers": ["provider_not_active", "connector_not_configured", "publish_not_supported"]
            },
            "remote_contract": {
                "schema_version": "entrance.hive.connector_remote_contract.v1",
                "remote_object_kind": "linear.issue"
            },
            "writer_blockers": ["provider_not_active", "connector_not_configured", "publish_not_supported"],
            "checks": [{
                "name": "remote_write_contract_ready",
                "passed": false,
                "owner": "remote-contract",
                "severity": "blocker",
                "required_evidence": ["connector_writer_adapter", "connector_remote_contract"],
                "policy_summary": "Remote writer/readback contract must be ready."
            }],
            "decision": {
                "admissible": false,
                "blockers": ["provider_not_active", "publish_not_supported"]
            },
            "policy": {
                "gate": "connector_mirror_receipt",
                "expected_object_kind": "ISSUE_CONNECTOR_MIRROR_RECEIPT",
                "required_checks": test_connector_admission_required_checks(),
                "check_registry": test_connector_admission_check_registry()
            },
            "commands": {}
        });

        let compact = compact_issue_connector_admission_preview(&report);

        assert_eq!(
            compact
                .pointer("/adapter/driver")
                .and_then(|value| value.as_str()),
            Some("unavailable")
        );
        assert_eq!(
            compact
                .pointer("/remote_contract/remote_object_kind")
                .and_then(|value| value.as_str()),
            Some("linear.issue")
        );
        assert_eq!(
            compact
                .pointer("/checks/0/name")
                .and_then(|value| value.as_str()),
            Some("remote_write_contract_ready")
        );
        assert_eq!(
            compact
                .pointer("/checks/0/owner")
                .and_then(|value| value.as_str()),
            Some("remote-contract")
        );
        assert_eq!(
            compact
                .pointer("/writer_blockers/2")
                .and_then(|value| value.as_str()),
            Some("publish_not_supported")
        );
        assert_eq!(
            compact
                .pointer("/required_checks/6")
                .and_then(|value| value.as_str()),
            Some("retry_policy_bound")
        );
        assert_eq!(
            compact
                .pointer("/check_registry/6/owner")
                .and_then(|value| value.as_str()),
            Some("retry-policy")
        );
    }

    #[test]
    fn compact_connector_publish_plan_is_digest_bound() {
        let registry = test_connector_registry();
        let issues = vec![serde_json::json!({
            "id": 7,
            "loop_id": 3,
            "title": "Loop #3: connector stale",
            "status": "Done",
            "connector": {
                "publish_required": true,
                "current": false,
                "provider": "file",
                "review_surface": "file:local-board",
                "reason": "mirror_stale",
                "path": "/tmp/issue.json",
                "current_sha256": "sha-before",
                "remote_sha256": "sha-remote",
                "current_comment_count": 4,
                "remote_comment_count": 3,
                "failed_checks": ["remote_digest_current"],
                "publish_command": "entrance hive issue mirror-publish 7 --compact",
                "readback_command": "entrance hive issue mirror-readback 7 --record --compact",
                "roundtrip_command": "entrance hive issue mirror-roundtrip 7 --compact"
            }
        })];
        let queue = compact_connector_queue(&registry, &issues, None);
        let plan = compact_connector_publish_plan(&queue).expect("plan should render");

        assert_eq!(
            plan.pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_publish_plan.v1")
        );
        assert_eq!(
            plan.pointer("/can_execute")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            plan.pointer("/issues/0/current_sha256")
                .and_then(|value| value.as_str()),
            Some("sha-before")
        );
        assert!(plan
            .pointer("/commands/execute")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .starts_with("entrance hive connector publish-execute --plan-id "));

        let changed_issues = vec![serde_json::json!({
            "id": 7,
            "loop_id": 3,
            "title": "Loop #3: connector stale",
            "status": "Done",
            "connector": {
                "publish_required": true,
                "current": false,
                "provider": "file",
                "review_surface": "file:local-board",
                "reason": "mirror_stale",
                "path": "/tmp/issue.json",
                "current_sha256": "sha-after",
                "remote_sha256": "sha-remote",
                "current_comment_count": 5,
                "remote_comment_count": 3,
                "failed_checks": ["remote_digest_current"],
                "publish_command": "entrance hive issue mirror-publish 7 --compact",
                "readback_command": "entrance hive issue mirror-readback 7 --record --compact",
                "roundtrip_command": "entrance hive issue mirror-roundtrip 7 --compact"
            }
        })];
        let changed_queue = compact_connector_queue(&registry, &changed_issues, None);
        let changed_plan =
            compact_connector_publish_plan(&changed_queue).expect("changed plan should render");
        assert_ne!(
            plan.pointer("/plan_id").and_then(|value| value.as_str()),
            changed_plan
                .pointer("/plan_id")
                .and_then(|value| value.as_str())
        );
    }

    #[test]
    fn compact_connector_roundtrip_plan_is_digest_bound_and_gated() {
        let registry = test_connector_registry();
        let issues = vec![serde_json::json!({
            "id": 7,
            "loop_id": 3,
            "title": "Loop #3: connector stale",
            "status": "Done",
            "connector": {
                "publish_required": true,
                "current": false,
                "provider": "file",
                "review_surface": "file:local-board",
                "reason": "mirror_stale",
                "path": "/tmp/issue.json",
                "current_sha256": "sha-before",
                "remote_sha256": "sha-remote",
                "current_comment_count": 4,
                "remote_comment_count": 3,
                "failed_checks": ["remote_digest_current"],
                "publish_command": "entrance hive issue mirror-publish 7 --compact",
                "readback_command": "entrance hive issue mirror-readback 7 --record --compact",
                "admit_command": "entrance hive issue mirror-admit 7 --record --compact",
                "roundtrip_command": "entrance hive issue mirror-roundtrip 7 --compact"
            }
        })];
        let queue = compact_connector_queue(&registry, &issues, None);
        let plan = compact_connector_roundtrip_plan(&queue).expect("roundtrip plan should render");

        assert_eq!(
            plan.pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_roundtrip_plan.v1")
        );
        assert_eq!(
            plan.pointer("/can_execute")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            plan.pointer("/issues/0/commands/roundtrip")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-roundtrip 7 --compact")
        );
        assert_eq!(
            plan.pointer("/issues/0/dry_run_action/action")
                .and_then(|value| value.as_str()),
            Some("roundtrip")
        );
        assert!(plan
            .pointer("/commands/execute")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .starts_with("entrance hive connector roundtrip-execute --plan-id "));

        let changed_issues = vec![serde_json::json!({
            "id": 7,
            "loop_id": 3,
            "title": "Loop #3: connector stale",
            "status": "Done",
            "connector": {
                "publish_required": true,
                "current": false,
                "provider": "file",
                "review_surface": "file:local-board",
                "reason": "mirror_stale",
                "path": "/tmp/issue.json",
                "current_sha256": "sha-after",
                "remote_sha256": "sha-remote",
                "current_comment_count": 5,
                "remote_comment_count": 3,
                "failed_checks": ["remote_digest_current"],
                "publish_command": "entrance hive issue mirror-publish 7 --compact",
                "readback_command": "entrance hive issue mirror-readback 7 --record --compact",
                "admit_command": "entrance hive issue mirror-admit 7 --record --compact",
                "roundtrip_command": "entrance hive issue mirror-roundtrip 7 --compact"
            }
        })];
        let changed_queue = compact_connector_queue(&registry, &changed_issues, None);
        let changed_plan = compact_connector_roundtrip_plan(&changed_queue)
            .expect("changed roundtrip plan should render");
        assert_ne!(
            plan.pointer("/plan_id").and_then(|value| value.as_str()),
            changed_plan
                .pointer("/plan_id")
                .and_then(|value| value.as_str())
        );

        let blocked_issues = vec![serde_json::json!({
            "id": 9,
            "loop_id": 5,
            "title": "Loop #5: connector linear planned",
            "status": "Blocked",
            "connector": {
                "publish_required": true,
                "current": false,
                "provider": "linear",
                "review_surface": "linear:ENT-9",
                "reason": "mirror_file_missing",
                "path": "/tmp/linear.json",
                "current_sha256": "sha-linear-current",
                "current_comment_count": 2,
                "failed_checks": ["remote_file_present"],
                "publish_command": "entrance hive issue mirror-publish 9 --compact",
                "readback_command": "entrance hive issue mirror-readback 9 --record --compact",
                "admit_command": "entrance hive issue mirror-admit 9 --record --compact",
                "roundtrip_command": "entrance hive issue mirror-roundtrip 9 --compact"
            }
        })];
        let blocked_queue = compact_connector_queue(&registry, &blocked_issues, Some("linear"));
        let blocked_plan = compact_connector_roundtrip_plan(&blocked_queue)
            .expect("blocked roundtrip plan should render");
        assert_eq!(
            blocked_plan
                .pointer("/can_execute")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            blocked_plan
                .pointer("/commands/execute")
                .and_then(|value| value.as_str()),
            None
        );
        let blockers = blocked_plan
            .pointer("/blockers")
            .and_then(|value| value.as_array())
            .expect("blocked plan blockers should be present")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(blockers.contains(&"provider_not_active"));
        assert!(blockers.contains(&"publish_not_supported"));
        assert!(blockers.contains(&"readback_not_supported"));
        assert!(blockers.contains(&"admission_not_supported"));
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
    fn provider_storage_templates_resolve_mirror_paths() {
        let mut provider =
            test_connector_provider("file", "File Mirror", "active", true, true, vec!["file:"]);
        provider.storage = "connectors/custom/{external_key}.json".to_string();
        assert_eq!(
            default_issue_mirror_path_for_provider(
                Path::new("/tmp/root"),
                "file:Loop/7?",
                Some(&provider),
            ),
            Path::new("/tmp/root/connectors/custom/file-Loop-7.json")
        );

        provider.storage = "connectors/custom/*.json".to_string();
        assert_eq!(
            default_issue_mirror_path_for_provider(
                Path::new("/tmp/root"),
                "file:Loop/7?",
                Some(&provider),
            ),
            Path::new("/tmp/root/connectors/custom/file-Loop-7.json")
        );

        provider.storage = "not-configured".to_string();
        assert_eq!(
            default_issue_mirror_path_for_provider(
                Path::new("/tmp/root"),
                "file:Loop/7?",
                Some(&provider),
            ),
            Path::new("/tmp/root/connectors/issue-mirrors/file-Loop-7.json")
        );

        provider.storage = "https://api.github.example".to_string();
        assert_eq!(
            default_issue_mirror_path_for_provider(
                Path::new("/tmp/root"),
                "github:owner/repo#7",
                Some(&provider),
            ),
            Path::new("/tmp/root/connectors/issue-mirrors/github-owner-repo-7.json")
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
            Path::new("/tmp/root/connectors/issue-mirrors/hive-loop-1-issue-2.receipt.json"),
            &MirrorFileDigest {
                bytes: 512,
                sha256: "abc123".to_string(),
            },
            None,
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
            report.pointer("/sha256").and_then(|value| value.as_str()),
            Some("abc123")
        );
        assert_eq!(
            report
                .pointer("/receipt_path")
                .and_then(|value| value.as_str()),
            Some("/tmp/root/connectors/issue-mirrors/hive-loop-1-issue-2.receipt.json")
        );
        assert_eq!(
            report
                .pointer("/sync_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-sync 2")
        );
        assert_eq!(
            report
                .pointer("/publish_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-publish 2 --compact")
        );
        assert_eq!(
            report
                .pointer("/verify_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-verify 2")
        );

        let publish = compact_issue_mirror_publish(&report);
        assert_eq!(
            publish
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_publish.v1")
        );
        assert_eq!(
            publish
                .pointer("/published")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            publish
                .pointer("/publish_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-publish 2 --compact")
        );
        assert_eq!(
            publish
                .pointer("/sync/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_sync.v1")
        );
    }

    #[test]
    fn remote_fixture_receipts_satisfy_remote_contract_readback() {
        let provider = test_connector_provider(
            "remote-fixture",
            "Remote Fixture",
            "active",
            true,
            true,
            vec!["remote-fixture:"],
        );
        let mirror = IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "remote-fixture".to_string(),
            review_surface: "remote-fixture:local".to_string(),
            external_key: "hive-loop-6-issue-10".to_string(),
            issue: HiveIssue {
                id: 10,
                loop_id: Some(6),
                title: "Loop #6: remote fixture".to_string(),
                status: "Done".to_string(),
                summary: Some("Remote fixture ready.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:05:00Z".to_string(),
            },
            loop_contract: Some(HiveLoopContract {
                id: 6,
                title: "remote fixture".to_string(),
                goal: "Validate remote contract fixture".to_string(),
                boundary: "No third-party writes".to_string(),
                approach_space: vec!["remote fixture".to_string()],
                eval_space: vec!["readback passes".to_string()],
                review_surface: "remote-fixture:local".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "codex".to_string(),
                status: "kept".to_string(),
                active_phase: "complete".to_string(),
                current_round: 1,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:05:00Z".to_string(),
            }),
            comments: vec![HiveComment {
                id: 44,
                issue_id: 10,
                author: "hive".to_string(),
                body: "Remote fixture comment.".to_string(),
                payload: serde_json::json!({
                    "schema_version": "entrance.hive.system_comment.v1",
                    "source": "hive"
                }),
                created_at: "2026-01-01T00:04:00Z".to_string(),
            }],
            actions: vec![],
            trace: None,
            doctor: None,
        };
        let path = Path::new("/tmp/root/connectors/remote-fixture/hive-loop-6-issue-10.json");
        let receipt_path = mirror_receipt_path(path);
        let digest = MirrorFileDigest {
            bytes: 4096,
            sha256: "remote-fixture-sha".to_string(),
        };

        let sync =
            compact_issue_mirror_sync(&mirror, path, &receipt_path, &digest, Some(&provider));
        assert_eq!(
            sync.pointer("/remote_contract/remote_object_kind")
                .and_then(|value| value.as_str()),
            Some("fixture.issue")
        );
        assert_eq!(
            sync.pointer("/remote_write_receipt/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_write_receipt.v1")
        );
        assert_eq!(
            sync.pointer("/remote_write_receipt/source_mirror_sha256")
                .and_then(|value| value.as_str()),
            Some("remote-fixture-sha")
        );

        let write_receipt = connector_write_receipt(&mirror, &sync, Some(&provider));
        assert_eq!(
            write_receipt
                .pointer("/remote_write_receipt/remote_object_kind")
                .and_then(|value| value.as_str()),
            Some("fixture.issue")
        );

        let receipt = issue_mirror_sync_receipt_for_provider(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&provider),
        );
        let verify = compact_issue_mirror_verify(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&digest),
            Some(&receipt),
        );
        let readback = compact_issue_mirror_readback(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&digest),
            Some(&receipt),
            Some(&mirror),
            None,
            &verify,
            Some(&provider),
        );

        assert_eq!(
            readback
                .pointer("/remote_readback/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_readback.v1")
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            readback
                .pointer("/remote_readback/checks/3/name")
                .and_then(|value| value.as_str()),
            Some("write_receipt_binding")
        );
        let compact = compact_issue_mirror_readback_summary(&readback);
        assert_eq!(
            compact
                .pointer("/remote_readback_passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            compact
                .pointer("/remote_write_receipt_schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.connector_remote_write_receipt.v1")
        );
    }

    #[test]
    fn compact_roundtrip_summary_tracks_publish_readback_admit_sequence() {
        let publish = serde_json::json!({
            "schema_version": "entrance.hive.issue_mirror_publish.v1",
            "published": true,
            "issue_id": 8,
            "provider": "remote-fixture",
            "review_surface": "remote-fixture:ENT-36",
            "external_key": "hive-loop-8-issue-8",
            "path": "/tmp/root/connectors/remote-fixture/hive-loop-8-issue-8.json",
            "sha256": "sha-initial"
        });
        let readback = serde_json::json!({
            "schema_version": "entrance.hive.issue_mirror_readback.v1",
            "passed": true,
            "issue_id": 8,
            "provider": "remote-fixture",
            "review_surface": "remote-fixture:ENT-36",
            "external_key": "hive-loop-8-issue-8",
            "current": {
                "digest": {
                    "sha256": "sha-initial"
                }
            },
            "remote_readback": {
                "schema_version": "entrance.hive.connector_remote_readback.v1",
                "remote_object_kind": "fixture.issue",
                "write_receipt": {
                    "schema_version": "entrance.hive.connector_remote_write_receipt.v1"
                }
            },
            "recorded": {
                "comment_id": 12,
                "evidence_id": 31,
                "publish": {
                    "required": true
                }
            }
        });
        let publish_after_readback = serde_json::json!({
            "schema_version": "entrance.hive.issue_mirror_publish.v1",
            "published": true,
            "issue_id": 8,
            "provider": "remote-fixture",
            "review_surface": "remote-fixture:ENT-36",
            "external_key": "hive-loop-8-issue-8",
            "path": "/tmp/root/connectors/remote-fixture/hive-loop-8-issue-8.json",
            "sha256": "sha-after-readback"
        });
        let admission = serde_json::json!({
            "schema_version": "entrance.hive.issue_mirror_admission.v1",
            "admitted": true,
            "issue_id": 8,
            "provider": "remote-fixture",
            "review_surface": "remote-fixture:ENT-36",
            "external_key": "hive-loop-8-issue-8",
            "receipt": {
                "sha256": "sha-after-readback",
                "path": "/tmp/root/connectors/remote-fixture/hive-loop-8-issue-8.json"
            },
            "recorded": {
                "comment_id": 13,
                "evidence_id": 32,
                "publish": {
                    "required": true
                }
            }
        });
        let publish_after_admission = serde_json::json!({
            "schema_version": "entrance.hive.issue_mirror_publish.v1",
            "published": true,
            "issue_id": 8,
            "provider": "remote-fixture",
            "review_surface": "remote-fixture:ENT-36",
            "external_key": "hive-loop-8-issue-8",
            "path": "/tmp/root/connectors/remote-fixture/hive-loop-8-issue-8.json",
            "sha256": "sha-after-admission"
        });
        let final_readback = serde_json::json!({
            "schema_version": "entrance.hive.issue_mirror_readback.v1",
            "passed": true,
            "issue_id": 8,
            "provider": "remote-fixture",
            "review_surface": "remote-fixture:ENT-36",
            "external_key": "hive-loop-8-issue-8",
            "current": {
                "digest": {
                    "sha256": "sha-after-admission"
                }
            },
            "remote_readback": {
                "schema_version": "entrance.hive.connector_remote_readback.v1",
                "remote_object_kind": "fixture.issue",
                "write_receipt": {
                    "schema_version": "entrance.hive.connector_remote_write_receipt.v1"
                }
            }
        });
        let stages = vec![
            issue_mirror_roundtrip_stage("publish_initial", "publish", true, &publish),
            issue_mirror_roundtrip_stage("readback", "readback", true, &readback),
            issue_mirror_roundtrip_stage(
                "publish_after_readback",
                "publish evidence",
                true,
                &publish_after_readback,
            ),
            issue_mirror_roundtrip_stage("admit", "admit", true, &admission),
            issue_mirror_roundtrip_stage(
                "publish_after_admission",
                "publish admission",
                true,
                &publish_after_admission,
            ),
            issue_mirror_roundtrip_stage("final_readback", "final readback", true, &final_readback),
        ];

        let report = compact_issue_mirror_roundtrip(
            8,
            true,
            stages,
            publish,
            readback,
            publish_after_readback,
            admission,
            publish_after_admission,
            final_readback,
        );
        let compact = compact_issue_mirror_roundtrip_summary(&report);

        assert_eq!(
            compact
                .pointer("/completed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            compact
                .pointer("/stage_count")
                .and_then(|value| value.as_u64()),
            Some(6)
        );
        assert_eq!(
            compact
                .pointer("/recorded_evidence_ids")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            compact
                .pointer("/remote_object_kind")
                .and_then(|value| value.as_str()),
            Some("fixture.issue")
        );
        assert_eq!(
            compact
                .pointer("/commands/roundtrip")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-roundtrip 8 --compact")
        );
    }

    #[test]
    fn issue_mirror_verify_detects_receipt_drift() {
        let mirror = IssueMirrorReport {
            schema_version: "entrance.hive.issue_mirror.v1".to_string(),
            provider: "file".to_string(),
            review_surface: "file:local-board".to_string(),
            external_key: "hive-loop-5-issue-8".to_string(),
            issue: HiveIssue {
                id: 8,
                loop_id: Some(5),
                title: "Loop #5: mirror verify".to_string(),
                status: "Done".to_string(),
                summary: Some("Mirror verified.".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:03:00Z".to_string(),
            },
            loop_contract: Some(HiveLoopContract {
                id: 5,
                title: "mirror verify".to_string(),
                goal: "Verify mirror receipt binding".to_string(),
                boundary: "No remote writes".to_string(),
                approach_space: vec!["file sink".to_string()],
                eval_space: vec!["verify passes".to_string()],
                review_surface: "file:local-board".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "codex".to_string(),
                status: "kept".to_string(),
                active_phase: "complete".to_string(),
                current_round: 3,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:03:00Z".to_string(),
            }),
            comments: vec![],
            actions: vec![],
            trace: None,
            doctor: None,
        };
        let path = Path::new("/tmp/root/connectors/issue-mirrors/hive-loop-5-issue-8.json");
        let receipt_path = mirror_receipt_path(path);
        let digest = MirrorFileDigest {
            bytes: 2048,
            sha256: "receipt-sha".to_string(),
        };
        let receipt = issue_mirror_sync_receipt(&mirror, path, &receipt_path, &digest);

        let report = compact_issue_mirror_verify(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&digest),
            Some(&receipt),
        );

        assert_eq!(
            report.pointer("/passed").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report
                .pointer("/loop_round")
                .and_then(|value| value.as_i64()),
            Some(3)
        );
        assert_eq!(
            report
                .pointer("/receipt/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_sync_receipt.v1")
        );

        let readback = compact_issue_mirror_readback(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&digest),
            Some(&receipt),
            Some(&mirror),
            None,
            &report,
            None,
        );
        assert_eq!(
            readback
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_readback.v1")
        );
        assert_eq!(
            readback
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            readback
                .pointer("/current/comments/count")
                .and_then(|value| value.as_u64()),
            Some(0)
        );
        assert_eq!(
            readback
                .pointer("/actions/1/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-readback 8 --record --compact")
        );
        let current_status = compact_issue_mirror_status(&readback);
        assert_eq!(
            current_status
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_status.v1")
        );
        assert_eq!(
            current_status
                .pointer("/current")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            current_status
                .pointer("/publish_required")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            current_status
                .pointer("/publish_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-publish 8 --compact")
        );
        assert_eq!(
            current_status
                .pointer("/checks")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(6)
        );
        assert_eq!(
            current_status
                .pointer("/checks/0/name")
                .and_then(|value| value.as_str()),
            Some("remote_file_present")
        );
        let mut recorded_readback = readback.clone();
        recorded_readback
            .as_object_mut()
            .expect("readback should be an object")
            .insert(
                "recorded".to_string(),
                serde_json::json!({
                    "schema_version": "entrance.hive.issue_mirror_readback_record.v1",
                    "comment_id": 17,
                    "evidence_id": 23,
                    "comment_body": "Connector readback current: external issue surface matches Hive.",
                    "publish": {
                        "schema_version": "entrance.hive.connector_publish_hint.v1",
                        "required": true,
                        "reason": "record_created_local_issue_event",
                        "command": "entrance hive issue mirror-publish 8 --compact"
                    }
                }),
            );
        let compact_readback = compact_issue_mirror_readback_summary(&recorded_readback);
        assert_eq!(
            compact_readback
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_readback.compact.v1")
        );
        assert_eq!(
            compact_readback
                .pointer("/remote_comment_count")
                .and_then(|value| value.as_u64()),
            Some(0)
        );
        assert_eq!(
            compact_readback
                .pointer("/recorded_comment_id")
                .and_then(|value| value.as_i64()),
            Some(17)
        );
        assert_eq!(
            compact_readback
                .pointer("/recorded_evidence_id")
                .and_then(|value| value.as_i64()),
            Some(23)
        );
        assert_eq!(
            compact_readback
                .pointer("/publish_required")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            compact_readback
                .pointer("/publish_command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-publish 8 --compact")
        );

        let mut stale_remote = mirror.clone();
        stale_remote.comments.push(HiveComment {
            id: 91,
            issue_id: 8,
            author: "human".to_string(),
            body: "Remote surface drifted.".to_string(),
            payload: serde_json::json!({
                "schema_version": "entrance.hive.operator_comment.v1",
                "source": "operator"
            }),
            created_at: "2026-01-01T00:05:00Z".to_string(),
        });
        let stale_digest = MirrorFileDigest {
            bytes: 4096,
            sha256: "stale-sha".to_string(),
        };
        let stale_verify = compact_issue_mirror_verify(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&stale_digest),
            Some(&receipt),
        );
        let stale_readback = compact_issue_mirror_readback(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&stale_digest),
            Some(&receipt),
            Some(&stale_remote),
            None,
            &stale_verify,
            None,
        );
        assert_eq!(
            stale_readback
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        let readback_failures = stale_readback
            .pointer("/failed_checks")
            .and_then(|value| value.as_array())
            .expect("readback failed_checks should be an array");
        assert!(readback_failures
            .iter()
            .any(|value| value.as_str() == Some("remote_digest_current")));
        assert!(readback_failures
            .iter()
            .any(|value| value.as_str() == Some("remote_comment_surface")));
        let stale_status = compact_issue_mirror_status(&stale_readback);
        assert_eq!(
            stale_status
                .pointer("/current")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            stale_status
                .pointer("/publish_required")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            stale_status
                .pointer("/reason")
                .and_then(|value| value.as_str()),
            Some("mirror_stale")
        );

        let mut drifted = receipt;
        *drifted
            .pointer_mut("/issue/status")
            .expect("receipt issue status should be mutable") = serde_json::json!("Blocked");
        let drift_report = compact_issue_mirror_verify(
            &mirror,
            path,
            &receipt_path,
            &digest,
            Some(&digest),
            Some(&drifted),
        );

        assert_eq!(
            drift_report
                .pointer("/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        let failures = drift_report
            .pointer("/failures")
            .and_then(|value| value.as_array())
            .expect("failures should be an array");
        assert!(failures
            .iter()
            .any(|value| value.as_str() == Some("receipt_issue_status_mismatch")));

        let audit = compact_issue_mirror_audit(&drift_report);
        assert_eq!(
            audit
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_audit.v1")
        );
        assert_eq!(
            audit.pointer("/passed").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            audit
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("receipt_binding")
        );
        assert_eq!(
            audit.pointer("/gate/gate").and_then(|value| value.as_str()),
            Some("connector_mirror_receipt_current")
        );
        assert_eq!(
            audit
                .pointer("/actions/0/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-publish 8 --compact")
        );

        let compact = compact_issue_mirror_audit_summary(&audit);
        assert_eq!(
            compact
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_audit.compact.v1")
        );
        assert_eq!(
            compact
                .pointer("/failed_checks/0")
                .and_then(|value| value.as_str()),
            Some("receipt_binding")
        );
        assert_eq!(
            compact.pointer("/gate").and_then(|value| value.as_str()),
            Some("connector_mirror_receipt_current")
        );

        let rejected_admission = compact_issue_mirror_admission(&audit);
        assert_eq!(
            rejected_admission
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_admission.v1")
        );
        assert_eq!(
            rejected_admission
                .pointer("/object_kind")
                .and_then(|value| value.as_str()),
            Some("ISSUE_CONNECTOR_ADMISSION")
        );
        assert_eq!(
            rejected_admission
                .pointer("/admitted")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            rejected_admission
                .pointer("/decision/route_to")
                .and_then(|value| value.as_str()),
            Some("operator")
        );
        assert_eq!(
            rejected_admission
                .pointer("/receipt/object_kind")
                .and_then(|value| value.as_str()),
            Some("ISSUE_MIRROR_SYNC_RECEIPT")
        );
        assert_eq!(
            rejected_admission
                .pointer("/actions/2/command")
                .and_then(|value| value.as_str()),
            Some("entrance hive issue mirror-admit 8 --compact")
        );
        assert_eq!(
            rejected_admission
                .pointer("/decision/human_options/0")
                .and_then(|value| value.as_str()),
            Some("publish")
        );

        let admitted_audit = compact_issue_mirror_audit(&report);
        let admitted = compact_issue_mirror_admission(&admitted_audit);
        assert_eq!(
            admitted
                .pointer("/admitted")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            admitted
                .pointer("/decision/route_to")
                .and_then(|value| value.as_str()),
            Some("external_issue_surface")
        );
        assert_eq!(
            admitted
                .pointer("/policy/gate")
                .and_then(|value| value.as_str()),
            Some("connector_mirror_receipt_current")
        );

        let compact_admission = compact_issue_mirror_admission_summary(&admitted);
        assert_eq!(
            compact_admission
                .pointer("/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_mirror_admission.compact.v1")
        );
        assert_eq!(
            compact_admission
                .pointer("/route_to")
                .and_then(|value| value.as_str()),
            Some("external_issue_surface")
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
