use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::task::JoinHandle;

use crate::core::data_store::{DataStore, StoredForgeTask};
use crate::core::event_bus::EventBus;
use crate::plugins::{
    forge::{ForgeTaskLogEvent, ForgeTaskMetadata, ForgeTaskStatusEvent},
    vault::VaultCipher,
};

pub struct TaskEngine {
    data_store: DataStore,
    event_bus: EventBus,
    vault_cipher: Option<Arc<VaultCipher>>,
    active_tasks: Mutex<HashMap<i64, JoinHandle<()>>>,
}

impl TaskEngine {
    pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
        Self {
            data_store,
            event_bus,
            vault_cipher: VaultCipher::from_device().ok().map(Arc::new),
            active_tasks: Mutex::new(HashMap::new()),
        }
    }

    pub fn spawn_task(self: &Arc<Self>, id: i64) -> Result<()> {
        let task_record = self
            .data_store
            .get_forge_task(id)?
            .ok_or_else(|| anyhow!("Task {id} not found"))?;

        if task_record.status != "Pending" {
            return Err(anyhow!("Task {id} is not Pending"));
        }

        let args: Vec<String> = serde_json::from_str(&task_record.args).unwrap_or_else(|_| vec![]);
        let required_tokens: Vec<String> = serde_json::from_str(&task_record.required_tokens)
            .map_err(|error| anyhow!("Task {id} has invalid required_tokens JSON: {error}"))?;
        let command = task_record.command.clone();
        let envs = match self.resolve_env_bindings(&required_tokens) {
            Ok(envs) => envs,
            Err(message) => {
                self.data_store
                    .update_forge_task_status(id, "Blocked", None, Some(&message))?;
                if let Ok(log) = self
                    .data_store
                    .append_forge_task_log(id, "system", &message)
                {
                    self.publish_task_log(&log);
                }
                self.publish_task_status(id);
                return Ok(());
            }
        };

        self.data_store
            .update_forge_task_status(id, "Running", None, None)?;
        self.publish_task_status(id);

        let engine_clone = self.clone();

        let handle = tokio::spawn(async move {
            engine_clone
                .run_process(task_record, command, args, envs)
                .await;
        });

        self.active_tasks.lock().unwrap().insert(id, handle);

        Ok(())
    }

    pub fn cancel_task(&self, id: i64) -> Result<()> {
        let mut tasks = self.active_tasks.lock().unwrap();
        if let Some(handle) = tasks.remove(&id) {
            handle.abort();
            self.data_store
                .update_forge_task_status(id, "Cancelled", None, None)?;
            self.publish_task_status(id);
            Ok(())
        } else {
            Err(anyhow!("Task {id} is not running or doesn't exist"))
        }
    }

    async fn run_process(
        &self,
        task: StoredForgeTask,
        command: String,
        args: Vec<String>,
        envs: HashMap<String, String>,
    ) {
        let id = task.id;
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .envs(&envs)
            .kill_on_drop(true)
            .stdin(if task.stdin_text.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(working_dir) = task.working_dir.as_deref() {
            cmd.current_dir(working_dir);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let message = format!("Failed to spawn process: {e}");
                self.finalize_task(&task, "Failed", Some(-1), Some(message), &envs)
                    .await;
                return;
            }
        };

        if let Some(stdin_text) = task.stdin_text.as_deref() {
            if let Some(mut stdin) = child.stdin.take() {
                if let Err(error) = stdin.write_all(stdin_text.as_bytes()).await {
                    self.append_system_log(id, &format!("Failed to write task stdin: {error}"));
                } else if let Err(error) = stdin.shutdown().await {
                    self.append_system_log(id, &format!("Failed to close task stdin: {error}"));
                }
            }
        }

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");

        let bus_out = self.event_bus.clone();
        let store_out = self.data_store.clone();
        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let log = match store_out.append_forge_task_log(id, "stdout", &line) {
                    Ok(log) => log,
                    Err(_) => continue,
                };
                let payload =
                    serde_json::to_string(&ForgeTaskLogEvent::from(&log)).unwrap_or_default();
                let _ = bus_out.publish("forge:task_output", payload);
            }
        });

        let bus_err = self.event_bus.clone();
        let store_err = self.data_store.clone();
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let log = match store_err.append_forge_task_log(id, "stderr", &line) {
                    Ok(log) => log,
                    Err(_) => continue,
                };
                let payload =
                    serde_json::to_string(&ForgeTaskLogEvent::from(&log)).unwrap_or_default();
                let _ = bus_err.publish("forge:task_output", payload);
            }
        });

        let status: std::result::Result<std::process::ExitStatus, std::io::Error> =
            child.wait().await;
        let _ = stdout_task.await;
        let _ = stderr_task.await;

        match status {
            Ok(exit_status) => {
                let code = exit_status.code().unwrap_or(0);
                let text_status = if exit_status.success() {
                    "Done"
                } else {
                    "Failed"
                };
                let status_message =
                    (!exit_status.success()).then(|| format!("Process exited with code {code}"));
                self.finalize_task(&task, text_status, Some(code), status_message, &envs)
                    .await;
            }
            Err(e) => {
                let message = format!("Failed while waiting for process completion: {e}");
                self.finalize_task(&task, "Failed", Some(-1), Some(message), &envs)
                    .await;
            }
        }
    }

    fn publish_task_status(&self, id: i64) {
        let Ok(Some(task)) = self.data_store.get_forge_task(id) else {
            return;
        };
        let payload = serde_json::to_string(&ForgeTaskStatusEvent::from(&task)).unwrap_or_default();
        let _ = self.event_bus.publish("forge:task_status", payload);
    }

    fn publish_task_log(&self, log: &crate::core::data_store::StoredForgeTaskLog) {
        let payload = serde_json::to_string(&ForgeTaskLogEvent::from(log)).unwrap_or_default();
        let _ = self.event_bus.publish("forge:task_output", payload);
    }

    fn append_system_log(&self, task_id: i64, line: &str) {
        if let Ok(log) = self
            .data_store
            .append_forge_task_log(task_id, "system", line)
        {
            self.publish_task_log(&log);
        }
    }

    async fn finalize_task(
        &self,
        task: &StoredForgeTask,
        status: &str,
        exit_code: Option<i32>,
        status_message: Option<String>,
        envs: &HashMap<String, String>,
    ) {
        let _ = self.data_store.update_forge_task_status(
            task.id,
            status,
            exit_code,
            status_message.as_deref(),
        );
        self.publish_task_status(task.id);
        self.active_tasks.lock().unwrap().remove(&task.id);

        if let Err(error) = self
            .sync_linear_completion(task, status, exit_code, envs)
            .await
        {
            self.append_system_log(task.id, &format!("Linear sync failed: {error}"));
        }
    }

    async fn sync_linear_completion(
        &self,
        task: &StoredForgeTask,
        status: &str,
        exit_code: Option<i32>,
        envs: &HashMap<String, String>,
    ) -> Result<()> {
        let metadata: ForgeTaskMetadata = serde_json::from_str(&task.metadata).unwrap_or_default();
        let Some(kind) = metadata.kind.as_deref() else {
            return Ok(());
        };
        if kind != "agent_dispatch" {
            return Ok(());
        }

        let issue_identifier = metadata
            .issue_id
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("agent dispatch task is missing issue_id metadata"))?;
        let Some(linear_api_key) = envs.get("LINEAR_API_KEY") else {
            self.append_system_log(
                task.id,
                "Skipping Linear sync because LINEAR_API_KEY is unavailable.",
            );
            return Ok(());
        };

        let linear_issue = fetch_linear_issue(linear_api_key, issue_identifier).await?;

        let next_state = match status {
            "Done" => Some("In Review"),
            "Failed" | "Blocked" => Some("Request"),
            _ => None,
        };

        if let Some(next_state) = next_state {
            let workflow_state_id =
                find_linear_workflow_state_id(linear_api_key, &linear_issue.team_key, next_state)
                    .await?;
            update_linear_issue_state(linear_api_key, &linear_issue.id, &workflow_state_id).await?;
            self.append_system_log(
                task.id,
                &format!(
                    "Linear issue {issue_identifier} moved to {next_state} after task completion."
                ),
            );
        }

        let comment_body = match status {
            "Done" => Some(format!(
                "> 🤖 **Forge | From Entrance**\n\nAgent 任务已完成，进程退出码 `{}`，Forge 已自动转为 `In Review`。",
                exit_code.unwrap_or(0)
            )),
            "Failed" => Some(format!(
                "> 🤖 **Forge | From Entrance**\n\nAgent 任务执行失败，退出码 `{}`。Forge 已自动转为 `Request`，请 Dev 检查日志。",
                exit_code.unwrap_or(-1)
            )),
            "Blocked" => Some(
                "> 🤖 **Forge | From Entrance**\n\nAgent 任务被阻塞，Forge 已自动转为 `Request`，请检查 Vault 凭证或执行环境。"
                    .to_string(),
            ),
            "Cancelled" => Some(
                "> 🤖 **Forge | From Entrance**\n\nAgent 任务已取消，Forge 保留当前任务日志供继续排查。"
                    .to_string(),
            ),
            _ => None,
        };

        if let Some(comment_body) = comment_body {
            create_linear_comment(linear_api_key, &linear_issue.id, &comment_body).await?;
        }

        Ok(())
    }

    fn resolve_env_bindings(
        &self,
        required_tokens: &[String],
    ) -> std::result::Result<HashMap<String, String>, String> {
        if required_tokens.is_empty() {
            return Ok(HashMap::new());
        }

        let Some(cipher) = self.vault_cipher.as_ref() else {
            return Err("Vault 当前不可用，无法注入所需凭证".to_string());
        };

        let mut envs = HashMap::new();
        let mut missing = Vec::new();

        for provider in required_tokens
            .iter()
            .map(|provider| provider.trim())
            .filter(|provider| !provider.is_empty())
        {
            let token = self
                .data_store
                .get_vault_token_by_provider(provider)
                .map_err(|error| format!("读取 Vault 凭证失败: {error}"))?;

            let Some(token) = token else {
                missing.push(provider.to_string());
                continue;
            };

            let value = cipher
                .decrypt(&token.encrypted_value)
                .map_err(|error| format!("解密 Vault 凭证 `{provider}` 失败: {error}"))?;
            envs.insert(provider_env_var(provider), value);
        }

        if missing.is_empty() {
            Ok(envs)
        } else {
            Err(format_missing_tokens_message(&missing))
        }
    }
}

