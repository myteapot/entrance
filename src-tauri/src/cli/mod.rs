mod compiler_cli;
mod forge_cli;
mod issues_cli;
mod mcp_cli;
mod memory_cli;
mod nota_cli;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::{
    core::{
        bootstrap_for_paths,
        hygiene::{list_spec_hygiene_v0, run_spec_hygiene_v0},
        landing::{
            import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
            list_landing_planning_items, list_landing_unreconciled_items,
        },
        recovery::{
            build_recovery_status_report, import_recovery_seed, list_recovery_seed_rows,
            list_recovery_seed_runs, RecoverySeedRowsQuery,
        },
        resolve_app_data_dir, AppPaths, StartupState,
    },
    run_tauri_app,
};

#[cfg(test)]
pub(crate) use forge_cli::{prepare_forge_dispatch_cli, verify_forge_dispatch_cli};

use self::{
    compiler_cli::run_compiler_cli,
    forge_cli::run_forge_cli,
    issues_cli::run_issues_cli,
    mcp_cli::{run_mcp_http, run_mcp_stdio},
    memory_cli::run_memory_cli,
    nota_cli::run_nota_cli,
};

pub(crate) const ROOT_CLI_HELP: &str = r#"Entrance V1 release candidate runtime shell

Usage:
  entrance
  entrance <command> [args...]
  entrance --help

Commands:
  compiler    Inspect the compiler registry query surface
  nota       Read or write NOTA runtime continuity surfaces
  mcp        Serve Entrance as an MCP server over stdio or HTTP
  forge      Run Forge dispatch and bootstrap helpers
  issues     Manage the built-in issue tracker (list, create, update)
  memory     Import NOTA memory store snapshots
  landing    Import and inspect landing snapshots
  recovery   Inspect import-only recovery seed data
  hygiene    Run runtime and spec hygiene checks

Notes:
  Running `entrance` with no command starts the GUI shell.
  Run `entrance <command> --help` for command-specific usage.
"#;

pub(crate) const LANDING_CLI_HELP: &str = r#"Usage:
  entrance landing import --file <path>
  entrance landing import <path>
  entrance landing runs
  entrance landing mirrors
  entrance landing planning
  entrance landing unreconciled
"#;

pub(crate) const RECOVERY_CLI_HELP: &str = r#"Usage:
  entrance recovery status
  entrance recovery import-seed --file <path>
  entrance recovery import-seed <path>
  entrance recovery runs
  entrance recovery rows [--ingest-run-id <id>] [--table <name>] [--limit <n>]
"#;

pub(crate) const HYGIENE_CLI_HELP: &str = r#"Usage:
  entrance hygiene spec-v0
  entrance hygiene list-spec-v0
"#;

pub(crate) const COMPILER_CLI_HELP: &str = r#"Usage:
  entrance compiler registry list [--format <json|table>] [--include-semantics]
"#;

pub(crate) const MEMORY_CLI_HELP: &str = r#"Usage:
  entrance memory import --source <path>
  entrance memory import <path>
"#;

pub(crate) const FORGE_CLI_HELP: &str = r#"Usage:
  entrance forge prepare-dispatch
  entrance forge prepare-dispatch --project-dir <path>
  entrance forge verify-dispatch
  entrance forge verify-dispatch --project-dir <path>
  entrance forge bootstrap-mcp-cycle [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--agent-count <n>]
  entrance forge run-bootstrap-dev-plan
  entrance forge supervise-task --task-id <id>
"#;

pub(crate) const ISSUES_CLI_HELP: &str = r#"Usage:
  entrance issues list
  entrance issues get <key>
  entrance issues create --title <text> [--desc <text>] [--priority <urgent|high|medium|low|none>] [--assignee <name>]
  entrance issues create <title>
  entrance issues status <key> --set <todo|in_progress|in_review|done|cancelled>
  entrance issues comments <key>
  entrance issues comment <key> --body <text> [--author <name>]
  entrance issues delete <key>
"#;

