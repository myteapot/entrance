use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    thread,
    time::Duration,
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

    let mut server = spawn_mcp_stdio(app_dir.path(), None)?;
    server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = server.read_response()?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");
    assert!(initialize["result"]["entranceSurface"]["actorRole"].is_null());

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
    assert!(tools["result"]["entranceSurface"]["actorRole"].is_null());
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
            "forge_prepare_dev_dispatch",
            "forge_verify_dev_dispatch",
            "forge_dispatch_agent",
            "forge_dispatch_dev",
            "forge_status",
            "forge_cancel",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );
    let tools = tools["result"]["tools"]
        .as_array()
        .context("tools/list should return an array")?;
    let dispatch_agent = tools
        .iter()
        .find(|tool| tool["name"] == "forge_dispatch_agent")
        .context("forge_dispatch_agent should be listed")?;
    let dispatch_dev = tools
        .iter()
        .find(|tool| tool["name"] == "forge_dispatch_dev")
        .context("forge_dispatch_dev should be listed")?;
    assert_eq!(dispatch_agent["permission"]["actorRole"], "dev");
    assert_eq!(dispatch_agent["permission"]["primitive"], "dispatch");
    assert_eq!(dispatch_dev["permission"]["actorRole"], "arch");
    assert_eq!(dispatch_dev["permission"]["room"], "strategy");

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
    assert!(forge_run["result"]["entranceSurface"]["actorRole"].is_null());
    assert!(forge_run["result"]["permission"].is_null());
    assert!(
        forge_run["result"]["structuredContent"]["task_id"]
            .as_i64()
            .context("forge_run should return a numeric task_id")?
            > 0
    );

    Ok(())
}

