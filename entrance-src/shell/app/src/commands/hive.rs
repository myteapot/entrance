use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use entrance_core::{HiveCommentCreate, HiveLoopEvidenceCreate};
use entrance_hive::{
    ConnectorProviderAdmissionSpec, ConnectorProviderSpec, ConnectorRegistryReport,
    HiveCallbackRequest, HiveDispatchRequest, HiveLoopAuditCheck, HiveLoopAuditReport,
    HiveLoopCreateRequest, HiveLoopRunRequest, IssueAction, IssueCard, IssueCommentRequest,
    IssueDecisionRequest, IssueMirrorReport, IssueRunRequest, PolicyGateSpec, PolicyRegistryReport,
    ReviewDecision, CONNECTOR_MIRROR_RECEIPT_GATE, CONNECTOR_MIRROR_RECEIPT_OBJECT_KIND,
};
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
const CONNECTOR_PUBLISH_HINT_SCHEMA_VERSION: &str = "entrance.hive.connector_publish_hint.v1";
const ISSUE_CONNECTOR_ADMISSION_PREVIEW_SCHEMA_VERSION: &str =
    "entrance.hive.issue_connector_admission_preview.v1";
const ISSUE_CONNECTOR_ADMISSION_OBJECT_KIND: &str = "ISSUE_CONNECTOR_ADMISSION";
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
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>\n  entrance hive loop create --title <text> --goal <text> [--runtime local|codex] [--compact]\n  entrance hive loop run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive loop show <id>\n  entrance hive loop trace <id>\n  entrance hive loop evidence <id>\n  entrance hive loop audit <id> [--compact]\n  entrance hive loop doctor <id>\n  entrance hive loop policies <id>\n  entrance hive loop list\n  entrance hive policy registry [--compact]\n  entrance hive connector registry [--compact]\n  entrance hive connector queue [--provider <name>] [--compact]\n  entrance hive issue list [--compact]\n  entrance hive issue show <id> [--compact]\n  entrance hive issue connector-admission <id> [--path <path>] [--compact]\n  entrance hive issue mirror <id> [--compact]\n  entrance hive issue mirror-sync <id> [--out <path>]\n  entrance hive issue mirror-publish <id> [--path <path>] [--compact]\n  entrance hive issue mirror-status <id> [--path <path>] [--compact]\n  entrance hive issue mirror-verify <id> [--path <path>]\n  entrance hive issue mirror-audit <id> [--path <path>] [--compact]\n  entrance hive issue mirror-readback <id> [--path <path>] [--record] [--compact]\n  entrance hive issue mirror-admit <id> [--path <path>] [--record] [--compact]\n  entrance hive issue comment <id> --body <text> [--compact]\n  entrance hive issue decide <id> <retry|request-review|cancel> [--body <text>] [--compact]\n  entrance hive issue run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]\n  entrance hive issue retry-run <id> [--body <text>] [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>] [--compact]"
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

