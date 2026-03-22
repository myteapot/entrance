use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::{json, Value};

struct TempAppDir {
    path: PathBuf,
}

impl TempAppDir {
    fn new(name: &str) -> Result<Self> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time should be after UNIX_EPOCH")?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("entrance-mcp-stdio-{name}-{suffix}"));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create temp dir at {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempAppDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct SpawnedMcp {
    child: Child,
    stderr: BufReader<ChildStderr>,
    stdout: BufReader<ChildStdout>,
}

impl SpawnedMcp {
    fn send(&mut self, request: Value) -> Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .context("child stdin should be available")?;
        serde_json::to_writer(&mut *stdin, &request)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    fn read_response(&mut self) -> Result<Value> {
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .context("failed to read MCP stdio response")?;
            if read == 0 {
                let mut stderr = String::new();
                let _ = self.stderr.read_to_string(&mut stderr);
                anyhow::bail!(
                    "MCP stdio process closed stdout before responding. stderr: {}",
                    stderr.trim()
                );
            }

            let payload = line.trim();
            if payload.is_empty() {
                continue;
            }

            let response =
                serde_json::from_str(payload).context("failed to parse MCP stdio response JSON")?;
            return Ok(response);
        }
    }
}

impl Drop for SpawnedMcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn external_client_can_list_tools_and_call_forge_run_over_stdio() -> Result<()> {
    let app_dir = TempAppDir::new("integration")?;
    seed_app_state(app_dir.path())?;

    let mut server = spawn_mcp_stdio(app_dir.path())?;
    server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = server.read_response()?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools",
        "method": "tools/list"
    }))?;
    let tools = server.read_response()?;
    let tool_names = tools["result"]["tools"]
        .as_array()
        .context("tools/list should return an array")?
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec![
            "forge_run",
            "forge_prepare_dispatch",
            "forge_verify_dispatch",
            "forge_status",
            "forge_cancel",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-run",
        "method": "tools/call",
        "params": {
            "name": "forge_run",
            "arguments": {
                "name": "Echo",
                "command": if cfg!(windows) { "cmd" } else { "sh" },
                "args": if cfg!(windows) {
                    json!(["/C", "echo", "hello from stdio"])
                } else {
                    json!(["-c", "echo hello from stdio"])
                }
            }
        }
    }))?;
    let forge_run = server.read_response()?;

    assert_eq!(forge_run["id"], "forge-run");
    assert_eq!(forge_run["result"]["isError"], false);
    assert!(
        forge_run["result"]["structuredContent"]["task_id"]
            .as_i64()
            .context("forge_run should return a numeric task_id")?
            > 0
    );

    Ok(())
}

#[test]
fn external_client_can_prepare_and_verify_forge_dispatch_over_stdio_without_agents_runtime(
) -> Result<()> {
    let app_dir = TempAppDir::new("forge-dispatch")?;
    seed_app_state(app_dir.path())?;

    let project_root = app_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    fs::create_dir_all(&bootstrap_skill)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

    let managed_worktree = app_dir
        .path()
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let mut server = spawn_mcp_stdio(app_dir.path())?;
    server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = server.read_response()?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-prepare",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dispatch",
            "arguments": {
                "project_dir": project_root
            }
        }
    }))?;
    let prepare = server.read_response()?;
    assert_eq!(prepare["id"], "forge-prepare");
    assert_eq!(prepare["result"]["isError"], false);
    assert_eq!(prepare["result"]["structuredContent"]["issue_id"], "MYT-48");
    assert_eq!(
        prepare["result"]["structuredContent"]["dispatch_role"],
        "agent"
    );
    assert_eq!(
        prepare["result"]["structuredContent"]["issue_status"],
        "Todo"
    );
    assert_eq!(
        prepare["result"]["structuredContent"]["issue_status_source"],
        "fallback"
    );
    assert_eq!(
        prepare["result"]["structuredContent"]["prompt_source"],
        "Entrance-owned harness/bootstrap prompt"
    );

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared dispatch prompt should be a string")?;
    assert_eq!(
        prepare["result"]["structuredContent"]["worktree_path"],
        worktree_path
    );
    assert!(prompt.contains("harness/bootstrap/duet/SKILL.md"));
    assert!(!prompt.contains(".agents"));

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-verify",
        "method": "tools/call",
        "params": {
            "name": "forge_verify_dispatch",
            "arguments": {
                "projectDir": project_root
            }
        }
    }))?;
    let verify = server.read_response()?;
    assert_eq!(verify["id"], "forge-verify");
    assert_eq!(verify["result"]["isError"], false);
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["issue_id"],
        "MYT-48"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["dispatch_role"],
        "agent"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["worktree_path"],
        worktree_path
    );
    assert_eq!(
        verify["result"]["structuredContent"]["task_status"],
        "Pending"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["task_command"],
        "codex"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["prompt_via_stdin"],
        true
    );

    let task_id = verify["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_verify_dispatch should return a numeric task_id")?;
    assert!(task_id > 0);

    let db_path = app_dir.path().join("entrance.db");
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

fn seed_app_state(app_dir: &PathBuf) -> Result<()> {
    fs::write(
        app_dir.join("entrance.toml"),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = true

[plugins.launcher]
enabled = true
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = true
http_port = 9721

[plugins.vault]
enabled = true
"#,
    )?;

    Ok(())
}

fn spawn_mcp_stdio(app_dir: &PathBuf) -> Result<SpawnedMcp> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(["mcp", "stdio"])
        .env("ENTRANCE_APP_DATA_DIR", app_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn `entrance mcp stdio`")?;

    let stdout = child
        .stdout
        .take()
        .context("child stdout should be piped")?;
    let stderr = child
        .stderr
        .take()
        .context("child stderr should be piped")?;

    Ok(SpawnedMcp {
        child,
        stderr: BufReader::new(stderr),
        stdout: BufReader::new(stdout),
    })
}

fn init_git_repo(path: &PathBuf) -> Result<()> {
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
