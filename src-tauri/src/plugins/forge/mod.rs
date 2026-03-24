pub mod commands;
pub mod engine;
pub mod http;

use std::{
    env,
    ffi::OsStr,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use crate::{
    core::{
        action::ActorRole,
        data_store::{
            DataStore, MigrationStep, NewForgeDispatchReceipt, StoredForgeDispatchReceipt,
            StoredForgeTask, StoredForgeTaskLog,
        },
        event_bus::EventBus,
        supervision::SupervisionStrategy,
    },
    plugins::vault::VaultCipher,
    plugins::{AppContext, Event, Manifest, McpToolDefinition, Plugin, TauriCommandDefinition},
};
use anyhow::Result;
use engine::TaskEngine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::async_runtime::JoinHandle;

const MANIFEST: Manifest = Manifest {
    name: "forge",
    version: env!("CARGO_PKG_VERSION"),
    description: "Agent task management and execution engine.",
};

const MIGRATIONS: [MigrationStep; 3] = [
    MigrationStep {
        name: "0002_create_plugin_forge_tasks",
        sql: include_str!("../../../migrations/0002_create_plugin_forge_tasks.sql"),
    },
    MigrationStep {
        name: "0004_create_plugin_forge_task_logs",
        sql: include_str!("../../../migrations/0004_create_plugin_forge_task_logs.sql"),
    },
    MigrationStep {
        name: "0006_create_plugin_forge_dispatch_receipts",
        sql: include_str!("../../../migrations/0006_create_plugin_forge_dispatch_receipts.sql"),
    },
];

const ENTRANCE_BOOTSTRAP_SKILL_RELATIVE_PATH: &str = "harness/bootstrap/duet/SKILL.md";
const ENTRANCE_BOOTSTRAP_PROMPT_SOURCE_LABEL: &str = "Entrance-owned harness/bootstrap prompt";
const ENTRANCE_BOOTSTRAP_DEV_ROLE_RELATIVE_PATH: &str = "harness/bootstrap/duet/roles/dev.md";
const ENTRANCE_BOOTSTRAP_DEV_PROMPT_SOURCE_LABEL: &str =
    "Entrance-owned harness/bootstrap dev prompt";
const FORGE_AGENT_DISPATCH_ROLE: ActorRole = ActorRole::Agent;
const FORGE_DEV_DISPATCH_ROLE: ActorRole = ActorRole::Dev;
const FORGE_AGENT_DISPATCH_TOOL_NAME: &str = "forge_dispatch_agent";
const FORGE_DEV_DISPATCH_TOOL_NAME: &str = "forge_dispatch_dev";
const FORGE_DISPATCH_PIPELINE_SCOPE: &str = "dispatch_pipeline";

pub fn migrations() -> &'static [MigrationStep] {
    &MIGRATIONS
}