fn format_missing_tokens_message(tokens: &[String]) -> String {
    format!("请先在 Vault 添加 {}", tokens.join(", "))
}

fn provider_env_var(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "openai" => "OPENAI_API_KEY".to_string(),
        "anthropic" | "claude" => "ANTHROPIC_API_KEY".to_string(),
        "google" | "gemini" => "GOOGLE_API_KEY".to_string(),
        "minimax" => "MINIMAX_API_KEY".to_string(),
        other => {
            let normalized = other
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch.to_ascii_uppercase()
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            format!("{normalized}_API_KEY")
        }
    }
}

#[derive(Debug)]
struct LinearIssueContext {
    id: String,
    team_key: String,
}

async fn fetch_linear_issue(api_key: &str, issue_identifier: &str) -> Result<LinearIssueContext> {
    let response = run_linear_graphql(
        api_key,
        r#"
        query ForgeIssue($id: String!) {
          issue(id: $id) {
            id
            team {
              key
              name
            }
          }
        }
        "#,
        serde_json::json!({ "id": issue_identifier }),
    )
    .await?;

    let issue = response
        .get("issue")
        .ok_or_else(|| anyhow!("Linear issue `{issue_identifier}` was not found"))?;
    let id = issue
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Linear issue `{issue_identifier}` is missing an id"))?;
    let team = issue
        .get("team")
        .ok_or_else(|| anyhow!("Linear issue `{issue_identifier}` is missing a team"))?;
    let team_key = team
        .get("key")
        .and_then(serde_json::Value::as_str)
        .or_else(|| team.get("name").and_then(serde_json::Value::as_str))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Linear issue `{issue_identifier}` is missing a team key"))?;

    Ok(LinearIssueContext {
        id: id.to_string(),
        team_key: team_key.to_string(),
    })
}