#[test]
fn external_client_can_scope_dispatch_surface_by_actor_role_over_stdio() -> Result<()> {
    let app_dir = TempAppDir::new("scoped-surface")?;
    seed_app_state(app_dir.path())?;

    let mut dev_server = spawn_mcp_stdio_with_actor_role(app_dir.path(), None, Some("dev"))?;
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-dev",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = dev_server.read_response()?;
    assert_eq!(initialize["id"], "initialize-dev");
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "dev");
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools-dev",
        "method": "tools/list"
    }))?;
    let tools = dev_server.read_response()?;
    assert_eq!(tools["result"]["entranceSurface"]["actorRole"], "dev");
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
            "forge_dispatch_agent",
            "forge_status",
            "forge_cancel",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forbidden-dev",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dev_dispatch",
            "arguments": {}
        }
    }))?;
    let forbidden = dev_server.read_response()?;
    assert_eq!(forbidden["result"]["isError"], true);
    assert_eq!(forbidden["result"]["entranceSurface"]["actorRole"], "dev");
    assert_eq!(forbidden["result"]["permission"]["actorRole"], "arch");
    assert_eq!(forbidden["result"]["permission"]["primitive"], "assign");
    assert_eq!(forbidden["result"]["permission"]["room"], "strategy");
    assert_eq!(forbidden["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(
        forbidden["result"]["structuredContent"]["message"],
        "tool `forge_prepare_dev_dispatch` is not available on the current `dev` MCP surface; requires `arch`"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["errorCode"],
        "surface_role_mismatch"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["toolName"],
        "forge_prepare_dev_dispatch"
    );
    assert_eq!(forbidden["result"]["structuredContent"]["currentActorRole"], "dev");
    assert_eq!(
        forbidden["result"]["structuredContent"]["requiredActorRole"],
        "arch"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["entranceSurface"]["actorRole"],
        "dev"
    );
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "vault-list-dev",
        "method": "tools/call",
        "params": {
            "name": "vault_list_mcp",
            "arguments": {}
        }
    }))?;
    let vault_list = dev_server.read_response()?;
    assert_eq!(vault_list["result"]["isError"], false);
    assert_eq!(vault_list["result"]["entranceSurface"]["actorRole"], "dev");
    assert!(vault_list["result"]["permission"].is_null());

    let mut arch_server = spawn_mcp_stdio_with_actor_role(app_dir.path(), None, Some("arch"))?;
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-arch",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = arch_server.read_response()?;
    assert_eq!(initialize["id"], "initialize-arch");
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "arch");
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools-arch",
        "method": "tools/list"
    }))?;
    let tools = arch_server.read_response()?;
    assert_eq!(tools["result"]["entranceSurface"]["actorRole"], "arch");
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
            "forge_prepare_dev_dispatch",
            "forge_verify_dev_dispatch",
            "forge_dispatch_dev",
            "forge_status",
            "forge_cancel",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forbidden-arch",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dispatch",
            "arguments": {}
        }
    }))?;
    let forbidden = arch_server.read_response()?;
    assert_eq!(forbidden["result"]["isError"], true);
    assert_eq!(forbidden["result"]["entranceSurface"]["actorRole"], "arch");
    assert_eq!(forbidden["result"]["permission"]["actorRole"], "dev");
    assert_eq!(forbidden["result"]["permission"]["primitive"], "prepare");
    assert_eq!(forbidden["result"]["permission"]["room"], "prep");
    assert_eq!(forbidden["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(
        forbidden["result"]["structuredContent"]["message"],
        "tool `forge_prepare_dispatch` is not available on the current `arch` MCP surface; requires `dev`"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["errorCode"],
        "surface_role_mismatch"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["toolName"],
        "forge_prepare_dispatch"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["currentActorRole"],
        "arch"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["requiredActorRole"],
        "dev"
    );
    assert_eq!(
        forbidden["result"]["structuredContent"]["entranceSurface"]["actorRole"],
        "arch"
    );
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "vault-list-arch",
        "method": "tools/call",
        "params": {
            "name": "vault_list_mcp",
            "arguments": {}
        }
    }))?;
    let vault_list = arch_server.read_response()?;
    assert_eq!(vault_list["result"]["isError"], false);
    assert_eq!(vault_list["result"]["entranceSurface"]["actorRole"], "arch");
    assert!(vault_list["result"]["permission"].is_null());

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

    let mut server = spawn_mcp_stdio(app_dir.path(), None)?;
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
    assert_eq!(prepare["result"]["permission"]["actorRole"], "dev");
    assert_eq!(prepare["result"]["permission"]["primitive"], "prepare");
    assert_eq!(prepare["result"]["permission"]["room"], "prep");
    assert_eq!(prepare["result"]["permission"]["targetLayer"], "hot");
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

#[test]
fn external_client_can_prepare_and_verify_forge_dev_dispatch_over_stdio_without_agents_runtime(
) -> Result<()> {
    let app_dir = TempAppDir::new("forge-dev-dispatch")?;
    seed_app_state(app_dir.path())?;

    let project_root = app_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    let dev_role = bootstrap_skill.join("roles");
    fs::create_dir_all(&dev_role)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
    fs::write(dev_role.join("dev.md"), "# test dev role\n")?;

    let managed_worktree = app_dir
        .path()
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let mut server = spawn_mcp_stdio(app_dir.path(), None)?;
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
        "id": "forge-prepare-dev",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dev_dispatch",
            "arguments": {
                "project_dir": project_root
            }
        }
    }))?;
    let prepare = server.read_response()?;
    assert_eq!(prepare["id"], "forge-prepare-dev");
    assert_eq!(prepare["result"]["isError"], false);
    assert_eq!(prepare["result"]["permission"]["actorRole"], "arch");
    assert_eq!(prepare["result"]["permission"]["primitive"], "assign");
    assert_eq!(prepare["result"]["permission"]["room"], "strategy");
    assert_eq!(prepare["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(prepare["result"]["structuredContent"]["issue_id"], "MYT-48");
    assert_eq!(
        prepare["result"]["structuredContent"]["dispatch_role"],
        "dev"
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
        "Entrance-owned harness/bootstrap dev prompt"
    );

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared dev dispatch prompt should be a string")?;
    assert_eq!(
        prepare["result"]["structuredContent"]["worktree_path"],
        worktree_path
    );
    assert!(prompt.contains("harness/bootstrap/duet/SKILL.md"));
    assert!(prompt.contains("harness/bootstrap/duet/roles/dev.md"));
    assert!(!prompt.contains(".agents"));

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-verify-dev",
        "method": "tools/call",
        "params": {
            "name": "forge_verify_dev_dispatch",
            "arguments": {
                "projectDir": project_root
            }
        }
    }))?;
    let verify = server.read_response()?;
    assert_eq!(verify["id"], "forge-verify-dev");
    assert_eq!(verify["result"]["isError"], false);
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["issue_id"],
        "MYT-48"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["dispatch_role"],
        "dev"
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
        .context("forge_verify_dev_dispatch should return a numeric task_id")?;
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
    let metadata: Value =
        serde_json::from_str(&stored.4).context("task metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "dev");
    assert_eq!(metadata["kind"], "dev_dispatch");
    assert!(!stored.3.as_deref().unwrap_or_default().contains(".agents"));

    Ok(())
}

