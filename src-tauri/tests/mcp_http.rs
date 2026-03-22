use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::PathBuf,
    process::{Child, ChildStderr, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
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
        let path = std::env::temp_dir().join(format!("entrance-mcp-http-{name}-{suffix}"));
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

struct SpawnedHttpMcp {
    child: Child,
    stderr: ChildStderr,
    endpoint: String,
    port: u16,
}

impl SpawnedHttpMcp {
    fn send(&mut self, request: Value) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            match post_json_rpc(self.port, &self.endpoint, &request) {
                Ok(response) => return Ok(response),
                Err(error) => {
                    if let Some(status) = self.child.try_wait()? {
                        let mut stderr = String::new();
                        let _ = self.stderr.read_to_string(&mut stderr);
                        bail!(
                            "MCP HTTP process exited before responding ({status}). stderr: {}",
                            stderr.trim()
                        );
                    }

                    if Instant::now() >= deadline {
                        bail!(
                            "timed out waiting for MCP HTTP server on port {}: {}",
                            self.port,
                            error
                        );
                    }

                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
}

impl Drop for SpawnedHttpMcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn external_client_can_list_tools_and_call_forge_run_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("integration")?;
    seed_app_state(app_dir.path())?;

    let port = reserve_port()?;
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp", None)?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");
    assert!(initialize["result"]["entranceSurface"]["actorRole"].is_null());

    let tools = server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools",
        "method": "tools/list"
    }))?;
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
            "forge_prepare_agent_dispatch",
            "forge_verify_agent_dispatch",
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
    let prepare_agent = tools
        .iter()
        .find(|tool| tool["name"] == "forge_prepare_agent_dispatch")
        .context("forge_prepare_agent_dispatch should be listed")?;
    assert_eq!(dispatch_agent["permission"]["actorRole"], "dev");
    assert_eq!(dispatch_agent["permission"]["primitive"], "dispatch");
    assert_eq!(dispatch_agent["dispatchRole"], "agent");
    assert_eq!(dispatch_dev["permission"]["actorRole"], "arch");
    assert_eq!(dispatch_dev["permission"]["room"], "strategy");
    assert_eq!(dispatch_dev["dispatchRole"], "dev");
    assert_eq!(prepare_agent["dispatchRole"], "agent");

    let forge_run = server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-run",
        "method": "tools/call",
        "params": {
            "name": "forge_run",
            "arguments": {
                "name": "Echo",
                "command": if cfg!(windows) { "cmd" } else { "sh" },
                "args": if cfg!(windows) {
                    json!(["/C", "echo", "hello from http"])
                } else {
                    json!(["-c", "echo hello from http"])
                }
            }
        }
    }))?;

    assert_eq!(forge_run["id"], "forge-run");
    assert_eq!(forge_run["result"]["isError"], false);
    assert!(forge_run["result"]["entranceSurface"]["actorRole"].is_null());
    assert!(forge_run["result"]["permission"].is_null());
    assert!(forge_run["result"]["dispatchRole"].is_null());
    assert!(
        forge_run["result"]["structuredContent"]["task_id"]
            .as_i64()
            .context("forge_run should return a numeric task_id")?
            > 0
    );

    Ok(())
}

