use anyhow::{bail, Result};
use entrance_hive::{
    HiveCallbackRequest, HiveDispatchRequest, HiveLoopCreateRequest, HiveLoopRunRequest,
    IssueCommentRequest, IssueDecisionRequest, ReviewDecision,
};

use crate::{app::AppServices, cli, print_json};

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>\n  entrance hive loop create --title <text> --goal <text> [--runtime local|codex]\n  entrance hive loop run <id> [--runtime local|codex] [--decision keep|reject|needs-review|blocked] [--worker-timeout-secs <n>] [--worker-attempts <n>]\n  entrance hive loop show <id>\n  entrance hive loop trace <id>\n  entrance hive loop evidence <id>\n  entrance hive loop audit <id>\n  entrance hive loop doctor <id>\n  entrance hive loop policies <id>\n  entrance hive loop list\n  entrance hive policy registry\n  entrance hive issue list\n  entrance hive issue show <id>\n  entrance hive issue comment <id> --body <text>\n  entrance hive issue decide <id> <retry|request-review|cancel> [--body <text>]"
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
        [scope, action, id, rest @ ..] if scope == "loop" && action == "run" => print_json(
            &services.hive.loop_run(HiveLoopRunRequest {
                loop_id: id.parse::<i64>()?,
                runtime: flag_value(rest, "--runtime").map(ToOwned::to_owned),
                decision: flag_value(rest, "--decision").map(ToOwned::to_owned),
                worker_timeout_secs: flag_value(rest, "--worker-timeout-secs")
                    .map(str::parse)
                    .transpose()?,
                worker_attempts: flag_value(rest, "--worker-attempts")
                    .map(str::parse)
                    .transpose()?,
            })?,
        ),
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
        [scope, action] if scope == "issue" && action == "list" => {
            print_json(&services.hive.panel()?)
        }
        [scope, action, id] if scope == "issue" && action == "show" => {
            print_json(&services.hive.issue_report(id.parse::<i64>()?)?)
        }
        [scope, action, id, rest @ ..] if scope == "issue" && action == "comment" => {
            let body = flag_value(rest, "--body").unwrap_or_default();
            print_json(&services.hive.issue_comment(IssueCommentRequest {
                issue_id: id.parse::<i64>()?,
                author: flag_value(rest, "--author").unwrap_or("human").to_string(),
                body: body.to_string(),
            })?)
        }
        [scope, action, id, decision, rest @ ..] if scope == "issue" && action == "decide" => {
            print_json(&services.hive.issue_decide(IssueDecisionRequest {
                issue_id: id.parse::<i64>()?,
                action: decision.to_string(),
                author: flag_value(rest, "--author").unwrap_or("human").to_string(),
                body: flag_value(rest, "--body").map(ToOwned::to_owned),
            })?)
        }
        _ => bail!("unsupported hive command"),
    }
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|values| values[0] == flag)
        .map(|values| values[1].as_str())
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
