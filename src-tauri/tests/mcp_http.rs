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

fn assert_spawnable_codex_command(command: &str) {
    if cfg!(windows) {
        let normalized = command.replace('\\', "/").to_ascii_lowercase();
        let is_spawnable_codex = normalized == "codex"
            || normalized == "codex.cmd"
            || normalized.ends_with("/codex.cmd")
            || normalized == "codex.exe"
            || normalized.ends_with("/codex.exe")
            || normalized == "codex.bat"
            || normalized.ends_with("/codex.bat")
            || normalized == "codex.com"
            || normalized.ends_with("/codex.com");
        assert!(
            is_spawnable_codex,
            "expected a spawnable codex command, got {command}"
        );
    } else {
        assert_eq!(command, "codex");
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
            "forge_bootstrap_mcp_cycle",
            "forge_status",
            "forge_cancel",
            "nota_runtime_overview",
            "nota_runtime_status",
            "nota_runtime_allocations",
            "nota_runtime_receipts",
            "nota_do",
            "nota_dev",
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
    let nota_receipts = tools
        .iter()
        .find(|tool| tool["name"] == "nota_runtime_receipts")
        .context("nota_runtime_receipts should be listed")?;
    let nota_do = tools
        .iter()
        .find(|tool| tool["name"] == "nota_do")
        .context("nota_do should be listed")?;
    let nota_dev = tools
        .iter()
        .find(|tool| tool["name"] == "nota_dev")
        .context("nota_dev should be listed")?;
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
    assert_eq!(nota_receipts["permission"]["actorRole"], "nota");
    assert_eq!(nota_receipts["permission"]["primitive"], "chat");
    assert_eq!(nota_receipts["permission"]["room"], "surface");
    assert_eq!(nota_receipts["permission"]["targetLayer"], "cold");
    assert!(nota_receipts["dispatchRole"].is_null());
    assert_eq!(nota_do["permission"]["actorRole"], "nota");
    assert_eq!(nota_do["permission"]["primitive"], "assign");
    assert_eq!(nota_do["permission"]["room"], "strategy");
    assert_eq!(nota_do["permission"]["targetLayer"], "hot");
    assert_eq!(nota_do["dispatchRole"], "agent");
    assert_eq!(nota_dev["permission"]["actorRole"], "nota");
    assert_eq!(nota_dev["permission"]["primitive"], "assign");
    assert_eq!(nota_dev["permission"]["room"], "strategy");
    assert_eq!(nota_dev["permission"]["targetLayer"], "hot");
    assert_eq!(nota_dev["dispatchRole"], "dev");
    assert_eq!(nota_checkpoint["permission"]["actorRole"], "nota");
    assert_eq!(nota_checkpoint["permission"]["primitive"], "learn");
    assert_eq!(nota_checkpoint["permission"]["room"], "memory");
    assert_eq!(nota_checkpoint["permission"]["targetLayer"], "cold");
    assert!(nota_checkpoint["dispatchRole"].is_null());
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
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
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
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
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

    let port = reserve_port()?;
    let mut nota_server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;
    let initialize = nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize-nota");
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");
    let tools = nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools-nota",
        "method": "tools/list"
    }))?;
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
            "nota_runtime_status",
            "nota_runtime_allocations",
            "nota_runtime_receipts",
            "nota_do",
            "nota_dev",
            "nota_write_checkpoint",
            "recovery_list_seed_runs",
            "recovery_list_seed_rows",
            "vault_get_token",
            "vault_list_mcp",
            "launcher_search",
            "launcher_launch",
        ]
    );
    let vault_list = nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "vault-list-nota",
        "method": "tools/call",
        "params": {
            "name": "vault_list_mcp",
            "arguments": {}
        }
    }))?;
    assert_eq!(vault_list["result"]["isError"], false);
    assert_eq!(vault_list["result"]["entranceSurface"]["actorRole"], "nota");
    assert!(vault_list["result"]["permission"].is_null());
    assert!(vault_list["result"]["dispatchRole"].is_null());

    Ok(())
}