async fn find_linear_workflow_state_id(
    api_key: &str,
    team_key: &str,
    target_state_name: &str,
) -> Result<String> {
    let response = run_linear_graphql(
        api_key,
        r#"
        query ForgeWorkflowStates {
          workflowStates {
            nodes {
              id
              name
              team {
                key
                name
              }
            }
          }
        }
        "#,
        serde_json::json!({}),
    )
    .await?;

    let nodes = response
        .get("workflowStates")
        .and_then(|value| value.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("Linear workflowStates query returned no nodes"))?;

    let workflow_state_id = nodes.iter().find_map(|node| {
        let name = node.get("name")?.as_str()?;
        if name != target_state_name {
            return None;
        }

        let node_team = node.get("team")?;
        let node_team_key = node_team
            .get("key")
            .and_then(serde_json::Value::as_str)
            .or_else(|| node_team.get("name").and_then(serde_json::Value::as_str))?;
        if node_team_key != team_key {
            return None;
        }

        node.get("id")?.as_str().map(ToOwned::to_owned)
    });

    workflow_state_id.ok_or_else(|| {
        anyhow!(
            "Unable to resolve Linear workflow state `{target_state_name}` for team `{team_key}`"
        )
    })
}

async fn update_linear_issue_state(api_key: &str, issue_id: &str, state_id: &str) -> Result<()> {
    run_linear_graphql(
        api_key,
        r#"
        mutation ForgeUpdateIssue($id: String!, $stateId: String!) {
          issueUpdate(id: $id, input: { stateId: $stateId }) {
            success
          }
        }
        "#,
        serde_json::json!({
            "id": issue_id,
            "stateId": state_id,
        }),
    )
    .await?;
    Ok(())
}

