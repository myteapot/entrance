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
    let mut server = spawn_mcp_http(app_dir.path(), port, "/mcp")?;

    let initialize = server.send(json!({
        "jsonrpc": "2.0",
        "id": "initialize",
        "method": "initialize",
        "params": {}
    }))?;
    assert_eq!(initialize["id"], "initialize");
    assert_eq!(initialize["result"]["protocolVersion"], "2024-11-05");

    let tools = server.send(json!({
        "jsonrpc": "2.0",
        "id": "tools",
        "method": "tools/list"
    }))?;
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
    assert!(
        forge_run["result"]["structuredContent"]["task_id"]
            .as_i64()
            .context("forge_run should return a numeric task_id")?
            > 0
    );

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

fn reserve_port() -> Result<u16> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).context("failed to reserve a local MCP HTTP port")?;
    Ok(listener.local_addr()?.port())
}

fn spawn_mcp_http(app_dir: &PathBuf, port: u16, endpoint: &str) -> Result<SpawnedHttpMcp> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args([
            "mcp",
            "http",
            "--port",
            &port.to_string(),
            "--endpoint",
            endpoint,
        ])
        .env("ENTRANCE_APP_DATA_DIR", app_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
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