#[test]
fn external_client_can_read_recovery_seed_runtime_surface_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("recovery-surface")?;
    seed_app_state(app_dir.path())?;
    seed_recovery_runtime_surface(app_dir.path())?;

    let port = reserve_port()?;
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp", None)?;

    let _ = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;

    let runs = server.send(json!({
        "jsonrpc": "2.0",
        "id": "recovery-runs",
        "method": "tools/call",
        "params": {
            "name": "recovery_list_seed_runs",
            "arguments": {}
        }
    }))?;
    assert_eq!(runs["result"]["isError"], false);
    let runs = runs["result"]["structuredContent"]
        .as_array()
        .context("recovery_list_seed_runs should return an array")?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["source_system"], "recovery_seed");
    assert_eq!(runs[0]["imported_table_count"], 3);
    assert_eq!(runs[0]["imported_row_count"], 3);
    assert_eq!(runs[0]["table_row_counts"]["documents"], 1);

    let rows = server.send(json!({
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
fn external_client_can_read_nota_runtime_overview_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-overview")?;
    seed_app_state(app_dir.path())?;
    seed_nota_runtime_overview(app_dir.path())?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-overview",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let overview = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-overview",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_overview",
            "arguments": {}
        }
    }))?;

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
        overview["result"]["structuredContent"]["allocations"]["allocation_count"],
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
fn external_client_can_read_nota_runtime_status_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-status")?;
    seed_app_state(app_dir.path())?;
    seed_nota_runtime_overview(app_dir.path())?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-status",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;

    assert_eq!(status["result"]["isError"], false);
    assert_eq!(status["result"]["entranceSurface"]["actorRole"], "nota");
    assert_eq!(status["result"]["permission"]["actorRole"], "nota");
    assert_eq!(status["result"]["permission"]["primitive"], "chat");
    assert_eq!(status["result"]["permission"]["room"], "surface");
    assert_eq!(status["result"]["permission"]["targetLayer"], "cold");
    assert_eq!(status["result"]["structuredContent"]["checkpoint_count"], 1);
    assert_eq!(
        status["result"]["structuredContent"]["current_checkpoint_id"],
        1
    );
    assert_eq!(
        status["result"]["structuredContent"]["current_checkpoint"]["payload"]["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host"
    );
    assert_eq!(
        status["result"]["structuredContent"]["transaction_count"],
        0
    );
    assert!(status["result"]["structuredContent"]["latest_transaction"].is_null());
    assert_eq!(status["result"]["structuredContent"]["allocation_count"], 0);
    assert!(status["result"]["structuredContent"]["latest_allocation"].is_null());
    assert_eq!(status["result"]["structuredContent"]["receipt_count"], 0);
    assert!(status["result"]["structuredContent"]["latest_receipt"].is_null());
    assert_eq!(status["result"]["structuredContent"]["decision_count"], 1);
    assert_eq!(
        status["result"]["structuredContent"]["latest_decision"]["title"],
        "Chat is the continuity surface"
    );
    assert_eq!(
        status["result"]["structuredContent"]["chat_capture_count"],
        1
    );
    assert_eq!(
        status["result"]["structuredContent"]["chat_policy"]["setting"]["archive_policy"],
        "full"
    );
    assert!(status["result"]["structuredContent"]["recommended_checkpoint"].is_null());

    Ok(())
}

