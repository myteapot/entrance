use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

use crate::core::config_store::{render_config, EntranceConfig};
use crate::core::data_store::{
    DataStore, MigrationPlan, NewNotaRuntimeAllocation, NewNotaRuntimeTransaction,
};
use crate::core::supervision::{SupervisionSignalFamily, SupervisorAction};
use crate::{
    build_nota_runtime_status, cli_help_for_args, prepare_forge_dispatch_cli,
    verify_forge_dispatch_cli, COMPILER_CLI_HELP, ELECTRON_BRIDGE_CLI_HELP, FORGE_CLI_HELP,
    MCP_CLI_HELP, NOTA_CLI_HELP, ROOT_CLI_HELP,
};

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
    crate::test_env_guard()
}

#[test]
fn cli_help_is_available_without_falling_back_to_gui() {
    let root = vec!["--help".to_string()];
    assert_eq!(cli_help_for_args(&root), Some(ROOT_CLI_HELP));

    let nota = vec!["nota".to_string(), "--help".to_string()];
    assert_eq!(cli_help_for_args(&nota), Some(NOTA_CLI_HELP));

    let mcp = vec!["mcp".to_string(), "--help".to_string()];
    assert_eq!(cli_help_for_args(&mcp), Some(MCP_CLI_HELP));

    let mcp_stdio = vec!["mcp".to_string(), "stdio".to_string(), "--help".to_string()];
    assert_eq!(cli_help_for_args(&mcp_stdio), Some(MCP_CLI_HELP));

    let electron = vec!["electron-bridge".to_string(), "--help".to_string()];
    assert_eq!(cli_help_for_args(&electron), Some(ELECTRON_BRIDGE_CLI_HELP));

    let electron_stdio = vec![
        "electron-bridge".to_string(),
        "stdio".to_string(),
        "--help".to_string(),
    ];
    assert_eq!(
        cli_help_for_args(&electron_stdio),
        Some(ELECTRON_BRIDGE_CLI_HELP)
    );

    let forge = vec!["forge".to_string(), "--help".to_string()];
    assert_eq!(cli_help_for_args(&forge), Some(FORGE_CLI_HELP));

    let compiler = vec!["compiler".to_string(), "--help".to_string()];
    assert_eq!(cli_help_for_args(&compiler), Some(COMPILER_CLI_HELP));

    let compiler_registry = vec![
        "compiler".to_string(),
        "registry".to_string(),
        "--help".to_string(),
    ];
    assert_eq!(
        cli_help_for_args(&compiler_registry),
        Some(COMPILER_CLI_HELP)
    );
}

#[test]
fn nota_status_can_project_runtime_invariants_on_readonly_store() -> Result<()> {
    let temp_dir = TestDir::new("nota-status-readonly");
    let db_path = temp_dir.path().join("data").join("entrance.db");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
    let writable_store = DataStore::open(&db_path, migration_plan)?;
    drop(writable_store);

    let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
    let readonly_store = DataStore::open_read_only(&db_path, migration_plan)?;
    let status = build_nota_runtime_status(&readonly_store)?;

    assert_eq!(status.invariants.failed_count, 1);
    assert_eq!(status.repair_lane.open_count, 1);
    assert!(status
        .invariants
        .invariants
        .iter()
        .any(|invariant| invariant.invariant_key == "runtime_host_snapshot"));

    Ok(())
}

#[test]
fn nota_status_surfaces_supervision_incident_visibility() -> Result<()> {
    let store = DataStore::in_memory(MigrationPlan::new(crate::hosts::plugins::forge::migrations()))?;
    let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
        actor_role: "nota",
        surface_action: "do",
        transaction_kind: "forge_agent_dispatch",
        title: "Incident visibility",
        payload_json: "{}",
        status: "opened",
        forge_task_id: None,
        cadence_checkpoint_id: None,
    })?;
    let allocation_payload = serde_json::to_string(&serde_json::json!({
        "issue_id": "MYT-83",
        "issue_status": "In Progress",
        "issue_status_source": "test",
        "issue_title": "Incident visibility",
        "project_root": "A:/Publish/entrance",
        "worktree_path": "A:/Publish/entrance/worktrees/feat-m8-3",
        "prompt_source": "test fixture",
        "model": "codex",
        "agent_command": null,
        "repair_of_allocation_id": null,
        "repair_of_transaction_id": null,
        "repair_of_lineage_ref": null,
        "execution_host": "in_process",
        "child_dispatch_role": "agent",
        "child_dispatch_tool_name": "forge_dispatch_agent",
        "terminal_outcome": null
    }))?;
    let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
        allocator_role: "nota",
        allocator_surface: "nota_do",
        allocation_kind: "forge_agent_dispatch",
        source_transaction_id: transaction.id,
        lineage_ref: "nota/do/transaction/1/forge-task/1",
        child_execution_kind: "forge_task",
        child_execution_ref: "1",
        return_target_kind: "nota_runtime_transaction",
        return_target_ref: "1",
        escalation_target_kind: "nota_runtime_transaction",
        escalation_target_ref: "1",
        status: "task_created",
        payload_json: &allocation_payload,
    })?;
    store.record_budget_consumption(
        allocation.id,
        SupervisionSignalFamily::ExecutionFailure.ledger_key(),
        1,
        SupervisorAction::RestartChild.ledger_key(),
        3,
        2,
        false,
        Some("first retry"),
    )?;
    store.record_budget_consumption(
        allocation.id,
        SupervisionSignalFamily::ExecutionFailure.ledger_key(),
        2,
        SupervisorAction::RestartChild.ledger_key(),
        3,
        1,
        false,
        Some("second retry"),
    )?;

    let status = build_nota_runtime_status(&store)?;
    let incident = status
        .current_supervision_incident
        .as_ref()
        .expect("status should surface the supervision incident summary");
    let latest_allocation = status
        .latest_allocation
        .as_ref()
        .expect("status should surface the latest allocation");
    let allocation_incident = latest_allocation
        .supervision_incident
        .as_ref()
        .expect("latest allocation should include supervision incident visibility");

    assert_eq!(incident.retry_count, 3);
    assert_eq!(incident.max_restarts, 3);
    assert_eq!(
        incident.last_supervisor_action,
        SupervisorAction::RestartChild
    );
    assert!(!incident.budget_exhausted);
    assert_eq!(incident.ledger_entry_count, 2);
    assert_eq!(allocation_incident.retry_count, 3);
    assert_eq!(
        status
            .current_supervision
            .as_ref()
            .map(|projection| projection.retry_count),
        Some(3)
    );

    Ok(())
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
    let bootstrap_skill = project_root.join("notes").join("harness").join("bootstrap").join("duet");
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
        "Entrance-owned notes/harness/bootstrap prompt"
    );
    assert_eq!(
        dispatch.worktree_path,
        managed_worktree.to_string_lossy().replace('\\', "/")
    );
    assert!(dispatch.prompt.contains("notes/harness/bootstrap/duet/SKILL.md"));
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
    let mut config = EntranceConfig::default();
    config.core.theme = "light".to_string();
    config.plugins.forge.enabled = false;
    fs::write(
        app_data_dir.join("entrance.toml"),
        render_config(&config)?,
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
    let bootstrap_skill = project_root.join("notes").join("harness").join("bootstrap").join("duet");
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
    assert!(
        report.task_command.contains("codex"),
        "task command `{}` should contain `codex`",
        report.task_command
    );
    assert_eq!(
        report.task_working_dir.as_deref(),
        Some(report.dispatch.worktree_path.as_str())
    );
    assert!(report.prompt_via_stdin);

    Ok(())
}
