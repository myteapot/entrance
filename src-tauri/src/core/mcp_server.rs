use std::{
    io::{self, BufRead, Write},
    net::SocketAddr,
    sync::Arc,
};

use anyhow::{anyhow, bail, Context, Result};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::permission::{permission_for_mcp_tool, McpToolPermission};
use crate::plugins::{
    forge::{
        build_agent_task_request, build_dev_task_request, prepare_agent_dispatch_blocking,
        prepare_dev_dispatch_blocking, verify_agent_dispatch, verify_dev_dispatch,
        CreateTaskRequest, ForgePlugin,
    },
    launcher::LauncherPlugin,
    vault::VaultPlugin,
};

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone)]
pub enum McpTransport {
    InProcess,
    Stdio,
    Http { endpoint: String },
}

#[derive(Clone, Default)]
pub struct McpPluginSet {
    pub forge: Option<ForgePlugin>,
    pub launcher: Option<LauncherPlugin>,
    pub vault: Option<VaultPlugin>,
}

#[derive(Clone)]
pub struct McpServer {
    transport: McpTransport,
    plugins: McpPluginSet,
    tools: Arc<Vec<McpToolDescriptor>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<McpToolPermission>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

impl McpServer {
    pub fn new(transport: McpTransport, plugins: McpPluginSet) -> Self {
        let tools = build_tool_descriptors(&plugins);
        Self {
            transport,
            plugins,
            tools: Arc::new(tools),
        }
    }

    pub fn transport(&self) -> &McpTransport {
        &self.transport
    }

    pub fn tools(&self) -> &[McpToolDescriptor] {
        self.tools.as_ref().as_slice()
    }

    pub fn handle_json_rpc_bytes(&self, request: &[u8]) -> Result<Option<Vec<u8>>> {
        let request = serde_json::from_slice::<Value>(request)
            .context("failed to decode JSON-RPC request body")?;
        let response = self.handle_json_rpc_value(request)?;
        response
            .map(|value| serde_json::to_vec(&value).context("failed to encode JSON-RPC response"))
            .transpose()
    }

    pub fn handle_http_json(&self, request: &[u8]) -> Result<Vec<u8>> {
        let response = self
            .handle_json_rpc_bytes(request)?
            .unwrap_or_else(|| b"{\"jsonrpc\":\"2.0\",\"result\":{},\"id\":null}".to_vec());
        Ok(response)
    }

    pub fn serve_stdio(&self) -> Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut writer = stdout.lock();
        self.serve_stdio_stream(&mut reader, &mut writer)
    }

    pub async fn serve_http(&self, address: SocketAddr) -> Result<()> {
        let endpoint = match self.transport() {
            McpTransport::Http { endpoint } => endpoint.clone(),
            _ => bail!("MCP transport is not configured for HTTP"),
        };
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .with_context(|| format!("failed to bind MCP HTTP listener on {address}"))?;
        let app = Router::new()
            .route(endpoint.as_str(), post(handle_http_request))
            .with_state(self.clone());

        tracing::info!("MCP HTTP API listening on http://{address}{endpoint}");
        axum::serve(listener, app)
            .await
            .context("MCP HTTP server stopped unexpectedly")
    }

    pub fn handle_json_rpc_value(&self, request: Value) -> Result<Option<Value>> {
        let request = serde_json::from_value::<JsonRpcRequest>(request)
            .context("failed to deserialize JSON-RPC request")?;

        if request.jsonrpc != "2.0" {
            return Ok(request
                .id
                .map(|id| json_rpc_error(id, -32600, "jsonrpc must be `2.0`")));
        }

        let Some(id) = request.id.clone() else {
            self.handle_notification(&request.method, request.params.as_ref())?;
            return Ok(None);
        };

        let response = match request.method.as_str() {
            "initialize" => json_rpc_result(
                id,
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": env!("CARGO_PKG_NAME"),
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),
            "ping" => json_rpc_result(id, json!({})),
            "tools/list" => json_rpc_result(id, json!({ "tools": self.tools() })),
            "tools/call" => {
                let result = self.handle_tool_call(request.params.as_ref());
                json_rpc_result(id, tool_call_result(result))
            }
            _ => json_rpc_error(
                id,
                -32601,
                &format!("method `{}` is not supported", request.method),
            ),
        };

        Ok(Some(response))
    }