async fn create_linear_comment(api_key: &str, issue_id: &str, body: &str) -> Result<()> {
    run_linear_graphql(
        api_key,
        r#"
        mutation ForgeCommentCreate($issueId: String!, $body: String!) {
          commentCreate(input: { issueId: $issueId, body: $body }) {
            success
          }
        }
        "#,
        serde_json::json!({
            "issueId": issue_id,
            "body": body,
        }),
    )
    .await?;
    Ok(())
}

async fn run_linear_graphql(
    api_key: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.linear.app/graphql")
        .header("Authorization", api_key)
        .json(&serde_json::json!({
            "query": query,
            "variables": variables,
        }))
        .send()
        .await?
        .error_for_status()?;

    let payload: serde_json::Value = response.json().await?;
    if let Some(errors) = payload.get("errors").and_then(serde_json::Value::as_array) {
        if !errors.is_empty() {
            let message = errors
                .iter()
                .filter_map(|error| error.get("message").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!("Linear GraphQL error: {message}"));
        }
    }

    payload
        .get("data")
        .cloned()
        .ok_or_else(|| anyhow!("Linear GraphQL response did not contain a data field"))
}

#[cfg(test)]
impl TaskEngine {
    fn with_vault_cipher(
        data_store: DataStore,
        event_bus: EventBus,
        vault_cipher: VaultCipher,
    ) -> Self {
        Self {
            data_store,
            event_bus,
            vault_cipher: Some(Arc::new(vault_cipher)),
            active_tasks: Mutex::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;
    use crate::core::data_store::{MigrationPlan, MigrationStep};

    #[test]
    fn blocks_tasks_when_required_vault_token_is_missing() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0003_create_plugin_vault_tables",
                sql: include_str!("../../../migrations/0003_create_plugin_vault_tables.sql"),
            },
            MigrationStep {
                name: "0004_create_plugin_forge_task_logs",
                sql: include_str!("../../../migrations/0004_create_plugin_forge_task_logs.sql"),
            },
        ]))?;
        let engine = Arc::new(TaskEngine::with_vault_cipher(
            store.clone(),
            EventBus::new(),
            VaultCipher::from_device_identifier("test-device")?,
        ));
        let blocked_args = test_shell_args("hello")?;
        let task_id = store.insert_forge_task(
            "Echo",
            test_shell(),
            &blocked_args,
            None,
            None,
            r#"["openai"]"#,
            "{}",
        )?;

        engine.spawn_task(task_id)?;

        let task = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        assert_eq!(task.status, "Blocked");
        assert_eq!(
            task.status_message.as_deref(),
            Some("请先在 Vault 添加 openai")
        );

        let logs = store.list_forge_task_logs(task_id)?;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].stream, "system");
        assert_eq!(logs[0].line, "请先在 Vault 添加 openai");

        Ok(())
    }

    #[test]
    fn injects_required_vault_tokens_into_process_env() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0003_create_plugin_vault_tables",
                sql: include_str!("../../../migrations/0003_create_plugin_vault_tables.sql"),
            },
            MigrationStep {
                name: "0004_create_plugin_forge_task_logs",
                sql: include_str!("../../../migrations/0004_create_plugin_forge_task_logs.sql"),
            },
        ]))?;
        let cipher = VaultCipher::from_device_identifier("test-device")?;
        let engine = Arc::new(TaskEngine::with_vault_cipher(
            store.clone(),
            EventBus::new(),
            VaultCipher::from_device_identifier("test-device")?,
        ));

        let encrypted = cipher.encrypt("secret-from-vault")?;
        store.insert_vault_token("Primary", "openai", &encrypted)?;

        let injected_args = test_shell_args(env_echo_expression())?;
        let task_id = store.insert_forge_task(
            "Echo",
            test_shell(),
            &injected_args,
            None,
            None,
            r#"["openai"]"#,
            "{}",
        )?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            engine.spawn_task(task_id)?;
            wait_for_terminal_status_async(&store, task_id).await
        })?;

        let task = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        assert_eq!(task.status, "Done");
        assert_eq!(task.status_message, None);

        let logs = store.list_forge_task_logs(task_id)?;
        assert!(logs
            .iter()
            .any(|log| log.line.contains("secret-from-vault")));

        Ok(())
    }

    async fn wait_for_terminal_status_async(store: &DataStore, task_id: i64) -> Result<()> {
        for _ in 0..1_000 {
            let task = store
                .get_forge_task(task_id)?
                .expect("task should remain queryable");
            if matches!(
                task.status.as_str(),
                "Done" | "Failed" | "Cancelled" | "Blocked"
            ) {
                return Ok(());
            }
            tokio::task::yield_now().await;
            thread::sleep(Duration::from_millis(5));
        }

        Err(anyhow!(
            "task {task_id} did not reach a terminal status in time"
        ))
    }

    #[cfg(target_os = "windows")]
    fn test_shell() -> &'static str {
        "cmd"
    }

    #[cfg(not(target_os = "windows"))]
    fn test_shell() -> &'static str {
        "sh"
    }

    #[cfg(target_os = "windows")]
    fn test_shell_args(input: &str) -> Result<String> {
        Ok(serde_json::to_string(&vec!["/C", input])?)
    }

    #[cfg(not(target_os = "windows"))]
    fn test_shell_args(input: &str) -> Result<String> {
        Ok(serde_json::to_string(&vec!["-c", input])?)
    }

    #[cfg(target_os = "windows")]
    fn env_echo_expression() -> &'static str {
        "echo %OPENAI_API_KEY%"
    }

    #[cfg(not(target_os = "windows"))]
    fn env_echo_expression() -> &'static str {
        "printf '%s\\n' \"$OPENAI_API_KEY\""
    }
}