#[test]
fn external_client_can_scope_dispatch_surface_by_actor_role_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("scoped-surface")?;
    seed_app_state(app_dir.path())?;

    let port = reserve_port()?;
    let mut dev_server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("dev"))?;
    let initialize = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-dev",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize-dev");
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "dev");
    let tools = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools-dev",
        "method": "tools/list"
    }))?;
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
            "forge_prepare_agent_dispatch",
            "forge_verify_agent_dispatch",
            "forge_dispatch_agent",
            "forge_status",
            "forge_cancel",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );
    let forbidden = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forbidden-dev",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dev_dispatch",
            "arguments": {}
        }
    }))?;
    assert_eq!(forbidden["result"]["isError"], true);
    assert_eq!(forbidden["result"]["entranceSurface"]["actorRole"], "dev");
    assert_eq!(forbidden["result"]["permission"]["actorRole"], "arch");
    assert_eq!(forbidden["result"]["permission"]["primitive"], "assign");
    assert_eq!(forbidden["result"]["permission"]["room"], "strategy");
    assert_eq!(forbidden["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(forbidden["result"]["dispatchRole"], "dev");
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
    let vault_list = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "vault-list-dev",
        "method": "tools/call",
        "params": {
            "name": "vault_list_mcp",
            "arguments": {}
        }
    }))?;
    assert_eq!(vault_list["result"]["isError"], false);
    assert_eq!(vault_list["result"]["entranceSurface"]["actorRole"], "dev");
    assert!(vault_list["result"]["permission"].is_null());
    assert!(vault_list["result"]["dispatchRole"].is_null());

    let port = reserve_port()?;
    let mut arch_server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("arch"))?;
    let initialize = arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-arch",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize-arch");
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "arch");
    let tools = arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools-arch",
        "method": "tools/list"
    }))?;
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
    let forbidden = arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forbidden-arch",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_dispatch",
            "arguments": {}
        }
    }))?;
    assert_eq!(forbidden["result"]["isError"], true);
    assert_eq!(forbidden["result"]["entranceSurface"]["actorRole"], "arch");
    assert_eq!(forbidden["result"]["permission"]["actorRole"], "dev");
    assert_eq!(forbidden["result"]["permission"]["primitive"], "prepare");
    assert_eq!(forbidden["result"]["permission"]["room"], "prep");
    assert_eq!(forbidden["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(forbidden["result"]["dispatchRole"], "agent");
    assert_eq!(
        forbidden["result"]["canonicalToolName"],
        "forge_prepare_agent_dispatch"
    );
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
    let vault_list = arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "vault-list-arch",
        "method": "tools/call",
        "params": {
            "name": "vault_list_mcp",
            "arguments": {}
        }
    }))?;
    assert_eq!(vault_list["result"]["isError"], false);
    assert_eq!(vault_list["result"]["entranceSurface"]["actorRole"], "arch");
    assert!(vault_list["result"]["permission"].is_null());
    assert!(vault_list["result"]["dispatchRole"].is_null());

    Ok(())
}

#[test]
fn external_client_can_prepare_and_verify_forge_dispatch_over_http_without_agents_runtime(
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

    let port = reserve_port()?;
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp", None)?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    let prepare = server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-prepare",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_agent_dispatch",
            "arguments": {
                "project_dir": project_root
            }
        }
    }))?;
    assert_eq!(prepare["id"], "forge-prepare");
    assert_eq!(prepare["result"]["isError"], false);
    assert_eq!(prepare["result"]["permission"]["actorRole"], "dev");
    assert_eq!(prepare["result"]["permission"]["primitive"], "prepare");
    assert_eq!(prepare["result"]["permission"]["room"], "prep");
    assert_eq!(prepare["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(prepare["result"]["dispatchRole"], "agent");
    assert_eq!(
        prepare["result"]["canonicalToolName"],
        "forge_prepare_agent_dispatch"
    );
    assert_eq!(prepare["result"]["structuredContent"]["issue_id"], "MYT-48");
    assert_eq!(
        prepare["result"]["structuredContent"]["dispatch_role"],
        "agent"
    );
    assert_eq!(
        prepare["result"]["structuredContent"]["dispatch_tool_name"],
        "forge_dispatch_agent"
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

    let verify = server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-verify",
        "method": "tools/call",
        "params": {
            "name": "forge_verify_agent_dispatch",
            "arguments": {
                "projectDir": project_root
            }
        }
    }))?;
    assert_eq!(verify["id"], "forge-verify");
    assert_eq!(verify["result"]["isError"], false);
    assert_eq!(
        verify["result"]["canonicalToolName"],
        "forge_verify_agent_dispatch"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["issue_id"],
        "MYT-48"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["dispatch_role"],
        "agent"
    );
    assert_eq!(
        verify["result"]["structuredContent"]["dispatch"]["dispatch_tool_name"],
        "forge_dispatch_agent"
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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_agent");

    Ok(())
}

