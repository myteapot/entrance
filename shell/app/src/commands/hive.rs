use anyhow::{bail, Result};
use entrance_hive::{HiveCallbackRequest, HiveDispatchRequest, ReviewDecision};

use crate::{app::AppServices, cli, print_json};

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive summary\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]\n  entrance hive engine <id>\n  entrance hive callback <id> <status> [summary]\n  entrance hive review <id> <approve|return|integrate>"
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
        _ => bail!("unsupported hive command"),
    }
}
