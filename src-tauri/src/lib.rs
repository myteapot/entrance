pub mod core;
mod plugins;

use std::{
    io::{self, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, Manager};

use core::{
    action::ActorRole,
    bootstrap_for_paths,
    bootstrap_mcp_cycle::{
        run_forge_bootstrap_dev_task, run_forge_bootstrap_mcp_cycle, ForgeBootstrapMcpCycleOptions,
        ForgeBootstrapMcpCycleReport,
    },
    data_store::StoredSourceIngestRun,
    event_bus::EventBus,
    hotkey,
    hygiene::{list_spec_hygiene_v0, run_spec_hygiene_v0, SpecHygieneReport},
    landing::{
        import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
        list_landing_planning_items, list_landing_unreconciled_items, LandingImportReport,
        LandingMirrorSummary, LandingPlanningItemSummary,
    },
    logging::LoggingSystem,
    mcp_server::{McpPluginSet, McpServer, McpTransport},
    nota_runtime::{
        list_nota_runtime_transactions, list_runtime_checkpoints, run_nota_do_agent_dispatch,
        write_runtime_checkpoint, NotaCheckpointRequest, NotaDoAgentDispatchRequest,
    },
    plugin_manager::PluginManager,
    recovery::{
        import_recovery_seed, list_recovery_seed_rows, list_recovery_seed_runs,
        promote_remaining_recovery_seed_v0, promote_safe_recovery_seed_v0,
        RecoverySeedPromotionQuery, RecoverySeedRowsQuery,
    },
    resolve_app_data_dir,
    theme::ThemeSystem,
    AppPaths, StartupState,
};
use plugins::{
    forge::commands::{
        forge_cancel_task, forge_create_task, forge_dispatch_agent, forge_get_task,
        forge_get_task_details, forge_list_tasks, forge_prepare_agent_dispatch,
    },
    forge::{
        prepare_agent_dispatch_blocking, verify_agent_dispatch, ForgeDispatchVerificationReport,
        PreparedAgentDispatch,
    },
    launcher::{launcher_launch, launcher_pin, launcher_search, LauncherPlugin},
    vault::{
        commands::{
            vault_add_token, vault_delete_token, vault_get_token, vault_get_token_by_provider,
            vault_list_mcp, vault_list_tokens, vault_update_mcp, vault_upsert_token,
        },
        VaultPlugin,
    },
    AppContext,
};

#[derive(Clone, Serialize)]
struct LauncherUiState {
    hotkey: Option<String>,
}

#[derive(Clone)]
struct DashboardUiState {
    app_version: String,
    launcher_hotkey: Option<String>,
    enabled_plugin_count: usize,
    launcher_enabled: bool,
    forge_enabled: bool,
    vault_enabled: bool,
}

#[derive(Clone, Serialize)]
struct DashboardSummary {
    app_version: String,
    launcher_hotkey: Option<String>,
    enabled_plugin_count: usize,
    running_task_count: usize,
    last_activity_at: Option<String>,
    token_count: usize,
    mcp_config_count: usize,
    enabled_mcp_count: usize,
}

fn setup_application<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_paths = AppPaths::new(app.path().app_data_dir()?);
    let startup = bootstrap_for_paths(app_paths)?;
    let launcher_hotkey = startup.launcher_hotkey().map(str::to_owned);
    app.manage(LauncherUiState {
        hotkey: launcher_hotkey.clone(),
    });

    let logging_system = LoggingSystem::init(
        startup.paths().log_dir(),
        startup.log_level(),
        Some(startup.data_store()),
    )?;
    app.manage(logging_system);

    let theme_system = ThemeSystem::new(startup.config_store());
    let app_handle = app.handle().clone();
    theme_system.emit_current_theme(&app_handle)?;
    app.manage(theme_system);

    let data_store = startup.data_store();
    let event_bus = EventBus::new();
    let enabled_plugin_count = [
        startup.launcher_enabled(),
        startup.forge_enabled(),
        startup.vault_enabled(),
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count();

    app.manage(event_bus.clone());
    app.manage(data_store.clone());
    app.manage(DashboardUiState {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        launcher_hotkey: launcher_hotkey.clone(),
        enabled_plugin_count,
        launcher_enabled: startup.launcher_enabled(),
        forge_enabled: startup.forge_enabled(),
        vault_enabled: startup.vault_enabled(),
    });

    let app_handle_for_events = app.handle().clone();
    let mut rx = event_bus.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if core::event_bus::match_topic("forge:*", &event.topic) {
                let _ = app_handle_for_events.emit(&event.topic, event.payload);
            }
        }
    });

    let app_context = AppContext::new(data_store.clone(), event_bus.clone());

    let mut plugin_manager = PluginManager::default();
    if startup.launcher_enabled() {
        let launcher_plugin = LauncherPlugin::new(data_store.clone());
        plugin_manager.register(Arc::new(launcher_plugin.clone()));
        app.manage(launcher_plugin);
    }

    if startup.forge_enabled() {
        let forge_plugin = plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone());
        if let Err(error) = forge_plugin.start_http_server(startup.forge_http_port()) {
            tracing::warn!(
                ?error,
                "Forge HTTP server failed to start (port may be in use), continuing without it"
            );
        }
        plugin_manager.register(Arc::new(forge_plugin.clone()));
        app.manage(forge_plugin);
    }

    if startup.vault_enabled() {
        let vault_plugin = VaultPlugin::new(data_store.clone())?;
        plugin_manager.register(Arc::new(vault_plugin.clone()));
        app.manage(vault_plugin);
    }

    plugin_manager.init_all(&app_context)?;
    app.manage(plugin_manager);

    if let Some(shortcut) = launcher_hotkey.as_deref() {
        if let Err(err) = hotkey::register_launcher_shortcut(app, shortcut) {
            tracing::warn!(
                "Failed to register launcher hotkey '{}': {}. Launcher shortcut disabled.",
                shortcut,
                err
            );
        }
    }

    Ok(())
}

