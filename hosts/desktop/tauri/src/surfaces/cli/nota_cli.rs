use anyhow::{bail, Context, Result};

use crate::{
    core::{
        chat_archive::{
            capture_chat_message, get_chat_archive_policy, list_chat_captures,
            set_chat_archive_policy, ChatArchivePolicyRequest, ChatCaptureRequest,
        },
        cold_docs_runtime::{canonicalize_cold_docs_from_repo, export_cold_docs_to_repo},
        design_governance::{list_design_decisions, record_design_decision, DesignDecisionRequest},
        environment_runtime::{current_runtime_host, list_owned_worktrees},
        event_bus::EventBus,
        invariant_runtime::refresh_runtime_invariants,
        nota_runtime::{
            accept_current_runtime_round, list_nota_runtime_allocations,
            list_nota_runtime_receipts, list_nota_runtime_transactions,
            list_runtime_acceptance_bundles, list_runtime_checkpoints, list_runtime_human_rounds,
            materialize_runtime_closure_checkpoint, record_dev_return_finalize,
            record_dev_return_integration, record_dev_return_review, record_nota_boundary_ask,
            record_nota_boundary_clarification, run_nota_dev_dispatch, run_nota_do_agent_dispatch,
            write_runtime_checkpoint, NotaBoundaryAskRequest, NotaBoundaryClarificationRequest,
            NotaCheckpointRequest, NotaCurrentRoundAcceptanceRequest, NotaDevDispatchRequest,
            NotaDevReturnFinalizeRequest, NotaDevReturnIntegrateRequest,
            NotaDevReturnReviewRequest, NotaDispatchExecutionHost, NotaDoAgentDispatchRequest,
        },
        overview::{
            build_nota_runtime_overview, build_nota_runtime_status, list_nota_todos,
            list_nota_visions,
        },
    },
    hosts::plugins,
    surfaces::tauri::{rebuild_nota_projections, write_hot_root_projection},
};

use super::{bootstrap_cli_state, print_json};

