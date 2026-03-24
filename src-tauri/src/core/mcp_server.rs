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

use crate::core::{
    action::ActorRole,
    bootstrap_mcp_cycle::{run_forge_bootstrap_mcp_cycle, ForgeBootstrapMcpCycleOptions},
    data_store::DataStore,
    nota_runtime::{
        list_nota_runtime_allocations, list_nota_runtime_receipts, run_nota_dev_dispatch,
        run_nota_do_agent_dispatch, write_runtime_checkpoint, NotaCheckpointRequest,
        NotaDevDispatchRequest, NotaDispatchExecutionHost, NotaDoAgentDispatchRequest,
    },
    permission::{permission_for_mcp_tool, McpToolPermission},
    recovery::{list_recovery_seed_rows, list_recovery_seed_runs, RecoverySeedRowsQuery},
    resolve_app_data_dir,
    supervision::SupervisionStrategy,
};
use crate::plugins::{
    forge::{
        build_agent_task_request, build_dev_task_request, prepare_agent_dispatch_blocking,
        prepare_agent_dispatch_for_worktree_blocking, prepare_dev_dispatch_blocking,
        verify_agent_dispatch, verify_dev_dispatch, CreateTaskRequest, DispatchReceiptRequest,
        ForgePlugin,
    },
    launcher::LauncherPlugin,
    vault::VaultPlugin,
};
use crate::{build_nota_runtime_overview, build_nota_runtime_status};

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone)]
pub enum McpTransport {
    InProcess,
    Stdio,
    Http { endpoint: String },
}

#[derive(Clone, Default)]
pub struct McpPluginSet {
    pub core_data_store: Option<DataStore>,
    pub forge: Option<ForgePlugin>,
    pub launcher: Option<LauncherPlugin>,
    pub vault: Option<VaultPlugin>,
}

#[derive(Clone)]
pub struct McpServer {
    transport: McpTransport,
    plugins: McpPluginSet,
    actor_role: Option<ActorRole>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_role: Option<ActorRole>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpSurfaceInfo {
    actor_role: Option<ActorRole>,
}

#[derive(Debug)]
struct McpToolSurfaceRoleError {
    tool_name: String,
    current_actor_role: ActorRole,
    required_actor_role: ActorRole,
}

impl std::fmt::Display for McpToolSurfaceRoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tool `{}` is not available on the current `{}` MCP surface; requires `{}`",
            self.tool_name,
            actor_role_slug(self.current_actor_role),
            actor_role_slug(self.required_actor_role)
        )
    }
}