#[test]
fn external_client_can_write_nota_runtime_checkpoint_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-write-checkpoint")?;
    seed_app_state(app_dir.path())?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-write",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let checkpoint = server.send(json!({
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
fn external_client_can_create_nota_do_transaction_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-do")?;
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

    let agent_command = write_stub_agent_command(app_dir.path())?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-do",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let do_report = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-do",
        "method": "tools/call",
        "params": {
            "name": "nota_do",
            "arguments": {
                "project_dir": project_root,
                "model": "codex",
                "agent_command": agent_command,
                "title": "MCP Do dispatch MYT-48"
            }
        }
    }))?;

    assert_eq!(do_report["result"]["isError"], false);
    assert_eq!(do_report["result"]["entranceSurface"]["actorRole"], "nota");
    assert_eq!(do_report["result"]["permission"]["actorRole"], "nota");
    assert_eq!(do_report["result"]["permission"]["primitive"], "assign");
    assert_eq!(do_report["result"]["permission"]["room"], "strategy");
    assert_eq!(do_report["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(do_report["result"]["dispatchRole"], "agent");
    assert_eq!(
        do_report["result"]["structuredContent"]["transaction"]["surface_action"],
        "do"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["transaction"]["transaction_kind"],
        "forge_agent_dispatch"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["dispatch"]["issue_id"],
        "MYT-48"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["allocation"]["allocator_role"],
        "nota"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["allocation"]["allocator_surface"],
        "nota_do"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["allocation"]["source_transaction_id"],
        do_report["result"]["structuredContent"]["transaction"]["id"]
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["allocation"]["child_execution_kind"],
        "forge_task"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["allocation"]["return_target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["allocation"]["escalation_target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["checkpoint"]["cadence_kind"],
        "CADENCE_CHECKPOINT"
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["spawn_error"],
        Value::Null
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["receipts"]
            .as_array()
            .context("nota_do receipts should be an array")?
            .len(),
        5
    );
    assert_eq!(
        do_report["result"]["structuredContent"]["receipts"][2]["receipt_kind"],
        "ALLOCATION_RECORDED"
    );
    let transaction_id = do_report["result"]["structuredContent"]["transaction"]["id"]
        .as_i64()
        .context("nota_do should return a transaction id")?;
    let allocation_id = do_report["result"]["structuredContent"]["allocation"]["id"]
        .as_i64()
        .context("nota_do should return an allocation id")?;
    let lineage_ref = do_report["result"]["structuredContent"]["allocation"]["lineage_ref"]
        .as_str()
        .context("nota_do should return an allocation lineage_ref")?;
    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let task_id = do_report["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("nota_do should return a task id")?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = NULL, finished_at = NULL WHERE id = ?1",
        rusqlite::params![task_id, "Running"],
    )?;

    let allocations = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-allocations",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_allocations",
            "arguments": {}
        }
    }))?;
    assert_eq!(allocations["result"]["isError"], false);
    assert_eq!(
        allocations["result"]["entranceSurface"]["actorRole"],
        "nota"
    );
    assert_eq!(allocations["result"]["permission"]["actorRole"], "nota");
    assert_eq!(allocations["result"]["permission"]["primitive"], "chat");
    assert_eq!(allocations["result"]["permission"]["room"], "surface");
    assert_eq!(allocations["result"]["permission"]["targetLayer"], "cold");
    assert_eq!(
        allocations["result"]["structuredContent"]["allocation_count"],
        1
    );
    assert_eq!(
        allocations["result"]["structuredContent"]["allocations"][0]["source_transaction_id"],
        do_report["result"]["structuredContent"]["transaction"]["id"]
    );
    assert_eq!(
        allocations["result"]["structuredContent"]["allocations"][0]["child_dispatch_role"],
        "agent"
    );
    assert_eq!(
        allocations["result"]["structuredContent"]["allocations"][0]["child_dispatch_tool_name"],
        "forge_dispatch_agent"
    );

    let overview = run_entrance_cli(app_dir.path(), &["nota", "overview"])?;
    let overview: Value =
        serde_json::from_str(&overview).context("nota overview output should be valid JSON")?;
    assert_eq!(overview["transactions"]["transaction_count"], 1);
    assert_eq!(
        overview["transactions"]["transactions"][0]["surface_action"],
        "do"
    );
    assert_eq!(overview["allocations"]["allocation_count"], 1);
    assert_eq!(
        overview["allocations"]["allocations"][0]["source_transaction_id"],
        do_report["result"]["structuredContent"]["transaction"]["id"]
    );
    assert_eq!(
        overview["allocations"]["allocations"][0]["child_dispatch_role"],
        "agent"
    );
    assert_eq!(
        overview["allocations"]["allocations"][0]["child_dispatch_tool_name"],
        "forge_dispatch_agent"
    );
    assert_eq!(overview["checkpoints"]["checkpoint_count"], 1);
    assert_eq!(
        overview["checkpoints"]["checkpoints"][0]["title"],
        "Do allocation: MYT-48"
    );
    assert!(overview["recommended_checkpoint"].is_null());
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM nota_runtime_transactions",
            [],
            |row| { row.get::<_, i64>(0) }
        )?,
        1
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM nota_runtime_receipts", [], |row| {
            row.get::<_, i64>(0)
        })?,
        5
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM nota_runtime_allocations", [], |row| {
            row.get::<_, i64>(0)
        })?,
        1
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM cadence_objects", [], |row| {
            row.get::<_, i64>(0)
        })?,
        1
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM plugin_forge_tasks", [], |row| {
            row.get::<_, i64>(0)
        })?,
        1
    );

    let blocked_message = "add openai to Vault first";
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = ?3, finished_at = ?4 WHERE id = ?1",
        rusqlite::params![task_id, "Blocked", blocked_message, "2026-03-23T00:00:00Z"],
    )?;

    let blocked_overview = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-overview-terminal",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_overview",
            "arguments": {}
        }
    }))?;
    assert_eq!(blocked_overview["result"]["isError"], false);
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["allocations"]["allocations"][0]["status"],
        "escalated_blocked"
    );
    let blocked_payload_json = blocked_overview["result"]["structuredContent"]["allocations"]
        ["allocations"][0]["payload_json"]
        .as_str()
        .context("allocation payload_json should be present")?;
    let blocked_payload: Value = serde_json::from_str(blocked_payload_json)
        .context("allocation payload_json should stay valid JSON")?;
    assert_eq!(
        blocked_payload["terminal_outcome"]["boundary_kind"],
        "escalation"
    );
    assert_eq!(
        blocked_payload["terminal_outcome"]["child_execution_status"],
        "Blocked"
    );
    assert_eq!(
        blocked_payload["terminal_outcome"]["child_execution_status_message"],
        blocked_message
    );
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["recommended_checkpoint"]["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host with a minimal NOTA-owned agent escalation boundary checkpointed into runtime continuity"
    );
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["recommended_checkpoint"]["selected_trunk"],
        "agent escalation continuity"
    );
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["recommended_checkpoint"]["landed"][0],
        format!(
            "NOTA-owned agent allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
            allocation_id, lineage_ref, transaction_id, task_id
        )
    );
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["recommended_checkpoint"]["landed"][2],
        format!(
            "Transaction {transaction_id} receipt history includes terminal receipt ALLOCATION_TERMINAL_OUTCOME_RECORDED capturing allocation {} back to nota_runtime_transaction {}.",
            allocation_id, transaction_id
        )
    );
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["recommended_checkpoint"]["remaining"][0],
        format!(
            "L3 remains open until the current Blocked gate is cleared: {}.",
            blocked_message
        )
    );
    assert_eq!(
        blocked_overview["result"]["structuredContent"]["recommended_checkpoint"]["next_start_hints"][2],
        format!(
            "Treat lineage `{}` as the current agent escalation boundary until the Blocked gate is cleared.",
            lineage_ref
        )
    );
    assert!(blocked_overview["result"]["structuredContent"]["next_step"].is_null());

    let blocked_allocations = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-allocations-terminal",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_allocations",
            "arguments": {}
        }
    }))?;
    assert_eq!(blocked_allocations["result"]["isError"], false);
    assert_eq!(
        blocked_allocations["result"]["structuredContent"]["allocations"][0]["status"],
        "escalated_blocked"
    );
    let blocked_allocations_payload_json = blocked_allocations["result"]["structuredContent"]
        ["allocations"][0]["payload_json"]
        .as_str()
        .context("allocation payload_json should be present on dedicated MCP surface")?;
    let blocked_allocations_payload: Value = serde_json::from_str(blocked_allocations_payload_json)
        .context("dedicated MCP allocation payload_json should stay valid JSON")?;
    assert_eq!(
        blocked_allocations_payload["terminal_outcome"]["boundary_kind"],
        "escalation"
    );
    assert_eq!(
        blocked_allocations_payload["terminal_outcome"]["child_execution_status"],
        "Blocked"
    );
    assert_eq!(
        blocked_allocations_payload["terminal_outcome"]["child_execution_status_message"],
        blocked_message
    );

    let blocked_receipts = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-receipts-terminal",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_receipts",
            "arguments": {
                "transaction_id": do_report["result"]["structuredContent"]["transaction"]["id"]
            }
        }
    }))?;
    assert_eq!(blocked_receipts["result"]["isError"], false);
    assert_eq!(
        blocked_receipts["result"]["permission"]["actorRole"],
        "nota"
    );
    assert_eq!(
        blocked_receipts["result"]["permission"]["primitive"],
        "chat"
    );
    assert_eq!(blocked_receipts["result"]["permission"]["room"], "surface");
    assert_eq!(
        blocked_receipts["result"]["permission"]["targetLayer"],
        "cold"
    );
    assert_eq!(
        blocked_receipts["result"]["structuredContent"]["requested_transaction_id"],
        transaction_id
    );
    assert_eq!(
        blocked_receipts["result"]["structuredContent"]["receipt_count"],
        6
    );
    assert_eq!(
        blocked_receipts["result"]["structuredContent"]["receipts"][5]["receipt_kind"],
        "ALLOCATION_TERMINAL_OUTCOME_RECORDED"
    );
    let blocked_receipt_payload_json = blocked_receipts["result"]["structuredContent"]["receipts"]
        [5]["payload_json"]
        .as_str()
        .context("receipt payload_json should be present on dedicated MCP receipt surface")?;
    let blocked_receipt_payload: Value = serde_json::from_str(blocked_receipt_payload_json)
        .context("dedicated MCP receipt payload_json should stay valid JSON")?;
    assert_eq!(
        blocked_receipt_payload["lineage_ref"],
        do_report["result"]["structuredContent"]["allocation"]["lineage_ref"]
    );
    assert_eq!(blocked_receipt_payload["boundary_kind"], "escalation");
    assert_eq!(blocked_receipt_payload["child_execution_status"], "Blocked");
    assert_eq!(
        blocked_receipt_payload["child_execution_status_message"],
        blocked_message
    );

    let stored_allocation_outcome = connection.query_row(
        "SELECT status, payload_json FROM nota_runtime_allocations",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    assert_eq!(stored_allocation_outcome.0, "escalated_blocked");
    let stored_payload: Value = serde_json::from_str(&stored_allocation_outcome.1)
        .context("stored allocation payload_json should be valid JSON")?;
    assert_eq!(
        stored_payload["terminal_outcome"]["target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM nota_runtime_receipts", [], |row| {
            row.get::<_, i64>(0)
        })?,
        6
    );
    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM nota_runtime_receipts WHERE receipt_kind = 'ALLOCATION_TERMINAL_OUTCOME_RECORDED'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    let terminal_receipt = connection.query_row(
        "SELECT payload_json, created_at FROM nota_runtime_receipts WHERE receipt_kind = 'ALLOCATION_TERMINAL_OUTCOME_RECORDED'",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    assert!(!terminal_receipt.1.is_empty());
    let terminal_receipt_payload: Value = serde_json::from_str(&terminal_receipt.0)
        .context("terminal outcome receipt payload_json should be valid JSON")?;
    assert_eq!(
        terminal_receipt_payload["allocation_id"],
        do_report["result"]["structuredContent"]["allocation"]["id"]
    );
    assert_eq!(
        terminal_receipt_payload["lineage_ref"],
        do_report["result"]["structuredContent"]["allocation"]["lineage_ref"]
    );
    assert_eq!(terminal_receipt_payload["boundary_kind"], "escalation");
    assert_eq!(
        terminal_receipt_payload["child_execution_status"],
        "Blocked"
    );
    assert_eq!(
        terminal_receipt_payload["child_execution_status_message"],
        blocked_message
    );
    assert_eq!(
        terminal_receipt_payload["target_kind"],
        stored_payload["terminal_outcome"]["target_kind"]
    );
    assert_eq!(
        terminal_receipt_payload["target_ref"],
        stored_payload["terminal_outcome"]["target_ref"]
    );
    assert_eq!(
        terminal_receipt_payload["allocation_status"],
        "escalated_blocked"
    );

    Ok(())
}

#[test]
fn external_client_can_create_nota_owned_dev_transaction_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-dev")?;
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

    let agent_command = write_stub_agent_command(app_dir.path())?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota-dev",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let dev_report = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-dev",
        "method": "tools/call",
        "params": {
            "name": "nota_dev",
            "arguments": {
                "project_dir": project_root,
                "model": "codex",
                "agent_command": agent_command,
                "title": "MCP Dev dispatch MYT-48"
            }
        }
    }))?;

    assert_eq!(dev_report["result"]["isError"], false);
    assert_eq!(dev_report["result"]["entranceSurface"]["actorRole"], "nota");
    assert_eq!(dev_report["result"]["permission"]["actorRole"], "nota");
    assert_eq!(dev_report["result"]["permission"]["primitive"], "assign");
    assert_eq!(dev_report["result"]["permission"]["room"], "strategy");
    assert_eq!(dev_report["result"]["permission"]["targetLayer"], "hot");
    assert_eq!(dev_report["result"]["dispatchRole"], "dev");
    assert_eq!(
        dev_report["result"]["structuredContent"]["transaction"]["surface_action"],
        "dev"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["transaction"]["transaction_kind"],
        "forge_dev_dispatch"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["dispatch"]["dispatch_role"],
        "dev"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["allocation"]["allocator_surface"],
        "nota_dev"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["allocation"]["allocation_kind"],
        "forge_dev_dispatch"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["checkpoint"]["title"],
        "Dev allocation: MYT-48"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["checkpoint"]["payload"]["selected_trunk"],
        "NOTA-owned dev runtime cut"
    );
    assert_eq!(
        dev_report["result"]["structuredContent"]["receipts"]
            .as_array()
            .context("nota_dev receipts should be an array")?
            .len(),
        5
    );

    let allocation_payload_json = dev_report["result"]["structuredContent"]["allocation"]
        ["payload_json"]
        .as_str()
        .context("nota_dev allocation payload_json should be present")?;
    let allocation_payload: Value = serde_json::from_str(allocation_payload_json)
        .context("nota_dev allocation payload_json should be valid JSON")?;
    assert_eq!(allocation_payload["child_dispatch_role"], "dev");
    assert_eq!(
        allocation_payload["child_dispatch_tool_name"],
        "forge_dispatch_dev"
    );

    let transaction_id = dev_report["result"]["structuredContent"]["transaction"]["id"]
        .as_i64()
        .context("nota_dev should return a transaction id")?;
    let task_id = dev_report["result"]["structuredContent"]["task_id"]
        .as_i64()
        .context("nota_dev should return a task id")?;
    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = NULL, finished_at = NULL WHERE id = ?1",
        rusqlite::params![task_id, "Running"],
    )?;

    let status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-dev",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(status["result"]["isError"], false);
    assert_eq!(
        status["result"]["structuredContent"]["latest_transaction"]["transaction_kind"],
        "forge_dev_dispatch"
    );
    assert_eq!(
        status["result"]["structuredContent"]["latest_allocation"]["allocator_surface"],
        "nota_dev"
    );
    assert_eq!(
        status["result"]["structuredContent"]["latest_allocation"]["child_dispatch_role"],
        "dev"
    );
    assert_eq!(
        status["result"]["structuredContent"]["latest_allocation"]["child_dispatch_tool_name"],
        "forge_dispatch_dev"
    );
    assert_eq!(
        status["result"]["structuredContent"]["current_checkpoint"]["title"],
        "Dev allocation: MYT-48"
    );
    assert!(status["result"]["structuredContent"]["recommended_checkpoint"].is_null());
    assert!(status["result"]["structuredContent"]["integrate"].is_null());
    assert!(status["result"]["structuredContent"]["review"].is_null());
    assert!(status["result"]["structuredContent"]["next_step"].is_null());

    let allocations = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-allocations-dev",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_allocations",
            "arguments": {}
        }
    }))?;
    assert_eq!(allocations["result"]["isError"], false);
    assert_eq!(
        allocations["result"]["structuredContent"]["allocations"][0]["child_dispatch_role"],
        "dev"
    );
    assert_eq!(
        allocations["result"]["structuredContent"]["allocations"][0]["child_dispatch_tool_name"],
        "forge_dispatch_dev"
    );

    let receipts = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-receipts-dev",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_receipts",
            "arguments": {
                "transaction_id": transaction_id
            }
        }
    }))?;
    assert_eq!(receipts["result"]["isError"], false);
    assert_eq!(
        receipts["result"]["structuredContent"]["requested_transaction_id"],
        transaction_id
    );
    assert_eq!(receipts["result"]["structuredContent"]["receipt_count"], 5);
    assert_eq!(
        receipts["result"]["structuredContent"]["receipts"][4]["receipt_kind"],
        "CADENCE_CHECKPOINT_WRITTEN"
    );

    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = ?3, finished_at = ?4 WHERE id = ?1",
        rusqlite::params![task_id, "Blocked", "simulated escalation for next_step gating", "2026-03-24T00:00:00Z"],
    )?;
    let blocked_status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-dev-blocked",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(blocked_status["result"]["isError"], false);
    assert_eq!(
        blocked_status["result"]["structuredContent"]["latest_allocation"]["status"],
        "escalated_blocked"
    );
    assert!(blocked_status["result"]["structuredContent"]["integrate"].is_null());
    assert!(blocked_status["result"]["structuredContent"]["review"].is_null());
    assert!(blocked_status["result"]["structuredContent"]["next_step"].is_null());

    assert_eq!(
        connection.query_row(
            "SELECT COUNT(*) FROM nota_runtime_transactions",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM nota_runtime_receipts", [], |row| {
            row.get::<_, i64>(0)
        })?,
        6
    );
    assert_eq!(
        connection.query_row("SELECT COUNT(*) FROM nota_runtime_allocations", [], |row| {
            row.get::<_, i64>(0)
        })?,
        1
    );

    Ok(())
}

