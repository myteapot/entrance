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
            "forge_prepare_agent_dispatch",
            "forge_verify_agent_dispatch",
            "forge_prepare_dev_dispatch",
            "forge_verify_dev_dispatch",
            "forge_dispatch_agent",
            "forge_dispatch_dev",
            "forge_bootstrap_mcp_cycle",
            "forge_status",
            "forge_cancel",
            "nota_runtime_overview",
            "nota_write_checkpoint",
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
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
    let bootstrap_cycle = tools
        .iter()
        .find(|tool| tool["name"] == "forge_bootstrap_mcp_cycle")
        .context("forge_bootstrap_mcp_cycle should be listed")?;
    let nota_overview = tools
        .iter()
        .find(|tool| tool["name"] == "nota_runtime_overview")
        .context("nota_runtime_overview should be listed")?;
    let nota_checkpoint = tools
        .iter()
        .find(|tool| tool["name"] == "nota_write_checkpoint")
        .context("nota_write_checkpoint should be listed")?;
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
    assert_eq!(bootstrap_cycle["permission"]["actorRole"], "nota");
    assert_eq!(bootstrap_cycle["permission"]["primitive"], "assign");
    assert_eq!(bootstrap_cycle["permission"]["room"], "strategy");
    assert!(bootstrap_cycle["dispatchRole"].is_null());
    assert_eq!(nota_overview["permission"]["actorRole"], "nota");
    assert_eq!(nota_overview["permission"]["primitive"], "chat");
    assert_eq!(nota_overview["permission"]["room"], "surface");
    assert_eq!(nota_overview["permission"]["targetLayer"], "cold");
    assert!(nota_overview["dispatchRole"].is_null());
    assert_eq!(nota_checkpoint["permission"]["actorRole"], "nota");
    assert_eq!(nota_checkpoint["permission"]["primitive"], "learn");
    assert_eq!(nota_checkpoint["permission"]["room"], "memory");
    assert_eq!(nota_checkpoint["permission"]["targetLayer"], "cold");
    assert!(nota_checkpoint["dispatchRole"].is_null());
    assert_eq!(prepare_agent["dispatchRole"], "agent");

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
            "forge_prepare_agent_dispatch",
            "forge_verify_agent_dispatch",
            "forge_dispatch_agent",
            "forge_status",
            "forge_cancel",
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
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
    assert_eq!(
        forbidden["result"]["structuredContent"]["currentActorRole"],
        "dev"
    );
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
    assert!(vault_list["result"]["dispatchRole"].is_null());

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
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
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
    assert!(vault_list["result"]["dispatchRole"].is_null());

    let mut nota_server = spawn_mcp_stdio_with_actor_role(app_dir.path(), None, Some("nota"))?;
    nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = nota_server.read_response()?;
    assert_eq!(initialize["id"], "initialize-nota");
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");
    nota_server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;
    nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools-nota",
        "method": "tools/list"
    }))?;
    let tools = nota_server.read_response()?;
    assert_eq!(tools["result"]["entranceSurface"]["actorRole"], "nota");
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
            "forge_bootstrap_mcp_cycle",
            "forge_status",
            "forge_cancel",
            "nota_runtime_overview",
            "nota_write_checkpoint",
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );
    nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "vault-list-nota",
        "method": "tools/call",
        "params": {
            "name": "vault_list_mcp",
            "arguments": {}
        }
    }))?;
    let vault_list = nota_server.read_response()?;
    assert_eq!(vault_list["result"]["isError"], false);
    assert_eq!(vault_list["result"]["entranceSurface"]["actorRole"], "nota");
    assert!(vault_list["result"]["permission"].is_null());
    assert!(vault_list["result"]["dispatchRole"].is_null());

    Ok(())
}