#[tauri::command]
fn launcher_hotkey(state: tauri::State<'_, LauncherUiState>) -> Option<String> {
    state.hotkey.clone()
}

pub fn dispatch_cli_or_run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command, rest @ ..] if command == "landing" => run_landing_cli(rest),
        [command, rest @ ..] if command == "recovery" => run_recovery_cli(rest),
        [command, rest @ ..] if command == "hygiene" => run_hygiene_cli(rest),
        [command, rest @ ..] if command == "nota" => run_nota_cli(rest),
        [command, rest @ ..] if command == "forge" => run_forge_cli(rest),
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
            run();
            Ok(())
        }
    }
}

fn run_recovery_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
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
            let query = parse_recovery_promotion_args(rest)?;
            print_json(&promote_safe_recovery_seed_v0(&startup.data_store(), query)?)
        }
        [command, rest @ ..] if command == "promote-remaining-v0" => {
            let query = parse_recovery_promotion_args(rest)?;
            print_json(&promote_remaining_recovery_seed_v0(
                &startup.data_store(),
                query,
            )?)
        }
        _ => bail!(
            "unsupported recovery command, expected `entrance recovery import-seed --file <path>`, `entrance recovery runs`, `entrance recovery rows [--ingest-run-id <id>] [--table <name>] [--limit <n>]`, `entrance recovery promote-safe-v0 [--ingest-run-id <id>] [--table <name>]`, or `entrance recovery promote-remaining-v0 [--ingest-run-id <id>] [--table <name>]`"
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

fn parse_recovery_promotion_args(args: &[String]) -> Result<RecoverySeedPromotionQuery> {
    let mut query = RecoverySeedPromotionQuery::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--ingest-run-id" => {
                let value = args.get(index + 1).context(
                    "`entrance recovery promote-safe-v0 --ingest-run-id` requires a value",
                )?;
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
                    .context("`entrance recovery promote-safe-v0 --table` requires a value")?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    bail!("`entrance recovery promote-safe-v0 --table` must not be empty");
                }
                query.table_name = Some(trimmed.to_string());
                index += 2;
            }
            other => bail!("unsupported recovery promote-safe-v0 argument `{other}`"),
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

