use std::io::{self, Read};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::{
    core::{
        bootstrap_mcp_cycle::{
            run_forge_bootstrap_dev_task, run_forge_bootstrap_mcp_cycle,
            ForgeBootstrapMcpCycleOptions, ForgeBootstrapMcpCycleReport,
        },
        event_bus::EventBus,
        StartupState,
    },
    plugins::{
        self,
        forge::{
            prepare_agent_dispatch_blocking, verify_agent_dispatch,
            ForgeDispatchVerificationReport, PreparedAgentDispatch,
        },
    },
};

use super::{bootstrap_cli_state, bootstrap_headless, print_json};

pub(super) fn run_forge_cli(args: &[String]) -> Result<()> {
    match args {
        [command] if command == "prepare-dispatch" => {
            print_json(&prepare_forge_dispatch_cli(None)?)
        }
        [command, flag, value] if command == "prepare-dispatch" && flag == "--project-dir" => {
            print_json(&prepare_forge_dispatch_cli(Some(value.to_string()))?)
        }
        [command] if command == "verify-dispatch" => {
            print_json(&verify_forge_dispatch_cli(None)?)
        }
        [command, flag, value] if command == "verify-dispatch" && flag == "--project-dir" => {
            print_json(&verify_forge_dispatch_cli(Some(value.to_string()))?)
        }
        [command, rest @ ..] if command == "bootstrap-mcp-cycle" => {
            print_json(&bootstrap_forge_mcp_cycle_cli(parse_forge_bootstrap_mcp_cycle_args(
                rest,
            )?)?)
        }
        [command] if command == "run-bootstrap-dev-plan" => {
            print_json(&run_forge_bootstrap_dev_plan_cli()?)
        }
        [command, rest @ ..] if command == "supervise-task" => {
            run_forge_supervise_task_cli(parse_forge_supervise_task_args(rest)?)
        }
        _ => bail!(
            "unsupported forge command, expected `entrance forge prepare-dispatch`, `entrance forge prepare-dispatch --project-dir <path>`, `entrance forge verify-dispatch`, `entrance forge verify-dispatch --project-dir <path>`, `entrance forge bootstrap-mcp-cycle [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--agent-count <n>]`, `entrance forge run-bootstrap-dev-plan`, or `entrance forge supervise-task --task-id <id>`"
        ),
    }
}

fn prepare_forge_dispatch_with_startup(
    startup: &StartupState,
    project_dir: Option<String>,
) -> Result<PreparedAgentDispatch> {
    prepare_agent_dispatch_blocking(startup.data_store(), project_dir).map_err(anyhow::Error::msg)
}

pub(crate) fn prepare_forge_dispatch_cli(
    project_dir: Option<String>,
) -> Result<PreparedAgentDispatch> {
    let startup = bootstrap_forge_cli_state()?;
    prepare_forge_dispatch_with_startup(&startup, project_dir)
}

pub(crate) fn verify_forge_dispatch_cli(
    project_dir: Option<String>,
) -> Result<ForgeDispatchVerificationReport> {
    let startup = bootstrap_forge_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    verify_agent_dispatch(&forge_plugin, project_dir).map_err(anyhow::Error::msg)
}

fn bootstrap_forge_cli_state() -> Result<StartupState> {
    let startup = bootstrap_cli_state()?;
    if !startup.forge_enabled() {
        bail!("Forge is disabled in entrance.toml");
    }

    Ok(startup)
}

fn bootstrap_forge_mcp_cli_state() -> Result<StartupState> {
    let startup = bootstrap_headless()?;
    if !startup.forge_enabled() {
        bail!("Forge is disabled in entrance.toml");
    }

    Ok(startup)
}

fn parse_forge_bootstrap_mcp_cycle_args(args: &[String]) -> Result<ForgeBootstrapMcpCycleOptions> {
    let mut options = ForgeBootstrapMcpCycleOptions {
        project_dir: None,
        model: "codex".to_string(),
        agent_command: None,
        agent_count: 1,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--project-dir" => {
                let value = args.get(index + 1).context(
                    "`entrance forge bootstrap-mcp-cycle --project-dir` requires a value",
                )?;
                options.project_dir = Some(value.to_string());
                index += 2;
            }
            "--model" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance forge bootstrap-mcp-cycle --model` requires a value")?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance forge bootstrap-mcp-cycle --model` must not be empty");
                }
                options.model = trimmed.to_string();
                index += 2;
            }
            "--agent-command" => {
                let value = args.get(index + 1).context(
                    "`entrance forge bootstrap-mcp-cycle --agent-command` requires a value",
                )?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance forge bootstrap-mcp-cycle --agent-command` must not be empty");
                }
                options.agent_command = Some(trimmed.to_string());
                index += 2;
            }
            "--agent-count" => {
                let value = args.get(index + 1).context(
                    "`entrance forge bootstrap-mcp-cycle --agent-count` requires a value",
                )?;
                let parsed = value.parse::<usize>().with_context(|| {
                    format!(
                        "`entrance forge bootstrap-mcp-cycle --agent-count` received invalid value `{value}`"
                    )
                })?;
                if parsed == 0 {
                    bail!("`entrance forge bootstrap-mcp-cycle --agent-count` must be >= 1");
                }
                options.agent_count = parsed;
                index += 2;
            }
            other => bail!("unsupported forge bootstrap-mcp-cycle argument `{other}`"),
        }
    }

    Ok(options)
}

fn parse_forge_supervise_task_args(args: &[String]) -> Result<i64> {
    let mut task_id = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--task-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance forge supervise-task --task-id` requires a value")?;
                let parsed = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid forge task id `{value}`"))?;
                if parsed <= 0 {
                    bail!("`entrance forge supervise-task --task-id` must be >= 1");
                }
                task_id = Some(parsed);
                index += 2;
            }
            other => bail!("unsupported forge supervise-task argument `{other}`"),
        }
    }

    task_id.context("`entrance forge supervise-task --task-id` is required")
}

fn bootstrap_forge_mcp_cycle_cli(
    options: ForgeBootstrapMcpCycleOptions,
) -> Result<ForgeBootstrapMcpCycleReport> {
    let startup = bootstrap_forge_mcp_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    run_forge_bootstrap_mcp_cycle(&forge_plugin, startup.paths().app_data_dir(), options)
}

fn run_forge_supervise_task_cli(task_id: i64) -> Result<()> {
    let startup = bootstrap_forge_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    if let Err(error) = forge_plugin.engine().spawn_task(task_id) {
        let task = forge_plugin.get_task(task_id)?.ok_or_else(|| {
            anyhow::anyhow!("forge task `{task_id}` disappeared during supervision")
        })?;
        if task.status != "Blocked" {
            return Err(error);
        }
    }

    loop {
        let task = forge_plugin.get_task(task_id)?.ok_or_else(|| {
            anyhow::anyhow!("forge task `{task_id}` disappeared during supervision")
        })?;
        if task.status == "Pending" {
            let _ = forge_plugin.engine().spawn_task(task_id);
        }
        if matches!(
            task.status.as_str(),
            "Done" | "Failed" | "Cancelled" | "Blocked"
        ) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn run_forge_bootstrap_dev_plan_cli() -> Result<Value> {
    let startup = bootstrap_forge_mcp_cli_state()?;
    let mut raw_plan = String::new();
    io::stdin()
        .read_to_string(&mut raw_plan)
        .context("failed to read bootstrap dev task plan from stdin")?;
    run_forge_bootstrap_dev_task(startup.paths().app_data_dir(), &raw_plan)
}
