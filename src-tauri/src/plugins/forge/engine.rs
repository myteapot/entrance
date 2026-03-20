use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::async_runtime::JoinHandle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::core::data_store::DataStore;
use crate::core::event_bus::EventBus;
use crate::plugins::{
    forge::{ForgeTaskLogEvent, ForgeTaskStatusEvent},
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

        let handle = tauri::async_runtime::spawn(async move {
            engine_clone.run_process(id, command, args, envs).await;
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
        id: i64,
        command: String,
        args: Vec<String>,
        envs: HashMap<String, String>,
    ) {
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .envs(&envs)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let message = format!("Failed to spawn process: {e}");
                let _ = self.data_store.update_forge_task_status(
                    id,
                    "Failed",
                    Some(-1),
                    Some(&message),
                );
                self.publish_task_status(id);
                self.active_tasks.lock().unwrap().remove(&id);
                return;
            }
        };

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

        self.active_tasks.lock().unwrap().remove(&id);

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
                let _ = self.data_store.update_forge_task_status(
                    id,
                    text_status,
                    Some(code),
                    status_message.as_deref(),
                );
                self.publish_task_status(id);
            }
            Err(e) => {
                let message = format!("Failed while waiting for process completion: {e}");
                let _ = self.data_store.update_forge_task_status(
                    id,
                    "Failed",
                    Some(-1),
                    Some(&message),
                );
                self.publish_task_status(id);
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
        let task_id =
            store.insert_forge_task("Echo", test_shell(), &blocked_args, r#"["openai"]"#)?;

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
        let task_id =
            store.insert_forge_task("Echo", test_shell(), &injected_args, r#"["openai"]"#)?;

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
