use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Result<Self> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time should be after UNIX_EPOCH")?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("entrance-forge-cli-{name}-{suffix}"));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create temp dir at {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn forge_verify_dispatch_cli_runs_without_agents_runtime() -> Result<()> {
    let temp_dir = TempDir::new("verify-dispatch")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let project_root = temp_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    fs::create_dir_all(&bootstrap_skill)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

    let managed_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "forge",
            "verify-dispatch",
            "--project-dir",
            project_root
                .to_str()
                .context("project path should be valid UTF-8")?,
        ])
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .context("failed to spawn `entrance forge verify-dispatch`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance forge verify-dispatch` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")?;
    let report: Value = serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;

    assert_eq!(report["dispatch"]["issue_id"], "MYT-48");
    assert_eq!(report["dispatch"]["dispatch_role"], "agent");
    assert_eq!(report["dispatch"]["issue_status"], "Todo");
    assert_eq!(report["dispatch"]["issue_status_source"], "fallback");
    assert_eq!(
        report["dispatch"]["prompt_source"],
        "Entrance-owned harness/bootstrap prompt"
    );
    assert_eq!(report["task_status"], "Pending");
    assert_eq!(report["task_command"], "codex");
    assert_eq!(report["prompt_via_stdin"], true);

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    assert_eq!(report["dispatch"]["worktree_path"], worktree_path);
    assert_eq!(report["task_working_dir"], worktree_path);

    let prompt = report["dispatch"]["prompt"]
        .as_str()
        .context("dispatch prompt should be a string")?;
    assert!(prompt.contains("harness/bootstrap/duet/SKILL.md"));
    assert!(!prompt.contains(".agents"));

    let task_id = report["task_id"]
        .as_i64()
        .context("task_id should be a numeric ID")?;
    assert!(task_id > 0);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored = connection.query_row(
        "SELECT status, command, working_dir, stdin_text, metadata FROM plugin_forge_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )?;

    assert_eq!(stored.0, "Pending");
    assert_eq!(stored.1, "codex");
    assert_eq!(stored.2.as_deref(), Some(worktree_path.as_str()));
    assert_eq!(stored.3.as_deref(), Some(prompt));
    assert!(!stored.3.as_deref().unwrap_or_default().contains(".agents"));
    let metadata: Value =
        serde_json::from_str(&stored.4).context("task metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "agent");

    Ok(())
}

#[test]
fn forge_verify_dispatch_cli_detects_managed_worktree_from_cwd() -> Result<()> {
    let temp_dir = TempDir::new("verify-dispatch-cwd")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let project_root = temp_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    fs::create_dir_all(&bootstrap_skill)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
    init_git_repo_with_commit(&project_root)?;

    let managed_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    add_git_worktree(&project_root, &managed_worktree, "feat-MYT-48")?;

    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(["forge", "verify-dispatch"])
        .current_dir(&managed_worktree)
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .context(
            "failed to spawn `entrance forge verify-dispatch` from the managed worktree CWD",
        )?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance forge verify-dispatch` from managed worktree CWD failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")?;
    let report: Value = serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;

    let project_root_path = project_root.to_string_lossy().replace('\\', "/");
    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let bootstrap_skill_path = format!("{project_root_path}/harness/bootstrap/duet/SKILL.md");

    assert_eq!(report["dispatch"]["issue_id"], "MYT-48");
    assert_eq!(report["dispatch"]["dispatch_role"], "agent");
    assert_eq!(report["dispatch"]["project_root"], project_root_path);
    assert_eq!(report["dispatch"]["worktree_path"], worktree_path);
    assert_eq!(
        report["dispatch"]["prompt_source"],
        "Entrance-owned harness/bootstrap prompt"
    );
    assert_eq!(report["task_status"], "Pending");
    assert_eq!(report["task_command"], "codex");
    assert_eq!(report["prompt_via_stdin"], true);

    let prompt = report["dispatch"]["prompt"]
        .as_str()
        .context("dispatch prompt should be a string")?;
    assert!(prompt.contains(&bootstrap_skill_path));
    assert!(!prompt.contains(".agents"));

    let task_id = report["task_id"]
        .as_i64()
        .context("task_id should be a numeric ID")?;
    assert!(task_id > 0);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored = connection.query_row(
        "SELECT status, command, working_dir, stdin_text, metadata FROM plugin_forge_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )?;

    assert_eq!(stored.0, "Pending");
    assert_eq!(stored.1, "codex");
    assert_eq!(stored.2.as_deref(), Some(worktree_path.as_str()));
    assert_eq!(stored.3.as_deref(), Some(prompt));
    assert!(!stored.3.as_deref().unwrap_or_default().contains(".agents"));
    let metadata: Value =
        serde_json::from_str(&stored.4).context("task metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "agent");

    Ok(())
}

#[test]
fn forge_prepare_dispatch_cli_reports_managed_worktree_boundary_without_legacy_fallback(
) -> Result<()> {
    let temp_dir = TempDir::new("prepare-dispatch-missing-worktree")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let project_root = temp_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    fs::create_dir_all(&bootstrap_skill)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "forge",
            "prepare-dispatch",
            "--project-dir",
            project_root
                .to_str()
                .context("project path should be valid UTF-8")?,
        ])
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .context("failed to spawn `entrance forge prepare-dispatch`")?;

    assert!(
        !output.status.success(),
        "`entrance forge prepare-dispatch` unexpectedly succeeded without a managed worktree"
    );

    let stderr = String::from_utf8(output.stderr).context("CLI stderr should be valid UTF-8")?;
    let expected_managed_root = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .display()
        .to_string();

    assert!(
        stderr.contains("No active worktree found for project `Entrance`"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains(&expected_managed_root),
        "expected managed root `{expected_managed_root}` in stderr: {stderr}"
    );
    assert!(
        stderr.contains("feat-<ISSUE>"),
        "unexpected stderr: {stderr}"
    );
    assert!(!stderr.contains(".agents"), "unexpected stderr: {stderr}");
    assert!(!stderr.contains("legacy"), "unexpected stderr: {stderr}");

    Ok(())
}

#[test]
fn forge_bootstrap_mcp_cycle_cli_runs_single_agent_bootstrap_without_human_data_bus() -> Result<()>
{
    let temp_dir = TempDir::new("bootstrap-mcp-cycle")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_mcp_app_state(&app_data_dir)?;

    let project_root = temp_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    let dev_role = bootstrap_skill.join("roles");
    fs::create_dir_all(&dev_role)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
    fs::write(dev_role.join("dev.md"), "# test dev role\n")?;
    init_git_repo_with_commit(&project_root)?;

    let managed_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    add_git_worktree(&project_root, &managed_worktree, "feat-MYT-48")?;

    let agent_command = write_stub_agent_command(temp_dir.path())?;
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "forge",
            "bootstrap-mcp-cycle",
            "--project-dir",
            project_root
                .to_str()
                .context("project path should be valid UTF-8")?,
            "--agent-command",
            agent_command
                .to_str()
                .context("agent command path should be valid UTF-8")?,
        ])
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .env("OPENAI_API_KEY", "test-openai-token")
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .context("failed to spawn `entrance forge bootstrap-mcp-cycle`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance forge bootstrap-mcp-cycle` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")?;
    let report: Value = serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;

    assert_eq!(report["bootstrap_surface"]["coordinator_role"], "nota");
    assert_eq!(report["bootstrap_surface"]["arch_surface_role"], "arch");
    assert_eq!(report["bootstrap_surface"]["dev_surface_role"], "dev");
    assert_eq!(
        report["bootstrap_surface"]["dev_assignment_surface"],
        "forge_verify_dev_dispatch"
    );
    assert_eq!(
        report["bootstrap_surface"]["dev_execution_mode"],
        "bootstrap_dev_runtime_task"
    );
    assert_eq!(
        report["bootstrap_surface"]["agent_dispatch_surface"],
        "forge_dispatch_agent"
    );
    assert_eq!(
        report["bootstrap_surface"]["agent_wait_mode"],
        "dev_parent_waits_children"
    );
    assert_eq!(report["requested_agent_count"], 1);
    assert_eq!(report["agent_worktree_mode"], "single_managed_worktree");
    assert!(report["shared_worktree_boundary"].is_null());

    let parent_task_id = report["dev_assignment"]["task_id"]
        .as_i64()
        .context("dev assignment should include a task id")?;
    assert!(parent_task_id > 0);
    assert_eq!(report["dev_assignment"]["dispatch"]["dispatch_role"], "dev");
    assert_eq!(
        report["dev_assignment"]["dispatch"]["prompt_source"],
        "Entrance-owned harness/bootstrap dev prompt"
    );
    assert_eq!(report["dev_assignment"]["task_status"], "Done");
    assert_eq!(
        report["dev_assignment"]["execution_mode"],
        "bootstrap_dev_runtime_task"
    );
    assert!(report["dev_assignment"]["dispatch"]["prompt"].is_null());
    assert_eq!(report["parent_status"]["task"]["status"], "Done");
    assert_eq!(
        report["parent_status"]["task"]["command"],
        report["dev_assignment"]["task_command"]
    );

    assert_eq!(
        report["agent_prepare"]["prompt_source"],
        "Entrance-owned harness/bootstrap prompt"
    );
    assert_eq!(report["agent_prepare"]["issue_id"], "MYT-48");
    assert_eq!(report["agent_prepare"]["child_slot"], "agent-1");
    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    assert_eq!(report["agent_prepare"]["worktree_path"], worktree_path);
    assert!(report["agent_prepare"]["prompt"].is_null());
    let agent_prepares = report["agent_prepares"]
        .as_array()
        .context("agent_prepares should be an array")?;
    assert_eq!(agent_prepares.len(), 1);
    assert_eq!(agent_prepares[0]["worktree_path"], worktree_path);
    assert_eq!(agent_prepares[0]["child_slot"], "agent-1");

    let agent_dispatches = report["agent_dispatches"]
        .as_array()
        .context("agent_dispatches should be an array")?;
    assert_eq!(agent_dispatches.len(), 1);
    assert_eq!(agent_dispatches[0]["dispatch"]["dispatch_role"], "agent");
    assert_eq!(
        agent_dispatches[0]["dispatch"]["dispatch_tool_name"],
        "forge_dispatch_agent"
    );
    assert_eq!(
        agent_dispatches[0]["dispatch"]["supervision"]["parent_receipt"]["parent_task_id"],
        parent_task_id
    );
    assert_eq!(
        agent_dispatches[0]["dispatch"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-1"
    );
    assert_eq!(
        agent_dispatches[0]["final_status"]["task"]["status"],
        "Done"
    );

    let child_receipts = report["parent_status"]["supervision"]["child_receipts"]
        .as_array()
        .context("parent_status should expose child receipts")?;
    assert_eq!(child_receipts.len(), 1);
    assert_eq!(child_receipts[0]["parent_task_id"], parent_task_id);
    assert_eq!(child_receipts[0]["child_dispatch_role"], "agent");
    assert_eq!(
        child_receipts[0]["child_dispatch_tool_name"],
        "forge_dispatch_agent"
    );
    assert_eq!(child_receipts[0]["child_slot"], "agent-1");

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored = connection.query_row(
        "SELECT COUNT(*) FROM plugin_forge_dispatch_receipts WHERE parent_task_id = ?1",
        [parent_task_id],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(stored, 1);

    Ok(())
}

#[test]
fn forge_bootstrap_mcp_cycle_cli_can_fan_out_multiple_agent_children() -> Result<()> {
    let temp_dir = TempDir::new("bootstrap-mcp-cycle-fanout")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_mcp_app_state(&app_data_dir)?;

    let project_root = temp_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    let dev_role = bootstrap_skill.join("roles");
    fs::create_dir_all(&dev_role)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
    fs::write(dev_role.join("dev.md"), "# test dev role\n")?;
    init_git_repo_with_commit(&project_root)?;

    let managed_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    add_git_worktree(&project_root, &managed_worktree, "feat-MYT-48")?;

    let agent_command = write_stub_agent_command(temp_dir.path())?;
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "forge",
            "bootstrap-mcp-cycle",
            "--project-dir",
            project_root
                .to_str()
                .context("project path should be valid UTF-8")?,
            "--agent-command",
            agent_command
                .to_str()
                .context("agent command path should be valid UTF-8")?,
            "--agent-count",
            "2",
        ])
        .env("ENTRANCE_APP_DATA_DIR", &app_data_dir)
        .env("OPENAI_API_KEY", "test-openai-token")
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .context("failed to spawn `entrance forge bootstrap-mcp-cycle --agent-count 2`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance forge bootstrap-mcp-cycle --agent-count 2` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")?;
    let report: Value = serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;

    let parent_task_id = report["dev_assignment"]["task_id"]
        .as_i64()
        .context("dev assignment should include a task id")?;
    assert_eq!(
        report["bootstrap_surface"]["dev_execution_mode"],
        "bootstrap_dev_runtime_task"
    );
    assert_eq!(
        report["bootstrap_surface"]["agent_wait_mode"],
        "dev_parent_waits_children"
    );
    assert_eq!(report["requested_agent_count"], 2);
    assert_eq!(report["agent_worktree_mode"], "per_agent_slot_worktree");
    assert!(report["shared_worktree_boundary"].is_null());
    assert_eq!(report["dev_assignment"]["task_status"], "Done");
    assert_eq!(
        report["dev_assignment"]["execution_mode"],
        "bootstrap_dev_runtime_task"
    );
    assert_eq!(report["parent_status"]["task"]["status"], "Done");

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let slot_one_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("slots")
        .join("MYT-48")
        .join("agent-1")
        .to_string_lossy()
        .replace('\\', "/");
    let slot_two_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("slots")
        .join("MYT-48")
        .join("agent-2")
        .to_string_lossy()
        .replace('\\', "/");
    assert_ne!(slot_one_worktree, worktree_path);
    assert_ne!(slot_two_worktree, worktree_path);

    let agent_prepares = report["agent_prepares"]
        .as_array()
        .context("agent_prepares should be an array")?;
    assert_eq!(agent_prepares.len(), 2);
    assert_eq!(agent_prepares[0]["child_slot"], "agent-1");
    assert_eq!(agent_prepares[1]["child_slot"], "agent-2");
    assert_eq!(agent_prepares[0]["worktree_path"], slot_one_worktree);
    assert_eq!(agent_prepares[1]["worktree_path"], slot_two_worktree);
    assert!(agent_prepares[0]["prompt"].is_null());
    assert!(agent_prepares[1]["prompt"].is_null());

    let agent_dispatches = report["agent_dispatches"]
        .as_array()
        .context("agent_dispatches should be an array")?;
    assert_eq!(agent_dispatches.len(), 2);
    assert_eq!(
        agent_dispatches[0]["dispatch"]["task"]["working_dir"],
        slot_one_worktree
    );
    assert_eq!(
        agent_dispatches[1]["dispatch"]["task"]["working_dir"],
        slot_two_worktree
    );
    assert_eq!(
        agent_dispatches[0]["dispatch"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-1"
    );
    assert_eq!(
        agent_dispatches[1]["dispatch"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-2"
    );
    assert_eq!(
        agent_dispatches[0]["final_status"]["task"]["status"],
        "Done"
    );
    assert_eq!(
        agent_dispatches[1]["final_status"]["task"]["status"],
        "Done"
    );

    let child_receipts = report["parent_status"]["supervision"]["child_receipts"]
        .as_array()
        .context("parent_status should expose child receipts")?;
    assert_eq!(child_receipts.len(), 2);
    assert_eq!(child_receipts[0]["parent_task_id"], parent_task_id);
    assert_eq!(child_receipts[1]["parent_task_id"], parent_task_id);
    assert_eq!(child_receipts[0]["child_slot"], "agent-1");
    assert_eq!(child_receipts[1]["child_slot"], "agent-2");

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored = connection.query_row(
        "SELECT COUNT(*) FROM plugin_forge_dispatch_receipts WHERE parent_task_id = ?1",
        [parent_task_id],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(stored, 2);

    Ok(())
}

fn seed_app_state(app_data_dir: &Path) -> Result<()> {
    fs::create_dir_all(app_data_dir)?;
    fs::write(
        app_data_dir.join("entrance.toml"),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = false

[plugins.launcher]
enabled = false
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = true
http_port = 9721

[plugins.vault]
enabled = false
"#,
    )?;

    Ok(())
}

fn seed_mcp_app_state(app_data_dir: &Path) -> Result<()> {
    fs::create_dir_all(app_data_dir)?;
    fs::write(
        app_data_dir.join("entrance.toml"),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = true

[plugins.launcher]
enabled = false
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = true
http_port = 9721

[plugins.vault]
enabled = false
"#,
    )?;

    Ok(())
}

fn init_git_repo(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("init")
        .arg("--quiet")
        .current_dir(path)
        .output()
        .context("failed to run `git init --quiet`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`git init --quiet` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn init_git_repo_with_commit(path: &Path) -> Result<()> {
    init_git_repo(path)?;

    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .context("failed to run `git add .`")?;
    if !add.status.success() {
        anyhow::bail!(
            "`git add .` failed: {}",
            String::from_utf8_lossy(&add.stderr).trim()
        );
    }

    let commit = Command::new("git")
        .args([
            "-c",
            "user.name=Entrance Test",
            "-c",
            "user.email=entrance@example.com",
            "commit",
            "--quiet",
            "-m",
            "initial commit",
        ])
        .current_dir(path)
        .output()
        .context("failed to run `git commit --quiet -m initial commit`")?;
    if !commit.status.success() {
        anyhow::bail!(
            "`git commit --quiet -m initial commit` failed: {}",
            String::from_utf8_lossy(&commit.stderr).trim()
        );
    }

    Ok(())
}

fn add_git_worktree(repo_root: &Path, worktree_path: &Path, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "--quiet",
            "-b",
            branch,
            worktree_path
                .to_str()
                .context("worktree path should be valid UTF-8")?,
        ])
        .current_dir(repo_root)
        .output()
        .context("failed to run `git worktree add`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`git worktree add` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn write_stub_agent_command(root: &Path) -> Result<PathBuf> {
    let path = if cfg!(windows) {
        root.join("noop-agent.cmd")
    } else {
        root.join("noop-agent.sh")
    };
    let contents = if cfg!(windows) {
        "@echo off\r\nexit /b 0\r\n"
    } else {
        "#!/bin/sh\nexit 0\n"
    };
    fs::write(&path, contents)
        .with_context(|| format!("failed to write stub agent command at {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
    }

    Ok(path)
}