    fn handle_notification(&self, method: &str, _params: Option<&Value>) -> Result<()> {
        match method {
            "notifications/initialized" => Ok(()),
            _ => Ok(()),
        }
    }

    fn handle_tool_call(&self, params: Option<&Value>) -> Result<Value> {
        let params = params.context("tools/call requires params")?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .context("tools/call requires a string `name` field")?;
        let arguments = params.get("arguments").unwrap_or(&Value::Null);

        match name {
            "forge_run" => self.handle_forge_run(arguments),
            "forge_prepare_dispatch" => self.handle_forge_prepare_dispatch(arguments),
            "forge_verify_dispatch" => self.handle_forge_verify_dispatch(arguments),
            "forge_prepare_dev_dispatch" => self.handle_forge_prepare_dev_dispatch(arguments),
            "forge_verify_dev_dispatch" => self.handle_forge_verify_dev_dispatch(arguments),
            "forge_dispatch_agent" => self.handle_forge_dispatch_agent(arguments),
            "forge_dispatch_dev" => self.handle_forge_dispatch_dev(arguments),
            "forge_status" => self.handle_forge_status(arguments),
            "forge_cancel" => self.handle_forge_cancel(arguments),
            "vault_get_token" => self.handle_vault_get_token(arguments),
            "vault_list_mcp" => self.handle_vault_list_mcp(),
            "launcher_search" => self.handle_launcher_search(arguments),
            "launcher_launch" => self.handle_launcher_launch(arguments),
            _ => bail!("tool `{name}` is not registered"),
        }
    }

    fn handle_forge_run(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let name = require_string(arguments, "name")?;
        let command = require_string(arguments, "command")?;
        let args = serialize_forge_args(arguments.get("args"))?;
        let required_tokens = serialize_forge_args(arguments.get("required_tokens"))?;

        let task_id = forge.create_task(CreateTaskRequest {
            name: name.to_string(),
            command: command.to_string(),
            args,
            working_dir: None,
            stdin_text: None,
            required_tokens,
            metadata: "{}".to_string(),
        })?;
        forge
            .engine()
            .spawn_task(task_id)
            .with_context(|| format!("failed to start forge task `{task_id}`"))?;

        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` disappeared after creation"))?;

        Ok(json!({
            "task_id": task.id,
            "task": task,
        }))
    }

    fn handle_forge_prepare_dispatch(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let project_dir = optional_string(arguments, "project_dir")
            .or_else(|| optional_string(arguments, "projectDir"))
            .map(str::to_string);
        let dispatch = prepare_agent_dispatch_blocking(forge.data_store(), project_dir)
            .map_err(anyhow::Error::msg)?;
        serde_json::to_value(dispatch).context("failed to serialize forge dispatch")
    }

    fn handle_forge_verify_dispatch(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let project_dir = optional_string(arguments, "project_dir")
            .or_else(|| optional_string(arguments, "projectDir"))
            .map(str::to_string);
        let report = verify_agent_dispatch(forge, project_dir).map_err(anyhow::Error::msg)?;
        serde_json::to_value(report)
            .context("failed to serialize forge dispatch verification report")
    }

    fn handle_forge_prepare_dev_dispatch(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let project_dir = optional_string(arguments, "project_dir")
            .or_else(|| optional_string(arguments, "projectDir"))
            .map(str::to_string);
        let dispatch = prepare_dev_dispatch_blocking(forge.data_store(), project_dir)
            .map_err(anyhow::Error::msg)?;
        serde_json::to_value(dispatch).context("failed to serialize forge dev dispatch")
    }

    fn handle_forge_verify_dev_dispatch(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let project_dir = optional_string(arguments, "project_dir")
            .or_else(|| optional_string(arguments, "projectDir"))
            .map(str::to_string);
        let report = verify_dev_dispatch(forge, project_dir).map_err(anyhow::Error::msg)?;
        serde_json::to_value(report)
            .context("failed to serialize forge dev dispatch verification report")
    }

    fn handle_forge_dispatch_agent(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let issue_id = require_string_any(arguments, &["issue_id", "issueId"])?;
        let worktree_path = require_string_any(arguments, &["worktree_path", "worktreePath"])?;
        let model = require_string(arguments, "model")?;
        let prompt = require_string(arguments, "prompt")?;
        let required_tokens =
            require_string_list(arguments, &["required_tokens", "requiredTokens"])?;
        let agent_command = optional_string(arguments, "agent_command")
            .or_else(|| optional_string(arguments, "agentCommand"))
            .map(str::to_string);

        let request = build_agent_task_request(
            issue_id.to_string(),
            worktree_path.to_string(),
            model.to_string(),
            prompt.to_string(),
            required_tokens,
            agent_command,
        )
        .map_err(anyhow::Error::msg)?;

        let task_id = forge.create_task(request).map_err(anyhow::Error::msg)?;
        forge
            .engine()
            .spawn_task(task_id)
            .with_context(|| format!("failed to start forge task `{task_id}`"))?;

        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` disappeared after creation"))?;

        Ok(json!({
            "dispatch_role": "agent",
            "task_id": task.id,
            "task": task,
        }))
    }