#[test]
fn external_client_can_read_dev_review_ready_next_step_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-dev-review-ready")?;
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
        .join("feat-MYT-49");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let agent_command = write_stub_agent_command(app_dir.path())?;
    let dev_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "dev",
            "--project-dir",
            project_root
                .to_str()
                .context("project root should be valid UTF-8")?,
            "--model",
            "codex",
            "--agent-command",
            agent_command
                .to_str()
                .context("agent command should be valid UTF-8")?,
            "--title",
            "HTTP review-ready MYT-49",
        ],
    )?;
    let dev_report: Value =
        serde_json::from_str(&dev_output).context("nota dev output should be valid JSON")?;
    let transaction_id = dev_report["transaction"]["id"]
        .as_i64()
        .context("transaction id should be present")?;
    let allocation_id = dev_report["allocation"]["id"]
        .as_i64()
        .context("allocation id should be present")?;
    let lineage_ref = dev_report["allocation"]["lineage_ref"]
        .as_str()
        .context("allocation lineage_ref should be present")?
        .to_string();
    let task_id = dev_report["task_id"]
        .as_i64()
        .context("task id should be present")?;

    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = NULL, finished_at = ?3 WHERE id = ?1",
        rusqlite::params![task_id, "Done", "2026-03-24T00:00:00Z"],
    )?;

    run_entrance_cli(app_dir.path(), &["nota", "allocations"])?;
    run_entrance_cli(app_dir.path(), &["nota", "checkpoint-runtime-closure"])?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;
    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-review-ready",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-review-ready",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(status["result"]["isError"], false);
    assert!(status["result"]["structuredContent"]["recommended_checkpoint"].is_null());
    assert_eq!(
        status["result"]["structuredContent"]["latest_receipt"]["receipt_kind"],
        "DEV_RETURN_REVIEW_READY"
    );
    assert!(status["result"]["structuredContent"]["integrate"].is_null());
    assert_eq!(
        status["result"]["structuredContent"]["review"]["state"],
        "review_ready"
    );
    assert_eq!(
        status["result"]["structuredContent"]["review"]["verdict"],
        Value::Null
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["step"],
        "review"
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["transaction_id"],
        transaction_id
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["allocation_id"],
        allocation_id
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["lineage_ref"],
        lineage_ref
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["child_dispatch_role"],
        "dev"
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["execution_host"],
        "detached_forge_cli_supervisor"
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"]["target_ref"],
        transaction_id.to_string()
    );

    let overview = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-overview-review-ready",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_overview",
            "arguments": {}
        }
    }))?;
    assert_eq!(overview["result"]["isError"], false);
    assert!(overview["result"]["structuredContent"]["recommended_checkpoint"].is_null());
    assert!(overview["result"]["structuredContent"]["integrate"].is_null());
    assert_eq!(
        overview["result"]["structuredContent"]["review"]["state"],
        "review_ready"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["step"],
        "review"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["transaction_id"],
        transaction_id
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["allocation_id"],
        allocation_id
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["lineage_ref"],
        lineage_ref
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["child_dispatch_role"],
        "dev"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["execution_host"],
        "detached_forge_cli_supervisor"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["target_ref"],
        transaction_id.to_string()
    );

    let receipts = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-receipts-review-ready",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_receipts",
            "arguments": {
                "transaction_id": transaction_id
            }
        }
    }))?;
    assert_eq!(receipts["result"]["isError"], false);
    assert_eq!(
        receipts["result"]["structuredContent"]["receipts"][8]["receipt_kind"],
        "DEV_RETURN_REVIEW_READY"
    );
    let review_ready_payload_json = receipts["result"]["structuredContent"]["receipts"][8]
        ["payload_json"]
        .as_str()
        .context("review-ready payload_json should be present")?;
    let review_ready_payload: Value = serde_json::from_str(review_ready_payload_json)
        .context("review-ready payload_json should be valid JSON")?;
    assert_eq!(review_ready_payload["step"], "review");
    assert_eq!(review_ready_payload["transaction_id"], transaction_id);
    assert_eq!(review_ready_payload["allocation_id"], allocation_id);
    assert_eq!(review_ready_payload["lineage_ref"], lineage_ref);
    assert_eq!(review_ready_payload["child_dispatch_role"], "dev");
    assert_eq!(
        review_ready_payload["execution_host"],
        "detached_forge_cli_supervisor"
    );
    assert_eq!(
        review_ready_payload["target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        review_ready_payload["target_ref"],
        transaction_id.to_string()
    );

    let review_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "review",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--verdict",
            "changes_requested",
            "--summary",
            "HTTP review requested a repair pass before integration.",
        ],
    )?;
    let review_report: Value =
        serde_json::from_str(&review_output).context("HTTP review output should be valid JSON")?;
    assert_eq!(review_report["status"], "recorded");
    assert_eq!(review_report["review"]["state"], "review_recorded");
    assert_eq!(review_report["review"]["verdict"], "changes_requested");
    assert_eq!(review_report["next_step"]["step"], "repair");

    let repaired_status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-review-recorded",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(repaired_status["result"]["isError"], false);
    assert_eq!(
        repaired_status["result"]["structuredContent"]["latest_receipt"]["receipt_kind"],
        "DEV_RETURN_REVIEW_RECORDED"
    );
    assert!(repaired_status["result"]["structuredContent"]["integrate"].is_null());
    assert_eq!(
        repaired_status["result"]["structuredContent"]["review"]["state"],
        "review_recorded"
    );
    assert_eq!(
        repaired_status["result"]["structuredContent"]["review"]["verdict"],
        "changes_requested"
    );
    assert_eq!(
        repaired_status["result"]["structuredContent"]["next_step"]["step"],
        "repair"
    );

    Ok(())
}