impl std::error::Error for McpToolSurfaceRoleError {}

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
        Self::with_actor_role(transport, plugins, None)
    }

    pub fn with_actor_role(
        transport: McpTransport,
        plugins: McpPluginSet,
        actor_role: Option<ActorRole>,
    ) -> Self {
        let tools = build_tool_descriptors(&plugins, actor_role);
        Self {
            transport,
            plugins,
            actor_role,
            tools: Arc::new(tools),
        }
    }

    pub fn transport(&self) -> &McpTransport {
        &self.transport
    }

    pub fn tools(&self) -> &[McpToolDescriptor] {
        self.tools.as_ref().as_slice()
    }

    fn surface_info(&self) -> McpSurfaceInfo {
        McpSurfaceInfo {
            actor_role: self.actor_role,
        }
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
                    },
                    "entranceSurface": self.surface_info()
                }),
            ),
            "ping" => json_rpc_result(id, json!({})),
            "tools/list" => json_rpc_result(
                id,
                json!({
                    "tools": self.tools(),
                    "entranceSurface": self.surface_info()
                }),
            ),
            "tools/call" => {
                let tool_name = tool_name_from_params(request.params.as_ref());
                let permission = tool_name.and_then(permission_for_registered_tool);
                let dispatch_role = tool_name.and_then(tool_dispatch_role_from_name);
                let canonical_tool_name = tool_name.and_then(canonical_tool_name_from_name);
                let result = self.handle_tool_call(request.params.as_ref());
                json_rpc_result(
                    id,
                    tool_call_result(
                        result,
                        self.surface_info(),
                        permission,
                        dispatch_role,
                        canonical_tool_name,
                    ),
                )
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
        self.ensure_tool_is_available(name)?;
        let arguments = params.get("arguments").unwrap_or(&Value::Null);

        match name {
            "forge_run" => self.handle_forge_run(arguments),
            "forge_prepare_dispatch" | "forge_prepare_agent_dispatch" => {
                self.handle_forge_prepare_dispatch(arguments)
            }
            "forge_verify_dispatch" | "forge_verify_agent_dispatch" => {
                self.handle_forge_verify_dispatch(arguments)
            }
            "forge_prepare_dev_dispatch" => self.handle_forge_prepare_dev_dispatch(arguments),
            "forge_verify_dev_dispatch" => self.handle_forge_verify_dev_dispatch(arguments),
            "forge_dispatch_agent" => self.handle_forge_dispatch_agent(arguments),
            "forge_dispatch_dev" => self.handle_forge_dispatch_dev(arguments),
            "forge_bootstrap_mcp_cycle" => self.handle_forge_bootstrap_mcp_cycle(arguments),
            "forge_status" => self.handle_forge_status(arguments),
            "forge_cancel" => self.handle_forge_cancel(arguments),
            "nota_runtime_overview" => self.handle_nota_runtime_overview(),
            "nota_runtime_status" => self.handle_nota_runtime_status(),
            "nota_runtime_allocations" => self.handle_nota_runtime_allocations(),
            "nota_runtime_receipts" => self.handle_nota_runtime_receipts(arguments),
            "nota_do" => self.handle_nota_do(arguments),
            "nota_dev" => self.handle_nota_dev(arguments),
            "nota_write_checkpoint" => self.handle_nota_write_checkpoint(arguments),
            "recovery_list_seed_runs" => self.handle_recovery_list_seed_runs(),
            "recovery_list_seed_rows" => self.handle_recovery_list_seed_rows(arguments),
            "vault_get_token" => self.handle_vault_get_token(arguments),
            "vault_list_mcp" => self.handle_vault_list_mcp(),
            "launcher_search" => self.handle_launcher_search(arguments),
            "launcher_launch" => self.handle_launcher_launch(arguments),
            _ => bail!("tool `{name}` is not registered"),
        }
    }

    fn ensure_tool_is_available(&self, name: &str) -> Result<()> {
        let Some(actor_role) = self.actor_role else {
            return Ok(());
        };

        let Some(permission) = permission_for_registered_tool(name) else {
            return Ok(());
        };

        if permission.actor_role != actor_role {
            return Err(McpToolSurfaceRoleError {
                tool_name: name.to_string(),
                current_actor_role: actor_role,
                required_actor_role: permission.actor_role,
            }
            .into());
        }

        Ok(())
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
            dispatch_receipt: None,
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
        let explicit_worktree = optional_string(arguments, "worktree_path")
            .or_else(|| optional_string(arguments, "worktreePath"))
            .map(str::to_string);
        let dispatch = match explicit_worktree {
            Some(worktree_path) => prepare_agent_dispatch_for_worktree_blocking(
                forge.data_store(),
                project_dir,
                worktree_path,
            )
            .map_err(anyhow::Error::msg)?,
            None => prepare_agent_dispatch_blocking(forge.data_store(), project_dir)
                .map_err(anyhow::Error::msg)?,
        };
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
        let dispatch_receipt =
            parse_dispatch_receipt_request(arguments, "forge_dispatch_agent", ActorRole::Agent)?;
        if let Some(receipt) = dispatch_receipt.as_ref() {
            forge.get_task(receipt.parent_task_id)?.ok_or_else(|| {
                anyhow!(
                    "parent forge task `{}` was not found for dispatch supervision",
                    receipt.parent_task_id
                )
            })?;
        }

        let mut request = build_agent_task_request(
            issue_id.to_string(),
            worktree_path.to_string(),
            model.to_string(),
            prompt.to_string(),
            required_tokens,
            agent_command,
        )
        .map_err(anyhow::Error::msg)?;
        request.dispatch_receipt = dispatch_receipt;

        let task_id = forge.create_task(request).map_err(anyhow::Error::msg)?;
        forge
            .engine()
            .spawn_task(task_id)
            .with_context(|| format!("failed to start forge task `{task_id}`"))?;

        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` disappeared after creation"))?;
        let supervision = forge.get_task_supervision(task_id)?;

        Ok(json!({
            "dispatch_role": "agent",
            "dispatch_tool_name": "forge_dispatch_agent",
            "task_id": task.id,
            "task": task,
            "supervision": supervision,
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
        let dispatch_receipt =
            parse_dispatch_receipt_request(arguments, "forge_dispatch_dev", ActorRole::Dev)?;
        if let Some(receipt) = dispatch_receipt.as_ref() {
            forge.get_task(receipt.parent_task_id)?.ok_or_else(|| {
                anyhow!(
                    "parent forge task `{}` was not found for dispatch supervision",
                    receipt.parent_task_id
                )
            })?;
        }

        let mut request = build_dev_task_request(
            issue_id.to_string(),
            worktree_path.to_string(),
            model.to_string(),
            prompt.to_string(),
            required_tokens,
            agent_command,
        )
        .map_err(anyhow::Error::msg)?;
        request.dispatch_receipt = dispatch_receipt;

        let task_id = forge.create_task(request).map_err(anyhow::Error::msg)?;
        forge
            .engine()
            .spawn_task(task_id)
            .with_context(|| format!("failed to start forge task `{task_id}`"))?;

        let task = forge
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("forge task `{task_id}` disappeared after creation"))?;
        let supervision = forge.get_task_supervision(task_id)?;

        Ok(json!({
            "dispatch_role": "dev",
            "dispatch_tool_name": "forge_dispatch_dev",
            "task_id": task.id,
            "task": task,
            "supervision": supervision,
        }))
    }

    fn handle_forge_bootstrap_mcp_cycle(&self, arguments: &Value) -> Result<Value> {
        self.plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;

        let project_dir = optional_string(arguments, "project_dir")
            .or_else(|| optional_string(arguments, "projectDir"))
            .map(str::to_string);
        let model = optional_string(arguments, "model")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("codex")
            .to_string();
        let agent_command = optional_string(arguments, "agent_command")
            .or_else(|| optional_string(arguments, "agentCommand"))
            .map(str::to_string);
        let agent_count = match optional_i64(arguments, &["agent_count", "agentCount"]) {
            Some(value) if value <= 0 => {
                bail!("tool argument `agent_count`/`agentCount` must be >= 1")
            }
            Some(value) => usize::try_from(value)
                .context("tool argument `agent_count`/`agentCount` is out of range")?,
            None => 1,
        };

        let report = run_forge_bootstrap_mcp_cycle(
            self.plugins
                .forge
                .as_ref()
                .context("forge plugin is not enabled")?,
            &resolve_app_data_dir()?,
            ForgeBootstrapMcpCycleOptions {
                project_dir,
                model,
                agent_command,
                agent_count,
            },
        )?;

        serde_json::to_value(report).context("failed to serialize forge bootstrap MCP cycle report")
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
        let supervision = forge.get_task_supervision(task_id)?;
        Ok(json!({
            "task_id": task.id,
            "task": task,
            "supervision": supervision,
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

    fn handle_recovery_list_seed_runs(&self) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        Ok(json!(list_recovery_seed_runs(data_store)?))
    }

    fn handle_nota_runtime_overview(&self) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        Ok(json!(build_nota_runtime_overview(data_store)?))
    }

    fn handle_nota_runtime_status(&self) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        Ok(json!(build_nota_runtime_status(data_store)?))
    }

    fn handle_nota_runtime_allocations(&self) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        Ok(json!(list_nota_runtime_allocations(data_store)?))
    }

    fn handle_nota_runtime_receipts(&self, arguments: &Value) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        let transaction_id = optional_i64(arguments, &["transaction_id", "transactionId"]);
        if let Some(transaction_id) = transaction_id {
            if transaction_id <= 0 {
                bail!("runtime receipts `transaction_id` must be >= 1");
            }
        }
        Ok(json!(list_nota_runtime_receipts(
            data_store,
            transaction_id
        )?))
    }

    fn handle_nota_do(&self, arguments: &Value) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let request = parse_nota_do_request(arguments);
        Ok(json!(run_nota_do_agent_dispatch(
            data_store, forge, request
        )?))
    }

    fn handle_nota_dev(&self, arguments: &Value) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        let forge = self
            .plugins
            .forge
            .as_ref()
            .context("forge plugin is not enabled")?;
        let request = parse_nota_dev_request(arguments);
        Ok(json!(run_nota_dev_dispatch(data_store, forge, request)?))
    }

    fn handle_nota_write_checkpoint(&self, arguments: &Value) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        let request = parse_nota_checkpoint_request(arguments)?;
        Ok(json!(write_runtime_checkpoint(data_store, request)?))
    }

    fn handle_recovery_list_seed_rows(&self, arguments: &Value) -> Result<Value> {
        let data_store = self
            .plugins
            .core_data_store
            .as_ref()
            .context("core data store is not available on the current MCP surface")?;
        let limit = optional_i64(arguments, &["limit"])
            .map(|value| {
                usize::try_from(value)
                    .map_err(|_| anyhow!("recovery rows `limit` must be a positive integer"))
            })
            .transpose()?;
        let query = RecoverySeedRowsQuery {
            ingest_run_id: optional_i64(arguments, &["ingest_run_id", "ingestRunId"]),
            table_name: optional_string_any(arguments, &["table_name", "tableName"])
                .map(str::to_string),
            limit,
        };
        Ok(json!(list_recovery_seed_rows(data_store, query)?))
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

fn build_tool_descriptors(
    plugins: &McpPluginSet,
    actor_role: Option<ActorRole>,
) -> Vec<McpToolDescriptor> {
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
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_prepare_agent_dispatch",
            description: "Prepare an Entrance-owned agent-lane Forge dispatch from the managed worktree for a project.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." },
                    "worktree_path": { "type": "string", "description": "Optional explicit managed worktree path. Useful for per-agent slot worktree allocation." },
                    "worktreePath": { "type": "string", "description": "CamelCase alias for worktree_path." }
                }
            }),
            permission: None,
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_verify_agent_dispatch",
            description: "Prepare and persist a Pending agent-lane Forge dispatch without starting agent execution.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." }
                }
            }),
            permission: None,
            dispatch_role: None,
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
            dispatch_role: None,
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
            dispatch_role: None,
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
            dispatch_role: None,
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
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "forge_bootstrap_mcp_cycle",
            description: "Run the current Nota-owned bootstrap allocator cut across Arch, Dev, and Agent MCP surfaces. Multi-agent fan-out still shares one resolved worktree.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." },
                    "model": { "type": "string", "description": "Runner or runner:model string used for child agent dispatches. Defaults to codex." },
                    "agent_command": { "type": "string", "description": "Optional executable path overriding the default agent CLI for child agent dispatches." },
                    "agentCommand": { "type": "string", "description": "CamelCase alias for agent_command." },
                    "agent_count": { "type": "integer", "description": "Number of agent children to fan out through the current shared-worktree bootstrap cut. Defaults to 1." },
                    "agentCount": { "type": "integer", "description": "CamelCase alias for agent_count." }
                }
            }),
            permission: None,
            dispatch_role: None,
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
            dispatch_role: None,
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
            dispatch_role: None,
        });
    }

    if plugins.core_data_store.is_some() {
        tools.push(McpToolDescriptor {
            name: "nota_runtime_overview",
            description: "Read the current NOTA runtime continuity bundle that powers `entrance nota overview`.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            permission: None,
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "nota_runtime_status",
            description:
                "Read the compact NOTA runtime status panel that powers `entrance nota status`.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            permission: None,
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "nota_runtime_allocations",
            description: "Read the persisted NOTA runtime allocations report that powers `entrance nota allocations`.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            permission: None,
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "nota_runtime_receipts",
            description: "Read the persisted NOTA runtime receipt history that powers `entrance nota receipts`, optionally filtered by transaction.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction_id": { "type": "integer", "description": "Optional NOTA runtime transaction identifier used to filter receipt history." },
                    "transactionId": { "type": "integer", "description": "CamelCase alias for transaction_id." }
                }
            }),
            permission: None,
            dispatch_role: None,
        });
        if plugins.forge.is_some() {
            tools.push(McpToolDescriptor {
                name: "nota_do",
                description: "Create a real NOTA `Do` transaction that records runtime receipts and a checkpoint while dispatching through the existing Forge runtime.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                        "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." },
                        "model": { "type": "string", "description": "Runner or runner:model string used for the dispatched task. Defaults to codex." },
                        "agent_command": { "type": "string", "description": "Optional executable path overriding the default agent CLI." },
                        "agentCommand": { "type": "string", "description": "CamelCase alias for agent_command." },
                        "title": { "type": "string", "description": "Optional human-readable transaction title." }
                    }
                }),
                permission: None,
                dispatch_role: None,
            });
            tools.push(McpToolDescriptor {
                name: "nota_dev",
                description: "Create a real NOTA-owned dev transaction that records runtime receipts and a checkpoint while dispatching through the existing Forge dev runtime.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_dir": { "type": "string", "description": "Optional repo root used to resolve the managed Forge worktree." },
                        "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." },
                        "model": { "type": "string", "description": "Runner or runner:model string used for the dispatched task. Defaults to codex." },
                        "agent_command": { "type": "string", "description": "Optional executable path overriding the default agent CLI." },
                        "agentCommand": { "type": "string", "description": "CamelCase alias for agent_command." },
                        "title": { "type": "string", "description": "Optional human-readable transaction title." }
                    }
                }),
                permission: None,
                dispatch_role: None,
            });
        }
        tools.push(McpToolDescriptor {
            name: "nota_write_checkpoint",
            description: "Write a NOTA runtime checkpoint into the canonical continuity storage cut.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Optional checkpoint title." },
                    "stable_level": { "type": "string", "description": "Stable-level summary for the checkpoint." },
                    "stableLevel": { "type": "string", "description": "CamelCase alias for stable_level." },
                    "landed": {
                        "type": "array",
                        "description": "One or more landed facts captured by this checkpoint.",
                        "items": { "type": "string" }
                    },
                    "remaining": {
                        "type": "array",
                        "description": "One or more remaining gates after this checkpoint.",
                        "items": { "type": "string" }
                    },
                    "human_continuity_bus": { "type": "string", "description": "Current human continuity-bus requirement." },
                    "humanContinuityBus": { "type": "string", "description": "CamelCase alias for human_continuity_bus." },
                    "selected_trunk": { "type": "string", "description": "Optional active trunk name." },
                    "selectedTrunk": { "type": "string", "description": "CamelCase alias for selected_trunk." },
                    "next_start_hints": {
                        "type": "array",
                        "description": "Optional next-window hints.",
                        "items": { "type": "string" }
                    },
                    "nextStartHints": {
                        "type": "array",
                        "description": "CamelCase alias for next_start_hints.",
                        "items": { "type": "string" }
                    },
                    "project_dir": { "type": "string", "description": "Optional project directory used to capture repo context." },
                    "projectDir": { "type": "string", "description": "CamelCase alias for project_dir." }
                },
                "required": ["stable_level", "landed", "remaining", "human_continuity_bus"]
            }),
            permission: None,
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "recovery_list_seed_runs",
            description: "List recovery-seed imports that have been absorbed into the runtime DB storage plane.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            permission: None,
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "recovery_list_seed_rows",
            description: "List absorbed recovery-seed rows from the runtime DB, optionally filtered by ingest run or source table.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ingest_run_id": { "type": "integer", "description": "Optional recovery ingest run identifier. Defaults to the latest recovery run." },
                    "ingestRunId": { "type": "integer", "description": "CamelCase alias for ingest_run_id." },
                    "table_name": { "type": "string", "description": "Optional recovery source table name such as documents or decisions." },
                    "tableName": { "type": "string", "description": "CamelCase alias for table_name." },
                    "limit": { "type": "integer", "description": "Optional maximum number of rows to return. Defaults to 50." }
                }
            }),
            permission: None,
            dispatch_role: None,
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
            dispatch_role: None,
        });
        tools.push(McpToolDescriptor {
            name: "vault_list_mcp",
            description: "List saved MCP endpoint configurations.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            permission: None,
            dispatch_role: None,
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
            dispatch_role: None,
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
            dispatch_role: None,
        });
    }

    for tool in &mut tools {
        tool.permission = permission_for_registered_tool(tool.name);
        tool.dispatch_role = tool_dispatch_role_from_name(tool.name);
    }

    tools
        .into_iter()
        .filter(|tool| tool_is_visible_to_actor(tool, actor_role))
        .collect()
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

fn optional_string_any<'a>(arguments: &'a Value, fields: &[&str]) -> Option<&'a str> {
    fields
        .iter()
        .find_map(|field| arguments.get(*field).and_then(Value::as_str))
}

fn optional_i64(arguments: &Value, fields: &[&str]) -> Option<i64> {
    fields
        .iter()
        .find_map(|field| arguments.get(*field).and_then(Value::as_i64))
}

fn require_i64(arguments: &Value, fields: &[&str]) -> Result<i64> {
    for field in fields {
        if let Some(value) = arguments.get(*field).and_then(Value::as_i64) {
            return Ok(value);
        }
    }

    bail!("tool arguments require one of: {}", fields.join(", "))
}

fn parse_dispatch_receipt_request(
    arguments: &Value,
    child_dispatch_tool_name: &str,
    child_dispatch_role: ActorRole,
) -> Result<Option<DispatchReceiptRequest>> {
    let parent_task_id = optional_i64(arguments, &["parent_task_id", "parentTaskId"]);
    let supervision_strategy =
        optional_string_any(arguments, &["supervision_strategy", "supervisionStrategy"]);
    let child_slot = optional_string_any(arguments, &["child_slot", "childSlot"]);

    if parent_task_id.is_none() {
        if supervision_strategy.is_some() || child_slot.is_some() {
            bail!("dispatch supervision fields require `parent_task_id`/`parentTaskId` to be set");
        }
        return Ok(None);
    }

    let supervision_strategy = match supervision_strategy.unwrap_or("one_for_one") {
        "one_for_one" => SupervisionStrategy::OneForOne,
        "rest_for_one" => SupervisionStrategy::RestForOne,
        "one_for_all" => SupervisionStrategy::OneForAll,
        other => {
            bail!(
                "unsupported `supervision_strategy`: `{other}`; use `one_for_one`, `rest_for_one`, or `one_for_all`"
            )
        }
    };

    Ok(Some(DispatchReceiptRequest {
        parent_task_id: parent_task_id.expect("parent task id should be present"),
        supervision_strategy,
        child_dispatch_role,
        child_dispatch_tool_name: child_dispatch_tool_name.to_string(),
        child_slot: child_slot.map(str::to_string),
    }))
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

fn require_nonempty_string_list(arguments: &Value, fields: &[&str]) -> Result<Vec<String>> {
    let values = require_string_list(arguments, fields)?;
    if values.is_empty() {
        bail!(
            "tool arguments require one of these non-empty string-array fields: {}",
            fields.join(", ")
        );
    }
    Ok(values)
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

fn parse_nota_checkpoint_request(arguments: &Value) -> Result<NotaCheckpointRequest> {
    Ok(NotaCheckpointRequest {
        title: optional_string(arguments, "title").map(str::to_string),
        stable_level: require_string_any(arguments, &["stable_level", "stableLevel"])?.to_string(),
        landed: require_nonempty_string_list(arguments, &["landed"])?,
        remaining: require_nonempty_string_list(arguments, &["remaining"])?,
        human_continuity_bus: require_string_any(
            arguments,
            &["human_continuity_bus", "humanContinuityBus"],
        )?
        .to_string(),
        selected_trunk: optional_string_any(arguments, &["selected_trunk", "selectedTrunk"])
            .map(str::to_string),
        next_start_hints: require_string_list(arguments, &["next_start_hints", "nextStartHints"])?,
        project_dir: optional_string_any(arguments, &["project_dir", "projectDir"])
            .map(str::to_string),
    })
}

fn parse_nota_dispatch_request(arguments: &Value) -> NotaDoAgentDispatchRequest {
    NotaDoAgentDispatchRequest {
        project_dir: optional_string_any(arguments, &["project_dir", "projectDir"])
            .map(str::to_string),
        model: optional_string(arguments, "model")
            .unwrap_or("codex")
            .to_string(),
        agent_command: optional_string_any(arguments, &["agent_command", "agentCommand"])
            .map(str::to_string),
        title: optional_string(arguments, "title").map(str::to_string),
        execution_host: NotaDispatchExecutionHost::InProcess,
    }
}

fn parse_nota_do_request(arguments: &Value) -> NotaDoAgentDispatchRequest {
    parse_nota_dispatch_request(arguments)
}

fn parse_nota_dev_request(arguments: &Value) -> NotaDevDispatchRequest {
    parse_nota_dispatch_request(arguments)
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

fn tool_is_visible_to_actor(tool: &McpToolDescriptor, actor_role: Option<ActorRole>) -> bool {
    let Some(actor_role) = actor_role else {
        return true;
    };

    tool.permission
        .map(|permission| permission.actor_role == actor_role)
        .unwrap_or(true)
}

fn actor_role_slug(role: ActorRole) -> &'static str {
    match role {
        ActorRole::Nota => "nota",
        ActorRole::Arch => "arch",
        ActorRole::Dev => "dev",
        ActorRole::Agent => "agent",
    }
}

fn tool_call_result(
    result: Result<Value>,
    surface_info: McpSurfaceInfo,
    permission: Option<McpToolPermission>,
    dispatch_role: Option<ActorRole>,
    canonical_tool_name: Option<&'static str>,
) -> Value {
    match result {
        Ok(value) => json!({
            "content": [
                {
                    "type": "text",
                    "text": to_pretty_json(&value),
                }
            ],
            "structuredContent": value,
            "entranceSurface": surface_info,
            "permission": permission,
            "dispatchRole": dispatch_role,
            "canonicalToolName": canonical_tool_name,
            "isError": false,
        }),
        Err(error) => tool_call_error_result(
            error,
            surface_info,
            permission,
            dispatch_role,
            canonical_tool_name,
        ),
    }
}

fn tool_call_error_result(
    error: anyhow::Error,
    surface_info: McpSurfaceInfo,
    permission: Option<McpToolPermission>,
    dispatch_role: Option<ActorRole>,
    canonical_tool_name: Option<&'static str>,
) -> Value {
    let message = error.to_string();
    let structured_content =
        if let Some(role_error) = error.downcast_ref::<McpToolSurfaceRoleError>() {
            json!({
                "message": message,
                "errorCode": "surface_role_mismatch",
                "toolName": role_error.tool_name,
                "currentActorRole": role_error.current_actor_role,
                "requiredActorRole": role_error.required_actor_role,
                "entranceSurface": {
                    "actorRole": role_error.current_actor_role
                }
            })
        } else {
            json!({
                "message": message,
            })
        };

    json!({
        "content": [
            {
                "type": "text",
                "text": message,
            }
        ],
        "structuredContent": structured_content,
        "entranceSurface": surface_info,
        "permission": permission,
        "dispatchRole": dispatch_role,
        "canonicalToolName": canonical_tool_name,
        "isError": true,
    })
}

fn tool_name_from_params(params: Option<&Value>) -> Option<&str> {
    params
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
}

fn permission_for_registered_tool(name: &str) -> Option<McpToolPermission> {
    permission_for_mcp_tool(name)
}

fn tool_dispatch_role_from_name(name: &str) -> Option<ActorRole> {
    match name {
        "nota_do" => Some(ActorRole::Agent),
        "nota_dev" => Some(ActorRole::Dev),
        "forge_prepare_dispatch"
        | "forge_verify_dispatch"
        | "forge_prepare_agent_dispatch"
        | "forge_verify_agent_dispatch"
        | "forge_dispatch_agent" => Some(ActorRole::Agent),
        "forge_prepare_dev_dispatch" | "forge_verify_dev_dispatch" | "forge_dispatch_dev" => {
            Some(ActorRole::Dev)
        }
        _ => None,
    }
}

fn canonical_tool_name_from_name(name: &str) -> Option<&'static str> {
    match name {
        "forge_prepare_dispatch" | "forge_prepare_agent_dispatch" => {
            Some("forge_prepare_agent_dispatch")
        }
        "forge_verify_dispatch" | "forge_verify_agent_dispatch" => {
            Some("forge_verify_agent_dispatch")
        }
        _ => None,
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

    use crate::core::action::ActorRole;

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
        assert!(response["result"]["entranceSurface"]["actorRole"].is_null());
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
        let bootstrap_cycle = tools
            .iter()
            .find(|tool| tool["name"] == "forge_bootstrap_mcp_cycle")
            .expect("forge_bootstrap_mcp_cycle should exist");
        let nota_overview = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_overview")
            .expect("nota_runtime_overview should exist");
        let nota_status = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_status")
            .expect("nota_runtime_status should exist");
        let nota_allocations = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_allocations")
            .expect("nota_runtime_allocations should exist");
        let nota_receipts = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_receipts")
            .expect("nota_runtime_receipts should exist");
        let nota_do = tools
            .iter()
            .find(|tool| tool["name"] == "nota_do")
            .expect("nota_do should exist");
        let nota_dev = tools
            .iter()
            .find(|tool| tool["name"] == "nota_dev")
            .expect("nota_dev should exist");
        let nota_checkpoint = tools
            .iter()
            .find(|tool| tool["name"] == "nota_write_checkpoint")
            .expect("nota_write_checkpoint should exist");
        let prepare_agent = tools
            .iter()
            .find(|tool| tool["name"] == "forge_prepare_agent_dispatch")
            .expect("forge_prepare_agent_dispatch should exist");
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
        assert_eq!(nota_status["permission"]["actorRole"], "nota");
        assert_eq!(nota_status["permission"]["primitive"], "chat");
        assert_eq!(nota_status["permission"]["room"], "surface");
        assert_eq!(nota_status["permission"]["targetLayer"], "cold");
        assert!(nota_status["dispatchRole"].is_null());
        assert_eq!(nota_allocations["permission"]["actorRole"], "nota");
        assert_eq!(nota_allocations["permission"]["primitive"], "chat");
        assert_eq!(nota_allocations["permission"]["room"], "surface");
        assert_eq!(nota_allocations["permission"]["targetLayer"], "cold");
        assert!(nota_allocations["dispatchRole"].is_null());
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

        Ok(())
    }

    #[test]
    fn arch_surface_lists_only_arch_dispatch_lane_plus_neutral_tools() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Arch))?;
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
        assert_eq!(response["result"]["entranceSurface"]["actorRole"], "arch");
        assert_eq!(response["result"]["tools"][1]["dispatchRole"], "dev");

        Ok(())
    }

    #[test]
    fn nota_surface_lists_bootstrap_allocator_and_continuity_tools() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Nota))?;
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
        assert_eq!(response["result"]["entranceSurface"]["actorRole"], "nota");
        let tools = response["result"]["tools"]
            .as_array()
            .expect("tools/list should return an array");
        let bootstrap_tool = tools
            .iter()
            .find(|tool| tool["name"] == "forge_bootstrap_mcp_cycle")
            .expect("forge_bootstrap_mcp_cycle should exist on nota surface");
        let overview_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_overview")
            .expect("nota_runtime_overview should exist on nota surface");
        let status_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_status")
            .expect("nota_runtime_status should exist on nota surface");
        let allocations_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_allocations")
            .expect("nota_runtime_allocations should exist on nota surface");
        let receipts_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_runtime_receipts")
            .expect("nota_runtime_receipts should exist on nota surface");
        let do_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_do")
            .expect("nota_do should exist on nota surface");
        let dev_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_dev")
            .expect("nota_dev should exist on nota surface");
        let checkpoint_tool = tools
            .iter()
            .find(|tool| tool["name"] == "nota_write_checkpoint")
            .expect("nota_write_checkpoint should exist on nota surface");
        assert!(bootstrap_tool["dispatchRole"].is_null());
        assert_eq!(bootstrap_tool["permission"]["actorRole"], "nota");
        assert_eq!(overview_tool["permission"]["primitive"], "chat");
        assert_eq!(overview_tool["permission"]["room"], "surface");
        assert_eq!(overview_tool["permission"]["targetLayer"], "cold");
        assert_eq!(status_tool["permission"]["primitive"], "chat");
        assert_eq!(status_tool["permission"]["room"], "surface");
        assert_eq!(status_tool["permission"]["targetLayer"], "cold");
        assert_eq!(allocations_tool["permission"]["actorRole"], "nota");
        assert_eq!(allocations_tool["permission"]["primitive"], "chat");
        assert_eq!(allocations_tool["permission"]["room"], "surface");
        assert_eq!(allocations_tool["permission"]["targetLayer"], "cold");
        assert_eq!(receipts_tool["permission"]["actorRole"], "nota");
        assert_eq!(receipts_tool["permission"]["primitive"], "chat");
        assert_eq!(receipts_tool["permission"]["room"], "surface");
        assert_eq!(receipts_tool["permission"]["targetLayer"], "cold");
        assert_eq!(do_tool["permission"]["actorRole"], "nota");
        assert_eq!(do_tool["permission"]["primitive"], "assign");
        assert_eq!(do_tool["permission"]["room"], "strategy");
        assert_eq!(do_tool["permission"]["targetLayer"], "hot");
        assert_eq!(do_tool["dispatchRole"], "agent");
        assert_eq!(dev_tool["permission"]["actorRole"], "nota");
        assert_eq!(dev_tool["permission"]["primitive"], "assign");
        assert_eq!(dev_tool["permission"]["room"], "strategy");
        assert_eq!(dev_tool["permission"]["targetLayer"], "hot");
        assert_eq!(dev_tool["dispatchRole"], "dev");
        assert_eq!(checkpoint_tool["permission"]["primitive"], "learn");
        assert_eq!(checkpoint_tool["permission"]["room"], "memory");
        assert_eq!(checkpoint_tool["permission"]["targetLayer"], "cold");

        Ok(())
    }

    #[test]
    fn dev_surface_lists_only_dev_dispatch_lane_plus_neutral_tools() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Dev))?;
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
        assert_eq!(response["result"]["entranceSurface"]["actorRole"], "dev");
        assert_eq!(response["result"]["tools"][1]["dispatchRole"], "agent");

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
        assert!(response["result"]["entranceSurface"]["actorRole"].is_null());

        Ok(())
    }

    #[test]
    fn scoped_initialize_reports_current_surface_role() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Dev))?;
        let response = server
            .handle_json_rpc_value(json!({
                "jsonrpc": "2.0",
                "id": "init",
                "method": "initialize",
                "params": {}
            }))?
            .expect("initialize should return a response");

        assert_eq!(response["result"]["entranceSurface"]["actorRole"], "dev");

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
        assert!(launcher_response["entranceSurface"]["actorRole"].is_null());
        assert!(launcher_response["permission"].is_null());
        assert!(launcher_response["dispatchRole"].is_null());
        assert!(launcher_response["canonicalToolName"].is_null());
        assert_eq!(
            launcher_response["structuredContent"]["results"][0]["path"],
            "C:\\Tools\\Code.exe"
        );

        let vault_response = call_tool(&server, "vault_get_token", json!({ "token_id": 1 }))?;
        assert_eq!(vault_response["isError"], false);
        assert!(vault_response["entranceSurface"]["actorRole"].is_null());
        assert!(vault_response["permission"].is_null());
        assert!(vault_response["dispatchRole"].is_null());
        assert!(vault_response["canonicalToolName"].is_null());
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
        assert!(mcp_list_response["entranceSurface"]["actorRole"].is_null());
        assert!(mcp_list_response["permission"].is_null());
        assert!(mcp_list_response["dispatchRole"].is_null());
        assert!(mcp_list_response["canonicalToolName"].is_null());
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
        assert!(forge_response["entranceSurface"]["actorRole"].is_null());
        assert!(forge_response["permission"].is_null());
        assert!(forge_response["dispatchRole"].is_null());
        assert!(forge_response["canonicalToolName"].is_null());
        assert!(
            forge_response["structuredContent"]["task_id"]
                .as_i64()
                .unwrap()
                > 0
        );

        Ok(())
    }

    #[test]
    fn scoped_surface_rejects_dispatch_tools_owned_by_other_role() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Dev))?;

        let response = call_tool(&server, "forge_prepare_dev_dispatch", json!({}))?;

        assert_eq!(response["isError"], true);
        assert_eq!(response["entranceSurface"]["actorRole"], "dev");
        assert_eq!(response["permission"]["actorRole"], "arch");
        assert_eq!(response["permission"]["primitive"], "assign");
        assert_eq!(response["permission"]["room"], "strategy");
        assert_eq!(response["permission"]["targetLayer"], "hot");
        assert_eq!(response["dispatchRole"], "dev");
        assert_eq!(
            response["structuredContent"]["message"],
            "tool `forge_prepare_dev_dispatch` is not available on the current `dev` MCP surface; requires `arch`"
        );
        assert_eq!(
            response["structuredContent"]["errorCode"],
            "surface_role_mismatch"
        );
        assert_eq!(
            response["structuredContent"]["toolName"],
            "forge_prepare_dev_dispatch"
        );
        assert_eq!(response["structuredContent"]["currentActorRole"], "dev");
        assert_eq!(response["structuredContent"]["requiredActorRole"], "arch");
        assert_eq!(
            response["structuredContent"]["entranceSurface"]["actorRole"],
            "dev"
        );

        Ok(())
    }

    #[test]
    fn scoped_agent_dispatch_alias_reports_canonical_tool_name() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Arch))?;

        let response = call_tool(&server, "forge_prepare_dispatch", json!({}))?;

        assert_eq!(response["isError"], true);
        assert_eq!(response["dispatchRole"], "agent");
        assert_eq!(
            response["canonicalToolName"],
            "forge_prepare_agent_dispatch"
        );
        assert_eq!(
            response["structuredContent"]["toolName"],
            "forge_prepare_dispatch"
        );

        Ok(())
    }

    #[test]
    fn scoped_tool_calls_report_current_surface_role() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Arch))?;

        let response = call_tool(&server, "vault_list_mcp", json!({}))?;

        assert_eq!(response["isError"], false);
        assert_eq!(response["entranceSurface"]["actorRole"], "arch");
        assert!(response["permission"].is_null());
        assert!(response["dispatchRole"].is_null());
        assert_eq!(
            response["structuredContent"]["servers"][0]["transport"],
            "stdio"
        );

        Ok(())
    }

    #[test]
    fn nota_surface_can_read_runtime_overview() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Nota))?;

        let response = call_tool(&server, "nota_runtime_overview", json!({}))?;

        assert_eq!(response["isError"], false);
        assert_eq!(response["entranceSurface"]["actorRole"], "nota");
        assert_eq!(response["permission"]["actorRole"], "nota");
        assert_eq!(response["permission"]["primitive"], "chat");
        assert_eq!(response["permission"]["room"], "surface");
        assert_eq!(response["permission"]["targetLayer"], "cold");
        assert!(response["dispatchRole"].is_null());
        assert_eq!(
            response["structuredContent"]["checkpoints"]["checkpoint_count"],
            0
        );
        assert_eq!(
            response["structuredContent"]["transactions"]["transaction_count"],
            0
        );
        assert_eq!(
            response["structuredContent"]["allocations"]["allocation_count"],
            0
        );

        Ok(())
    }

    #[test]
    fn nota_surface_can_read_runtime_receipts() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Nota))?;

        let response = call_tool(
            &server,
            "nota_runtime_receipts",
            json!({ "transaction_id": 1 }),
        )?;

        assert_eq!(response["isError"], false);
        assert_eq!(response["entranceSurface"]["actorRole"], "nota");
        assert_eq!(response["permission"]["actorRole"], "nota");
        assert_eq!(response["permission"]["primitive"], "chat");
        assert_eq!(response["permission"]["room"], "surface");
        assert_eq!(response["permission"]["targetLayer"], "cold");
        assert_eq!(response["structuredContent"]["requested_transaction_id"], 1);
        assert_eq!(response["structuredContent"]["receipt_count"], 0);
        assert_eq!(response["structuredContent"]["receipts"], json!([]));

        Ok(())
    }

    #[test]
    fn nota_surface_can_write_runtime_checkpoint() -> Result<()> {
        let server = build_test_server_with_actor_role(Some(ActorRole::Nota))?;

        let response = call_tool(
            &server,
            "nota_write_checkpoint",
            json!({
                "title": "MCP checkpoint",
                "stable_level": "single-ingress, checkpointed, DB-first NOTA host with MCP checkpoint write",
                "landed": ["MCP checkpoint writer landed"],
                "remaining": ["Drive a live runtime transaction"],
                "human_continuity_bus": "reduced but still partially required",
                "selected_trunk": "MCP continuity write primitive",
                "next_start_hints": ["Call nota_runtime_overview before other MCP work."]
            }),
        )?;

        assert_eq!(response["isError"], false);
        assert_eq!(response["entranceSurface"]["actorRole"], "nota");
        assert_eq!(response["permission"]["actorRole"], "nota");
        assert_eq!(response["permission"]["primitive"], "learn");
        assert_eq!(response["permission"]["room"], "memory");
        assert_eq!(response["permission"]["targetLayer"], "cold");
        assert_eq!(
            response["structuredContent"]["checkpoint"]["title"],
            "MCP checkpoint"
        );
        assert_eq!(
            response["structuredContent"]["checkpoint"]["payload"]["selected_trunk"],
            "MCP continuity write primitive"
        );

        let overview = call_tool(&server, "nota_runtime_overview", json!({}))?;
        assert_eq!(
            overview["structuredContent"]["checkpoints"]["checkpoint_count"],
            1
        );
        assert_eq!(
            overview["structuredContent"]["checkpoints"]["checkpoints"][0]["title"],
            "MCP checkpoint"
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
        build_test_server_with_actor_role(None)
    }

    fn build_test_server_with_actor_role(actor_role: Option<ActorRole>) -> Result<McpServer> {
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
        let vault = VaultPlugin::new(data_store.clone())?;
        vault.add_token("Primary", "openai", "secret-token")?;
        vault.update_mcp_config(None, "Local MCP", "stdio", "npx -y some-mcp", true)?;

        Ok(McpServer::with_actor_role(
            McpTransport::InProcess,
            McpPluginSet {
                core_data_store: Some(data_store.clone()),
                forge: Some(forge),
                launcher: Some(launcher),
                vault: Some(vault),
            },
            actor_role,
        ))
    }
}