#[test]
fn external_client_can_read_recovery_seed_runtime_surface_over_stdio() -> Result<()> {
    let app_dir = TempAppDir::new("recovery-surface")?;
    seed_app_state(app_dir.path())?;
    seed_recovery_runtime_surface(app_dir.path())?;

    let mut server = spawn_mcp_stdio(app_dir.path(), None)?;
    server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    let _ = server.read_response()?;
    server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "recovery-runs",
        "method": "tools/call",
        "params": {
            "name": "recovery_list_seed_runs",
            "arguments": {}
        }
    }))?;
    let runs = server.read_response()?;
    assert_eq!(runs["result"]["isError"], false);
    let runs = runs["result"]["structuredContent"]
        .as_array()
        .context("recovery_list_seed_runs should return an array")?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["source_system"], "recovery_seed");
    assert_eq!(runs[0]["imported_table_count"], 3);
    assert_eq!(runs[0]["imported_row_count"], 3);
    assert_eq!(runs[0]["table_row_counts"]["documents"], 1);

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "recovery-rows",
        "method": "tools/call",
        "params": {
            "name": "recovery_list_seed_rows",
            "arguments": {
                "table_name": "documents",
                "limit": 5
            }
        }
    }))?;
    let rows = server.read_response()?;
    assert_eq!(rows["result"]["isError"], false);
    assert_eq!(
        rows["result"]["structuredContent"]["requested_table"],
        "documents"
    );
    assert_eq!(
        rows["result"]["structuredContent"]["total_matching_rows"],
        1
    );
    assert_eq!(
        rows["result"]["structuredContent"]["rows"][0]["source_row"]["title"],
        "Recovered MCP doc"
    );
    assert_eq!(
        rows["result"]["structuredContent"]["rows"][0]["promotion_state"],
        "storage_only"
    );

    Ok(())
}

#[test]
fn external_client_can_read_nota_runtime_overview_over_stdio() -> Result<()> {
    let app_dir = TempAppDir::new("nota-overview")?;
    seed_app_state(app_dir.path())?;
    seed_nota_runtime_overview(app_dir.path())?;

    let mut server = spawn_mcp_stdio_with_actor_role(app_dir.path(), None, Some("nota"))?;
    server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-overview",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = server.read_response()?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");
    server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-overview",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_overview",
            "arguments": {}
        }
    }))?;
    let overview = server.read_response()?;

    assert_eq!(overview["result"]["isError"], false);
    assert_eq!(overview["result"]["entranceSurface"]["actorRole"], "nota");
    assert_eq!(overview["result"]["permission"]["actorRole"], "nota");
    assert_eq!(overview["result"]["permission"]["primitive"], "chat");
    assert_eq!(overview["result"]["permission"]["room"], "surface");
    assert_eq!(overview["result"]["permission"]["targetLayer"], "cold");
    assert_eq!(
        overview["result"]["structuredContent"]["checkpoints"]["checkpoint_count"],
        1
    );
    assert_eq!(
        overview["result"]["structuredContent"]["decisions"]["decision_count"],
        1
    );
    assert_eq!(
        overview["result"]["structuredContent"]["chat_captures"]["capture_count"],
        1
    );
    assert_eq!(
        overview["result"]["structuredContent"]["transactions"]["transaction_count"],
        0
    );
    assert_eq!(
        overview["result"]["structuredContent"]["chat_policy"]["setting"]["archive_policy"],
        "full"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["checkpoints"]["checkpoints"][0]["payload"]
            ["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host"
    );

    Ok(())
}

