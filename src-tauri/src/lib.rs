pub mod core;
mod plugins;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use tauri::{Emitter, Manager};

use core::{
    bootstrap_for_paths,
    landing::{
        import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
        list_landing_planning_items, list_landing_unreconciled_items, LandingImportReport,
        LandingMirrorSummary, LandingPlanningItemSummary,
    },
    event_bus::EventBus,
    hotkey,
    logging::LoggingSystem,
    mcp_server::{McpPluginSet, McpServer, McpTransport},
    plugin_manager::PluginManager,
    resolve_app_data_dir,
    data_store::StoredSourceIngestRun,
    theme::ThemeSystem,
    AppPaths, StartupState,
};
use plugins::{
    forge::{
        build_agent_task_request, prepare_agent_dispatch as prepare_forge_agent_dispatch,
        PreparedAgentDispatch,
    },
    forge::commands::{
        forge_cancel_task, forge_create_task, forge_dispatch_agent, forge_get_task,
        forge_get_task_details, forge_list_tasks, forge_prepare_agent_dispatch,
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

#[derive(Clone, Serialize)]
struct ForgeDispatchVerificationReport {
    dispatch: PreparedAgentDispatch,
    task_id: i64,
    task_status: String,
    task_command: String,
    task_working_dir: Option<String>,
    prompt_via_stdin: bool,
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
            tracing::warn!(?error, "Forge HTTP server failed to start (port may be in use), continuing without it");
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
            tracing::warn!("Failed to register launcher hotkey '{}': {}. Launcher shortcut disabled.", shortcut, err);
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
        [command, rest @ ..] if command == "forge" => run_forge_cli(rest),
        [command, transport] if command == "mcp" && transport == "stdio" => run_mcp_stdio(),
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
        _ => bail!(
            "unsupported forge command, expected `entrance forge prepare-dispatch`, `entrance forge prepare-dispatch --project-dir <path>`, `entrance forge verify-dispatch`, or `entrance forge verify-dispatch --project-dir <path>`"
        ),
    }
}

fn run_mcp_stdio() -> Result<()> {
    let startup = bootstrap_headless()?;
    let server = build_mcp_server(&startup, McpTransport::Stdio)?;
    server.serve_stdio()
}

fn run_mcp_http(args: &[String]) -> Result<()> {
    let mut port = 9720u16;
    let mut endpoint = "/mcp".to_string();
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
            other => bail!("unsupported MCP HTTP argument `{other}`"),
        }
    }

    let startup = bootstrap_headless()?;
    let server = build_mcp_server(
        &startup,
        McpTransport::Http {
            endpoint: endpoint.clone(),
        },
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

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime for Forge CLI")?;

    runtime
        .block_on(prepare_forge_agent_dispatch(startup.data_store(), project_dir))
        .map_err(anyhow::Error::msg)
}

fn prepare_forge_dispatch_cli(project_dir: Option<String>) -> Result<PreparedAgentDispatch> {
    let startup = bootstrap_forge_cli_state()?;
    prepare_forge_dispatch_with_startup(&startup, project_dir)
}

fn verify_forge_dispatch_cli(project_dir: Option<String>) -> Result<ForgeDispatchVerificationReport> {
    let startup = bootstrap_forge_cli_state()?;
    let dispatch = prepare_forge_dispatch_with_startup(&startup, project_dir)?;

    let request = build_agent_task_request(
        dispatch.issue_id.clone(),
        dispatch.worktree_path.clone(),
        "codex".to_string(),
        dispatch.prompt.clone(),
        Vec::new(),
        None,
    )
    .map_err(anyhow::Error::msg)?;

    let forge_plugin = plugins::forge::ForgePlugin::new(startup.data_store(), EventBus::new());
    let task_id = forge_plugin.create_task(request)?;
    let task = forge_plugin
        .get_task(task_id)?
        .context("stored Forge verification task should exist")?;

    Ok(ForgeDispatchVerificationReport {
        dispatch,
        task_id,
        task_status: task.status,
        task_command: task.command,
        task_working_dir: task.working_dir,
        prompt_via_stdin: task.stdin_text.is_some(),
    })
}

fn build_mcp_server(startup: &StartupState, transport: McpTransport) -> Result<McpServer> {
    let data_store = startup.data_store();
    let event_bus = EventBus::new();

    Ok(McpServer::new(
        transport,
        McpPluginSet {
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
        fs::write(
            app_data_dir.join("entrance.toml"),
            render_config(&config)?,
        )?;

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir.join("worktrees").join("Entrance").join("feat-MYT-48");
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
        fs::write(
            app_data_dir.join("entrance.toml"),
            render_config(&config)?,
        )?;

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir.join("worktrees").join("Entrance").join("feat-MYT-48");
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