#[test]
fn external_client_can_prepare_and_verify_forge_dev_dispatch_over_http_without_agents_runtime(
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

    let port = reserve_port()?;
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp", None)?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    let prepare = server.send(json!({
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
    assert_eq!(prepare["id"], "forge-prepare-dev");
    assert_eq!(prepare["result"]["isError"], false);
    assert_eq!(prepare["result"]["permission"]["actorRole"], "arch");
    assert_eq!(prepare["result"]["permission"]["primitive"], "assign");
    assert_eq!(prepare["result"]["permission"]["room"], "strategy");
    assert_eq!(prepare["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(prepare["result"]["dispatchRole"], "dev");
    assert_eq!(prepare["result"]["structuredContent"]["issue_id"], "MYT-48");
    assert_eq!(
        prepare["result"]["structuredContent"]["dispatch_role"],
        "dev"
    );
    assert_eq!(
        prepare["result"]["structuredContent"]["dispatch_tool_name"],
        "forge_dispatch_dev"
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

    let verify = server.send(json!({
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
        verify["result"]["structuredContent"]["dispatch"]["dispatch_tool_name"],
        "forge_dispatch_dev"
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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_dev");
    assert!(!stored.3.as_deref().unwrap_or_default().contains(".agents"));

    Ok(())
}

#[test]
fn external_client_can_dispatch_agent_over_http_with_agent_lane_runtime() -> Result<()> {
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

    let port = reserve_port()?;
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp", Some("test-openai-token"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    let prepare = server.send(json!({
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
    assert_eq!(prepare["result"]["isError"], false);
    assert_eq!(
        prepare["result"]["canonicalToolName"],
        "forge_prepare_agent_dispatch"
    );

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared dispatch prompt should be a string")?;

    let dispatch = server.send(json!({
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
    assert_eq!(dispatch["id"], "forge-dispatch-agent");
    assert_eq!(dispatch["result"]["isError"], false);
    assert_eq!(
        dispatch["result"]["structuredContent"]["dispatch_role"],
        "agent"
    );
    assert_eq!(
        dispatch["result"]["structuredContent"]["dispatch_tool_name"],
        "forge_dispatch_agent"
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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_agent");

    let status = wait_for_terminal_status_http(&mut server, task_id)?;
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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_agent");

    Ok(())
}

#[test]
fn external_client_can_dispatch_dev_over_http_with_dev_lane_runtime() -> Result<()> {
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

    let port = reserve_port()?;
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp", Some("test-openai-token"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    let prepare = server.send(json!({
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
    assert_eq!(prepare["result"]["isError"], false);

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared dev dispatch prompt should be a string")?;

    let dispatch = server.send(json!({
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
    assert_eq!(dispatch["id"], "forge-dispatch-dev");
    assert_eq!(dispatch["result"]["isError"], false);
    assert_eq!(
        dispatch["result"]["structuredContent"]["dispatch_role"],
        "dev"
    );
    assert_eq!(
        dispatch["result"]["structuredContent"]["dispatch_tool_name"],
        "forge_dispatch_dev"
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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_dev");

    let status = wait_for_terminal_status_http(&mut server, task_id)?;
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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_dev");

    Ok(())
}

#[test]
fn external_client_can_observe_dispatch_supervision_receipts_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("forge-supervision-http")?;
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

    let arch_port = reserve_port()?;
    let mut arch_server = spawn_mcp_http_with_actor_role(
        app_dir.path(),
        arch_port,
        "/mcp",
        Some("test-openai-token"),
        Some("arch"),
    )?;
    let _ = arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-arch",
        "method": "initialize",
        "params": {}
    }))?;

    let dev_port = reserve_port()?;
    let mut dev_server = spawn_mcp_http_with_actor_role(
        app_dir.path(),
        dev_port,
        "/mcp",
        Some("test-openai-token"),
        Some("dev"),
    )?;
    let _ = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-dev",
        "method": "initialize",
        "params": {}
    }))?;

    let parent = arch_server.send(json!({
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
    let parent_task_id = parent["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_verify_dev_dispatch should return a parent task id")?;

    let prepare = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-prepare-agent",
        "method": "tools/call",
        "params": {
            "name": "forge_prepare_agent_dispatch",
            "arguments": {
                "project_dir": project_root
            }
        }
    }))?;
    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared agent dispatch prompt should be a string")?;

    let child = dev_server.send(json!({
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
                "agent_command": agent_command,
                "parent_task_id": parent_task_id,
                "supervision_strategy": "one_for_one",
                "child_slot": "agent-1"
            }
        }
    }))?;
    let child_task_id = child["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_dispatch_agent should return a child task id")?;
    assert_eq!(
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]["parent_task_id"],
        parent_task_id
    );
    assert_eq!(
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]["supervision_strategy"],
        "one_for_one"
    );
    assert_eq!(
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-1"
    );

    let parent_status = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-status-parent",
        "method": "tools/call",
        "params": {
            "name": "forge_status",
            "arguments": {
                "task_id": parent_task_id
            }
        }
    }))?;
    assert!(parent_status["result"]["structuredContent"]["supervision"]["parent_receipt"].is_null());
    let child_receipts = parent_status["result"]["structuredContent"]["supervision"]["child_receipts"]
        .as_array()
        .context("parent supervision child_receipts should be an array")?;
    assert_eq!(child_receipts.len(), 1);
    assert_eq!(child_receipts[0]["child_task_id"], child_task_id);
    assert_eq!(child_receipts[0]["child_dispatch_role"], "agent");
    assert_eq!(child_receipts[0]["child_dispatch_tool_name"], "forge_dispatch_agent");
    assert_eq!(child_receipts[0]["child_slot"], "agent-1");

    let child_status = dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "forge-status-child",
        "method": "tools/call",
        "params": {
            "name": "forge_status",
            "arguments": {
                "task_id": child_task_id
            }
        }
    }))?;
    assert_eq!(
        child_status["result"]["structuredContent"]["supervision"]["parent_receipt"]["parent_task_id"],
        parent_task_id
    );
    assert_eq!(
        child_status["result"]["structuredContent"]["supervision"]["child_receipts"],
        json!([])
    );

    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let stored_receipt = connection.query_row(
        "SELECT parent_task_id, child_task_id, supervision_scope, supervision_strategy, child_dispatch_role, child_dispatch_tool_name, child_slot FROM plugin_forge_dispatch_receipts WHERE child_task_id = ?1",
        [child_task_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    assert_eq!(stored_receipt.0, parent_task_id);
    assert_eq!(stored_receipt.1, child_task_id);
    assert_eq!(stored_receipt.2, "dispatch_pipeline");
    assert_eq!(stored_receipt.3, "one_for_one");
    assert_eq!(stored_receipt.4, "agent");
    assert_eq!(stored_receipt.5, "forge_dispatch_agent");
    assert_eq!(stored_receipt.6.as_deref(), Some("agent-1"));

    let _ = wait_for_terminal_status_http(&mut dev_server, child_task_id)?;

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

fn wait_for_terminal_status_http(server: &mut SpawnedHttpMcp, task_id: i64) -> Result<Value> {
    for _ in 0..200 {
        let status = server.send(json!({
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
        let task_status = status["result"]["structuredContent"]["task"]["status"]
            .as_str()
            .context("forge_status should return a task status string")?;
        if matches!(task_status, "Done" | "Failed" | "Cancelled" | "Blocked") {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(25));
    }

    bail!("timed out waiting for forge task {task_id} to reach a terminal state")
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

fn reserve_port() -> Result<u16> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).context("failed to reserve a local MCP HTTP port")?;
    Ok(listener.local_addr()?.port())
}

fn spawn_mcp_http(
    app_dir: &PathBuf,
    port: u16,
    endpoint: &str,
    openai_api_key: Option<&str>,
) -> Result<SpawnedHttpMcp> {
    spawn_mcp_http_with_actor_role(app_dir, port, endpoint, openai_api_key, None)
}

fn spawn_mcp_http_with_actor_role(
    app_dir: &PathBuf,
    port: u16,
    endpoint: &str,
    openai_api_key: Option<&str>,
    actor_role: Option<&str>,
) -> Result<SpawnedHttpMcp> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_entrance"));
    command
        .arg("mcp")
        .arg("http")
        .arg("--port")
        .arg(port.to_string())
        .arg("--endpoint")
        .arg(endpoint);
    if let Some(actor_role) = actor_role {
        command.args(["--actor-role", actor_role]);
    }
    command
        .env("ENTRANCE_APP_DATA_DIR", app_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(openai_api_key) = openai_api_key {
        command.env("OPENAI_API_KEY", openai_api_key);
    } else {
        command.env_remove("OPENAI_API_KEY");
    }

    let mut child = command
        .spawn()
        .context("failed to spawn `entrance mcp http`")?;

    let stderr = child
        .stderr
        .take()
        .context("child stderr should be piped")?;

    Ok(SpawnedHttpMcp {
        child,
        stderr,
        endpoint: endpoint.to_string(),
        port,
    })
}

fn post_json_rpc(port: u16, endpoint: &str, request: &Value) -> Result<Value> {
    let response = ureq::post(&format!("http://127.0.0.1:{port}{endpoint}"))
        .set("content-type", "application/json")
        .send_string(&serde_json::to_string(request)?)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let body = response
        .into_string()
        .context("failed to read MCP HTTP response body")?;

    serde_json::from_str(&body).context("failed to parse MCP HTTP response JSON")
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