pub(super) fn run_nota_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "overview" => {
            print_json(&build_nota_runtime_overview(&startup.data_store())?)
        }
        [command] if command == "status" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?)
        }
        [command] if command == "chat-policy" => {
            print_json(&get_chat_archive_policy(&startup.data_store(), None, None)?)
        }
        [command] if command == "chat-captures" => {
            print_json(&list_chat_captures(&startup.data_store())?)
        }
        [command] if command == "checkpoints" => {
            print_json(&list_runtime_checkpoints(&startup.data_store())?)
        }
        [command] if command == "rounds" => {
            print_json(&list_runtime_human_rounds(&startup.data_store())?)
        }
        [command] if command == "acceptance-bundles" => {
            print_json(&list_runtime_acceptance_bundles(&startup.data_store())?)
        }
        [command] if command == "projections" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.projections)
        }
        [command] if command == "anti-zeno" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.anti_zeno_budget)
        }
        [command] if command == "invariants" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.invariants)
        }
        [command] if command == "repair" => {
            print_json(&build_nota_runtime_status(&startup.data_store())?.repair_lane)
        }
        [command] if command == "cold-docs" => {
            let status = build_nota_runtime_status(&startup.data_store())?;
            print_json(&status.cold_docs)
        }
        [command] if command == "host" => {
            print_json(&current_runtime_host(&startup.data_store())?)
        }
        [command] if command == "worktrees" => {
            let host = current_runtime_host(&startup.data_store())?;
            print_json(&list_owned_worktrees(
                &startup.data_store(),
                host.as_ref().map(|value| value.host_key.as_str()),
            )?)
        }
        [command, flag, value] if command == "canonicalize-cold-docs" && flag == "--project-dir" => {
            let report = canonicalize_cold_docs_from_repo(&startup.data_store(), value)?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            print_json(&report)
        }
        [command, flag, value] if command == "export-cold-docs" && flag == "--project-dir" => {
            let status = build_nota_runtime_status(&startup.data_store())?;
            let report = export_cold_docs_to_repo(
                &startup.data_store(),
                value,
                &status.projections.current_truth_revision,
            )?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            print_json(&report)
        }
        [command] if command == "export-hot-root" => {
            print_json(&write_hot_root_projection(&startup, None)?)
        }
        [command, flag, value] if command == "export-hot-root" && flag == "--project-dir" => {
            print_json(&write_hot_root_projection(&startup, Some(value))?)
        }
        [command] if command == "rebuild-projections" => {
            print_json(&rebuild_nota_projections(&startup, None)?)
        }
        [command, flag, value] if command == "rebuild-projections" && flag == "--project-dir" => {
            print_json(&rebuild_nota_projections(&startup, Some(value))?)
        }
        [command] if command == "decisions" => {
            print_json(&list_design_decisions(&startup.data_store())?)
        }
        [command] if command == "visions" => print_json(&list_nota_visions(&startup.data_store())?),
        [command] if command == "todos" => print_json(&list_nota_todos(&startup.data_store())?),
        [command] if command == "allocations" => {
            print_json(&list_nota_runtime_allocations(&startup.data_store())?)
        }
        [command] if command == "receipts" => {
            print_json(&list_nota_runtime_receipts(&startup.data_store(), None)?)
        }
        [command] if command == "transactions" => {
            print_json(&list_nota_runtime_transactions(&startup.data_store())?)
        }
        [command, rest @ ..] if command == "clarify" => {
            let request = parse_nota_clarify_args(rest)?;
            let report = record_nota_boundary_clarification(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "ask" => {
            let request = parse_nota_ask_args(rest)?;
            let report = record_nota_boundary_ask(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "accept-current-round" => {
            let request = parse_nota_accept_current_round_args(rest)?;
            let report = accept_current_runtime_round(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "receipts" => {
            let transaction_id = parse_nota_receipts_args(rest)?;
            print_json(&list_nota_runtime_receipts(
                &startup.data_store(),
                transaction_id,
            )?)
        }
        [command, rest @ ..] if command == "chat-policy" => {
            let request = parse_nota_chat_policy_args(rest)?;
            print_json(&set_chat_archive_policy(&startup.data_store(), request)?)
        }
        [command, rest @ ..] if command == "capture-chat" => {
            let request = parse_nota_chat_capture_args(rest)?;
            let report = capture_chat_message(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "decision" => {
            let request = parse_nota_decision_args(rest)?;
            let report = record_design_decision(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "do" => {
            if !startup.forge_enabled() {
                bail!("Forge is disabled in entrance.toml");
            }

            let request = parse_nota_dispatch_args(rest, "do")?;
            let config = startup.config_store();
            let forge_config = &config.config().plugins.forge;
            let forge_plugin =
                plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
            let project_dir = request.project_dir.or_else(|| forge_config.project_dir.clone());
            let agent_command = request
                .agent_command
                .or_else(|| forge_config.agent_command.clone());

            let report = run_nota_do_agent_dispatch(
                &startup.data_store(),
                &forge_plugin,
                NotaDoAgentDispatchRequest {
                    project_dir,
                    model: request.model,
                    agent_command,
                    title: request.title,
                    repair_of_allocation_id: request.repair_of_allocation_id,
                    execution_host: NotaDispatchExecutionHost::DetachedForgeCliSupervisor,
                },
            )?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "dev" => {
            if !startup.forge_enabled() {
                bail!("Forge is disabled in entrance.toml");
            }

            let request = parse_nota_dispatch_args(rest, "dev")?;
            let config = startup.config_store();
            let forge_config = &config.config().plugins.forge;
            let forge_plugin =
                plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
            let project_dir = request.project_dir.or_else(|| forge_config.project_dir.clone());
            let agent_command = request
                .agent_command
                .or_else(|| forge_config.agent_command.clone());

            let report = run_nota_dev_dispatch(
                &startup.data_store(),
                &forge_plugin,
                NotaDevDispatchRequest {
                    project_dir,
                    model: request.model,
                    agent_command,
                    title: request.title,
                    repair_of_allocation_id: request.repair_of_allocation_id,
                    execution_host: NotaDispatchExecutionHost::DetachedForgeCliSupervisor,
                },
            )?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "checkpoint" => {
            let request = parse_nota_checkpoint_args(rest)?;
            let mirror_project_dir = request.project_dir.clone();
            let report = write_runtime_checkpoint(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, mirror_project_dir.as_deref())?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "review" => {
            let request = parse_nota_review_args(rest)?;
            let report = record_dev_return_review(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "integrate" => {
            let request = parse_nota_integrate_args(rest)?;
            let report = record_dev_return_integration(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command, rest @ ..] if command == "finalize" => {
            let request = parse_nota_finalize_args(rest)?;
            let report = record_dev_return_finalize(&startup.data_store(), request)?;
            write_hot_root_projection(&startup, None)?;
            print_json(&report)
        }
        [command] if command == "checkpoint-runtime-closure" => {
            let report = materialize_runtime_closure_checkpoint(&startup.data_store())?;
            let mirror_project_dir = report
                .checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.payload.repo_context.as_ref())
                .map(|context| context.project_dir.as_str());
            write_hot_root_projection(&startup, mirror_project_dir)?;
            print_json(&report)
        }
        _ => bail!(
            "unsupported nota command, expected `entrance nota overview`, `entrance nota status`, `entrance nota clarify --summary <text>`, `entrance nota ask --ask-code <unblock|decide|replace|override> --summary <text>`, `entrance nota accept-current-round [--summary <text>]`, `entrance nota do [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>]`, `entrance nota dev [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>] [--repair-of-allocation-id <id>]`, `entrance nota review --transaction-id <id> --allocation-id <id> --verdict <approved|changes_requested> [--summary <text>]`, `entrance nota integrate --transaction-id <id> --allocation-id <id> --state <started|integrated|repair_requested> [--summary <text>]`, `entrance nota finalize --transaction-id <id> --allocation-id <id> [--summary <text>]`, `entrance nota decision --title <text> --statement <text> [--rationale <text>] [--decision-type <text>] [--scope-type <text>] [--scope-ref <text>] [--source-ref <text>] [--decided-by <text>] [--enforcement-level <text>] [--actor-scope <text>] [--confidence <float>] [--supersedes <id> ...] [--conflicts-with <id> ...]`, `entrance nota chat-policy [--policy <off|summary|full>]`, `entrance nota capture-chat --role <human|nota> --content <text> [--summary <text>] [--session-ref <id>] [--scope-type <text>] [--scope-ref <text>] [--linked-decision-id <id>]`, `entrance nota checkpoint --stable-level <text> --landed <text> [--landed <text> ...] --remaining <text> [--remaining <text> ...] --human-continuity-bus <text> [--selected-trunk <text>] [--next-start-hint <text> ...] [--title <text>] [--project-dir <path>]`, `entrance nota checkpoint-runtime-closure`, `entrance nota checkpoints`, `entrance nota rounds`, `entrance nota acceptance-bundles`, `entrance nota projections`, `entrance nota anti-zeno`, `entrance nota invariants`, `entrance nota repair`, `entrance nota cold-docs`, `entrance nota host`, `entrance nota worktrees`, `entrance nota canonicalize-cold-docs --project-dir <path>`, `entrance nota export-cold-docs --project-dir <path>`, `entrance nota export-hot-root [--project-dir <path>]`, `entrance nota rebuild-projections [--project-dir <path>]`, `entrance nota decisions`, `entrance nota visions`, `entrance nota todos`, `entrance nota chat-captures`, `entrance nota allocations`, `entrance nota receipts [--transaction-id <id>]`, or `entrance nota transactions`"
        ),
    }
}

fn parse_nota_receipts_args(args: &[String]) -> Result<Option<i64>> {
    let mut transaction_id = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota receipts --transaction-id` requires a value")?;
                let parsed = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if parsed <= 0 {
                    bail!("`entrance nota receipts --transaction-id` must be >= 1");
                }
                transaction_id = Some(parsed);
                index += 2;
            }
            other => bail!("unsupported nota receipts argument `{other}`"),
        }
    }

    Ok(transaction_id)
}

fn parse_nota_checkpoint_args(args: &[String]) -> Result<NotaCheckpointRequest> {
    let mut request = NotaCheckpointRequest {
        title: None,
        stable_level: String::new(),
        landed: Vec::new(),
        remaining: Vec::new(),
        human_continuity_bus: String::new(),
        selected_trunk: None,
        next_start_hints: Vec::new(),
        project_dir: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--title" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --title` requires a value")?;
                request.title = Some(value.to_string());
                index += 2;
            }
            "--stable-level" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --stable-level` requires a value")?;
                request.stable_level = value.to_string();
                index += 2;
            }
            "--landed" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --landed` requires a value")?;
                request.landed.push(value.to_string());
                index += 2;
            }
            "--remaining" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --remaining` requires a value")?;
                request.remaining.push(value.to_string());
                index += 2;
            }
            "--human-continuity-bus" => {
                let value = args.get(index + 1).context(
                    "`entrance nota checkpoint --human-continuity-bus` requires a value",
                )?;
                request.human_continuity_bus = value.to_string();
                index += 2;
            }
            "--selected-trunk" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --selected-trunk` requires a value")?;
                request.selected_trunk = Some(value.to_string());
                index += 2;
            }
            "--next-start-hint" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --next-start-hint` requires a value")?;
                request.next_start_hints.push(value.to_string());
                index += 2;
            }
            "--project-dir" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota checkpoint --project-dir` requires a value")?;
                request.project_dir = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota checkpoint argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_review_args(args: &[String]) -> Result<NotaDevReturnReviewRequest> {
    let mut request = NotaDevReturnReviewRequest {
        transaction_id: 0,
        allocation_id: 0,
        verdict: String::new(),
        summary: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --transaction-id` requires a value")?;
                request.transaction_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if request.transaction_id <= 0 {
                    bail!("`entrance nota review --transaction-id` must be >= 1");
                }
                index += 2;
            }
            "--allocation-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --allocation-id` requires a value")?;
                request.allocation_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if request.allocation_id <= 0 {
                    bail!("`entrance nota review --allocation-id` must be >= 1");
                }
                index += 2;
            }
            "--verdict" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --verdict` requires a value")?;
                request.verdict = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota review --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota review argument `{other}`"),
        }
    }

    if request.transaction_id <= 0 {
        bail!("`entrance nota review --transaction-id` is required");
    }
    if request.allocation_id <= 0 {
        bail!("`entrance nota review --allocation-id` is required");
    }
    if request.verdict.trim().is_empty() {
        bail!("`entrance nota review --verdict` is required");
    }

    Ok(request)
}

fn parse_nota_integrate_args(args: &[String]) -> Result<NotaDevReturnIntegrateRequest> {
    let mut request = NotaDevReturnIntegrateRequest {
        transaction_id: 0,
        allocation_id: 0,
        state: String::new(),
        summary: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --transaction-id` requires a value")?;
                request.transaction_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if request.transaction_id <= 0 {
                    bail!("`entrance nota integrate --transaction-id` must be >= 1");
                }
                index += 2;
            }
            "--allocation-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --allocation-id` requires a value")?;
                request.allocation_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if request.allocation_id <= 0 {
                    bail!("`entrance nota integrate --allocation-id` must be >= 1");
                }
                index += 2;
            }
            "--state" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --state` requires a value")?;
                request.state = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota integrate --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota integrate argument `{other}`"),
        }
    }

    if request.transaction_id <= 0 {
        bail!("`entrance nota integrate --transaction-id` is required");
    }
    if request.allocation_id <= 0 {
        bail!("`entrance nota integrate --allocation-id` is required");
    }
    if request.state.trim().is_empty() {
        bail!("`entrance nota integrate --state` is required");
    }

    Ok(request)
}

fn parse_nota_finalize_args(args: &[String]) -> Result<NotaDevReturnFinalizeRequest> {
    let mut request = NotaDevReturnFinalizeRequest {
        transaction_id: 0,
        allocation_id: 0,
        summary: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--transaction-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota finalize --transaction-id` requires a value")?;
                request.transaction_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime transaction id `{value}`"))?;
                if request.transaction_id <= 0 {
                    bail!("`entrance nota finalize --transaction-id` must be >= 1");
                }
                index += 2;
            }
            "--allocation-id" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota finalize --allocation-id` requires a value")?;
                request.allocation_id = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if request.allocation_id <= 0 {
                    bail!("`entrance nota finalize --allocation-id` must be >= 1");
                }
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota finalize --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota finalize argument `{other}`"),
        }
    }

    if request.transaction_id <= 0 {
        bail!("`entrance nota finalize --transaction-id` is required");
    }
    if request.allocation_id <= 0 {
        bail!("`entrance nota finalize --allocation-id` is required");
    }

    Ok(request)
}

fn parse_nota_clarify_args(args: &[String]) -> Result<NotaBoundaryClarificationRequest> {
    let mut request = NotaBoundaryClarificationRequest {
        summary: String::new(),
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota clarify --summary` requires a value")?;
                request.summary = value.to_string();
                index += 2;
            }
            other => bail!("unsupported nota clarify argument `{other}`"),
        }
    }

    if request.summary.trim().is_empty() {
        bail!("`entrance nota clarify --summary` is required");
    }

    Ok(request)
}

fn parse_nota_ask_args(args: &[String]) -> Result<NotaBoundaryAskRequest> {
    let mut request = NotaBoundaryAskRequest {
        ask_code: String::new(),
        summary: String::new(),
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--ask-code" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota ask --ask-code` requires a value")?;
                request.ask_code = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota ask --summary` requires a value")?;
                request.summary = value.to_string();
                index += 2;
            }
            other => bail!("unsupported nota ask argument `{other}`"),
        }
    }

    if request.ask_code.trim().is_empty() {
        bail!("`entrance nota ask --ask-code` is required");
    }
    if request.summary.trim().is_empty() {
        bail!("`entrance nota ask --summary` is required");
    }

    Ok(request)
}

fn parse_nota_accept_current_round_args(
    args: &[String],
) -> Result<NotaCurrentRoundAcceptanceRequest> {
    let mut request = NotaCurrentRoundAcceptanceRequest { summary: None };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota accept-current-round --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota accept-current-round argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_dispatch_args(
    args: &[String],
    command_name: &str,
) -> Result<NotaDoAgentDispatchRequest> {
    let mut request = NotaDoAgentDispatchRequest {
        project_dir: None,
        model: "codex".to_string(),
        agent_command: None,
        title: None,
        repair_of_allocation_id: None,
        execution_host: NotaDispatchExecutionHost::InProcess,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--project-dir" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --project-dir` requires a value")
                })?;
                request.project_dir = Some(value.to_string());
                index += 2;
            }
            "--model" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --model` requires a value")
                })?;
                request.model = value.to_string();
                index += 2;
            }
            "--agent-command" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --agent-command` requires a value")
                })?;
                request.agent_command = Some(value.to_string());
                index += 2;
            }
            "--title" => {
                let value = args.get(index + 1).with_context(|| {
                    format!("`entrance nota {command_name} --title` requires a value")
                })?;
                request.title = Some(value.to_string());
                index += 2;
            }
            "--repair-of-allocation-id" => {
                if command_name != "dev" {
                    bail!(
                        "`entrance nota {command_name}` does not support `--repair-of-allocation-id`"
                    );
                }
                let value = args.get(index + 1).with_context(|| {
                    format!(
                        "`entrance nota {command_name} --repair-of-allocation-id` requires a value"
                    )
                })?;
                let parsed = value
                    .parse::<i64>()
                    .with_context(|| format!("invalid runtime allocation id `{value}`"))?;
                if parsed <= 0 {
                    bail!("`entrance nota {command_name} --repair-of-allocation-id` must be >= 1");
                }
                request.repair_of_allocation_id = Some(parsed);
                index += 2;
            }
            other => bail!("unsupported nota {command_name} argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_decision_args(args: &[String]) -> Result<DesignDecisionRequest> {
    let mut request = DesignDecisionRequest {
        title: String::new(),
        statement: String::new(),
        rationale: String::new(),
        decision_type: String::new(),
        decision_status: "accepted".to_string(),
        scope_type: String::new(),
        scope_ref: String::new(),
        source_ref: String::new(),
        decided_by: "NOTA".to_string(),
        enforcement_level: "runtime_canonical".to_string(),
        actor_scope: "system".to_string(),
        confidence: 1.0,
        supersedes: Vec::new(),
        conflicts_with: Vec::new(),
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--title" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --title` requires a value")?;
                request.title = value.to_string();
                index += 2;
            }
            "--statement" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --statement` requires a value")?;
                request.statement = value.to_string();
                index += 2;
            }
            "--rationale" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --rationale` requires a value")?;
                request.rationale = value.to_string();
                index += 2;
            }
            "--decision-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --decision-type` requires a value")?;
                request.decision_type = value.to_string();
                index += 2;
            }
            "--decision-status" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --decision-status` requires a value")?;
                request.decision_status = value.to_string();
                index += 2;
            }
            "--scope-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --scope-type` requires a value")?;
                request.scope_type = value.to_string();
                index += 2;
            }
            "--scope-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --scope-ref` requires a value")?;
                request.scope_ref = value.to_string();
                index += 2;
            }
            "--source-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --source-ref` requires a value")?;
                request.source_ref = value.to_string();
                index += 2;
            }
            "--decided-by" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --decided-by` requires a value")?;
                request.decided_by = value.to_string();
                index += 2;
            }
            "--enforcement-level" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --enforcement-level` requires a value")?;
                request.enforcement_level = value.to_string();
                index += 2;
            }
            "--actor-scope" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --actor-scope` requires a value")?;
                request.actor_scope = value.to_string();
                index += 2;
            }
            "--confidence" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --confidence` requires a value")?;
                request.confidence = value
                    .parse::<f64>()
                    .with_context(|| format!("invalid nota decision confidence `{value}`"))?;
                index += 2;
            }
            "--supersedes" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --supersedes` requires a value")?;
                request.supersedes.push(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid superseded decision id `{value}`"))?,
                );
                index += 2;
            }
            "--conflicts-with" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota decision --conflicts-with` requires a value")?;
                request.conflicts_with.push(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid conflicted decision id `{value}`"))?,
                );
                index += 2;
            }
            other => bail!("unsupported nota decision argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_chat_policy_args(args: &[String]) -> Result<ChatArchivePolicyRequest> {
    let mut request = ChatArchivePolicyRequest {
        scope_type: None,
        scope_ref: None,
        archive_policy: "off".to_string(),
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--policy" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota chat-policy --policy` requires a value")?;
                request.archive_policy = value.to_string();
                index += 2;
            }
            "--scope-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota chat-policy --scope-type` requires a value")?;
                request.scope_type = Some(value.to_string());
                index += 2;
            }
            "--scope-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota chat-policy --scope-ref` requires a value")?;
                request.scope_ref = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota chat-policy argument `{other}`"),
        }
    }

    Ok(request)
}

