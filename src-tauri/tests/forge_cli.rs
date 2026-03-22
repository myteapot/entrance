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

    let managed_worktree = app_data_dir.join("worktrees").join("Entrance").join("feat-MYT-48");
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
    let report: Value =
        serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;

    assert_eq!(report["dispatch"]["issue_id"], "MYT-48");
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
        "SELECT status, command, working_dir, stdin_text FROM plugin_forge_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;

    assert_eq!(stored.0, "Pending");
    assert_eq!(stored.1, "codex");
    assert_eq!(stored.2.as_deref(), Some(worktree_path.as_str()));
    assert_eq!(stored.3.as_deref(), Some(prompt));
    assert!(!stored.3.as_deref().unwrap_or_default().contains(".agents"));

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

    let managed_worktree = app_data_dir.join("worktrees").join("Entrance").join("feat-MYT-48");
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
    let report: Value =
        serde_json::from_str(&stdout).context("CLI stdout should be valid JSON")?;

    let project_root_path = project_root.to_string_lossy().replace('\\', "/");
    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let bootstrap_skill_path = format!("{project_root_path}/harness/bootstrap/duet/SKILL.md");

    assert_eq!(report["dispatch"]["issue_id"], "MYT-48");
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
        "SELECT status, command, working_dir, stdin_text FROM plugin_forge_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        },
    )?;

    assert_eq!(stored.0, "Pending");
    assert_eq!(stored.1, "codex");
    assert_eq!(stored.2.as_deref(), Some(worktree_path.as_str()));
    assert_eq!(stored.3.as_deref(), Some(prompt));
    assert!(!stored.3.as_deref().unwrap_or_default().contains(".agents"));

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