fn run_forge_cli(args: &[String]) -> Result<()> {
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
        _ => bail!(
            "unsupported forge command, expected `entrance forge prepare-dispatch`, `entrance forge prepare-dispatch --project-dir <path>`, `entrance forge verify-dispatch`, `entrance forge verify-dispatch --project-dir <path>`, `entrance forge bootstrap-mcp-cycle [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--agent-count <n>]`, or `entrance forge run-bootstrap-dev-plan`"
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

fn run_nota_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command] if command == "checkpoints" => {
            print_json(&list_runtime_checkpoints(&startup.data_store())?)
        }
        [command] if command == "transactions" => {
            print_json(&list_nota_runtime_transactions(&startup.data_store())?)
        }
        [command, rest @ ..] if command == "do" => {
            if !startup.forge_enabled() {
                bail!("Forge is disabled in entrance.toml");
            }

            let request = parse_nota_do_args(rest)?;
            let config = startup.config_store();
            let forge_config = &config.config().plugins.forge;
            let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
            let project_dir = request.project_dir.or_else(|| forge_config.project_dir.clone());
            let agent_command = request
                .agent_command
                .or_else(|| forge_config.agent_command.clone());

            print_json(&run_nota_do_agent_dispatch(
                &startup.data_store(),
                &forge_plugin,
                NotaDoAgentDispatchRequest {
                    project_dir,
                    model: request.model,
                    agent_command,
                    title: request.title,
                },
            )?)
        }
        [command, rest @ ..] if command == "checkpoint" => {
            let request = parse_nota_checkpoint_args(rest)?;
            print_json(&write_runtime_checkpoint(&startup.data_store(), request)?)
        }
        _ => bail!(
            "unsupported nota command, expected `entrance nota do [--project-dir <path>] [--model <runner>] [--agent-command <path>] [--title <text>]`, `entrance nota checkpoint --stable-level <text> --landed <text> [--landed <text> ...] --remaining <text> [--remaining <text> ...] --human-continuity-bus <text> [--selected-trunk <text>] [--next-start-hint <text> ...] [--title <text>] [--project-dir <path>]`, `entrance nota checkpoints`, or `entrance nota transactions`"
        ),
    }
}

fn run_mcp_stdio(args: &[String]) -> Result<()> {
    let actor_role = parse_mcp_actor_role_args(args)?;
    let startup = bootstrap_headless()?;
    let server = build_mcp_server(&startup, McpTransport::Stdio, actor_role)?;
    server.serve_stdio()
}

fn run_mcp_http(args: &[String]) -> Result<()> {
    let mut port = 9720u16;
    let mut endpoint = "/mcp".to_string();
    let mut actor_role = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--port" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --port` requires a value")?;
                port = value
                    .parse::<u16>()
                    .with_context(|| format!("invalid MCP HTTP port `{value}`"))?;
                index += 2;
            }
            "--endpoint" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --endpoint` requires a value")?;
                endpoint = normalize_http_endpoint(value)?;
                index += 2;
            }
            "--actor-role" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --actor-role` requires a value")?;
                actor_role = Some(parse_mcp_actor_role(value)?);
                index += 2;
            }
            other => bail!("unsupported MCP HTTP argument `{other}`"),
        }
    }

    let startup = bootstrap_headless()?;
    let server = build_mcp_server(
        &startup,
        McpTransport::Http {
            endpoint: endpoint.clone(),
        },
        actor_role,
    )?;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime for MCP HTTP transport")?;

    runtime.block_on(server.serve_http(address))
}

fn bootstrap_headless() -> Result<StartupState> {
    let startup = bootstrap_cli_state()?;
    if !startup.mcp_enabled() {
        bail!("MCP server is disabled in entrance.toml");
    }

    let _logging_system = LoggingSystem::init(
        startup.paths().log_dir(),
        startup.log_level(),
        Some(startup.data_store()),
    )?;

    Ok(startup)
}