#[test]
fn external_client_can_dispatch_agent_over_stdio_with_agent_lane_runtime() -> Result<()> {
    let app_dir = TempAppDir::new("forge-dispatch-agent")?;
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

    let agent_command = write_stub_agent_command(app_dir.path())?
        .to_string_lossy()
        .to_string();

    let mut server = spawn_mcp_stdio(app_dir.path(), Some("test-openai-token"))?;
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
    assert_eq!(prepare["result"]["isError"], false);

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared dispatch prompt should be a string")?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-dispatch-agent",
        "method": "tools/call",
        "params": {
            "name": "forge_dispatch_agent",
            "arguments": {
                "issue_id": "MYT-48",
                "worktree_path": worktree_path,
                "model": "codex",
                "prompt": prompt,
                "agent_command": agent_command
            }
        }
    }))?;
    let dispatch = server.read_response()?;
    assert_eq!(dispatch["id"], "forge-dispatch-agent");
    assert_eq!(dispatch["result"]["isError"], false);
    assert_eq!(
        dispatch["result"]["structuredContent"]["dispatch_role"],
        "agent"
    );

    let task_id = dispatch["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_dispatch_agent should return a numeric task_id")?;
    assert!(task_id > 0);

    let task = &dispatch["result"]["structuredContent"]["task"];
    assert_eq!(task["working_dir"], worktree_path);
    let metadata = task["metadata"]
        .as_str()
        .context("forge_dispatch_agent task metadata should be a string")?;
    let metadata: Value =
        serde_json::from_str(metadata).context("forge_dispatch_agent metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "agent");
    assert_eq!(metadata["kind"], "agent_dispatch");

    let status = wait_for_terminal_status_stdio(&mut server, task_id)?;
    assert_eq!(
        status["result"]["structuredContent"]["task"]["status"],
        "Done"
    );

    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored = connection.query_row(
        "SELECT status, command, working_dir, metadata FROM plugin_forge_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )?;

    assert_eq!(stored.0, "Done");
    assert_eq!(stored.1, agent_command);
    assert_eq!(stored.2.as_deref(), Some(worktree_path.as_str()));
    let metadata: Value =
        serde_json::from_str(&stored.3).context("task metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "agent");
    assert_eq!(metadata["issue_id"], "MYT-48");

    Ok(())
}

#[test]
fn external_client_can_dispatch_dev_over_stdio_with_dev_lane_runtime() -> Result<()> {
    let app_dir = TempAppDir::new("forge-dispatch-dev")?;
    seed_app_state(app_dir.path())?;

    let project_root = app_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    let dev_role = bootstrap_skill.join("roles");
    fs::create_dir_all(&dev_role)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
    fs::write(dev_role.join("dev.md"), "# test dev role\n")?;

    let managed_worktree = app_dir
        .path()
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let agent_command = write_stub_agent_command(app_dir.path())?
        .to_string_lossy()
        .to_string();

    let mut server = spawn_mcp_stdio(app_dir.path(), Some("test-openai-token"))?;
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
        "id": "forge-prepare-dev",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dev_dispatch",
            "arguments": {
                "project_dir": project_root
            }
        }
    }))?;
    let prepare = server.read_response()?;
    assert_eq!(prepare["result"]["isError"], false);

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared dev dispatch prompt should be a string")?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-dispatch-dev",
        "method": "tools/call",
        "params": {
            "name": "forge_dispatch_dev",
            "arguments": {
                "issue_id": "MYT-48",
                "worktree_path": worktree_path,
                "model": "codex",
                "prompt": prompt,
                "agent_command": agent_command
            }
        }
    }))?;
    let dispatch = server.read_response()?;
    assert_eq!(dispatch["id"], "forge-dispatch-dev");
    assert_eq!(dispatch["result"]["isError"], false);
    assert_eq!(
        dispatch["result"]["structuredContent"]["dispatch_role"],
        "dev"
    );

    let task_id = dispatch["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_dispatch_dev should return a numeric task_id")?;
    assert!(task_id > 0);

    let task = &dispatch["result"]["structuredContent"]["task"];
    assert_eq!(task["working_dir"], worktree_path);
    let metadata = task["metadata"]
        .as_str()
        .context("forge_dispatch_dev task metadata should be a string")?;
    let metadata: Value =
        serde_json::from_str(metadata).context("forge_dispatch_dev metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "dev");
    assert_eq!(metadata["kind"], "dev_dispatch");

    let status = wait_for_terminal_status_stdio(&mut server, task_id)?;
    assert_eq!(
        status["result"]["structuredContent"]["task"]["status"],
        "Done"
    );

    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored = connection.query_row(
        "SELECT status, command, working_dir, metadata FROM plugin_forge_tasks WHERE id = ?1",
        [task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )?;

    assert_eq!(stored.0, "Done");
    assert_eq!(stored.1, agent_command);
    assert_eq!(stored.2.as_deref(), Some(worktree_path.as_str()));
    let metadata: Value =
        serde_json::from_str(&stored.3).context("task metadata should be JSON")?;
    assert_eq!(metadata["dispatch_role"], "dev");
    assert_eq!(metadata["issue_id"], "MYT-48");

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

fn wait_for_terminal_status_stdio(server: &mut SpawnedMcp, task_id: i64) -> Result<Value> {
    for _ in 0..200 {
        server.send(json!({
            "jsonrpc": "2.0",
            "id": "forge-status",
            "method": "tools/call",
            "params": {
                "name": "forge_status",
                "arguments": {
                    "task_id": task_id
                }
            }
        }))?;
        let status = server.read_response()?;
        let task_status = status["result"]["structuredContent"]["task"]["status"]
            .as_str()
            .context("forge_status should return a task status string")?;
        if matches!(task_status, "Done" | "Failed" | "Cancelled" | "Blocked") {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(25));
    }

    anyhow::bail!("timed out waiting for forge task {task_id} to reach a terminal state")
}

fn write_stub_agent_command(root: &PathBuf) -> Result<PathBuf> {
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

fn spawn_mcp_stdio(app_dir: &PathBuf, openai_api_key: Option<&str>) -> Result<SpawnedMcp> {
    spawn_mcp_stdio_with_actor_role(app_dir, openai_api_key, None)
}

fn spawn_mcp_stdio_with_actor_role(
    app_dir: &PathBuf,
    openai_api_key: Option<&str>,
    actor_role: Option<&str>,
) -> Result<SpawnedMcp> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_entrance"));
    command.arg("mcp").arg("stdio");
    if let Some(actor_role) = actor_role {
        command.args(["--actor-role", actor_role]);
    }
    command
        .env("ENTRANCE_APP_DATA_DIR", app_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(openai_api_key) = openai_api_key {
        command.env("OPENAI_API_KEY", openai_api_key);
    } else {
        command.env_remove("OPENAI_API_KEY");
    }

    let mut child = command
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