fn csv_values(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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
    serde_json::json!({
        "schema_version": "entrance.hive.issue.compact.v1",
        "issue": compact_issue_card(card),
        "recent_comments": compact_recent_comments(card, 5),
        "recent_evidence": compact_recent_evidence(card, 5),
        "stages": compact_stage_rows(card)
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
    serde_json::json!({
        "schema_version": "entrance.hive.issue.compact.v1",
        "issue": issue,
        "connector": connector,
        "recent_comments": compact_recent_comments(card, 5),
        "recent_evidence": compact_recent_evidence(card, 5),
        "stages": compact_stage_rows(card)
    })
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
        "admissible": report.pointer("/decision/admissible").and_then(|value| value.as_bool()),
        "route_to": report.pointer("/decision/route_to").and_then(|value| value.as_str()),
        "blockers": report.pointer("/decision/blockers").and_then(|value| value.as_array()).cloned().unwrap_or_default(),
        "gate": report.pointer("/policy/gate").and_then(|value| value.as_str()),
        "expected_object_kind": report.pointer("/policy/expected_object_kind").and_then(|value| value.as_str()),
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
    let path = out_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_issue_mirror_path(&services.kernel.root, &mirror.external_key));
    let receipt_path = mirror_receipt_path(&path);
    let digest = write_issue_mirror_file(&mirror, &path)?;
    write_issue_mirror_receipt(&mirror, &path, &receipt_path, &digest)?;
    Ok(compact_issue_mirror_sync(
        &mirror,
        &path,
        &receipt_path,
        &digest,
    ))
}

pub(crate) fn publish_issue_mirror_to_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let sync = sync_issue_mirror_to_file(services, issue_id, path)?;
    Ok(compact_issue_mirror_publish(&sync))
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
    let mut blockers = Vec::new();
    if provider.is_none() {
        blockers.push("unsupported_provider".to_string());
    }
    if let Some(provider_admission) = provider_admission {
        blockers.extend(provider_admission.blockers.iter().cloned());
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
        "policy": {
            "schema_version": registry.admission.schema_version,
            "gate": registry.admission.gate,
            "route_to": registry.admission.route_to,
            "expected_object_kind": registry.admission.expected_object_kind,
            "check": registry.admission.check,
            "required_receipts": registry.admission.required_receipts
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
            "admit": format!("entrance hive issue mirror-admit {issue_id} --record --compact")
        }
    }))
}

pub(crate) fn verify_issue_mirror_file(
    services: &AppServices,
    issue_id: i64,
    path: Option<&str>,
) -> Result<serde_json::Value> {
    let mirror = services.hive.issue_mirror(issue_id)?;
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_issue_mirror_path(&services.kernel.root, &mirror.external_key));
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
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_issue_mirror_path(&services.kernel.root, &mirror.external_key));
    let receipt_path = mirror_receipt_path(&path);
    let expected_digest = digest_bytes(&mirror_payload(&mirror)?);
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

    if admissible {
        if let Some(decision) = object
            .get_mut("decision")
            .and_then(|value| value.as_object_mut())
        {
            decision.insert("route_to".to_string(), serde_json::json!(route_to));
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
) -> Result<MirrorFileDigest> {
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mirror receipt directory {}",
                parent.display()
            )
        })?;
    }
    let receipt = issue_mirror_sync_receipt(mirror, path, receipt_path, digest);
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

fn issue_mirror_sync_receipt(
    mirror: &IssueMirrorReport,
    path: &Path,
    receipt_path: &Path,
    digest: &MirrorFileDigest,
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
        "refresh_command": format!("entrance hive issue mirror {} --compact", mirror.issue.id),
        "sync_command": format!("entrance hive issue mirror-sync {}", mirror.issue.id),
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", mirror.issue.id),
        "verify_command": format!("entrance hive issue mirror-verify {}", mirror.issue.id),
        "readback_command": format!("entrance hive issue mirror-readback {} --record --compact", mirror.issue.id)
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
        "admit_command": format!("entrance hive issue mirror-admit {} --record --compact", issue_id.unwrap_or_default())
    })
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
        "publish_command": format!("entrance hive issue mirror-publish {} --compact", issue_id.unwrap_or_default()),
        "readback_command": format!("entrance hive issue mirror-readback {} --record --compact", issue_id.unwrap_or_default()),
        "admit_command": format!("entrance hive issue mirror-admit {} --record --compact", issue_id.unwrap_or_default())
    })
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
            "schema_version": "entrance.hive.policy.v1",
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
            "loop_round": receipt.and_then(|value| json_pointer_i64(value, "/loop/round"))
        },
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
            )
        ]
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
            "schema_version": "entrance.hive.policy.v1",
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