#[test]
fn external_client_can_write_nota_runtime_checkpoint_over_stdio() -> Result<()> {
    let app_dir = TempAppDir::new("nota-write-checkpoint")?;
    seed_app_state(app_dir.path())?;

    let mut server = spawn_mcp_stdio_with_actor_role(app_dir.path(), None, Some("nota"))?;
    server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-write",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = server.read_response()?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");
    server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-write-checkpoint",
        "method": "tools/call",
        "params": {
            "name": "nota_write_checkpoint",
            "arguments": {
                "title": "MCP checkpoint write",
                "stable_level": "single-ingress, checkpointed, DB-first NOTA host with MCP checkpoint write",
                "landed": ["MCP checkpoint write landed"],
                "remaining": ["Drive a real MCP/runtime Do transaction"],
                "human_continuity_bus": "reduced but still partially required",
                "selected_trunk": "MCP/runtime Do host proof",
                "next_start_hints": [
                    "Call nota_runtime_overview before other MCP work."
                ]
            }
        }
    }))?;
    let checkpoint = server.read_response()?;

    assert_eq!(checkpoint["result"]["isError"], false);
    assert_eq!(checkpoint["result"]["entranceSurface"]["actorRole"], "nota");
    assert_eq!(checkpoint["result"]["permission"]["actorRole"], "nota");
    assert_eq!(checkpoint["result"]["permission"]["primitive"], "learn");
    assert_eq!(checkpoint["result"]["permission"]["room"], "memory");
    assert_eq!(checkpoint["result"]["permission"]["targetLayer"], "cold");
    assert_eq!(
        checkpoint["result"]["structuredContent"]["checkpoint"]["title"],
        "MCP checkpoint write"
    );
    assert_eq!(
        checkpoint["result"]["structuredContent"]["checkpoint"]["payload"]["selected_trunk"],
        "MCP/runtime Do host proof"
    );

    let overview = run_entrance_cli(app_dir.path(), &["nota", "overview"])?;
    let overview: Value =
        serde_json::from_str(&overview).context("nota overview output should be valid JSON")?;
    assert_eq!(overview["checkpoints"]["checkpoint_count"], 1);
    assert_eq!(overview["checkpoints"]["current_checkpoint_id"], 1);
    assert_eq!(
        overview["checkpoints"]["checkpoints"][0]["title"],
        "MCP checkpoint write"
    );
    assert_eq!(
        overview["checkpoints"]["checkpoints"][0]["payload"]["next_start_hints"][0],
        "Call nota_runtime_overview before other MCP work."
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
            "name": "forge_prepare_agent_dispatch",
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

    server.send(json!({
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
    let verify = server.read_response()?;
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
    assert_eq!(
        prepare["result"]["canonicalToolName"],
        "forge_prepare_agent_dispatch"
    );

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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_agent");

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
    assert_eq!(metadata["dispatch_tool_name"], "forge_dispatch_dev");

    Ok(())
}

#[test]
fn external_client_can_observe_dispatch_supervision_receipts_over_stdio() -> Result<()> {
    let app_dir = TempAppDir::new("forge-supervision-stdio")?;
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

    let mut arch_server =
        spawn_mcp_stdio_with_actor_role(app_dir.path(), Some("test-openai-token"), Some("arch"))?;
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-arch",
        "method": "initialize",
        "params": {}
    }))?;
    let _ = arch_server.read_response()?;
    arch_server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    let mut dev_server =
        spawn_mcp_stdio_with_actor_role(app_dir.path(), Some("test-openai-token"), Some("dev"))?;
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-dev",
        "method": "initialize",
        "params": {}
    }))?;
    let _ = dev_server.read_response()?;
    dev_server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    arch_server.send(json!({
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
    let parent = arch_server.read_response()?;
    let parent_task_id = parent["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_verify_dev_dispatch should return a parent task id")?;

    dev_server.send(json!({
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
    let prepare = dev_server.read_response()?;
    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let prompt = prepare["result"]["structuredContent"]["prompt"]
        .as_str()
        .context("prepared agent dispatch prompt should be a string")?;

    dev_server.send(json!({
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
    let child = dev_server.read_response()?;
    let child_task_id = child["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("forge_dispatch_agent should return a child task id")?;
    assert_eq!(
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]["parent_task_id"],
        parent_task_id
    );
    assert_eq!(
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]
            ["supervision_strategy"],
        "one_for_one"
    );
    assert_eq!(
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-1"
    );

    dev_server.send(json!({
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
    let parent_status = dev_server.read_response()?;
    assert!(
        parent_status["result"]["structuredContent"]["supervision"]["parent_receipt"].is_null()
    );
    let child_receipts = parent_status["result"]["structuredContent"]["supervision"]
        ["child_receipts"]
        .as_array()
        .context("parent supervision child_receipts should be an array")?;
    assert_eq!(child_receipts.len(), 1);
    assert_eq!(child_receipts[0]["child_task_id"], child_task_id);
    assert_eq!(child_receipts[0]["child_dispatch_role"], "agent");
    assert_eq!(
        child_receipts[0]["child_dispatch_tool_name"],
        "forge_dispatch_agent"
    );
    assert_eq!(child_receipts[0]["child_slot"], "agent-1");

    dev_server.send(json!({
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
    let child_status = dev_server.read_response()?;
    assert_eq!(
        child_status["result"]["structuredContent"]["supervision"]["parent_receipt"]
            ["parent_task_id"],
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

    let _ = wait_for_terminal_status_stdio(&mut dev_server, child_task_id)?;

    Ok(())
}

#[test]
fn external_client_can_bootstrap_allocator_cycle_over_nota_stdio_surface() -> Result<()> {
    let app_dir = TempAppDir::new("forge-bootstrap-allocator-stdio")?;
    seed_app_state(app_dir.path())?;

    let project_root = app_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    let role_dir = bootstrap_skill.join("roles");
    fs::create_dir_all(&role_dir)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
    fs::write(role_dir.join("dev.md"), "# test dev role\n")?;
    init_git_repo_with_commit(&project_root)?;

    let managed_worktree = app_dir
        .path()
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    add_git_worktree(&project_root, &managed_worktree, "feat-MYT-48")?;

    let agent_command = write_stub_agent_command(app_dir.path())?
        .to_string_lossy()
        .to_string();

    let mut nota_server =
        spawn_mcp_stdio_with_actor_role(app_dir.path(), Some("test-openai-token"), Some("nota"))?;
    nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota",
        "method": "initialize",
        "params": {}
    }))?;
    let initialize = nota_server.read_response()?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");
    nota_server.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))?;

    nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "bootstrap-cycle",
        "method": "tools/call",
        "params": {
            "name": "forge_bootstrap_mcp_cycle",
            "arguments": {
                "project_dir": project_root,
                "agent_command": agent_command,
                "agent_count": 2
            }
        }
    }))?;
    let bootstrap = nota_server.read_response()?;

    assert_eq!(bootstrap["id"], "bootstrap-cycle");
    assert_eq!(bootstrap["result"]["isError"], false);
    assert_eq!(bootstrap["result"]["entranceSurface"]["actorRole"], "nota");
    assert_eq!(bootstrap["result"]["permission"]["actorRole"], "nota");
    assert_eq!(bootstrap["result"]["permission"]["primitive"], "assign");
    assert_eq!(bootstrap["result"]["permission"]["room"], "strategy");
    assert_eq!(bootstrap["result"]["permission"]["targetLayer"], "hot");
    assert!(bootstrap["result"]["dispatchRole"].is_null());
    assert!(bootstrap["result"]["canonicalToolName"].is_null());

    let report = &bootstrap["result"]["structuredContent"];
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
    assert_eq!(report["requested_agent_count"], 2);
    assert_eq!(report["agent_worktree_mode"], "per_agent_slot_worktree");
    assert!(report["shared_worktree_boundary"].is_null());

    let worktree_path = managed_worktree.to_string_lossy().replace('\\', "/");
    let slot_one_worktree = app_dir
        .path()
        .join("worktrees")
        .join("Entrance")
        .join("slots")
        .join("MYT-48")
        .join("agent-1")
        .to_string_lossy()
        .replace('\\', "/");
    let slot_two_worktree = app_dir
        .path()
        .join("worktrees")
        .join("Entrance")
        .join("slots")
        .join("MYT-48")
        .join("agent-2")
        .to_string_lossy()
        .replace('\\', "/");
    assert_ne!(slot_one_worktree, worktree_path);
    assert_ne!(slot_two_worktree, worktree_path);

    let parent_task_id = report["dev_assignment"]["task_id"]
        .as_i64()
        .context("dev assignment should include a task id")?;
    assert!(parent_task_id > 0);
    assert_eq!(report["dev_assignment"]["dispatch"]["dispatch_role"], "dev");
    assert_eq!(report["dev_assignment"]["task_status"], "Done");
    assert_eq!(
        report["dev_assignment"]["execution_mode"],
        "bootstrap_dev_runtime_task"
    );
    assert_eq!(
        report["dev_assignment"]["dispatch"]["dispatch_tool_name"],
        "forge_dispatch_dev"
    );
    assert!(report["dev_assignment"]["dispatch"]["prompt"].is_null());
    assert_eq!(report["parent_status"]["task"]["status"], "Done");

    assert_eq!(report["agent_prepare"]["dispatch_role"], "agent");
    assert_eq!(
        report["agent_prepare"]["dispatch_tool_name"],
        "forge_dispatch_agent"
    );
    assert_eq!(report["agent_prepare"]["worktree_path"], slot_one_worktree);
    assert_eq!(report["agent_prepare"]["child_slot"], "agent-1");
    assert!(report["agent_prepare"]["prompt"].is_null());
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
    assert_eq!(agent_dispatches[0]["dispatch"]["dispatch_role"], "agent");
    assert_eq!(
        agent_dispatches[0]["dispatch"]["supervision"]["parent_receipt"]["parent_task_id"],
        parent_task_id
    );
    assert_eq!(
        agent_dispatches[0]["dispatch"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-1"
    );
    assert_eq!(
        agent_dispatches[0]["dispatch"]["task"]["working_dir"],
        slot_one_worktree
    );
    assert_eq!(
        agent_dispatches[1]["dispatch"]["supervision"]["parent_receipt"]["child_slot"],
        "agent-2"
    );
    assert_eq!(
        agent_dispatches[1]["dispatch"]["task"]["working_dir"],
        slot_two_worktree
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

    let db_path = app_dir.path().join("entrance.db");
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

fn seed_recovery_runtime_surface(app_dir: &PathBuf) -> Result<()> {
    let recovery_seed_path = write_test_recovery_seed(app_dir)?;
    run_entrance_cli(
        app_dir,
        &[
            "recovery",
            "import-seed",
            "--file",
            recovery_seed_path
                .to_str()
                .context("recovery seed path should be valid UTF-8")?,
        ],
    )?;
    Ok(())
}

fn seed_nota_runtime_overview(app_dir: &PathBuf) -> Result<()> {
    run_entrance_cli(
        app_dir,
        &[
            "nota",
            "checkpoint",
            "--stable-level",
            "single-ingress, checkpointed, DB-first NOTA host",
            "--landed",
            "cadence cut landed",
            "--remaining",
            "headless continuity bundle",
            "--human-continuity-bus",
            "reduced but still present",
        ],
    )?;
    run_entrance_cli(
        app_dir,
        &[
            "nota",
            "decision",
            "--title",
            "Chat is the continuity surface",
            "--statement",
            "Chat should read the runtime DB continuity bundle instead of replaying raw chat.",
            "--rationale",
            "Resume should start from canonical runtime state.",
            "--decision-type",
            "ui_surface",
            "--scope-type",
            "project",
            "--scope-ref",
            "Entrance",
            "--source-ref",
            "nota:test:mcp-stdio-overview",
        ],
    )?;
    run_entrance_cli(app_dir, &["nota", "chat-policy", "--policy", "full"])?;
    run_entrance_cli(
        app_dir,
        &[
            "nota",
            "capture-chat",
            "--role",
            "nota",
            "--content",
            "Overview should expose checkpoint, decision, and archive state together.",
        ],
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

fn run_entrance_cli(app_dir: &PathBuf, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(args)
        .env("ENTRANCE_APP_DATA_DIR", app_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .with_context(|| format!("failed to spawn `entrance {}`", args.join(" ")))?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")
}

fn write_test_recovery_seed(root: &PathBuf) -> Result<PathBuf> {
    let db_path = root.join("recovery-seed.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute_batch(
        r#"
        CREATE TABLE schema_meta (
            version INTEGER,
            applied_at TEXT
        );
        CREATE TABLE documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            category TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            priority INTEGER NOT NULL DEFAULT 2,
            project TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            done_at TEXT,
            temperature TEXT NOT NULL DEFAULT 'warm',
            due_on TEXT NOT NULL DEFAULT '',
            remind_every_days INTEGER NOT NULL DEFAULT 0,
            remind_next_on TEXT NOT NULL DEFAULT '',
            last_reminded_at TEXT NOT NULL DEFAULT '',
            reminder_status TEXT NOT NULL DEFAULT 'none'
        );
        "#,
    )?;
    connection.execute(
        "INSERT INTO schema_meta (version, applied_at) VALUES (?1, ?2)",
        (8, "2026-03-23T00:00:00Z"),
    )?;
    connection.execute(
        r#"
        INSERT INTO documents (id, slug, title, content, category, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        (
            1,
            "recovery-doc",
            "Recovered MCP doc",
            "# recovery",
            "architecture",
            "2026-03-23T00:00:00Z",
            "2026-03-23T00:10:00Z",
        ),
    )?;
    connection.execute(
        r#"
        INSERT INTO todos (
            id, title, status, priority, project, created_at, done_at, temperature,
            due_on, remind_every_days, remind_next_on, last_reminded_at, reminder_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        (
            1,
            "Recovered MCP todo",
            "pending",
            1,
            "Entrance",
            "2026-03-23T00:15:00Z",
            "warm",
            "",
            0,
            "",
            "",
            "none",
        ),
    )?;

    Ok(db_path)
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

fn init_git_repo_with_commit(path: &PathBuf) -> Result<()> {
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

fn add_git_worktree(repo_root: &PathBuf, worktree_path: &PathBuf, branch: &str) -> Result<()> {
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