#[test]
fn external_client_can_read_dev_integrate_truth_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-dev-integrate-http")?;
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
        .join("feat-MYT-50");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let agent_command = write_stub_agent_command(app_dir.path())?;
    let dev_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "dev",
            "--project-dir",
            project_root
                .to_str()
                .context("project root should be valid UTF-8")?,
            "--model",
            "codex",
            "--agent-command",
            agent_command
                .to_str()
                .context("agent command should be valid UTF-8")?,
            "--title",
            "HTTP integrate MYT-50",
        ],
    )?;
    let dev_report: Value =
        serde_json::from_str(&dev_output).context("nota dev output should be valid JSON")?;
    let transaction_id = dev_report["transaction"]["id"]
        .as_i64()
        .context("transaction id should be present")?;
    let allocation_id = dev_report["allocation"]["id"]
        .as_i64()
        .context("allocation id should be present")?;
    let lineage_ref = dev_report["allocation"]["lineage_ref"]
        .as_str()
        .context("allocation lineage_ref should be present")?
        .to_string();
    let task_id = dev_report["task_id"]
        .as_i64()
        .context("task id should be present")?;

    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = NULL, finished_at = ?3 WHERE id = ?1",
        rusqlite::params![task_id, "Done", "2026-03-24T00:00:00Z"],
    )?;

    run_entrance_cli(app_dir.path(), &["nota", "allocations"])?;
    run_entrance_cli(app_dir.path(), &["nota", "checkpoint-runtime-closure"])?;
    run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "review",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--verdict",
            "approved",
            "--summary",
            "HTTP review accepted the returned dev boundary for integration.",
        ],
    )?;

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;
    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-integrate",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let review_status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-integrate-review",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(review_status["result"]["isError"], false);
    assert!(review_status["result"]["structuredContent"]["integrate"].is_null());
    assert_eq!(
        review_status["result"]["structuredContent"]["review"]["verdict"],
        "approved"
    );
    assert_eq!(
        review_status["result"]["structuredContent"]["next_step"]["step"],
        "integrate"
    );

    let integrate_started_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "integrate",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--state",
            "started",
            "--summary",
            "HTTP integration is in progress.",
        ],
    )?;
    let integrate_started_report: Value = serde_json::from_str(&integrate_started_output)
        .context("HTTP integrate started output should be valid JSON")?;
    assert_eq!(integrate_started_report["status"], "recorded");
    assert_eq!(
        integrate_started_report["integrate"]["state"],
        "integrate_started"
    );
    assert_eq!(integrate_started_report["next_step"], Value::Null);

    let started_status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-integrate-started",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(started_status["result"]["isError"], false);
    assert_eq!(
        started_status["result"]["structuredContent"]["latest_receipt"]["receipt_kind"],
        "DEV_RETURN_INTEGRATE_RECORDED"
    );
    assert_eq!(
        started_status["result"]["structuredContent"]["integrate"]["state"],
        "integrate_started"
    );
    assert_eq!(
        started_status["result"]["structuredContent"]["integrate"]["transaction_id"],
        transaction_id
    );
    assert_eq!(
        started_status["result"]["structuredContent"]["integrate"]["allocation_id"],
        allocation_id
    );
    assert_eq!(
        started_status["result"]["structuredContent"]["integrate"]["lineage_ref"],
        lineage_ref
    );
    assert_eq!(
        started_status["result"]["structuredContent"]["integrate"]["summary"],
        "HTTP integration is in progress."
    );
    assert_eq!(
        started_status["result"]["structuredContent"]["next_step"],
        Value::Null
    );

    let integrate_recorded_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "integrate",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--state",
            "repair_requested",
            "--summary",
            "HTTP integration requested repair before finalize.",
        ],
    )?;
    let integrate_recorded_report: Value = serde_json::from_str(&integrate_recorded_output)
        .context("HTTP integrate recorded output should be valid JSON")?;
    assert_eq!(integrate_recorded_report["status"], "recorded");
    assert_eq!(
        integrate_recorded_report["integrate"]["state"],
        "integrate_recorded"
    );
    assert_eq!(
        integrate_recorded_report["integrate"]["outcome"],
        "repair_requested"
    );
    assert_eq!(integrate_recorded_report["next_step"]["step"], "repair");

    let repair_status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-integrate-recorded",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(repair_status["result"]["isError"], false);
    assert_eq!(
        repair_status["result"]["structuredContent"]["integrate"]["state"],
        "integrate_recorded"
    );
    assert_eq!(
        repair_status["result"]["structuredContent"]["integrate"]["outcome"],
        "repair_requested"
    );
    assert_eq!(
        repair_status["result"]["structuredContent"]["next_step"]["step"],
        "repair"
    );

    let overview = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-overview-integrate-recorded",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_overview",
            "arguments": {}
        }
    }))?;
    assert_eq!(overview["result"]["isError"], false);
    assert_eq!(
        overview["result"]["structuredContent"]["integrate"]["state"],
        "integrate_recorded"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["integrate"]["outcome"],
        "repair_requested"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"]["step"],
        "repair"
    );

    let receipts = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-receipts-integrate-recorded",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_receipts",
            "arguments": {
                "transaction_id": transaction_id
            }
        }
    }))?;
    assert_eq!(receipts["result"]["isError"], false);
    let integrate_payload_json = receipts["result"]["structuredContent"]["receipts"][11]
        ["payload_json"]
        .as_str()
        .context("integrate payload_json should be present")?;
    let integrate_payload: Value = serde_json::from_str(integrate_payload_json)
        .context("integrate payload_json should be valid JSON")?;
    assert_eq!(
        integrate_payload["integrate"]["state"],
        "integrate_recorded"
    );
    assert_eq!(
        integrate_payload["integrate"]["outcome"],
        "repair_requested"
    );
    assert_eq!(integrate_payload["next_step"]["step"], "repair");

    Ok(())
}