fn compact_digest(digest: &MirrorFileDigest) -> serde_json::Value {
    serde_json::json!({
        "bytes": digest.bytes,
        "sha256": digest.sha256.as_str()
    })
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
            "provider": "entrance hive connector queue --provider <name> --compact"
        }
    })
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
        "storage": provider.storage.as_str(),
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
            "would_write": "local connector mirror file",
            "remote_write": false
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
    serde_json::json!({
        "id": issue_id,
        "loop_id": issue.pointer("/loop_id").and_then(|value| value.as_i64()),
        "title": issue.pointer("/title").and_then(|value| value.as_str()),
        "status": issue.pointer("/status").and_then(|value| value.as_str()),
        "provider": provider_name.as_str(),
        "provider_status": provider.map(|provider| provider.status.as_str()),
        "configured": provider.map(|provider| provider.configured),
        "supports_publish": provider.map(|provider| provider.supports_publish),
        "admission_status": admission.map(|admission| admission.status.as_str()),
        "admission_blockers": admission
            .map(|admission| admission.blockers.iter().map(String::as_str).collect::<Vec<_>>())
            .unwrap_or_default(),
        "review_surface": issue.pointer("/connector/review_surface").and_then(|value| value.as_str()),
        "publish_required": true,
        "current": issue.pointer("/connector/current").and_then(|value| value.as_bool()),
        "reason": issue.pointer("/connector/reason").and_then(|value| value.as_str()),
        "path": issue.pointer("/connector/path").and_then(|value| value.as_str()),
        "failed_checks": failed_checks,
        "failed_check_count": failed_check_count,
        "commands": {
            "publish": publish_command.as_str(),
            "readback": issue
                .pointer("/connector/readback_command")
                .and_then(|value| value.as_str()),
            "admit": issue
                .pointer("/connector/admit_command")
                .and_then(|value| value.as_str())
        },
        "dry_run_action": {
            "schema_version": "entrance.hive.connector_publish_dry_run.v1",
            "action": "publish",
            "provider": provider_name.as_str(),
            "provider_status": provider.map(|provider| provider.status.as_str()),
            "provider_configured": provider.map(|provider| provider.configured),
            "supports_publish": provider.map(|provider| provider.supports_publish),
            "would_write": "local connector mirror file",
            "remote_write": false,
            "command": publish_command
        }
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
        compact_connector_queue, compact_issue_board, compact_issue_detail, compact_issue_mirror,
        compact_issue_mirror_admission, compact_issue_mirror_admission_summary,
        compact_issue_mirror_audit, compact_issue_mirror_audit_summary,
        compact_issue_mirror_publish, compact_issue_mirror_readback,
        compact_issue_mirror_readback_summary, compact_issue_mirror_status,
        compact_issue_mirror_sync, compact_issue_mirror_verify, compact_loop_audit,
        default_issue_mirror_path, flag_present, flag_value, issue_mirror_sync_receipt,
        mirror_receipt_path, MirrorFileDigest,
    };
    use entrance_core::{HiveComment, HiveIssue, HiveLoopContract};
    use entrance_hive::{
        ConnectorAdmissionPolicySpec, ConnectorProviderAdmissionSpec, ConnectorProviderSpec,
        ConnectorRegistryReport, HiveLoopAuditCheck, HiveLoopAuditReport, HiveLoopDoctorCounts,
        IssueAction, IssueCard, IssueDoctorSummary, IssueMirrorReport,
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
                dry_run_command: "entrance hive issue connector-admission <id> --compact"
                    .to_string(),
            },
        }
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
            mode: "test".to_string(),
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
                    "failed_checks": ["remote_digest_current"],
                    "publish_command": "entrance hive issue mirror-publish 7 --compact",
                    "readback_command": "entrance hive issue mirror-readback 7 --record --compact"
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
                    "failed_checks": ["remote_file_present"],
                    "publish_command": "entrance hive issue mirror-publish 9 --compact",
                    "readback_command": "entrance hive issue mirror-readback 9 --record --compact",
                    "admit_command": "entrance hive issue mirror-admit 9 --record --compact"
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
                .pointer("/issues/0/dry_run_action/would_write")
                .and_then(|value| value.as_str()),
            Some("local connector mirror file")
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
            Path::new("/tmp/root/connectors/issue-mirrors/hive-loop-1-issue-2.receipt.json"),
            &MirrorFileDigest {
                bytes: 512,
                sha256: "abc123".to_string(),
            },
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