fn parse_nota_chat_capture_args(args: &[String]) -> Result<ChatCaptureRequest> {
    let mut request = ChatCaptureRequest {
        session_ref: None,
        role: String::new(),
        content: String::new(),
        summary: None,
        scope_type: None,
        scope_ref: None,
        linked_decision_id: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --session-ref` requires a value")?;
                request.session_ref = Some(value.to_string());
                index += 2;
            }
            "--role" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --role` requires a value")?;
                request.role = value.to_string();
                index += 2;
            }
            "--content" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --content` requires a value")?;
                request.content = value.to_string();
                index += 2;
            }
            "--summary" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --summary` requires a value")?;
                request.summary = Some(value.to_string());
                index += 2;
            }
            "--scope-type" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --scope-type` requires a value")?;
                request.scope_type = Some(value.to_string());
                index += 2;
            }
            "--scope-ref" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota capture-chat --scope-ref` requires a value")?;
                request.scope_ref = Some(value.to_string());
                index += 2;
            }
            "--linked-decision-id" => {
                let value = args.get(index + 1).context(
                    "`entrance nota capture-chat --linked-decision-id` requires a value",
                )?;
                request.linked_decision_id = Some(
                    value
                        .parse::<i64>()
                        .with_context(|| format!("invalid linked decision id `{value}`"))?,
                );
                index += 2;
            }
            other => bail!("unsupported nota capture-chat argument `{other}`"),
        }
    }

    Ok(request)
}

fn refresh_runtime_invariant_truth(data_store: &crate::core::data_store::DataStore) -> Result<()> {
    refresh_runtime_invariants(data_store).map(|_| ())
}
