use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::core::action::ActorRole;

pub struct SpawnedMcpStdioClient {
    child: Child,
    stderr: BufReader<ChildStderr>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl SpawnedMcpStdioClient {
    pub fn spawn(app_data_dir: &Path, actor_role: ActorRole) -> Result<Self> {
        let executable = std::env::current_exe().context("failed to resolve current executable")?;
        let mut command = Command::new(executable);
        command.arg("mcp").arg("stdio");
        command.args(["--actor-role", actor_role_slug(actor_role)]);
        command
            .env("ENTRANCE_APP_DATA_DIR", app_data_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .context("failed to spawn child `entrance mcp stdio` surface")?;
        let stdout = child
            .stdout
            .take()
            .context("child MCP stdout should be piped")?;
        let stderr = child
            .stderr
            .take()
            .context("child MCP stderr should be piped")?;

        Ok(Self {
            child,
            stderr: BufReader::new(stderr),
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    pub fn initialize(&mut self) -> Result<Value> {
        let response = self.send_request(json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {}
        }))?;

        self.send_notification(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))?;

        Ok(response)
    }

    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let id = format!("tool-{}", self.next_request_id());
        let response = self.send_request(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
            }
        }))?;

        if let Some(error) = response.get("error") {
            bail!(
                "child MCP tool `{name}` returned JSON-RPC error: {}",
                serde_json::to_string(error).unwrap_or_else(|_| error.to_string())
            );
        }

        let result = response
            .get("result")
            .cloned()
            .context("child MCP tool response missing `result`")?;

        if result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let message = result
                .get("structuredContent")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .or_else(|| {
                    result
                        .get("content")
                        .and_then(Value::as_array)
                        .and_then(|content| content.first())
                        .and_then(|item| item.get("text"))
                        .and_then(Value::as_str)
                })
                .unwrap_or("child MCP tool returned an unknown error");
            bail!("child MCP tool `{name}` failed: {message}");
        }

        Ok(result)
    }

    fn next_request_id(&mut self) -> u64 {
        let current = self.next_id;
        self.next_id += 1;
        current
    }

    fn send_notification(&mut self, request: Value) -> Result<()> {
        self.send_raw(request)?;
        Ok(())
    }

    fn send_request(&mut self, request: Value) -> Result<Value> {
        self.send_raw(request)?;
        self.read_response()
    }

    fn send_raw(&mut self, request: Value) -> Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .context("child MCP stdin should be available")?;
        serde_json::to_writer(&mut *stdin, &request)
            .context("failed to serialize MCP stdio request")?;
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
                .context("failed to read MCP stdio response line")?;
            if read == 0 {
                let mut stderr = String::new();
                let _ = self.stderr.read_to_string(&mut stderr);
                bail!(
                    "child MCP stdio surface closed before responding. stderr: {}",
                    stderr.trim()
                );
            }

            let payload = line.trim();
            if payload.is_empty() {
                continue;
            }

            return serde_json::from_str(payload)
                .context("failed to parse MCP stdio response JSON");
        }
    }
}

impl Drop for SpawnedMcpStdioClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn actor_role_slug(role: ActorRole) -> &'static str {
    match role {
        ActorRole::Nota => "nota",
        ActorRole::Arch => "arch",
        ActorRole::Dev => "dev",
        ActorRole::Agent => "agent",
    }
}