    fn handle_forge_dispatch_dev(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let issue_id = require_string_any(arguments, &["issue_id", "issueId"])?;
        let worktree_path = require_string_any(arguments, &["worktree_path", "worktreePath"])?;
        let model = require_string(arguments, "model")?;
        let prompt = require_string(arguments, "prompt")?;
        let required_tokens =
            require_string_list(arguments, &["required_tokens", "requiredTokens"])?;
        let agent_command = optional_string(arguments, "agent_command")
            .or_else(|| optional_string(arguments, "agentCommand"))
            .map(str::to_string);

        let request = build_dev_task_request(
            issue_id.to_string(),
            worktree_path.to_string(),
            model.to_string(),
            prompt.to_string(),
            required_tokens,
            agent_command,
        )
        .map_err(anyhow::Error::msg)?;

        let task_id = forge.create_task(request).map_err(anyhow::Error::msg)?;
        forge
            .engine()
            .spawn_task(task_id)
            .with_context(|| format!("failed to start forge task `{task_id}`"))?;

        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` disappeared after creation"))?;

        Ok(json!({
            "dispatch_role": "dev",
            "task_id": task.id,
            "task": task,
        }))
    }

    fn handle_forge_status(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let task_id = require_i64(arguments, &["task_id", "id"])?;
        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` was not found"))?;
        Ok(json!({
            "task_id": task.id,
            "task": task,
        }))
    }

