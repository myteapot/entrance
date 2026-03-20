pub mod commands;
pub mod engine;
pub mod http;

use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use crate::{
    core::{
        data_store::{DataStore, MigrationStep, StoredForgeTask, StoredForgeTaskLog},
        event_bus::EventBus,
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

const MIGRATIONS: [MigrationStep; 2] = [
    MigrationStep {
        name: "0002_create_plugin_forge_tasks",
        sql: include_str!("../../../migrations/0002_create_plugin_forge_tasks.sql"),
    },
    MigrationStep {
        name: "0004_create_plugin_forge_task_logs",
        sql: include_str!("../../../migrations/0004_create_plugin_forge_task_logs.sql"),
    },
];

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

#[derive(Debug, Clone)]
pub struct CreateTaskRequest {
    pub name: String,
    pub command: String,
    pub args: String,
    pub working_dir: Option<String>,
    pub stdin_text: Option<String>,
    pub required_tokens: String,
    pub metadata: String,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedAgentDispatch {
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt: String,
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

    pub fn cancel_task(&self, id: i64) -> Result<()> {
        self.engine.cancel_task(id)
    }

    pub fn engine(&self) -> Arc<TaskEngine> {
        self.engine.clone()
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
    let paths = tauri::async_runtime::spawn_blocking(move || resolve_dispatch_paths(project_dir.as_deref()))
        .await
        .map_err(|error| error.to_string())??;

    let issue_summary = fetch_linear_issue_summary(data_store, &paths.issue_id).await?;
    let (issue_status, issue_status_source) = match issue_summary.as_ref() {
        Some(summary) => (summary.issue_status.clone(), "linear".to_string()),
        None => ("Todo".to_string(), "fallback".to_string()),
    };
    let task = build_agent_task_text(&paths.issue_id, issue_summary.as_ref());
    let project_root = paths.project_root.clone();
    let issue_id = paths.issue_id.clone();
    let issue_status_for_prompt = issue_status.clone();
    let prompt = tauri::async_runtime::spawn_blocking(move || {
        generate_agent_prompt(&project_root, &issue_id, &issue_status_for_prompt, &task)
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(PreparedAgentDispatch {
        issue_id: paths.issue_id,
        issue_status,
        issue_status_source,
        issue_title: issue_summary.map(|summary| summary.issue_title),
        project_root: paths.project_root,
        worktree_path: paths.worktree_path,
        prompt,
    })
}

pub(crate) fn build_agent_task_request(
    issue_id: String,
    worktree_path: String,
    model: String,
    prompt: String,
    mut required_tokens: Vec<String>,
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

    let (command, args, stdin_text, provider_token) = match runner {
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

    push_required_token(&mut required_tokens, provider_token);

    let metadata = serde_json::to_string(&ForgeTaskMetadata {
        kind: Some("agent_dispatch".to_string()),
        issue_id: Some(issue_id.clone()),
        worktree_path: Some(worktree_path.clone()),
        model: Some(raw_model.clone()),
    })
    .map_err(|error| error.to_string())?;

    Ok(CreateTaskRequest {
        name: format!("Agent {issue_id}"),
        command,
        args: serde_json::to_string(&args).map_err(|error| error.to_string())?,
        working_dir: Some(worktree_path),
        stdin_text,
        required_tokens: serde_json::to_string(&required_tokens)
            .map_err(|error| error.to_string())?,
        metadata,
    })
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

fn resolve_dispatch_paths(project_dir: Option<&str>) -> Result<DispatchPaths, String> {
    // If project_dir is given, scan for worktrees under the agents directory
    if let Some(project_dir) = project_dir {
        let project_root = PathBuf::from(project_dir);
        if !project_root.exists() {
            return Err(format!("Project directory `{}` does not exist", project_dir));
        }

        // Find the first feat-* worktree under .agents/.worktrees/<project_name>/
        let project_name = project_root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let worktrees_dir = PathBuf::from("A:/.agents/.worktrees").join(&project_name);

        if worktrees_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("feat-") && entry.path().is_dir() {
                        let worktree_path = entry.path();
                        let issue_id = parse_issue_id_from_branch(&name)?;
                        // Verify it's a git worktree
                        if run_git_command(&worktree_path, ["rev-parse", "--show-toplevel"]).is_ok() {
                            return Ok(DispatchPaths {
                                issue_id,
                                project_root: project_dir.replace('\\', "/"),
                                worktree_path: worktree_path.to_string_lossy().replace('\\', "/"),
                            });
                        }
                    }
                }
            }
        }

        return Err(format!(
            "No active worktree found for project `{}`. Create a worktree first with control.py.",
            project_name
        ));
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

fn normalize_command_path(cwd: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw.trim());
    if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    }
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

fn generate_agent_prompt(
    project_root: &str,
    issue_id: &str,
    issue_status: &str,
    task: &str,
) -> Result<String, String> {
    let output = Command::new("python")
        .arg("A:/.agents/nota/scripts/control.py")
        .arg("prompt")
        .arg(project_root)
        .arg(issue_id)
        .arg(issue_status)
        .arg(task)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Failed to generate Agent prompt with control.py".to_string()
        } else {
            format!("Failed to generate Agent prompt with control.py: {stderr}")
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    extract_generated_prompt(&stdout)
}

fn extract_generated_prompt(output: &str) -> Result<String, String> {
    const MARKER: &str = "GENERATED AGENT PROMPT";

    let mut marker_seen = false;
    let mut collecting = false;
    let mut buffer = Vec::new();

    for line in output.lines() {
        if !marker_seen {
            if line.contains(MARKER) {
                marker_seen = true;
            }
            continue;
        }

        if !collecting {
            if line.starts_with('=') {
                collecting = true;
            }
            continue;
        }

        if line.starts_with('=') {
            break;
        }

        if buffer.is_empty() && line.trim().is_empty() {
            continue;
        }

        buffer.push(line);
    }

    let prompt = buffer.join("\n").trim().to_string();
    if prompt.is_empty() {
        Err("control.py did not return a generated Agent prompt".to_string())
    } else {
        Ok(prompt)
    }
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
    use super::{build_agent_task_request, extract_generated_prompt, parse_issue_id_from_branch};

    #[test]
    fn codex_agent_requests_are_translated_into_cli_tasks() {
        let request = build_agent_task_request(
            "MYT-48".to_string(),
            "A:/.agents/.worktrees/Entrance/feat-MYT-48".to_string(),
            "codex:gpt-5-codex".to_string(),
            "implement the task".to_string(),
            vec!["openai".to_string()],
        )
        .expect("agent request should be valid");

        assert_eq!(request.command, "codex");
        assert_eq!(
            request.working_dir.as_deref(),
            Some("A:/.agents/.worktrees/Entrance/feat-MYT-48")
        );
        assert_eq!(request.stdin_text.as_deref(), Some("implement the task"));
        assert!(request.args.contains("\"exec\""));
        assert!(request.args.contains("\"--model\""));
        assert!(request.args.contains("\"gpt-5-codex\""));
        assert!(request.required_tokens.contains("openai"));
        assert!(!request.required_tokens.contains("linear"));
        assert!(request.metadata.contains("\"issue_id\":\"MYT-48\""));
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
    fn generated_prompt_is_extracted_from_control_output() {
        let prompt = extract_generated_prompt(
            "============================================================\nGENERATED AGENT PROMPT\n============================================================\n\nprompt line 1\nprompt line 2\n============================================================\n\nCopy the prompt above into a new Agent window.\n",
        )
        .expect("prompt should be extracted");

        assert_eq!(prompt, "prompt line 1\nprompt line 2");
    }
}