pub(crate) const NOTA_CLI_HELP: &str = r#"Usage:
  entrance nota overview
  entrance nota status
  entrance nota chat-policy [--policy <off|summary|full>]
  entrance nota chat-captures
  entrance nota checkpoints
  entrance nota rounds
  entrance nota acceptance-bundles
  entrance nota projections
  entrance nota anti-zeno
  entrance nota invariants
  entrance nota repair
  entrance nota cold-docs
  entrance nota host
  entrance nota worktrees
  entrance nota canonicalize-cold-docs --project-dir <path>
  entrance nota export-cold-docs --project-dir <path>
  entrance nota export-hot-root [--project-dir <path>]
  entrance nota rebuild-projections [--project-dir <path>]
  entrance nota decisions
  entrance nota visions
  entrance nota todos
  entrance nota allocations
  entrance nota receipts [--transaction-id <id>]
  entrance nota transactions
  entrance nota clarify --summary <text>
  entrance nota ask --ask-code <unblock|decide|replace|override> --summary <text>
  entrance nota accept-current-round [--summary <text>]
  entrance nota do [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>]
  entrance nota dev [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>] [--repair-of-allocation-id <id>]
  entrance nota review --transaction-id <id> --allocation-id <id> --verdict <approved|changes_requested> [--summary <text>]
  entrance nota integrate --transaction-id <id> --allocation-id <id> --state <started|integrated|repair_requested> [--summary <text>]
  entrance nota finalize --transaction-id <id> --allocation-id <id> [--summary <text>]
  entrance nota decision --title <text> --statement <text> [--rationale <text>] [--decision-type <text>] [--scope-type <text>] [--scope-ref <text>] [--source-ref <text>] [--decided-by <text>] [--enforcement-level <text>] [--actor-scope <text>] [--confidence <float>] [--supersedes <id> ...] [--conflicts-with <id> ...]
  entrance nota capture-chat --role <human|nota> --content <text> [--summary <text>] [--session-ref <id>] [--scope-type <text>] [--scope-ref <text>] [--linked-decision-id <id>]
  entrance nota checkpoint --stable-level <text> --landed <text> [--landed <text> ...] --remaining <text> [--remaining <text> ...] --human-continuity-bus <text> [--selected-trunk <text>] [--next-start-hint <text> ...] [--title <text>] [--project-dir <path>]
  entrance nota checkpoint-runtime-closure
"#;

pub(crate) const MCP_CLI_HELP: &str = r#"Usage:
  entrance mcp stdio [--actor-role <nota|arch|dev>]
  entrance mcp http [--port <port>] [--endpoint <path>] [--actor-role <nota|arch|dev>]
"#;

fn is_help_flag(value: &str) -> bool {
    matches!(value, "help" | "-h" | "--help")
}

pub(crate) fn cli_help_for_args(args: &[String]) -> Option<&'static str> {
    match args {
        [flag] if is_help_flag(flag) => Some(ROOT_CLI_HELP),
        [command, flag] if command == "landing" && is_help_flag(flag) => Some(LANDING_CLI_HELP),
        [command, flag] if command == "recovery" && is_help_flag(flag) => Some(RECOVERY_CLI_HELP),
        [command, flag] if command == "hygiene" && is_help_flag(flag) => Some(HYGIENE_CLI_HELP),
        [command, flag] if command == "compiler" && is_help_flag(flag) => Some(COMPILER_CLI_HELP),
        [command, flag] if command == "memory" && is_help_flag(flag) => Some(MEMORY_CLI_HELP),
        [command, subcommand, flag]
            if command == "compiler" && subcommand == "registry" && is_help_flag(flag) =>
        {
            Some(COMPILER_CLI_HELP)
        }
        [command, flag] if command == "nota" && is_help_flag(flag) => Some(NOTA_CLI_HELP),
        [command, flag] if command == "forge" && is_help_flag(flag) => Some(FORGE_CLI_HELP),
        [command, flag] if command == "issues" && is_help_flag(flag) => Some(ISSUES_CLI_HELP),
        [command, flag] if command == "mcp" && is_help_flag(flag) => Some(MCP_CLI_HELP),
        [command, transport, flag]
            if command == "mcp"
                && matches!(transport.as_str(), "stdio" | "http")
                && is_help_flag(flag) =>
        {
            Some(MCP_CLI_HELP)
        }
        _ => None,
    }
}

fn print_cli_help(help: &str) {
    println!("{help}");
}

pub fn dispatch_cli_or_run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(help) = cli_help_for_args(&args) {
        print_cli_help(help);
        return Ok(());
    }

    match args.as_slice() {
        [command, rest @ ..] if command == "landing" => run_landing_cli(rest),
        [command, rest @ ..] if command == "recovery" => run_recovery_cli(rest),
        [command, rest @ ..] if command == "hygiene" => run_hygiene_cli(rest),
        [command, rest @ ..] if command == "compiler" => run_compiler_cli(rest),
        [command, rest @ ..] if command == "memory" => run_memory_cli(rest),
        [command, rest @ ..] if command == "nota" => run_nota_cli(rest),
        [command, rest @ ..] if command == "forge" => run_forge_cli(rest),
        [command, rest @ ..] if command == "issues" => run_issues_cli(rest),
        [command, transport, rest @ ..] if command == "mcp" && transport == "stdio" => {
            run_mcp_stdio(rest)
        }
        [command, transport, rest @ ..] if command == "mcp" && transport == "http" => {
            run_mcp_http(rest)
        }
        [command, ..] if command == "mcp" => {
            bail!("unsupported MCP transport, expected `entrance mcp stdio` or `entrance mcp http`")
        }
        _ => {
            run_tauri_app();
            Ok(())
        }
    }
}