#[derive(Clone)]
pub struct ForgePlugin {
    manifest: Manifest,
    data_store: DataStore,
    event_bus: EventBus,
    engine: Arc<TaskEngine>,
    http_server: Arc<Mutex<Option<JoinHandle<()>>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgeTaskDetails {
    #[serde(flatten)]
    pub task: StoredForgeTask,
    pub logs: Vec<StoredForgeTaskLog>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgeTaskSupervisionSnapshot {
    pub parent_receipt: Option<StoredForgeDispatchReceipt>,
    pub child_receipts: Vec<StoredForgeDispatchReceipt>,
}

#[derive(Debug, Clone)]
pub struct DispatchReceiptRequest {
    pub parent_task_id: i64,
    pub supervision_strategy: SupervisionStrategy,
    pub child_dispatch_role: ActorRole,
    pub child_dispatch_tool_name: String,
    pub child_slot: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateTaskRequest {
    pub name: String,
    pub command: String,
    pub args: String,
    pub working_dir: Option<String>,
    pub stdin_text: Option<String>,
    pub required_tokens: String,
    pub metadata: String,
    pub dispatch_receipt: Option<DispatchReceiptRequest>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForgeTaskMetadata {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub issue_id: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub dispatch_role: Option<ActorRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocator_role: Option<ActorRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocator_surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedAgentDispatch {
    pub dispatch_role: ActorRole,
    pub dispatch_tool_name: String,
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt_source: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgeDispatchVerificationReport {
    pub dispatch: PreparedAgentDispatch,
    pub task_id: i64,
    pub task_status: String,
    pub task_command: String,
    pub task_working_dir: Option<String>,
    pub prompt_via_stdin: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedDevDispatch {
    pub dispatch_role: ActorRole,
    pub dispatch_tool_name: String,
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt_source: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgeDevDispatchVerificationReport {
    pub dispatch: PreparedDevDispatch,
    pub task_id: i64,
    pub task_status: String,
    pub task_command: String,
    pub task_working_dir: Option<String>,
    pub prompt_via_stdin: bool,
}

#[derive(Debug, Clone)]
struct DispatchPaths {
    issue_id: String,
    project_root: String,
    worktree_path: String,
}

#[derive(Debug, Clone)]
struct LinearIssueSummary {
    issue_status: String,
    issue_title: String,
}

#[derive(Debug, Deserialize)]
struct LinearIssueEnvelope {
    data: Option<LinearIssueData>,
    errors: Option<Vec<LinearGraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct LinearIssueData {
    issues: LinearIssueConnection,
}

#[derive(Debug, Deserialize)]
struct LinearIssueConnection {
    nodes: Vec<LinearIssueNode>,
}

#[derive(Debug, Deserialize)]
struct LinearIssueNode {
    title: String,
    state: LinearIssueState,
}

#[derive(Debug, Deserialize)]
struct LinearIssueState {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LinearGraphQlError {
    message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgeTaskStatusEvent {
    pub id: i64,
    pub status: String,
    pub status_message: Option<String>,
    pub exit_code: Option<i64>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgeTaskLogEvent {
    pub id: i64,
    pub task_id: i64,
    pub stream: String,
    pub line: String,
    pub created_at: String,
}

impl From<&StoredForgeTask> for ForgeTaskStatusEvent {
    fn from(task: &StoredForgeTask) -> Self {
        Self {
            id: task.id,
            status: task.status.clone(),
            status_message: task.status_message.clone(),
            exit_code: task.exit_code,
            finished_at: task.finished_at.clone(),
        }
    }
}

impl From<&StoredForgeTaskLog> for ForgeTaskLogEvent {
    fn from(log: &StoredForgeTaskLog) -> Self {
        Self {
            id: log.id,
            task_id: log.task_id,
            stream: log.stream.clone(),
            line: log.line.clone(),
            created_at: log.created_at.clone(),
        }
    }
}

impl ForgePlugin {
    pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
        Self {
            manifest: MANIFEST,
            data_store: data_store.clone(),
            event_bus: event_bus.clone(),
            engine: Arc::new(TaskEngine::new(data_store, event_bus)),
            http_server: Arc::new(Mutex::new(None)),
        }
    }

    pub fn create_task(&self, request: CreateTaskRequest) -> Result<i64> {
        if let Some(dispatch_receipt) = request.dispatch_receipt.as_ref() {
            let (task_id, _) = self.data_store.insert_forge_task_with_dispatch_receipt(
                &request.name,
                &request.command,
                &request.args,
                request.working_dir.as_deref(),
                request.stdin_text.as_deref(),
                &request.required_tokens,
                &request.metadata,
                &NewForgeDispatchReceipt {
                    parent_task_id: dispatch_receipt.parent_task_id,
                    supervision_scope: FORGE_DISPATCH_PIPELINE_SCOPE,
                    supervision_strategy: supervision_strategy_slug(
                        dispatch_receipt.supervision_strategy,
                    ),
                    child_dispatch_role: actor_role_slug(dispatch_receipt.child_dispatch_role),
                    child_dispatch_tool_name: &dispatch_receipt.child_dispatch_tool_name,
                    child_slot: dispatch_receipt.child_slot.as_deref(),
                },
            )?;
            Ok(task_id)
        } else {
            self.data_store.insert_forge_task(
                &request.name,
                &request.command,
                &request.args,
                request.working_dir.as_deref(),
                request.stdin_text.as_deref(),
                &request.required_tokens,
                &request.metadata,
            )
        }
    }

    pub fn replace_pending_task_request(&self, id: i64, request: CreateTaskRequest) -> Result<()> {
        self.data_store.update_pending_forge_task_request(
            id,
            &request.command,
            &request.args,
            request.working_dir.as_deref(),
            request.stdin_text.as_deref(),
            &request.required_tokens,
            &request.metadata,
        )
    }

    pub fn list_tasks(&self) -> Result<Vec<StoredForgeTask>> {
        self.data_store.list_forge_tasks()
    }

    pub fn get_task(&self, id: i64) -> Result<Option<StoredForgeTask>> {
        self.data_store.get_forge_task(id)
    }

    pub fn list_task_logs(&self, id: i64) -> Result<Vec<StoredForgeTaskLog>> {
        self.data_store.list_forge_task_logs(id)
    }

    pub fn get_task_details(&self, id: i64) -> Result<Option<ForgeTaskDetails>> {
        let Some(task) = self.get_task(id)? else {
            return Ok(None);
        };

        let logs = self.list_task_logs(id)?;
        Ok(Some(ForgeTaskDetails { task, logs }))
    }

    pub fn get_task_supervision(&self, id: i64) -> Result<ForgeTaskSupervisionSnapshot> {
        Ok(ForgeTaskSupervisionSnapshot {
            parent_receipt: self.data_store.get_forge_dispatch_parent_receipt(id)?,
            child_receipts: self.data_store.list_forge_dispatch_child_receipts(id)?,
        })
    }

    pub fn cancel_task(&self, id: i64) -> Result<()> {
        self.engine.cancel_task(id)
    }

    pub fn engine(&self) -> Arc<TaskEngine> {
        self.engine.clone()
    }

    pub fn data_store(&self) -> DataStore {
        self.data_store.clone()
    }

    pub fn subscribe_events(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::core::event_bus::EventPayload> {
        self.event_bus.subscribe()
    }

    pub fn start_http_server(&self, port: u16) -> Result<()> {
        let mut server = self
            .http_server
            .lock()
            .map_err(|_| anyhow::anyhow!("forge HTTP server lock poisoned"))?;

        if server.is_some() {
            return Ok(());
        }

        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let listener = StdTcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let plugin = self.clone();

        let handle = tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(error) => {
                    tracing::error!(?error, "failed to create async TCP listener for forge HTTP");
                    return;
                }
            };
            let app = http::router(plugin);
            if let Err(error) = axum::serve(listener, app).await {
                tracing::error!(?error, "forge HTTP server stopped unexpectedly");
            }
        });

        *server = Some(handle);
        tracing::info!("forge HTTP API listening on http://127.0.0.1:{port}");
        Ok(())
    }
}

pub async fn prepare_agent_dispatch(
    data_store: DataStore,
    project_dir: Option<String>,
) -> Result<PreparedAgentDispatch, String> {
    let paths = tauri::async_runtime::spawn_blocking(move || {
        resolve_dispatch_paths(project_dir.as_deref())
    })
    .await
    .map_err(|error| error.to_string())??;

    let issue_summary = fetch_linear_issue_summary(data_store, &paths.issue_id).await?;
    build_prepared_agent_dispatch(paths, issue_summary).await
}

pub async fn prepare_agent_dispatch_for_worktree(
    data_store: DataStore,
    project_dir: Option<String>,
    worktree_path: String,
) -> Result<PreparedAgentDispatch, String> {
    let project_dir_for_resolve = project_dir.clone();
    let worktree_path_for_resolve = worktree_path.clone();
    let paths = tauri::async_runtime::spawn_blocking(move || {
        let worktree_roots = resolve_dispatch_worktree_roots()?;
        resolve_dispatch_paths_for_explicit_worktree(
            project_dir_for_resolve.as_deref(),
            &worktree_path_for_resolve,
            &worktree_roots,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    let issue_summary = fetch_linear_issue_summary(data_store, &paths.issue_id).await?;
    build_prepared_agent_dispatch(paths, issue_summary).await
}

pub fn prepare_agent_dispatch_blocking(
    data_store: DataStore,
    project_dir: Option<String>,
) -> Result<PreparedAgentDispatch, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build Tokio runtime for Forge dispatch: {error}"))?;

    runtime.block_on(prepare_agent_dispatch(data_store, project_dir))
}

pub fn prepare_agent_dispatch_for_worktree_blocking(
    data_store: DataStore,
    project_dir: Option<String>,
    worktree_path: String,
) -> Result<PreparedAgentDispatch, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build Tokio runtime for Forge dispatch: {error}"))?;

    runtime.block_on(prepare_agent_dispatch_for_worktree(
        data_store,
        project_dir,
        worktree_path,
    ))
}

pub fn verify_agent_dispatch(
    forge: &ForgePlugin,
    project_dir: Option<String>,
) -> Result<ForgeDispatchVerificationReport, String> {
    let dispatch = prepare_agent_dispatch_blocking(forge.data_store(), project_dir)?;
    let request = build_agent_task_request(
        dispatch.issue_id.clone(),
        dispatch.worktree_path.clone(),
        "codex".to_string(),
        dispatch.prompt.clone(),
        Vec::new(),
        None,
    )?;

    let task_id = forge
        .create_task(request)
        .map_err(|error| error.to_string())?;
    let task = forge
        .get_task(task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "stored Forge verification task should exist".to_string())?;

    Ok(ForgeDispatchVerificationReport {
        dispatch,
        task_id,
        task_status: task.status,
        task_command: task.command,
        task_working_dir: task.working_dir,
        prompt_via_stdin: task.stdin_text.is_some(),
    })
}

pub async fn prepare_dev_dispatch(
    data_store: DataStore,
    project_dir: Option<String>,
) -> Result<PreparedDevDispatch, String> {
    let paths = tauri::async_runtime::spawn_blocking(move || {
        resolve_dispatch_paths(project_dir.as_deref())
    })
    .await
    .map_err(|error| error.to_string())??;

    let issue_summary = fetch_linear_issue_summary(data_store, &paths.issue_id).await?;
    build_prepared_dev_dispatch(paths, issue_summary).await
}

pub fn prepare_dev_dispatch_blocking(
    data_store: DataStore,
    project_dir: Option<String>,
) -> Result<PreparedDevDispatch, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build Tokio runtime for Forge dispatch: {error}"))?;

    runtime.block_on(prepare_dev_dispatch(data_store, project_dir))
}

pub fn verify_dev_dispatch(
    forge: &ForgePlugin,
    project_dir: Option<String>,
) -> Result<ForgeDevDispatchVerificationReport, String> {
    let dispatch = prepare_dev_dispatch_blocking(forge.data_store(), project_dir)?;
    let request = build_dev_task_request(
        dispatch.issue_id.clone(),
        dispatch.worktree_path.clone(),
        "codex".to_string(),
        dispatch.prompt.clone(),
        Vec::new(),
        None,
    )?;

    let task_id = forge
        .create_task(request)
        .map_err(|error| error.to_string())?;
    let task = forge
        .get_task(task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "stored Forge verification task should exist".to_string())?;

    Ok(ForgeDevDispatchVerificationReport {
        dispatch,
        task_id,
        task_status: task.status,
        task_command: task.command,
        task_working_dir: task.working_dir,
        prompt_via_stdin: task.stdin_text.is_some(),
    })
}

async fn build_prepared_agent_dispatch(
    paths: DispatchPaths,
    issue_summary: Option<LinearIssueSummary>,
) -> Result<PreparedAgentDispatch, String> {
    let (issue_status, issue_status_source) = match issue_summary.as_ref() {
        Some(summary) => (summary.issue_status.clone(), "linear".to_string()),
        None => ("Todo".to_string(), "fallback".to_string()),
    };
    let task = build_agent_task_text(&paths.issue_id, issue_summary.as_ref());
    let project_root = paths.project_root.clone();
    let worktree_path_for_prompt = paths.worktree_path.clone();
    let issue_id = paths.issue_id.clone();
    let issue_status_for_prompt = issue_status.clone();
    let prompt = tauri::async_runtime::spawn_blocking(move || {
        generate_agent_prompt(
            &project_root,
            &worktree_path_for_prompt,
            &issue_id,
            &issue_status_for_prompt,
            &task,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(PreparedAgentDispatch {
        dispatch_role: FORGE_AGENT_DISPATCH_ROLE,
        dispatch_tool_name: FORGE_AGENT_DISPATCH_TOOL_NAME.to_string(),
        issue_id: paths.issue_id,
        issue_status,
        issue_status_source,
        issue_title: issue_summary.map(|summary| summary.issue_title),
        project_root: paths.project_root,
        worktree_path: paths.worktree_path,
        prompt_source: ENTRANCE_BOOTSTRAP_PROMPT_SOURCE_LABEL.to_string(),
        prompt,
    })
}

async fn build_prepared_dev_dispatch(
    paths: DispatchPaths,
    issue_summary: Option<LinearIssueSummary>,
) -> Result<PreparedDevDispatch, String> {
    let (issue_status, issue_status_source) = match issue_summary.as_ref() {
        Some(summary) => (summary.issue_status.clone(), "linear".to_string()),
        None => ("Todo".to_string(), "fallback".to_string()),
    };
    let task = build_dev_task_text(&paths.issue_id, issue_summary.as_ref());
    let project_root = paths.project_root.clone();
    let worktree_path_for_prompt = paths.worktree_path.clone();
    let issue_id = paths.issue_id.clone();
    let issue_status_for_prompt = issue_status.clone();
    let prompt = tauri::async_runtime::spawn_blocking(move || {
        generate_dev_prompt(
            &project_root,
            &worktree_path_for_prompt,
            &issue_id,
            &issue_status_for_prompt,
            &task,
        )
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(PreparedDevDispatch {
        dispatch_role: FORGE_DEV_DISPATCH_ROLE,
        dispatch_tool_name: FORGE_DEV_DISPATCH_TOOL_NAME.to_string(),
        issue_id: paths.issue_id,
        issue_status,
        issue_status_source,
        issue_title: issue_summary.map(|summary| summary.issue_title),
        project_root: paths.project_root,
        worktree_path: paths.worktree_path,
        prompt_source: ENTRANCE_BOOTSTRAP_DEV_PROMPT_SOURCE_LABEL.to_string(),
        prompt,
    })
}

pub(crate) fn build_agent_task_request(
    issue_id: String,
    worktree_path: String,
    model: String,
    prompt: String,
    required_tokens: Vec<String>,
    agent_command: Option<String>,
) -> Result<CreateTaskRequest, String> {
    build_dispatch_task_request(
        FORGE_AGENT_DISPATCH_ROLE,
        "agent_dispatch",
        "Agent",
        Some(FORGE_AGENT_DISPATCH_TOOL_NAME),
        issue_id,
        worktree_path,
        model,
        prompt,
        required_tokens,
        agent_command,
    )
}

pub(crate) fn build_dev_task_request(
    issue_id: String,
    worktree_path: String,
    model: String,
    prompt: String,
    required_tokens: Vec<String>,
    agent_command: Option<String>,
) -> Result<CreateTaskRequest, String> {
    build_dispatch_task_request(
        FORGE_DEV_DISPATCH_ROLE,
        "dev_dispatch",
        "Dev",
        Some(FORGE_DEV_DISPATCH_TOOL_NAME),
        issue_id,
        worktree_path,
        model,
        prompt,
        required_tokens,
        agent_command,
    )
}

fn build_dispatch_task_request(
    dispatch_role: ActorRole,
    metadata_kind: &str,
    task_name_prefix: &str,
    dispatch_tool_name: Option<&str>,
    issue_id: String,
    worktree_path: String,
    model: String,
    prompt: String,
    mut required_tokens: Vec<String>,
    agent_command: Option<String>,
) -> Result<CreateTaskRequest, String> {
    let issue_id = issue_id.trim().to_string();
    if issue_id.is_empty() {
        return Err("`issueId` must not be empty".to_string());
    }

    let worktree_path = worktree_path.trim().to_string();
    if worktree_path.is_empty() {
        return Err("`worktreePath` must not be empty".to_string());
    }

    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("`prompt` must not be empty".to_string());
    }

    let raw_model = model.trim().to_string();
    if raw_model.is_empty() {
        return Err("`model` must not be empty".to_string());
    }

    let (runner, model_variant) = split_runner_and_variant(&raw_model);

    let (default_command, args, stdin_text, provider_token) = match runner {
        "codex" | "codex-cli" => {
            let mut args = vec![
                "exec".to_string(),
                "--dangerously-bypass-approvals-and-sandbox".to_string(),
                "--skip-git-repo-check".to_string(),
                "--cd".to_string(),
                worktree_path.clone(),
            ];
            if let Some(model_variant) = model_variant {
                args.push("--model".to_string());
                args.push(model_variant.to_string());
            }
            args.push("-".to_string());

            ("codex".to_string(), args, Some(prompt.clone()), "openai")
        }
        "claude" => {
            let mut args = Vec::new();
            if let Some(model_variant) = model_variant {
                args.push("--model".to_string());
                args.push(model_variant.to_string());
            }
            args.push("-p".to_string());
            args.push(prompt.clone());
            ("claude".to_string(), args, None, "anthropic")
        }
        "gemini" => {
            let mut args = Vec::new();
            if let Some(model_variant) = model_variant {
                args.push("--model".to_string());
                args.push(model_variant.to_string());
            }
            args.push("-p".to_string());
            args.push(prompt.clone());
            ("gemini".to_string(), args, None, "google")
        }
        other => {
            return Err(format!(
                "Unsupported agent model `{other}`. Use `codex`, `claude`, `gemini`, or `runner:model`."
            ));
        }
    };

    // Resolve the stored command into a child entry point the OS can actually spawn.
    let command =
        resolve_dispatch_command_for_runner(runner, agent_command.unwrap_or(default_command));

    push_required_token(&mut required_tokens, provider_token);

    let metadata = serde_json::to_string(&ForgeTaskMetadata {
        kind: Some(metadata_kind.to_string()),
        issue_id: Some(issue_id.clone()),
        worktree_path: Some(worktree_path.clone()),
        model: Some(raw_model.clone()),
        dispatch_role: Some(dispatch_role),
        dispatch_tool_name: dispatch_tool_name.map(str::to_string),
        ..ForgeTaskMetadata::default()
    })
    .map_err(|error| error.to_string())?;

    Ok(CreateTaskRequest {
        name: format!("{task_name_prefix} {issue_id}"),
        command,
        args: serde_json::to_string(&args).map_err(|error| error.to_string())?,
        working_dir: Some(worktree_path),
        stdin_text,
        required_tokens: serde_json::to_string(&required_tokens)
            .map_err(|error| error.to_string())?,
        metadata,
        dispatch_receipt: None,
    })
}

fn resolve_dispatch_command_for_runner(runner: &str, command: String) -> String {
    #[cfg(target_os = "windows")]
    {
        if matches!(runner, "codex" | "codex-cli") {
            if let Some(resolved) =
                resolve_windows_spawnable_command(&command, env::var_os("PATH").as_deref())
            {
                return resolved;
            }
        }
    }

    command
}

#[cfg(target_os = "windows")]
fn resolve_windows_spawnable_command(command: &str, path_env: Option<&OsStr>) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    if dispatch_command_looks_like_path(command) {
        if let Some(resolved) = resolve_windows_spawnable_path(Path::new(command)) {
            return Some(resolved);
        }
        return Some(command.to_string());
    }

    let path_env = path_env?;
    for directory in env::split_paths(path_env) {
        for candidate_name in windows_spawnable_command_candidates(command) {
            let candidate = directory.join(candidate_name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn windows_spawnable_command_candidates(command: &str) -> Vec<String> {
    let path = Path::new(command);
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        if is_windows_spawnable_extension(extension) {
            return vec![command.to_string()];
        }

        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            return windows_spawnable_command_variants(stem);
        }
    }

    windows_spawnable_command_variants(command)
}

#[cfg(target_os = "windows")]
fn windows_spawnable_command_variants(stem: &str) -> Vec<String> {
    vec![
        format!("{stem}.cmd"),
        format!("{stem}.exe"),
        format!("{stem}.bat"),
        format!("{stem}.com"),
        stem.to_string(),
    ]
}

#[cfg(target_os = "windows")]
fn resolve_windows_spawnable_path(path: &Path) -> Option<String> {
    let extension = path.extension().and_then(|value| value.to_str())?;
    if is_windows_spawnable_extension(extension) {
        return Some(path.to_string_lossy().into_owned());
    }

    let stem = path.file_stem().and_then(|value| value.to_str())?;
    for candidate_name in windows_spawnable_command_variants(stem) {
        let candidate = path.with_file_name(candidate_name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn is_windows_spawnable_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "cmd" | "exe" | "bat" | "com"
    )
}

#[cfg(target_os = "windows")]
fn dispatch_command_looks_like_path(command: &str) -> bool {
    Path::new(command).is_absolute()
        || command.starts_with('.')
        || command.contains(std::path::MAIN_SEPARATOR)
        || command.contains('/')
        || command.contains('\\')
}

fn actor_role_slug(role: ActorRole) -> &'static str {
    match role {
        ActorRole::Nota => "nota",
        ActorRole::Arch => "arch",
        ActorRole::Dev => "dev",
        ActorRole::Agent => "agent",
    }
}

fn supervision_strategy_slug(strategy: SupervisionStrategy) -> &'static str {
    match strategy {
        SupervisionStrategy::OneForOne => "one_for_one",
        SupervisionStrategy::RestForOne => "rest_for_one",
        SupervisionStrategy::OneForAll => "one_for_all",
    }
}

fn push_required_token(required_tokens: &mut Vec<String>, token: &str) {
    if required_tokens
        .iter()
        .any(|current| current.eq_ignore_ascii_case(token))
    {
        return;
    }

    required_tokens.push(token.to_string());
}

fn split_runner_and_variant(model: &str) -> (&str, Option<&str>) {
    match model.split_once(':') {
        Some((runner, variant)) if !runner.trim().is_empty() && !variant.trim().is_empty() => {
            (runner.trim(), Some(variant.trim()))
        }
        _ => (model, None),
    }
}

fn managed_worktrees_root_for_app_data_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("worktrees")
}

fn resolve_dispatch_worktree_roots() -> Result<Vec<PathBuf>, String> {
    let app_data_dir = crate::core::resolve_app_data_dir().map_err(|error| error.to_string())?;
    Ok(vec![managed_worktrees_root_for_app_data_dir(&app_data_dir)])
}

pub(crate) fn allocate_agent_slot_worktree(
    base_worktree_path: &str,
    child_slot: &str,
) -> Result<String, String> {
    let base_worktree_path = PathBuf::from(base_worktree_path);
    if !base_worktree_path.exists() {
        return Err(format!(
            "Base worktree `{}` does not exist",
            normalize_display_path(&base_worktree_path)
        ));
    }

    let sanitized_slot = sanitize_child_slot(child_slot)?;
    let base_branch = run_git_command(&base_worktree_path, ["branch", "--show-current"])?;
    let issue_id = parse_issue_id_from_branch(&base_branch)?;
    let repo_root = resolve_repo_root_from_worktree(&base_worktree_path)?;
    let child_worktree_path = base_worktree_path
        .parent()
        .ok_or_else(|| {
            format!(
                "Unable to derive managed project directory from `{}`",
                normalize_display_path(&base_worktree_path)
            )
        })?
        .join("slots")
        .join(&issue_id)
        .join(&sanitized_slot);

    if run_git_command(&child_worktree_path, ["rev-parse", "--show-toplevel"]).is_ok() {
        return Ok(normalize_display_path(&child_worktree_path));
    }

    if child_worktree_path.exists() {
        return Err(format!(
            "Slot worktree path `{}` already exists but is not a valid git worktree",
            normalize_display_path(&child_worktree_path)
        ));
    }

    let child_parent = child_worktree_path.parent().ok_or_else(|| {
        format!(
            "Unable to derive parent directory for slot worktree `{}`",
            normalize_display_path(&child_worktree_path)
        )
    })?;
    std::fs::create_dir_all(child_parent).map_err(|error| {
        format!(
            "Failed to create slot worktree parent `{}`: {error}",
            normalize_display_path(child_parent)
        )
    })?;

    let child_branch = format!("slot/{sanitized_slot}/feat-{issue_id}");
    let add_output = if git_branch_exists(&repo_root, &child_branch)? {
        Command::new("git")
            .args([
                "worktree",
                "add",
                "--quiet",
                child_worktree_path
                    .to_str()
                    .ok_or_else(|| "slot worktree path is not valid UTF-8".to_string())?,
                child_branch.as_str(),
            ])
            .current_dir(&repo_root)
            .output()
            .map_err(|error| error.to_string())?
    } else {
        Command::new("git")
            .args([
                "worktree",
                "add",
                "--quiet",
                "-b",
                child_branch.as_str(),
                child_worktree_path
                    .to_str()
                    .ok_or_else(|| "slot worktree path is not valid UTF-8".to_string())?,
                base_branch.as_str(),
            ])
            .current_dir(&repo_root)
            .output()
            .map_err(|error| error.to_string())?
    };

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr)
            .trim()
            .to_string();
        return Err(if stderr.is_empty() {
            format!(
                "Failed to allocate slot worktree `{}` from `{}`",
                normalize_display_path(&child_worktree_path),
                normalize_display_path(&repo_root)
            )
        } else {
            format!(
                "Failed to allocate slot worktree `{}` from `{}`: {stderr}",
                normalize_display_path(&child_worktree_path),
                normalize_display_path(&repo_root)
            )
        });
    }

    Ok(normalize_display_path(&child_worktree_path))
}

fn resolve_dispatch_paths(project_dir: Option<&str>) -> Result<DispatchPaths, String> {
    if let Some(project_dir) = project_dir {
        let worktree_roots = resolve_dispatch_worktree_roots()?;
        return resolve_dispatch_paths_for_project(project_dir, &worktree_roots);
    }

    // Fallback: detect from CWD
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    let worktree_root = run_git_command(&cwd, ["rev-parse", "--show-toplevel"])?;
    let git_common_dir = run_git_command(&cwd, ["rev-parse", "--git-common-dir"])?;
    let branch = run_git_command(&cwd, ["branch", "--show-current"])?;
    let worktree_path = normalize_command_path(&cwd, &worktree_root);
    let common_dir = normalize_command_path(&cwd, &git_common_dir);
    let project_root = common_dir.parent().ok_or_else(|| {
        format!(
            "Unable to resolve project root from `{}`",
            common_dir.display()
        )
    })?;
    let issue_id = parse_issue_id_from_branch(&branch)?;

    Ok(DispatchPaths {
        issue_id,
        project_root: project_root.to_string_lossy().replace('\\', "/"),
        worktree_path: worktree_path.to_string_lossy().replace('\\', "/"),
    })
}

fn resolve_dispatch_paths_for_explicit_worktree(
    project_dir: Option<&str>,
    worktree_path: &str,
    worktree_roots: &[PathBuf],
) -> Result<DispatchPaths, String> {
    let requested_worktree_path = PathBuf::from(worktree_path);
    if !requested_worktree_path.exists() {
        return Err(format!(
            "Explicit worktree `{}` does not exist",
            worktree_path
        ));
    }

    let resolved_worktree = normalize_command_path(
        &requested_worktree_path,
        &run_git_command(&requested_worktree_path, ["rev-parse", "--show-toplevel"])?,
    );
    let derived_project_root = resolve_repo_root_from_worktree(&resolved_worktree)?;
    let branch = run_git_command(&resolved_worktree, ["branch", "--show-current"])?;
    let issue_id = parse_issue_id_from_branch(&branch)?;

    let project_root = if let Some(project_dir) = project_dir {
        let explicit_project_root = PathBuf::from(project_dir);
        if !explicit_project_root.exists() {
            return Err(format!(
                "Project directory `{}` does not exist",
                project_dir
            ));
        }

        let explicit_project_root = canonicalize_for_compare(&explicit_project_root)?;
        let derived_project_root = canonicalize_for_compare(&derived_project_root)?;
        if explicit_project_root != derived_project_root {
            return Err(format!(
                "Explicit project directory `{}` does not match worktree repository `{}`",
                normalize_display_path(&explicit_project_root),
                normalize_display_path(&derived_project_root)
            ));
        }

        explicit_project_root
    } else {
        derived_project_root
    };

    let project_name = project_root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let allowed_root = worktree_roots
        .iter()
        .map(|root| root.join(&project_name))
        .find(|candidate| resolved_worktree.starts_with(candidate))
        .ok_or_else(|| {
            let expected_roots = worktree_roots
                .iter()
                .map(|root| format!("`{}`", root.join(&project_name).display()))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Explicit worktree `{}` is outside the managed Forge roots for project `{}`. Expected under {}.",
                normalize_display_path(&resolved_worktree),
                project_name,
                expected_roots
            )
        })?;
    let _ = allowed_root;

    Ok(DispatchPaths {
        issue_id,
        project_root: normalize_display_path(&project_root),
        worktree_path: normalize_display_path(&resolved_worktree),
    })
}

fn resolve_dispatch_paths_for_project(
    project_dir: &str,
    worktree_roots: &[PathBuf],
) -> Result<DispatchPaths, String> {
    let project_root = PathBuf::from(project_dir);
    if !project_root.exists() {
        return Err(format!(
            "Project directory `{}` does not exist",
            project_dir
        ));
    }

    let project_name = project_root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    if let Some(worktree_path) = find_project_worktree(&project_name, worktree_roots)? {
        let issue_id = parse_issue_id_from_branch(
            &worktree_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
        )?;

        return Ok(DispatchPaths {
            issue_id,
            project_root: project_dir.replace('\\', "/"),
            worktree_path: worktree_path.to_string_lossy().replace('\\', "/"),
        });
    }

    Err(format_missing_worktree_error(&project_name, worktree_roots))
}

fn find_project_worktree(
    project_name: &str,
    worktree_roots: &[PathBuf],
) -> Result<Option<PathBuf>, String> {
    for worktree_root in worktree_roots {
        let project_worktrees_dir = worktree_root.join(project_name);
        if !project_worktrees_dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&project_worktrees_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("feat-") && entry.path().is_dir() {
                    let worktree_path = entry.path();
                    if run_git_command(&worktree_path, ["rev-parse", "--show-toplevel"]).is_ok() {
                        return Ok(Some(worktree_path));
                    }
                }
            }
        }
    }

    Ok(None)
}

fn format_missing_worktree_error(project_name: &str, worktree_roots: &[PathBuf]) -> String {
    let searched_roots = worktree_roots
        .iter()
        .map(|root| format!("`{}`", root.join(project_name).display()))
        .collect::<Vec<_>>()
        .join(", ");
    let managed_root = worktree_roots
        .first()
        .map(|root| root.join(project_name))
        .unwrap_or_else(|| PathBuf::from(project_name));

    format!(
        "No active worktree found for project `{project_name}`. Forge looked under {searched_roots}. Create a `feat-<ISSUE>` worktree under `{}` and try again.",
        managed_root.display()
    )
}

fn run_git_command<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let command = format!("git {}", args.join(" "));
        return Err(if stderr.is_empty() {
            format!("`{command}` failed")
        } else {
            format!("`{command}` failed: {stderr}")
        });
    }

    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| error.to_string())
}

fn git_branch_exists(repo_root: &Path, branch_name: &str) -> Result<bool, String> {
    let output = Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ])
        .current_dir(repo_root)
        .output()
        .map_err(|error| error.to_string())?;

    Ok(output.status.success())
}

fn resolve_repo_root_from_worktree(worktree_path: &Path) -> Result<PathBuf, String> {
    let git_common_dir = run_git_command(worktree_path, ["rev-parse", "--git-common-dir"])?;
    let common_dir = normalize_command_path(worktree_path, &git_common_dir);
    common_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "Unable to resolve repository root from git common dir `{}`",
            normalize_display_path(&common_dir)
        )
    })
}