fn bootstrap_cli_state() -> Result<StartupState> {
    let app_paths = AppPaths::new(resolve_app_data_dir()?);
    bootstrap_for_paths(app_paths)
}

fn bootstrap_forge_cli_state() -> Result<StartupState> {
    let startup = bootstrap_cli_state()?;
    if !startup.forge_enabled() {
        bail!("Forge is disabled in entrance.toml");
    }

    Ok(startup)
}

fn prepare_forge_dispatch_with_startup(
    startup: &StartupState,
    project_dir: Option<String>,
) -> Result<PreparedAgentDispatch> {
    prepare_agent_dispatch_blocking(startup.data_store(), project_dir).map_err(anyhow::Error::msg)
}

fn prepare_forge_dispatch_cli(project_dir: Option<String>) -> Result<PreparedAgentDispatch> {
    let startup = bootstrap_forge_cli_state()?;
    prepare_forge_dispatch_with_startup(&startup, project_dir)
}

fn verify_forge_dispatch_cli(
    project_dir: Option<String>,
) -> Result<ForgeDispatchVerificationReport> {
    let startup = bootstrap_forge_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    verify_agent_dispatch(&forge_plugin, project_dir).map_err(anyhow::Error::msg)
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

fn bootstrap_forge_mcp_cycle_cli(
    options: ForgeBootstrapMcpCycleOptions,
) -> Result<ForgeBootstrapMcpCycleReport> {
    let startup = bootstrap_forge_mcp_cli_state()?;
    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    run_forge_bootstrap_mcp_cycle(&forge_plugin, startup.paths().app_data_dir(), options)
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

fn parse_nota_do_args(args: &[String]) -> Result<NotaDoAgentDispatchRequest> {
    let mut request = NotaDoAgentDispatchRequest {
        project_dir: None,
        model: "codex".to_string(),
        agent_command: None,
        title: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--project-dir" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota do --project-dir` requires a value")?;
                request.project_dir = Some(value.to_string());
                index += 2;
            }
            "--model" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota do --model` requires a value")?;
                request.model = value.to_string();
                index += 2;
            }
            "--agent-command" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota do --agent-command` requires a value")?;
                request.agent_command = Some(value.to_string());
                index += 2;
            }
            "--title" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance nota do --title` requires a value")?;
                request.title = Some(value.to_string());
                index += 2;
            }
            other => bail!("unsupported nota do argument `{other}`"),
        }
    }

    Ok(request)
}

fn run_forge_bootstrap_dev_plan_cli() -> Result<Value> {
    let startup = bootstrap_forge_mcp_cli_state()?;
    let mut raw_plan = String::new();
    io::stdin()
        .read_to_string(&mut raw_plan)
        .context("failed to read bootstrap dev task plan from stdin")?;
    run_forge_bootstrap_dev_task(startup.paths().app_data_dir(), &raw_plan)
}

fn build_mcp_server(
    startup: &StartupState,
    transport: McpTransport,
    actor_role: Option<ActorRole>,
) -> Result<McpServer> {
    let data_store = startup.data_store();
    let event_bus = EventBus::new();

    Ok(McpServer::with_actor_role(
        transport,
        McpPluginSet {
            core_data_store: Some(data_store.clone()),
            forge: startup
                .forge_enabled()
                .then(|| plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone())),
            launcher: startup
                .launcher_enabled()
                .then(|| LauncherPlugin::new(data_store.clone())),
            vault: if startup.vault_enabled() {
                Some(VaultPlugin::new(data_store)?)
            } else {
                None
            },
        },
        actor_role,
    ))
}

fn normalize_http_endpoint(raw: &str) -> Result<String> {
    let endpoint = raw.trim();
    if endpoint.is_empty() {
        bail!("MCP HTTP endpoint must not be empty");
    }

    if endpoint.starts_with('/') {
        Ok(endpoint.to_string())
    } else {
        Ok(format!("/{endpoint}"))
    }
}

fn parse_mcp_actor_role_args(args: &[String]) -> Result<Option<ActorRole>> {
    match args {
        [] => Ok(None),
        [flag, value] if flag == "--actor-role" => Ok(Some(parse_mcp_actor_role(value)?)),
        [other, ..] => bail!("unsupported MCP stdio argument `{other}`"),
    }
}

fn parse_mcp_actor_role(value: &str) -> Result<ActorRole> {
    match value.trim() {
        "nota" => Ok(ActorRole::Nota),
        "arch" => Ok(ActorRole::Arch),
        "dev" => Ok(ActorRole::Dev),
        other => bail!("unsupported MCP actor role `{other}`, expected `nota`, `arch`, or `dev`"),
    }
}

#[tauri::command]
fn dashboard_summary(
    dashboard: tauri::State<'_, DashboardUiState>,
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<DashboardSummary, String> {
    let tasks = if dashboard.forge_enabled {
        data_store
            .list_forge_tasks()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let tokens = if dashboard.vault_enabled {
        data_store
            .list_vault_tokens()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let mcp_configs = if dashboard.vault_enabled {
        data_store
            .list_vault_mcp_configs()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let launcher_apps = if dashboard.launcher_enabled {
        data_store
            .list_launcher_apps()
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };

    let mut last_activity_at = None;
    for task in &tasks {
        update_latest_timestamp(&mut last_activity_at, Some(task.created_at.as_str()));
        update_latest_timestamp(&mut last_activity_at, task.finished_at.as_deref());
    }
    for token in &tokens {
        update_latest_timestamp(&mut last_activity_at, Some(token.updated_at.as_str()));
    }
    for config in &mcp_configs {
        update_latest_timestamp(&mut last_activity_at, Some(config.updated_at.as_str()));
    }
    for app in &launcher_apps {
        update_latest_timestamp(&mut last_activity_at, app.last_used.as_deref());
        update_latest_timestamp(&mut last_activity_at, Some(app.updated_at.as_str()));
    }

    Ok(DashboardSummary {
        app_version: dashboard.app_version.clone(),
        launcher_hotkey: dashboard.launcher_hotkey.clone(),
        enabled_plugin_count: dashboard.enabled_plugin_count,
        running_task_count: tasks.iter().filter(|task| task.status == "Running").count(),
        last_activity_at,
        token_count: tokens.len(),
        mcp_config_count: mcp_configs.len(),
        enabled_mcp_count: mcp_configs.iter().filter(|config| config.enabled).count(),
    })
}

#[tauri::command]
fn landing_import_snapshot(
    path: String,
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<LandingImportReport, String> {
    import_linear_entrance_snapshot(&data_store, path).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_ingest_runs(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<StoredSourceIngestRun>, String> {
    list_landing_ingest_runs(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_mirror_items(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<LandingMirrorSummary>, String> {
    list_landing_mirror_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_planning_items(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<LandingPlanningItemSummary>, String> {
    list_landing_planning_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn landing_list_unreconciled_items(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<Vec<LandingPlanningItemSummary>, String> {
    list_landing_unreconciled_items(&data_store).map_err(|error| error.to_string())
}

#[tauri::command]
fn hygiene_list_spec_v0(
    data_store: tauri::State<'_, core::data_store::DataStore>,
) -> Result<SpecHygieneReport, String> {
    list_spec_hygiene_v0(&data_store).map_err(|error| error.to_string())
}

fn update_latest_timestamp(current: &mut Option<String>, candidate: Option<&str>) {
    let Some(candidate) = candidate.filter(|value| !value.is_empty()) else {
        return;
    };

    let should_replace = current
        .as_deref()
        .map(|value| candidate > value)
        .unwrap_or(true);
    if should_replace {
        *current = Some(candidate.to_string());
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("failed to serialize CLI output")?
    );
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(hotkey::plugin::<tauri::Wry>().expect("failed to initialize global hotkey plugin"))
        .setup(setup_application)
        .invoke_handler(tauri::generate_handler![
            launcher_hotkey,
            dashboard_summary,
            landing_import_snapshot,
            landing_list_ingest_runs,
            landing_list_mirror_items,
            landing_list_planning_items,
            landing_list_unreconciled_items,
            hygiene_list_spec_v0,
            core::theme::get_theme,
            core::theme::set_theme,
            launcher_search,
            launcher_launch,
            launcher_pin,
            forge_create_task,
            forge_dispatch_agent,
            forge_prepare_agent_dispatch,
            forge_list_tasks,
            forge_get_task,
            forge_get_task_details,
            forge_cancel_task,
            vault_list_tokens,
            vault_add_token,
            vault_upsert_token,
            vault_delete_token,
            vault_get_token,
            vault_get_token_by_provider,
            vault_list_mcp,
            vault_update_mcp
        ])
        .run(tauri::generate_context!())
        .expect("error while running Entrance application");
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::{OsStr, OsString},
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::core::config_store::{render_config, EntranceConfig};

    use super::{prepare_forge_dispatch_cli, verify_forge_dispatch_cli};

    static CLI_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TestDir {
        path: PathBuf,
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "entrance-lib-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test temp directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
            Self { key, original }
        }

        fn remove(key: &'static str) -> Self {
            let original = env::var_os(key);
            env::remove_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    fn cli_test_guard() -> std::sync::MutexGuard<'static, ()> {
        CLI_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("CLI test lock should not be poisoned")
    }

    fn init_git_repo(path: &Path) {
        let output = std::process::Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(path)
            .output()
            .expect("git init should run");
        assert!(
            output.status.success(),
            "git init should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn prepare_forge_dispatch_cli_works_without_agents_runtime() -> Result<()> {
        let _guard = cli_test_guard();

        let temp_dir = TestDir::new("forge-cli-no-agents");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let dispatch = prepare_forge_dispatch_cli(Some(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8")
                .to_string(),
        ))?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.issue_status, "Todo");
        assert_eq!(dispatch.issue_status_source, "fallback");
        assert!(dispatch.issue_title.is_none());
        assert_eq!(
            dispatch.prompt_source,
            "Entrance-owned harness/bootstrap prompt"
        );
        assert_eq!(
            dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(dispatch.prompt.contains("harness/bootstrap/duet/SKILL.md"));
        assert!(!dispatch.prompt.contains(".agents"));

        Ok(())
    }

    #[test]
    fn prepare_forge_dispatch_cli_requires_enabled_forge_plugin() -> Result<()> {
        let _guard = cli_test_guard();

        let temp_dir = TestDir::new("forge-cli-disabled");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);

        fs::create_dir_all(&app_data_dir)?;
        fs::write(
            app_data_dir.join("entrance.toml"),
            render_config(&EntranceConfig::default())?,
        )?;

        let error = prepare_forge_dispatch_cli(None).expect_err("forge-disabled CLI should fail");
        assert!(error.to_string().contains("Forge is disabled"));

        Ok(())
    }

    #[test]
    fn verify_forge_dispatch_cli_persists_task_without_agents_runtime() -> Result<()> {
        let _guard = cli_test_guard();

        let temp_dir = TestDir::new("forge-cli-verify-no-agents");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let report = verify_forge_dispatch_cli(Some(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8")
                .to_string(),
        ))?;

        assert_eq!(report.dispatch.issue_id, "MYT-48");
        assert_eq!(report.dispatch.issue_status, "Todo");
        assert_eq!(
            report.dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(!report.dispatch.prompt.contains(".agents"));
        assert!(report.task_id > 0);
        assert_eq!(report.task_status, "Pending");
        assert_eq!(report.task_command, "codex");
        assert_eq!(
            report.task_working_dir.as_deref(),
            Some(report.dispatch.worktree_path.as_str())
        );
        assert!(report.prompt_via_stdin);

        Ok(())
    }
}