    fn handle_forge_cancel(&self, arguments: &Value) -> Result<Value> {
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let task_id = require_i64(arguments, &["task_id", "id"])?;
        forge.cancel_task(task_id)?;
        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` was not found after cancellation"))?;
        Ok(json!({
            "task_id": task.id,
            "cancelled": true,
            "task": task,
        }))
    }

    fn handle_vault_get_token(&self, arguments: &Value) -> Result<Value> {
        let vault = self
            .plugins
            .vault
            .as_ref()
            .context("vault plugin is not enabled")?;
        let token_id = require_i64(arguments, &["token_id", "id"])?;
        let token = vault.get_token(token_id)?;
        Ok(json!({
            "token_id": token_id,
            "token": token,
        }))
    }

    fn handle_vault_list_mcp(&self) -> Result<Value> {
        let vault = self
            .plugins
            .vault
            .as_ref()
            .context("vault plugin is not enabled")?;
        Ok(json!({
            "servers": vault.list_mcp_configs()?,
        }))
    }

    fn handle_launcher_search(&self, arguments: &Value) -> Result<Value> {
        let launcher = self
            .plugins
            .launcher
            .as_ref()
            .context("launcher plugin is not enabled")?;
        let query = require_string(arguments, "query")?;
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(20);
        Ok(json!({
            "results": launcher.search(query, limit)?,
        }))
    }

    fn handle_launcher_launch(&self, arguments: &Value) -> Result<Value> {
        let launcher = self
            .plugins
            .launcher
            .as_ref()
            .context("launcher plugin is not enabled")?;
        let path = require_string(arguments, "path")?;
        let command_arguments = optional_string(arguments, "arguments");
        let working_dir = optional_string(arguments, "working_dir");

        launcher.launch(path, command_arguments, working_dir)?;
        Ok(json!({
            "launched": true,
            "path": path,
        }))
    }

    fn serve_stdio_stream<R: BufRead, W: Write>(
        &self,
        reader: &mut R,
        writer: &mut W,
    ) -> Result<()> {
        let mut line = String::new();

        loop {
            line.clear();
            let read = reader
                .read_line(&mut line)
                .context("failed to read MCP stdio request")?;
            if read == 0 {
                break;
            }

            let request = line.trim();
            if request.is_empty() {
                continue;
            }

            let response = self.handle_stdio_request(request);
            if let Some(response) = response {
                serde_json::to_writer(&mut *writer, &response)
                    .context("failed to encode MCP stdio response")?;
                writer
                    .write_all(b"\n")
                    .context("failed to write MCP stdio response delimiter")?;
                writer
                    .flush()
                    .context("failed to flush MCP stdio response")?;
            }
        }

        Ok(())
    }

    fn handle_stdio_request(&self, request: &str) -> Option<Value> {
        let request = match serde_json::from_str::<Value>(request) {
            Ok(request) => request,
            Err(error) => {
                return Some(json_rpc_error(
                    Value::Null,
                    -32700,
                    &format!("failed to parse JSON-RPC request: {error}"),
                ));
            }
        };

        match self.handle_json_rpc_value(request) {
            Ok(response) => response,
            Err(error) => Some(json_rpc_error(Value::Null, -32600, &error.to_string())),
        }
    }
}

#[derive(Debug)]
struct McpHttpError(anyhow::Error);

impl IntoResponse for McpHttpError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            serde_json::to_vec(&json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": -32600,
                    "message": self.0.to_string(),
                }
            }))
            .unwrap_or_else(|_| b"{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32600,\"message\":\"failed to encode MCP error\"}}".to_vec()),
        )
            .into_response()
    }
}

async fn handle_http_request(
    State(server): State<McpServer>,
    body: Bytes,
) -> Result<Response, McpHttpError> {
    let payload = tokio::task::spawn_blocking(move || server.handle_http_json(&body))
        .await
        .map_err(|error| McpHttpError(anyhow!("failed to join MCP HTTP request worker: {error}")))?
        .map_err(McpHttpError)?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        payload,
    )
        .into_response())
}

fn build_tool_descriptors(plugins: &McpPluginSet) -> Vec<McpToolDescriptor> {
    let mut tools = Vec::new();

    if plugins.forge.is_some() {
        tools.push(McpToolDescriptor {
            name: "forge_run",
            description: "Create and start a Forge task.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Human-readable task name." },
                    "command": { "type": "string", "description": "Executable to run." },
                    "args": {
                        "type": "array",
                        "description": "Command-line arguments passed to the executable.",
                        "items": { "type": "string" }
                    }
                },
                "required": ["name", "command"]
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_prepare_dispatch",
            description: "Prepare an Entrance-owned agent-lane Forge dispatch from the managed worktree for a project.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." }
                }
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_verify_dispatch",
            description: "Prepare and persist a Pending agent-lane Forge dispatch without starting agent execution.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." }
                }
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_prepare_dev_dispatch",
            description: "Prepare an Entrance-owned dev-lane Forge dispatch from the managed worktree for a project.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." }
                }
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_verify_dev_dispatch",
            description: "Prepare and persist a Pending dev-lane Forge dispatch without starting execution.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." }
                }
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_dispatch_agent",
            description: "Create and start an agent-lane Forge dispatch from issue, worktree, model, and prompt inputs.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "string", "description": "Issue identifier for the agent dispatch." },
                    "issueId": { "type": "string", "description": "CamelCase alias for issue_id." },
                    "worktree_path": { "type": "string", "description": "Managed worktree path where the agent should run." },
                    "worktreePath": { "type": "string", "description": "CamelCase alias for worktree_path." },
                    "model": { "type": "string", "description": "Agent runner or runner:model string such as codex or codex:gpt-5-codex." },
                    "prompt": { "type": "string", "description": "Prompt sent to the agent." },
                    "required_tokens": {
                        "type": "array",
                        "description": "Optional provider tokens that must be available before launch.",
                        "items": { "type": "string" }
                    },
                    "requiredTokens": {
                        "type": "array",
                        "description": "CamelCase alias for required_tokens.",
                        "items": { "type": "string" }
                    },
                    "agent_command": { "type": "string", "description": "Optional executable path overriding the default agent CLI." },
                    "agentCommand": { "type": "string", "description": "CamelCase alias for agent_command." }
                },
                "required": ["issue_id", "worktree_path", "model", "prompt"]
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_dispatch_dev",
            description: "Create and start a dev-lane Forge dispatch from issue, worktree, model, and prompt inputs.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "string", "description": "Issue identifier for the dev dispatch." },
                    "issueId": { "type": "string", "description": "CamelCase alias for issue_id." },
                    "worktree_path": { "type": "string", "description": "Managed worktree path where Dev should run." },
                    "worktreePath": { "type": "string", "description": "CamelCase alias for worktree_path." },
                    "model": { "type": "string", "description": "Dev runner or runner:model string such as codex or codex:gpt-5-codex." },
                    "prompt": { "type": "string", "description": "Prompt sent to the Dev role." },
                    "required_tokens": {
                        "type": "array",
                        "description": "Optional provider tokens that must be available before launch.",
                        "items": { "type": "string" }
                    },
                    "requiredTokens": {
                        "type": "array",
                        "description": "CamelCase alias for required_tokens.",
                        "items": { "type": "string" }
                    },
                    "agent_command": { "type": "string", "description": "Optional executable path overriding the default CLI." },
                    "agentCommand": { "type": "string", "description": "CamelCase alias for agent_command." }
                },
                "required": ["issue_id", "worktree_path", "model", "prompt"]
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_status",
            description: "Fetch a Forge task and its latest execution status.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "integer", "description": "Forge task identifier." }
                },
                "required": ["task_id"]
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_cancel",
            description: "Cancel a running Forge task.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "integer", "description": "Forge task identifier." }
                },
                "required": ["task_id"]
            }),
            permission: None,
        });
    }

    if plugins.vault.is_some() {
        tools.push(McpToolDescriptor {
            name: "vault_get_token",
            description: "Decrypt and return a stored provider token.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "token_id": { "type": "integer", "description": "Vault token identifier." }
                },
                "required": ["token_id"]
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "vault_list_mcp",
            description: "List saved MCP endpoint configurations.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            permission: None,
        });
    }

    if plugins.launcher.is_some() {
        tools.push(McpToolDescriptor {
            name: "launcher_search",
            description: "Search indexed desktop applications.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search keywords." },
                    "limit": { "type": "integer", "description": "Maximum number of results to return." }
                },
                "required": ["query"]
            }),
            permission: None,
        });
        tools.push(McpToolDescriptor {
            name: "launcher_launch",
            description: "Launch a desktop application by executable path.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Executable path to launch." },
                    "arguments": { "type": "string", "description": "Optional command-line arguments." },
                    "working_dir": { "type": "string", "description": "Optional working directory." }
                },
                "required": ["path"]
            }),
            permission: None,
        });
    }

    for tool in &mut tools {
        tool.permission = permission_for_mcp_tool(tool.name);
    }

    tools
}

fn require_string<'a>(arguments: &'a Value, field: &str) -> Result<&'a str> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("tool arguments require a string `{field}` field"))
}

fn require_string_any<'a>(arguments: &'a Value, fields: &[&str]) -> Result<&'a str> {
    for field in fields {
        if let Some(value) = arguments.get(*field).and_then(Value::as_str) {
            return Ok(value);
        }
    }

    bail!(
        "tool arguments require one of these string fields: {}",
        fields.join(", ")
    )
}

fn optional_string<'a>(arguments: &'a Value, field: &str) -> Option<&'a str> {
    arguments.get(field).and_then(Value::as_str)
}

fn require_i64(arguments: &Value, fields: &[&str]) -> Result<i64> {
    for field in fields {
        if let Some(value) = arguments.get(*field).and_then(Value::as_i64) {
            return Ok(value);
        }
    }

    bail!("tool arguments require one of: {}", fields.join(", "))
}

fn serialize_forge_args(arguments: Option<&Value>) -> Result<String> {
    let Some(arguments) = arguments else {
        return Ok("[]".to_string());
    };

    match arguments {
        Value::Null => Ok("[]".to_string()),
        Value::Array(items) => {
            for item in items {
                if !item.is_string() {
                    bail!("forge_run args must be an array of strings");
                }
            }
            serde_json::to_string(arguments).context("failed to serialize forge args")
        }
        Value::String(raw) => {
            let parsed = serde_json::from_str::<Value>(raw)
                .unwrap_or_else(|_| Value::Array(vec![Value::String(raw.clone())]));
            if !parsed.is_array() {
                bail!("forge_run args string must decode to a JSON array");
            }
            serde_json::to_string(&parsed).context("failed to serialize forge args")
        }
        _ => bail!("forge_run args must be either an array or JSON string"),
    }
}

fn require_string_list(arguments: &Value, fields: &[&str]) -> Result<Vec<String>> {
    for field in fields {
        if let Some(value) = arguments.get(*field) {
            return parse_string_list(value, field);
        }
    }

    Ok(Vec::new())
}

fn parse_string_list(value: &Value, field: &str) -> Result<Vec<String>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| anyhow!("tool argument `{field}` must be an array of strings"))
            })
            .collect(),
        Value::String(raw) => {
            let parsed =
                serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.clone()));
            match parsed {
                Value::Array(items) => items
                    .iter()
                    .map(|item| {
                        item.as_str().map(str::to_string).ok_or_else(|| {
                            anyhow!("tool argument `{field}` must decode to an array of strings")
                        })
                    })
                    .collect(),
                Value::String(value) => Ok(vec![value]),
                _ => bail!("tool argument `{field}` string must decode to a JSON string array"),
            }
        }
        _ => bail!("tool argument `{field}` must be either an array or a JSON string"),
    }
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn json_rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

fn tool_call_result(result: Result<Value>) -> Value {
    match result {
        Ok(value) => json!({
            "content": [
                {
                    "type": "text",
                    "text": to_pretty_json(&value),
                }
            ],
            "structuredContent": value,
            "isError": false,
        }),
        Err(error) => json!({
            "content": [
                {
                    "type": "text",
                    "text": error.to_string(),
                }
            ],
            "structuredContent": {
                "message": error.to_string(),
            },
            "isError": true,
        }),
    }
}

fn to_pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use anyhow::Result;
    use serde_json::{json, Value};

    use crate::{
        core::{
            data_store::{DataStore, MigrationPlan},
            event_bus::EventBus,
        },
        plugins::{
            forge::ForgePlugin,
            launcher::{scanner::DiscoveredApp, search::normalize_text, LauncherPlugin},
            vault::VaultPlugin,
        },
    };

    use super::{McpPluginSet, McpServer, McpTransport, MCP_PROTOCOL_VERSION};

    #[test]
    fn tools_list_contains_registered_plugin_tools() -> Result<()> {
        let server = build_test_server()?;
        let response = server
            .handle_json_rpc_value(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list"
            }))?
            .expect("tools/list should return a response");

        let names = response["result"]["tools"]
            .as_array()
            .expect("tools/list should return an array")
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(
            names,
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
        let tools = response["result"]["tools"]
            .as_array()
            .expect("tools/list should return an array");
        let dispatch_agent = tools
            .iter()
            .find(|tool| tool["name"] == "forge_dispatch_agent")
            .expect("forge_dispatch_agent should exist");
        let dispatch_dev = tools
            .iter()
            .find(|tool| tool["name"] == "forge_dispatch_dev")
            .expect("forge_dispatch_dev should exist");
        assert_eq!(dispatch_agent["permission"]["actorRole"], "dev");
        assert_eq!(dispatch_agent["permission"]["primitive"], "dispatch");
        assert_eq!(dispatch_dev["permission"]["actorRole"], "arch");
        assert_eq!(dispatch_dev["permission"]["room"], "strategy");

        Ok(())
    }

    #[test]
    fn initialize_reports_server_capabilities() -> Result<()> {
        let server = build_test_server()?;
        let response = server
            .handle_json_rpc_value(json!({
                "jsonrpc": "2.0",
                "id": "init",
                "method": "initialize",
                "params": {}
            }))?
            .expect("initialize should return a response");

        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(
            response["result"]["serverInfo"]["name"],
            env!("CARGO_PKG_NAME")
        );
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );

        Ok(())
    }

    #[test]
    fn tool_calls_bridge_into_plugins() -> Result<()> {
        let server = build_test_server()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let launcher_response = call_tool(
            &server,
            "launcher_search",
            json!({
                "query": "code",
                "limit": 5
            }),
        )?;
        assert_eq!(launcher_response["isError"], false);
        assert_eq!(
            launcher_response["structuredContent"]["results"][0]["path"],
            "C:\\Tools\\Code.exe"
        );

        let vault_response = call_tool(&server, "vault_get_token", json!({ "token_id": 1 }))?;
        assert_eq!(vault_response["isError"], false);
        assert_eq!(
            vault_response["structuredContent"]["token"]["provider"],
            "openai"
        );
        assert_eq!(
            vault_response["structuredContent"]["token"]["value"],
            "secret-token"
        );

        let mcp_list_response = call_tool(&server, "vault_list_mcp", json!({}))?;
        assert_eq!(mcp_list_response["isError"], false);
        assert_eq!(
            mcp_list_response["structuredContent"]["servers"][0]["transport"],
            "stdio"
        );

        let forge_response = runtime.block_on(async {
            call_tool(
                &server,
                "forge_run",
                json!({
                    "name": "Echo",
                    "command": if cfg!(windows) { "cmd" } else { "sh" },
                    "args": if cfg!(windows) {
                        json!(["/C", "echo", "hello"])
                    } else {
                        json!(["-c", "echo hello"])
                    }
                }),
            )
        })?;
        assert_eq!(forge_response["isError"], false);
        assert!(
            forge_response["structuredContent"]["task_id"]
                .as_i64()
                .unwrap()
                > 0
        );

        Ok(())
    }

    #[test]
    fn stdio_transport_uses_line_delimited_json() -> Result<()> {
        let server = build_test_server()?;
        let request = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":\"init\",\"method\":\"initialize\",\"params\":{}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":\"tools\",\"method\":\"tools/list\"}\n"
        );
        let mut reader = Cursor::new(request.as_bytes());
        let mut writer = Vec::new();

        server.serve_stdio_stream(&mut reader, &mut writer)?;

        let responses = String::from_utf8(writer)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["id"], "init");
        assert_eq!(
            responses[0]["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert_eq!(responses[1]["id"], "tools");
        assert_eq!(responses[1]["result"]["tools"][0]["name"], "forge_run");

        Ok(())
    }

    fn call_tool(server: &McpServer, name: &str, arguments: Value) -> Result<Value> {
        let response = server
            .handle_json_rpc_value(json!({
                "jsonrpc": "2.0",
                "id": name,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": arguments,
                }
            }))?
            .expect("tools/call should return a response");

        Ok(response["result"].clone())
    }

    fn build_test_server() -> Result<McpServer> {
        let data_store = DataStore::in_memory(MigrationPlan::new(&[
            crate::plugins::launcher::migrations()[0],
            crate::plugins::forge::migrations()[0],
            crate::plugins::vault::migrations()[0],
        ]))?;
        let event_bus = EventBus::new();

        data_store.upsert_launcher_apps(&[DiscoveredApp {
            name: "Code".to_string(),
            normalized_name: normalize_text("Code"),
            path: "C:\\Tools\\Code.exe".to_string(),
            arguments: None,
            working_dir: Some("C:\\Tools".to_string()),
            icon_path: None,
            source: "test".to_string(),
        }])?;

        let launcher = LauncherPlugin::new(data_store.clone());
        let forge = ForgePlugin::new(data_store.clone(), event_bus);
        let vault = VaultPlugin::new(data_store)?;
        vault.add_token("Primary", "openai", "secret-token")?;
        vault.update_mcp_config(None, "Local MCP", "stdio", "npx -y some-mcp", true)?;

        Ok(McpServer::new(
            McpTransport::InProcess,
            McpPluginSet {
                forge: Some(forge),
                launcher: Some(launcher),
                vault: Some(vault),
            },
        ))
    }
}