fn run_recovery_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "status" => print_json(&build_recovery_status_report(&startup.data_store())?),
        [command, flag, value] if command == "import-seed" && flag == "--file" => {
            let report = import_recovery_seed(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command, value] if command == "import-seed" => {
            let report = import_recovery_seed(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command] if command == "runs" => print_json(&list_recovery_seed_runs(&startup.data_store())?),
        [command, rest @ ..] if command == "rows" => {
            let query = parse_recovery_rows_args(rest)?;
            print_json(&list_recovery_seed_rows(&startup.data_store(), query)?)
        }
        [command, rest @ ..] if command == "promote-safe-v0" => {
            let suffix = if rest.is_empty() {
                String::new()
            } else {
                format!(" ({})", rest.join(" "))
            };
            bail!(
                "recovery promotion is permanently disabled; `entrance recovery promote-safe-v0{suffix}` is no longer available because recovery is import-only"
            )
        }
        [command, rest @ ..] if command == "promote-remaining-v0" => {
            let suffix = if rest.is_empty() {
                String::new()
            } else {
                format!(" ({})", rest.join(" "))
            };
            bail!(
                "recovery promotion is permanently disabled; `entrance recovery promote-remaining-v0{suffix}` is no longer available because recovery is import-only"
            )
        }
        _ => bail!(
            "unsupported recovery command, expected `entrance recovery status`, `entrance recovery import-seed --file <path>`, `entrance recovery runs`, or `entrance recovery rows [--ingest-run-id <id>] [--table <name>] [--limit <n>]`"
        ),
    }
}

fn parse_recovery_rows_args(args: &[String]) -> Result<RecoverySeedRowsQuery> {
    let mut query = RecoverySeedRowsQuery::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--ingest-run-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance recovery rows --ingest-run-id` requires a value")?;
                query.ingest_run_id = Some(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid recovery ingest run id `{value}`"))?,
                );
                index += 2;
            }
            "--table" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance recovery rows --table` requires a value")?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance recovery rows --table` must not be empty");
                }
                query.table_name = Some(trimmed.to_string());
                index += 2;
            }
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance recovery rows --limit` requires a value")?;
                let limit = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid recovery row limit `{value}`"))?;
                if limit == 0 {
                    bail!("`entrance recovery rows --limit` must be >= 1");
                }
                query.limit = Some(limit);
                index += 2;
            }
            other => bail!("unsupported recovery rows argument `{other}`"),
        }
    }

    Ok(query)
}

fn run_landing_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command, flag, value] if command == "import" && flag == "--file" => {
            let report = import_linear_entrance_snapshot(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command, value] if command == "import" => {
            let report = import_linear_entrance_snapshot(&startup.data_store(), value)?;
            print_json(&report)
        }
        [command] if command == "runs" => print_json(&list_landing_ingest_runs(&startup.data_store())?),
        [command] if command == "mirrors" => {
            print_json(&list_landing_mirror_items(&startup.data_store())?)
        }
        [command] if command == "planning" => {
            print_json(&list_landing_planning_items(&startup.data_store())?)
        }
        [command] if command == "unreconciled" => {
            print_json(&list_landing_unreconciled_items(&startup.data_store())?)
        }
        _ => bail!(
            "unsupported landing command, expected one of `entrance landing import --file <path>`, `entrance landing runs`, `entrance landing mirrors`, `entrance landing planning`, or `entrance landing unreconciled`"
        ),
    }
}

fn run_hygiene_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "spec-v0" => print_json(&run_spec_hygiene_v0(&startup.data_store())?),
        [command] if command == "list-spec-v0" => {
            print_json(&list_spec_hygiene_v0(&startup.data_store())?)
        }
        _ => bail!(
            "unsupported hygiene command, expected `entrance hygiene spec-v0` or `entrance hygiene list-spec-v0`"
        ),
    }
}

pub(crate) fn bootstrap_headless() -> Result<StartupState> {
    let startup = bootstrap_cli_state()?;
    if !startup.mcp_enabled() {
        bail!("MCP server is disabled in entrance.toml");
    }

    let _logging_system = crate::core::logging::LoggingSystem::init(
        startup.paths().log_dir(),
        startup.log_level(),
        Some(startup.data_store()),
    )?;

    Ok(startup)
}

pub(crate) fn bootstrap_cli_state() -> Result<StartupState> {
    let app_paths = AppPaths::new(resolve_app_data_dir()?);
    bootstrap_for_paths(app_paths)
}

pub(crate) fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("failed to serialize CLI output")?
    );
    Ok(())
}