fn canonicalize_for_compare(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|error| {
        format!(
            "Failed to canonicalize `{}`: {error}",
            normalize_display_path(path)
        )
    })
}

fn normalize_command_path(cwd: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw.trim());
    if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    }
}

fn normalize_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_issue_id_from_branch(branch: &str) -> Result<String, String> {
    let candidate = branch.trim().rsplit('/').next().unwrap_or(branch.trim());
    match candidate.strip_prefix("feat-") {
        Some(issue_id) if !issue_id.trim().is_empty() => Ok(issue_id.trim().to_string()),
        _ => Err(format!(
            "Current branch `{}` is not an issue worktree branch. Open Entrance from a `feat-<ISSUE>` worktree to use auto-dispatch.",
            branch.trim()
        )),
    }
}

fn sanitize_child_slot(child_slot: &str) -> Result<String, String> {
    let trimmed = child_slot.trim();
    if trimmed.is_empty() {
        return Err("child slot must not be empty".to_string());
    }

    if trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(format!(
            "child slot `{trimmed}` contains unsupported characters; use only ASCII letters, digits, `-`, or `_`"
        ))
    }
}

async fn fetch_linear_issue_summary(
    data_store: DataStore,
    issue_id: &str,
) -> Result<Option<LinearIssueSummary>, String> {
    let Some(token) = resolve_linear_token(&data_store)? else {
        return Ok(None);
    };

    let response = reqwest::Client::new()
        .post("https://api.linear.app/graphql")
        .header("Authorization", token)
        .json(&json!({
            "query": "query AutoDispatchIssue($identifier: String!) { issues(filter: { identifier: { eq: $identifier } }, first: 1) { nodes { title state { name } } } }",
            "variables": {
                "identifier": issue_id,
            },
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let payload = response
        .json::<LinearIssueEnvelope>()
        .await
        .map_err(|error| error.to_string())?;

    if let Some(errors) = payload.errors {
        let summary = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("Linear issue lookup failed: {summary}"));
    }

    let issue = payload
        .data
        .and_then(|data| data.issues.nodes.into_iter().next())
        .map(|issue| LinearIssueSummary {
            issue_status: issue.state.name,
            issue_title: issue.title,
        });

    Ok(issue)
}

fn resolve_linear_token(data_store: &DataStore) -> Result<Option<String>, String> {
    for key in ["LINEAR_API_KEY", "LINEAR_TOKEN"] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }
    }

    let Some(token) = data_store
        .get_vault_token_by_provider("linear")
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    let cipher = VaultCipher::from_device().map_err(|error| error.to_string())?;
    let value = cipher
        .decrypt(&token.encrypted_value)
        .map_err(|error| error.to_string())?;
    let trimmed = value.trim();

    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn build_agent_task_text(issue_id: &str, issue_summary: Option<&LinearIssueSummary>) -> String {
    match issue_summary {
        Some(summary) if summary.issue_status.eq_ignore_ascii_case("Request") => format!(
            "按 Dev 审核意见返工 Linear issue {issue_id}: {}",
            summary.issue_title
        ),
        Some(summary) => format!("完成 Linear issue {issue_id}: {}", summary.issue_title),
        None => {
            format!("完成 Linear issue {issue_id}，以 issue description、验收标准和最新评论为准")
        }
    }
}

fn build_dev_task_text(issue_id: &str, issue_summary: Option<&LinearIssueSummary>) -> String {
    match issue_summary {
        Some(summary) if summary.issue_status.eq_ignore_ascii_case("In Review") => format!(
            "以 Dev 身份审核并整合 Linear issue {issue_id}: {}",
            summary.issue_title
        ),
        Some(summary) if summary.issue_status.eq_ignore_ascii_case("Request") => format!(
            "以 Dev 身份处理返工并重新派发 Linear issue {issue_id}: {}",
            summary.issue_title
        ),
        Some(summary) => format!(
            "以 Dev 身份为 Linear issue {issue_id} 执行 prepare / dispatch: {}",
            summary.issue_title
        ),
        None => {
            format!("以 Dev 身份为 Linear issue {issue_id} 执行 prepare / dispatch，以 issue description、验收标准和最新评论为准")
        }
    }
}

fn generate_agent_prompt(
    project_root: &str,
    worktree_path: &str,
    issue_id: &str,
    issue_status: &str,
    task: &str,
) -> Result<String, String> {
    let project_root = PathBuf::from(project_root);
    let worktree_path = PathBuf::from(worktree_path);

    if !worktree_path.exists() {
        return Err(format!(
            "Resolved worktree `{}` does not exist",
            normalize_display_path(&worktree_path)
        ));
    }

    let bootstrap_skill_path = project_root.join(ENTRANCE_BOOTSTRAP_SKILL_RELATIVE_PATH);
    if !bootstrap_skill_path.exists() {
        return Err(format!(
            "Entrance bootstrap skill file `{}` does not exist",
            normalize_display_path(&bootstrap_skill_path)
        ));
    }

    let project_root = normalize_display_path(&project_root);
    let worktree_path = normalize_display_path(&worktree_path);
    let bootstrap_skill_path = normalize_display_path(&bootstrap_skill_path);

    Ok(format!(
        "读 `{bootstrap_skill_path}`，以 Agent 身份启动。\n从 Linear 获取 `{issue_id}`（当前状态: `{issue_status}`）。\n**只在 worktree `{worktree_path}` 中工作。**\n\n项目根目录 `{project_root}`\nSpecs 目录: `{project_root}/specs/`\n\n第一动作，在写任何代码前必须完成:\n1. 调用 Linear MCP，把 `{issue_id}` 标记为 `In Progress`\n2. 在 issue comment 留言:\n   `> Agent ({{当前模型名}}) 已领取，开始工作`\n\n这是硬约束:\n- 所有文件修改都必须发生在 `{worktree_path}`\n- 禁止在主目录 `{project_root}` 里执行 `git checkout` / `git switch`\n- 禁止在主目录 `{project_root}` 新增或修改业务文件\n- commit 只允许发生在 worktree 内\n任务:\n{task}\n\n参考:\n- (none specified)\n\n完成后在 Linear issue comment 中汇报结果。"
    ))
}

fn generate_dev_prompt(
    project_root: &str,
    worktree_path: &str,
    issue_id: &str,
    issue_status: &str,
    task: &str,
) -> Result<String, String> {
    let project_root = PathBuf::from(project_root);
    let worktree_path = PathBuf::from(worktree_path);

    if !worktree_path.exists() {
        return Err(format!(
            "Resolved worktree `{}` does not exist",
            normalize_display_path(&worktree_path)
        ));
    }

    let bootstrap_skill_path = project_root.join(ENTRANCE_BOOTSTRAP_SKILL_RELATIVE_PATH);
    if !bootstrap_skill_path.exists() {
        return Err(format!(
            "Entrance bootstrap skill file `{}` does not exist",
            normalize_display_path(&bootstrap_skill_path)
        ));
    }

    let dev_role_path = project_root.join(ENTRANCE_BOOTSTRAP_DEV_ROLE_RELATIVE_PATH);
    if !dev_role_path.exists() {
        return Err(format!(
            "Entrance bootstrap dev role file `{}` does not exist",
            normalize_display_path(&dev_role_path)
        ));
    }

    let project_root = normalize_display_path(&project_root);
    let worktree_path = normalize_display_path(&worktree_path);
    let bootstrap_skill_path = normalize_display_path(&bootstrap_skill_path);
    let dev_role_path = normalize_display_path(&dev_role_path);

    Ok(format!(
        "读 `{bootstrap_skill_path}` 和 `{dev_role_path}`，以 Dev 身份启动。\n从 Linear 获取 `{issue_id}`（当前状态: `{issue_status}`）。\n**只在 worktree `{worktree_path}` 中工作。**\n\n项目根目录 `{project_root}`\nSpecs 目录: `{project_root}/specs/`\n\n这是当前 dev dispatch cut 的边界:\n- 这次只落 `prepare / dispatch` 启动面，不把它当成完整的 Dev 状态机\n- 如需派发 Agent，优先使用 Entrance-owned Forge runtime，不手写 agent prompt\n- 所有文件修改都必须发生在 `{worktree_path}`\n- 禁止在主目录 `{project_root}` 里执行 `git checkout` / `git switch`\n- 禁止在主目录 `{project_root}` 新增或修改业务文件\n- commit 只允许发生在 worktree 内\n\n当前任务:\n{task}\n\n参考:\n- (none specified)\n\n完成后在 Linear issue comment 中汇报 Dev 侧结果。"
    ))
}

impl Plugin for ForgePlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn init(&self, _ctx: &AppContext) -> Result<()> {
        Ok(())
    }

    fn on_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }

    fn register_commands(&self) -> Vec<TauriCommandDefinition> {
        vec![
            TauriCommandDefinition {
                name: "forge_create_task",
                description: "Create a new agent task",
            },
            TauriCommandDefinition {
                name: "forge_dispatch_agent",
                description: "Launch an Agent task from structured issue metadata",
            },
            TauriCommandDefinition {
                name: "forge_prepare_agent_dispatch",
                description: "Prepare a one-click Agent dispatch from the current worktree context",
            },
            TauriCommandDefinition {
                name: "forge_list_tasks",
                description: "List all agent tasks",
            },
            TauriCommandDefinition {
                name: "forge_get_task",
                description: "Get details and status of a specific task",
            },
            TauriCommandDefinition {
                name: "forge_get_task_details",
                description: "Get a forge task together with stored logs",
            },
            TauriCommandDefinition {
                name: "forge_cancel_task",
                description: "Cancel a running task",
            },
        ]
    }

    fn mcp_tools(&self) -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "forge.create_task",
                description: "Create a new forge task",
            },
            McpToolDefinition {
                name: "forge.run_agent",
                description: "Launch an Agent task from issue, worktree and prompt",
            },
            McpToolDefinition {
                name: "forge.run_dev",
                description: "Launch a Dev task from issue, worktree and prompt",
            },
            McpToolDefinition {
                name: "forge.prepare_agent_dispatch",
                description: "Prepare an Entrance-owned agent-lane dispatch from the current or explicit managed worktree context",
            },
            McpToolDefinition {
                name: "forge.verify_agent_dispatch",
                description: "Prepare and persist a Pending agent-lane Forge dispatch without starting agent execution",
            },
            McpToolDefinition {
                name: "forge.prepare_dev_dispatch",
                description: "Prepare an Entrance-owned dev-lane dispatch from the current worktree context",
            },
            McpToolDefinition {
                name: "forge.verify_dev_dispatch",
                description: "Prepare and persist a Pending dev-lane Forge dispatch without starting execution",
            },
            McpToolDefinition {
                name: "forge.list_tasks",
                description: "List all forge tasks",
            },
            McpToolDefinition {
                name: "forge.get_task",
                description: "Get a forge task by ID",
            },
            McpToolDefinition {
                name: "forge.get_task_details",
                description: "Get a forge task and its stored logs",
            },
            McpToolDefinition {
                name: "forge.cancel_task",
                description: "Cancel a running forge task",
            },
        ]
    }

    fn shutdown(&self) -> Result<()> {
        if let Ok(mut server) = self.http_server.lock() {
            if let Some(handle) = server.take() {
                handle.abort();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::{OsStr, OsString},
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::{
        core::{
            action::ActorRole,
            bootstrap_for_paths,
            config_store::{render_config, EntranceConfig},
            data_store::MigrationPlan,
            event_bus::EventBus,
            AppPaths,
        },
        plugins::vault,
    };

    use super::{
        allocate_agent_slot_worktree, build_agent_task_request, build_dev_task_request,
        build_prepared_agent_dispatch, build_prepared_dev_dispatch, generate_agent_prompt,
        generate_dev_prompt, managed_worktrees_root_for_app_data_dir, normalize_display_path,
        parse_issue_id_from_branch, prepare_agent_dispatch, prepare_agent_dispatch_for_worktree,
        prepare_dev_dispatch, resolve_dispatch_paths_for_project, ForgePlugin, ForgeTaskMetadata,
    };

    static FORGE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TestDir {
        path: PathBuf,
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "entrance-forge-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test temp directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
            Self { key, original }
        }

        fn remove(key: &'static str) -> Self {
            let original = env::var_os(key);
            env::remove_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    fn init_git_repo(path: &Path) {
        let output = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(path)
            .output()
            .expect("git init should run");
        assert!(
            output.status.success(),
            "git init should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_git_repo_with_commit(path: &Path) {
        init_git_repo(path);

        let add = Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .expect("git add should run");
        assert!(
            add.status.success(),
            "git add should succeed: {}",
            String::from_utf8_lossy(&add.stderr)
        );

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
            .expect("git commit should run");
        assert!(
            commit.status.success(),
            "git commit should succeed: {}",
            String::from_utf8_lossy(&commit.stderr)
        );
    }

    fn add_git_worktree(repo_root: &Path, worktree_path: &Path, branch: &str) {
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "--quiet",
                "-b",
                branch,
                worktree_path
                    .to_str()
                    .expect("worktree path should be valid UTF-8"),
            ])
            .current_dir(repo_root)
            .output()
            .expect("git worktree add should run");
        assert!(
            output.status.success(),
            "git worktree add should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn forge_test_guard() -> std::sync::MutexGuard<'static, ()> {
        FORGE_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("forge test lock should not be poisoned")
    }

    fn assert_default_codex_command(command: &str) {
        #[cfg(target_os = "windows")]
        {
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
        }

        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(command, "codex");
        }
    }

    #[test]
    fn codex_agent_requests_are_translated_into_cli_tasks() {
        let request = build_agent_task_request(
            "MYT-48".to_string(),
            "C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-48".to_string(),
            "codex:gpt-5-codex".to_string(),
            "implement the task".to_string(),
            vec!["openai".to_string()],
            None,
        )
        .expect("agent request should be valid");

        assert_default_codex_command(&request.command);
        assert_eq!(
            request.working_dir.as_deref(),
            Some("C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-48")
        );
        assert_eq!(request.stdin_text.as_deref(), Some("implement the task"));
        assert!(request.args.contains("\"exec\""));
        assert!(request.args.contains("\"--model\""));
        assert!(request.args.contains("\"gpt-5-codex\""));
        assert!(request.required_tokens.contains("openai"));
        assert!(!request.required_tokens.contains("linear"));
        let metadata: ForgeTaskMetadata =
            serde_json::from_str(&request.metadata).expect("request metadata should be valid JSON");
        assert_eq!(metadata.kind.as_deref(), Some("agent_dispatch"));
        assert_eq!(metadata.issue_id.as_deref(), Some("MYT-48"));
        assert_eq!(metadata.dispatch_role, Some(ActorRole::Agent));
        assert_eq!(
            metadata.dispatch_tool_name.as_deref(),
            Some("forge_dispatch_agent")
        );
    }

    #[test]
    fn codex_dev_requests_are_translated_into_cli_tasks() {
        let request = build_dev_task_request(
            "MYT-48".to_string(),
            "C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-48".to_string(),
            "codex:gpt-5-codex".to_string(),
            "manage the issue".to_string(),
            vec!["openai".to_string()],
            None,
        )
        .expect("dev request should be valid");

        assert_default_codex_command(&request.command);
        assert_eq!(
            request.working_dir.as_deref(),
            Some("C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-48")
        );
        assert_eq!(request.stdin_text.as_deref(), Some("manage the issue"));
        assert!(request.args.contains("\"exec\""));
        assert!(request.args.contains("\"--model\""));
        assert!(request.args.contains("\"gpt-5-codex\""));
        let metadata: ForgeTaskMetadata =
            serde_json::from_str(&request.metadata).expect("request metadata should be valid JSON");
        assert_eq!(metadata.kind.as_deref(), Some("dev_dispatch"));
        assert_eq!(metadata.issue_id.as_deref(), Some("MYT-48"));
        assert_eq!(metadata.dispatch_role, Some(ActorRole::Dev));
        assert_eq!(
            metadata.dispatch_tool_name.as_deref(),
            Some("forge_dispatch_dev")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn codex_dev_requests_resolve_windows_spawnable_runner_entries() {
        let _guard = forge_test_guard();

        let temp_dir = TestDir::new("codex-runner-resolution");
        let codex_cmd = temp_dir.path().join("codex.cmd");
        let codex_ps1 = temp_dir.path().join("codex.ps1");
        fs::write(&codex_cmd, "@echo off\r\n").expect("fake codex.cmd should be written");
        fs::write(&codex_ps1, "Write-Output 'codex'\r\n")
            .expect("fake codex.ps1 should be written");
        let _path_guard = EnvVarGuard::set("PATH", temp_dir.path().as_os_str());

        let request = build_dev_task_request(
            "MYT-48".to_string(),
            "C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-48".to_string(),
            "codex:gpt-5-codex".to_string(),
            "manage the issue".to_string(),
            Vec::new(),
            None,
        )
        .expect("dev request should resolve the default codex runner");

        assert_eq!(
            normalize_display_path(Path::new(&request.command)),
            normalize_display_path(&codex_cmd)
        );

        let overridden_request = build_dev_task_request(
            "MYT-48".to_string(),
            "C:/Users/test/AppData/Local/Entrance/worktrees/Entrance/feat-MYT-48".to_string(),
            "codex:gpt-5-codex".to_string(),
            "manage the issue".to_string(),
            Vec::new(),
            Some(codex_ps1.to_string_lossy().into_owned()),
        )
        .expect("dev request should resolve a PowerShell codex shim to a spawnable sibling");

        assert_eq!(
            normalize_display_path(Path::new(&overridden_request.command)),
            normalize_display_path(&codex_cmd)
        );
    }

    #[test]
    fn issue_ids_are_parsed_from_feature_branches() {
        assert_eq!(
            parse_issue_id_from_branch("feat-MYT-48").expect("feature branch should parse"),
            "MYT-48"
        );
        assert_eq!(
            parse_issue_id_from_branch("codex/feat-MYT-99")
                .expect("scoped feature branch should parse"),
            "MYT-99"
        );
    }

    #[test]
    fn managed_worktree_root_is_derived_from_app_data_dir() {
        let app_data_dir = PathBuf::from("C:/Users/test/AppData/Local/Entrance");
        assert_eq!(
            managed_worktrees_root_for_app_data_dir(&app_data_dir),
            app_data_dir.join("worktrees")
        );
    }

    #[test]
    fn project_dispatch_uses_managed_worktree_root() {
        let temp_dir = TestDir::new("dispatch-managed");
        let project_root = temp_dir.path().join("Entrance");
        fs::create_dir_all(&project_root).expect("project root should exist");

        let managed_root = temp_dir.path().join("appdata").join("worktrees");
        let managed_worktree = managed_root.join("Entrance").join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree).expect("managed worktree should exist");
        init_git_repo(&managed_worktree);

        let paths = resolve_dispatch_paths_for_project(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            &[managed_root.clone()],
        )
        .expect("managed worktree should be used");

        assert_eq!(paths.issue_id, "MYT-48");
        assert_eq!(
            paths.project_root,
            project_root.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(
            paths.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn slot_worktree_is_allocated_under_managed_project_slots_root() {
        let temp_dir = TestDir::new("dispatch-slot-worktree");
        let project_root = temp_dir.path().join("Entrance");
        fs::create_dir_all(&project_root).expect("project root should exist");
        fs::write(project_root.join("README.md"), "slot worktree test\n")
            .expect("repo file should be written");
        init_git_repo_with_commit(&project_root);

        let managed_root = temp_dir.path().join("appdata").join("worktrees");
        let managed_worktree = managed_root.join("Entrance").join("feat-MYT-48");
        fs::create_dir_all(
            managed_worktree
                .parent()
                .expect("managed worktree parent should exist"),
        )
        .expect("managed worktree parent should be created");
        add_git_worktree(&project_root, &managed_worktree, "feat-MYT-48");

        let slot_worktree = allocate_agent_slot_worktree(
            managed_worktree
                .to_str()
                .expect("managed worktree path should be valid UTF-8"),
            "agent-1",
        )
        .expect("slot worktree should be allocated");

        let slot_worktree = PathBuf::from(&slot_worktree);
        assert_eq!(
            slot_worktree,
            managed_root
                .join("Entrance")
                .join("slots")
                .join("MYT-48")
                .join("agent-1")
        );

        let top_level = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&slot_worktree)
            .output()
            .expect("git rev-parse should run for slot worktree");
        assert!(top_level.status.success());
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&slot_worktree)
            .output()
            .expect("git branch should run for slot worktree");
        assert!(branch.status.success());
        assert_eq!(
            String::from_utf8_lossy(&branch.stdout).trim(),
            "slot/agent-1/feat-MYT-48"
        );
    }

    #[test]
    fn missing_project_worktree_error_points_to_managed_root() {
        let temp_dir = TestDir::new("dispatch-missing");
        let project_root = temp_dir.path().join("Entrance");
        fs::create_dir_all(&project_root).expect("project root should exist");

        let managed_root = temp_dir.path().join("appdata").join("worktrees");

        let error = resolve_dispatch_paths_for_project(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            &[managed_root.clone()],
        )
        .expect_err("missing worktree should return an error");

        assert!(error.contains(&managed_root.join("Entrance").display().to_string()));
        assert!(!error.contains("control.py"));
        assert!(!error.contains("legacy-agents"));
    }

    #[test]
    fn generated_prompt_uses_entrance_bootstrap_skill() {
        let temp_dir = TestDir::new("dispatch-prompt");
        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill).expect("bootstrap skill directory should exist");
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")
            .expect("bootstrap skill should be written");

        let worktree_path = temp_dir
            .path()
            .join("appdata")
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&worktree_path).expect("worktree path should exist");

        let prompt = generate_agent_prompt(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            worktree_path
                .to_str()
                .expect("worktree path should be valid UTF-8"),
            "MYT-48",
            "Todo",
            "implement the task",
        )
        .expect("prompt should be generated");

        assert!(prompt.contains("harness/bootstrap/duet/SKILL.md"));
        assert!(prompt.contains(&worktree_path.to_string_lossy().replace('\\', "/")));
        assert!(!prompt.contains(".agents/nota/scripts/control.py"));
    }

    #[test]
    fn generated_dev_prompt_uses_entrance_bootstrap_dev_role() {
        let temp_dir = TestDir::new("dispatch-dev-prompt");
        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        let dev_role = bootstrap_skill.join("roles");
        fs::create_dir_all(&dev_role).expect("bootstrap dev role directory should exist");
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")
            .expect("bootstrap skill should be written");
        fs::write(dev_role.join("dev.md"), "# test dev role\n")
            .expect("bootstrap dev role should be written");

        let worktree_path = temp_dir
            .path()
            .join("appdata")
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&worktree_path).expect("worktree path should exist");

        let prompt = generate_dev_prompt(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            worktree_path
                .to_str()
                .expect("worktree path should be valid UTF-8"),
            "MYT-48",
            "Todo",
            "manage the issue",
        )
        .expect("prompt should be generated");

        assert!(prompt.contains("harness/bootstrap/duet/SKILL.md"));
        assert!(prompt.contains("harness/bootstrap/duet/roles/dev.md"));
        assert!(prompt.contains("以 Dev 身份启动"));
        assert!(!prompt.contains(".agents"));
    }

    #[test]
    fn prompt_generation_requires_repo_bootstrap_skill() {
        let temp_dir = TestDir::new("dispatch-prompt-missing-skill");
        let project_root = temp_dir.path().join("Entrance");
        fs::create_dir_all(&project_root).expect("project root should exist");

        let worktree_path = temp_dir
            .path()
            .join("appdata")
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&worktree_path).expect("worktree path should exist");

        let error = generate_agent_prompt(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            worktree_path
                .to_str()
                .expect("worktree path should be valid UTF-8"),
            "MYT-48",
            "Todo",
            "implement the task",
        )
        .expect_err("missing bootstrap skill should fail");

        assert!(error.contains("harness/bootstrap/duet/SKILL.md"));
    }

    #[test]
    fn prepare_dispatch_pipeline_builds_without_agents_runtime() -> Result<()> {
        let _guard = forge_test_guard();

        let temp_dir = TestDir::new("dispatch-pipeline-no-agents");
        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_root = temp_dir.path().join("appdata").join("worktrees");
        let managed_worktree = managed_root.join("Entrance").join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let paths = resolve_dispatch_paths_for_project(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            &[managed_root.clone()],
        )
        .expect("managed worktree should resolve");

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let dispatch = runtime
            .block_on(async { build_prepared_agent_dispatch(paths, None).await })
            .map_err(anyhow::Error::msg)?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.dispatch_role, ActorRole::Agent);
        assert_eq!(dispatch.dispatch_tool_name, "forge_dispatch_agent");
        assert_eq!(dispatch.issue_status, "Todo");
        assert_eq!(dispatch.issue_status_source, "fallback");
        assert!(dispatch.issue_title.is_none());
        assert_eq!(
            dispatch.prompt_source,
            "Entrance-owned harness/bootstrap prompt"
        );
        assert_eq!(
            dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(dispatch.prompt.contains("harness/bootstrap/duet/SKILL.md"));
        assert!(!dispatch.prompt.contains(".agents"));

        let request = build_agent_task_request(
            dispatch.issue_id.clone(),
            dispatch.worktree_path.clone(),
            "codex:gpt-5-codex".to_string(),
            dispatch.prompt.clone(),
            Vec::new(),
            None,
        )
        .expect("dispatch payload should translate into an agent task");

        assert_eq!(
            request.working_dir.as_deref(),
            Some(dispatch.worktree_path.as_str())
        );
        assert_eq!(
            request.stdin_text.as_deref(),
            Some(dispatch.prompt.as_str())
        );
        assert!(request.args.contains(&dispatch.worktree_path));
        assert!(!request.args.contains(".agents"));
        let metadata: ForgeTaskMetadata =
            serde_json::from_str(&request.metadata).expect("request metadata should be valid JSON");
        assert_eq!(metadata.dispatch_role, Some(ActorRole::Agent));
        assert_eq!(
            metadata.dispatch_tool_name.as_deref(),
            Some("forge_dispatch_agent")
        );

        let store =
            crate::core::data_store::DataStore::in_memory(MigrationPlan::new(vault::migrations()))?;
        assert!(
            store.get_vault_token_by_provider("linear")?.is_none(),
            "test store should not require a legacy `.agents` token source"
        );

        Ok(())
    }

    #[test]
    fn prepare_dev_dispatch_pipeline_builds_without_agents_runtime() -> Result<()> {
        let _guard = forge_test_guard();

        let temp_dir = TestDir::new("dispatch-dev-pipeline-no-agents");
        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        let dev_role = bootstrap_skill.join("roles");
        fs::create_dir_all(&dev_role)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
        fs::write(dev_role.join("dev.md"), "# test dev role\n")?;

        let managed_root = temp_dir.path().join("appdata").join("worktrees");
        let managed_worktree = managed_root.join("Entrance").join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let paths = resolve_dispatch_paths_for_project(
            project_root
                .to_str()
                .expect("project path should be valid UTF-8"),
            &[managed_root.clone()],
        )
        .expect("managed worktree should resolve");

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let dispatch = runtime
            .block_on(async { build_prepared_dev_dispatch(paths, None).await })
            .map_err(anyhow::Error::msg)?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.dispatch_role, ActorRole::Dev);
        assert_eq!(dispatch.dispatch_tool_name, "forge_dispatch_dev");
        assert_eq!(dispatch.issue_status, "Todo");
        assert_eq!(dispatch.issue_status_source, "fallback");
        assert!(dispatch.issue_title.is_none());
        assert_eq!(
            dispatch.prompt_source,
            "Entrance-owned harness/bootstrap dev prompt"
        );
        assert_eq!(
            dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(dispatch.prompt.contains("harness/bootstrap/duet/SKILL.md"));
        assert!(dispatch
            .prompt
            .contains("harness/bootstrap/duet/roles/dev.md"));
        assert!(!dispatch.prompt.contains(".agents"));

        let request = build_dev_task_request(
            dispatch.issue_id.clone(),
            dispatch.worktree_path.clone(),
            "codex:gpt-5-codex".to_string(),
            dispatch.prompt.clone(),
            Vec::new(),
            None,
        )
        .expect("dispatch payload should translate into a dev task");

        assert_eq!(
            request.working_dir.as_deref(),
            Some(dispatch.worktree_path.as_str())
        );
        assert_eq!(
            request.stdin_text.as_deref(),
            Some(dispatch.prompt.as_str())
        );
        let metadata: ForgeTaskMetadata =
            serde_json::from_str(&request.metadata).expect("request metadata should be valid JSON");
        assert_eq!(metadata.dispatch_role, Some(ActorRole::Dev));
        assert_eq!(metadata.kind.as_deref(), Some("dev_dispatch"));
        assert_eq!(
            metadata.dispatch_tool_name.as_deref(),
            Some("forge_dispatch_dev")
        );

        Ok(())
    }

    #[test]
    fn prepare_agent_dispatch_works_after_bootstrap_without_agents_runtime() -> Result<()> {
        let _guard = forge_test_guard();

        let temp_dir = TestDir::new("dispatch-bootstrap-no-agents");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let startup = bootstrap_for_paths(AppPaths::new(app_data_dir.clone()))?;
        assert_eq!(startup.paths().app_data_dir(), app_data_dir.as_path());
        assert!(startup.forge_enabled());

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let dispatch = runtime
            .block_on(async {
                prepare_agent_dispatch(
                    startup.data_store(),
                    Some(
                        project_root
                            .to_str()
                            .expect("project path should be valid UTF-8")
                            .to_string(),
                    ),
                )
                .await
            })
            .map_err(anyhow::Error::msg)?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.dispatch_role, ActorRole::Agent);
        assert_eq!(dispatch.dispatch_tool_name, "forge_dispatch_agent");
        assert_eq!(dispatch.issue_status, "Todo");
        assert_eq!(dispatch.issue_status_source, "fallback");
        assert!(dispatch.issue_title.is_none());
        assert_eq!(
            dispatch.prompt_source,
            "Entrance-owned harness/bootstrap prompt"
        );
        assert_eq!(
            dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(!dispatch.prompt.contains(".agents"));

        let request = build_agent_task_request(
            dispatch.issue_id.clone(),
            dispatch.worktree_path.clone(),
            "codex:gpt-5-codex".to_string(),
            dispatch.prompt.clone(),
            Vec::new(),
            None,
        )
        .expect("dispatch payload should translate into an agent task");

        let forge_plugin = ForgePlugin::new(startup.data_store(), EventBus::new());
        let task_id = forge_plugin.create_task(request)?;
        let stored_task = forge_plugin
            .get_task(task_id)?
            .expect("stored forge task should exist");

        assert_eq!(
            stored_task.working_dir.as_deref(),
            Some(dispatch.worktree_path.as_str())
        );
        assert_eq!(
            stored_task.stdin_text.as_deref(),
            Some(dispatch.prompt.as_str())
        );
        let metadata: ForgeTaskMetadata = serde_json::from_str(&stored_task.metadata)
            .expect("stored forge task metadata should be valid JSON");
        assert_eq!(metadata.dispatch_role, Some(ActorRole::Agent));
        assert_eq!(
            metadata.dispatch_tool_name.as_deref(),
            Some("forge_dispatch_agent")
        );

        Ok(())
    }

    #[test]
    fn prepare_agent_dispatch_can_target_an_explicit_slot_worktree() -> Result<()> {
        let _guard = forge_test_guard();

        let temp_dir = TestDir::new("dispatch-slot-prepare");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let startup = bootstrap_for_paths(AppPaths::new(app_data_dir.clone()))?;
        assert!(startup.forge_enabled());

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        fs::create_dir_all(&bootstrap_skill)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
        fs::write(project_root.join("README.md"), "slot dispatch test\n")?;
        init_git_repo_with_commit(&project_root);

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(
            managed_worktree
                .parent()
                .expect("managed worktree parent should exist"),
        )?;
        add_git_worktree(&project_root, &managed_worktree, "feat-MYT-48");

        let slot_worktree = allocate_agent_slot_worktree(
            managed_worktree
                .to_str()
                .expect("managed worktree path should be valid UTF-8"),
            "agent-1",
        )
        .map_err(anyhow::Error::msg)?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let dispatch = runtime
            .block_on(async {
                prepare_agent_dispatch_for_worktree(
                    startup.data_store(),
                    Some(
                        project_root
                            .to_str()
                            .expect("project path should be valid UTF-8")
                            .to_string(),
                    ),
                    slot_worktree.clone(),
                )
                .await
            })
            .map_err(anyhow::Error::msg)?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.dispatch_role, ActorRole::Agent);
        assert_eq!(dispatch.dispatch_tool_name, "forge_dispatch_agent");
        assert_eq!(dispatch.worktree_path, slot_worktree.replace('\\', "/"));
        assert!(dispatch.prompt.contains(&slot_worktree.replace('\\', "/")));
        assert!(!dispatch.prompt.contains(".agents"));

        Ok(())
    }

    #[test]
    fn prepare_dev_dispatch_works_after_bootstrap_without_agents_runtime() -> Result<()> {
        let _guard = forge_test_guard();

        let temp_dir = TestDir::new("dispatch-dev-bootstrap-no-agents");
        let app_data_dir = temp_dir.path().join("appdata");
        let _app_data_guard = EnvVarGuard::set("ENTRANCE_APP_DATA_DIR", &app_data_dir);
        let _linear_api_key_guard = EnvVarGuard::remove("LINEAR_API_KEY");
        let _linear_token_guard = EnvVarGuard::remove("LINEAR_TOKEN");

        fs::create_dir_all(&app_data_dir)?;
        let mut config = EntranceConfig::default();
        config.plugins.forge.enabled = true;
        fs::write(app_data_dir.join("entrance.toml"), render_config(&config)?)?;

        let startup = bootstrap_for_paths(AppPaths::new(app_data_dir.clone()))?;
        assert_eq!(startup.paths().app_data_dir(), app_data_dir.as_path());
        assert!(startup.forge_enabled());

        let project_root = temp_dir.path().join("Entrance");
        let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
        let dev_role = bootstrap_skill.join("roles");
        fs::create_dir_all(&dev_role)?;
        fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;
        fs::write(dev_role.join("dev.md"), "# test dev role\n")?;

        let managed_worktree = app_data_dir
            .join("worktrees")
            .join("Entrance")
            .join("feat-MYT-48");
        fs::create_dir_all(&managed_worktree)?;
        init_git_repo(&managed_worktree);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let dispatch = runtime
            .block_on(async {
                prepare_dev_dispatch(
                    startup.data_store(),
                    Some(
                        project_root
                            .to_str()
                            .expect("project path should be valid UTF-8")
                            .to_string(),
                    ),
                )
                .await
            })
            .map_err(anyhow::Error::msg)?;

        assert_eq!(dispatch.issue_id, "MYT-48");
        assert_eq!(dispatch.dispatch_role, ActorRole::Dev);
        assert_eq!(dispatch.dispatch_tool_name, "forge_dispatch_dev");
        assert_eq!(dispatch.issue_status, "Todo");
        assert_eq!(dispatch.issue_status_source, "fallback");
        assert!(dispatch.issue_title.is_none());
        assert_eq!(
            dispatch.prompt_source,
            "Entrance-owned harness/bootstrap dev prompt"
        );
        assert_eq!(
            dispatch.worktree_path,
            managed_worktree.to_string_lossy().replace('\\', "/")
        );
        assert!(!dispatch.prompt.contains(".agents"));

        let request = build_dev_task_request(
            dispatch.issue_id.clone(),
            dispatch.worktree_path.clone(),
            "codex:gpt-5-codex".to_string(),
            dispatch.prompt.clone(),
            Vec::new(),
            None,
        )
        .expect("dispatch payload should translate into a dev task");

        let forge_plugin = ForgePlugin::new(startup.data_store(), EventBus::new());
        let task_id = forge_plugin.create_task(request)?;
        let stored_task = forge_plugin
            .get_task(task_id)?
            .expect("stored forge task should exist");

        assert_eq!(
            stored_task.working_dir.as_deref(),
            Some(dispatch.worktree_path.as_str())
        );
        assert_eq!(
            stored_task.stdin_text.as_deref(),
            Some(dispatch.prompt.as_str())
        );
        let metadata: ForgeTaskMetadata = serde_json::from_str(&stored_task.metadata)
            .expect("stored forge task metadata should be valid JSON");
        assert_eq!(metadata.dispatch_role, Some(ActorRole::Dev));
        assert_eq!(metadata.kind.as_deref(), Some("dev_dispatch"));
        assert_eq!(
            metadata.dispatch_tool_name.as_deref(),
            Some("forge_dispatch_dev")
        );

        Ok(())
    }
}