#[test]
fn external_client_can_read_dev_finalize_truth_over_http() -> Result<()> {
    let app_dir = TempAppDir::new("nota-dev-finalize-http")?;
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
        .join("feat-MYT-51");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let agent_command = write_stub_agent_command(app_dir.path())?;
    let dev_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "dev",
            "--project-dir",
            project_root
                .to_str()
                .context("project root should be valid UTF-8")?,
            "--model",
            "codex",
            "--agent-command",
            agent_command
                .to_str()
                .context("agent command should be valid UTF-8")?,
            "--title",
            "HTTP finalize MYT-51",
        ],
    )?;
    let dev_report: Value =
        serde_json::from_str(&dev_output).context("nota dev output should be valid JSON")?;
    let transaction_id = dev_report["transaction"]["id"]
        .as_i64()
        .context("transaction id should be present")?;
    let allocation_id = dev_report["allocation"]["id"]
        .as_i64()
        .context("allocation id should be present")?;
    let task_id = dev_report["task_id"]
        .as_i64()
        .context("task id should be present")?;

    let db_path = app_dir.path().join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = NULL, finished_at = ?3 WHERE id = ?1",
        rusqlite::params![task_id, "Done", "2026-03-24T00:00:00Z"],
    )?;

    run_entrance_cli(app_dir.path(), &["nota", "allocations"])?;
    run_entrance_cli(app_dir.path(), &["nota", "checkpoint-runtime-closure"])?;
    run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "review",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--verdict",
            "approved",
            "--summary",
            "HTTP finalize review accepted the returned dev boundary.",
        ],
    )?;
    run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "integrate",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--state",
            "integrated",
            "--summary",
            "HTTP finalize path integrated cleanly.",
        ],
    )?;
    let finalize_output = run_entrance_cli(
        app_dir.path(),
        &[
            "nota",
            "finalize",
            "--transaction-id",
            &transaction_id.to_string(),
            "--allocation-id",
            &allocation_id.to_string(),
            "--summary",
            "HTTP finalize closed the boundary.",
        ],
    )?;
    let finalize_report: Value = serde_json::from_str(&finalize_output)
        .context("HTTP finalize output should be valid JSON")?;
    assert_eq!(finalize_report["status"], "recorded");
    assert_eq!(finalize_report["finalize"]["state"], "closed");
    assert_eq!(finalize_report["next_step"], Value::Null);

    let port = reserve_port()?;
    let mut server =
        spawn_mcp_http_with_actor_role(app_dir.path(), port, "/mcp", None, Some("nota"))?;
    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-finalize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let status = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-status-finalize",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_status",
            "arguments": {}
        }
    }))?;
    assert_eq!(status["result"]["isError"], false);
    assert_eq!(
        status["result"]["structuredContent"]["latest_receipt"]["receipt_kind"],
        "DEV_RETURN_FINALIZE_RECORDED"
    );
    assert_eq!(
        status["result"]["structuredContent"]["integrate"]["outcome"],
        "integrated"
    );
    assert_eq!(
        status["result"]["structuredContent"]["finalize"]["state"],
        "closed"
    );
    assert_eq!(
        status["result"]["structuredContent"]["next_step"],
        Value::Null
    );

    let overview = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-overview-finalize",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_overview",
            "arguments": {}
        }
    }))?;
    assert_eq!(overview["result"]["isError"], false);
    assert_eq!(
        overview["result"]["structuredContent"]["finalize"]["state"],
        "closed"
    );
    assert_eq!(
        overview["result"]["structuredContent"]["next_step"],
        Value::Null
    );

    let receipts = server.send(json!({
        "jsonrpc": "2.0",
        "id": "nota-runtime-receipts-finalize",
        "method": "tools/call",
        "params": {
            "name": "nota_runtime_receipts",
            "arguments": {
                "transaction_id": transaction_id
            }
        }
    }))?;
    assert_eq!(receipts["result"]["isError"], false);
    let finalize_payload_json = receipts["result"]["structuredContent"]["receipts"][11]
        ["payload_json"]
        .as_str()
        .context("HTTP finalize payload_json should be present")?;
    let finalize_payload: Value = serde_json::from_str(finalize_payload_json)
        .context("HTTP finalize payload_json should be valid JSON")?;
    assert_eq!(finalize_payload["finalize"]["state"], "closed");
    assert_eq!(
        finalize_payload["finalize"]["summary"],
        "HTTP finalize closed the boundary."
    );

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
    assert_spawnable_codex_command(
        verify["result"]["structuredContent"]["task_command"]
            .as_str()
            .context("forge_verify_dispatch should return a task_command string")?,
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
    assert_spawnable_codex_command(&stored.1);
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
    assert_spawnable_codex_command(
        verify["result"]["structuredContent"]["task_command"]
            .as_str()
            .context("forge_verify_dev_dispatch should return a task_command string")?,
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
    assert_spawnable_codex_command(&stored.1);
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
        child["result"]["structuredContent"]["supervision"]["parent_receipt"]
            ["supervision_strategy"],
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

    let _ = wait_for_terminal_status_http(&mut dev_server, child_task_id)?;

    Ok(())
}

#[test]
fn external_client_can_bootstrap_allocator_cycle_over_nota_http_surface() -> Result<()> {
    let app_dir = TempAppDir::new("forge-bootstrap-allocator-http")?;
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

    let port = reserve_port()?;
    let mut nota_server = spawn_mcp_http_with_actor_role(
        app_dir.path(),
        port,
        "/mcp",
        Some("test-openai-token"),
        Some("nota"),
    )?;
    let initialize = nota_server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize-nota",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["result"]["entranceSurface"]["actorRole"], "nota");

    let bootstrap = nota_server.send(json!({
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
            "nota:test:mcp-http-overview",
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

fn run_entrance_cli(app_dir: &PathBuf, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(args)
        .env("ENTRANCE_APP_DATA_DIR", app_dir)
        .env_remove("LINEAR_API_KEY")
        .env_remove("LINEAR_TOKEN")
        .output()
        .with_context(|| format!("failed to spawn `entrance {}`", args.join(" ")))?;

    if !output.status.success() {
        bail!(
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

fn init_git_repo_with_commit(path: &PathBuf) -> Result<()> {
    init_git_repo(path)?;

    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .context("failed to run `git add .`")?;
    if !add.status.success() {
        bail!(
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
        bail!(
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
        bail!(
            "`git worktree add` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}
