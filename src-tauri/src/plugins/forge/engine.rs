use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::core::data_store::DataStore;
use crate::core::event_bus::EventBus;
use crate::plugins::{
    forge::{ForgeTaskLogEvent, ForgeTaskStatusEvent},
    vault::VaultCipher,
};

const TASK_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

pub struct TaskEngine {
    data_store: DataStore,
    event_bus: EventBus,
    vault_cipher: Option<Arc<VaultCipher>>,
    active_tasks: Mutex<HashMap<i64, JoinHandle<()>>>,
    heartbeat_interval: Duration,
}

impl TaskEngine {
    pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
        Self {
            data_store,
            event_bus,
            vault_cipher: VaultCipher::from_device().ok().map(Arc::new),
            active_tasks: Mutex::new(HashMap::new()),
            heartbeat_interval: TASK_HEARTBEAT_INTERVAL,
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
        let working_dir = task_record.working_dir.clone();
        let stdin_text = task_record.stdin_text.clone();
        let envs = match self.resolve_env_bindings(&required_tokens) {
            Ok(envs) => envs,
            Err(message) => {
                self.data_store
                    .update_forge_task_status(id, "Blocked", None, Some(&message))?;
                self.append_system_log(id, &message);
                self.publish_task_status(id);
                return Ok(());
            }
        };

        self.data_store
            .update_forge_task_status(id, "Running", None, None)?;
        self.publish_task_status(id);

        let engine_clone = self.clone();

        let handle = tauri::async_runtime::spawn(async move {
            engine_clone
                .run_process(id, command, args, working_dir, stdin_text, envs)
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
            self.append_system_log(id, "Task cancelled by operator");
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
        working_dir: Option<String>,
        stdin_text: Option<String>,
        envs: HashMap<String, String>,
    ) {
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .envs(&envs)
            .kill_on_drop(true)
            .stdin(if stdin_text.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(working_dir) = working_dir.as_deref() {
            cmd.current_dir(working_dir);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let message = format!("Failed to spawn process: {e}");
                self.record_terminal_failure(id, Some(-1), &message);
                return;
            }
        };

        let stdin_task = match (child.stdin.take(), stdin_text) {
            (Some(mut stdin), Some(stdin_text)) => Some(tokio::spawn(async move {
                let _ = stdin.write_all(stdin_text.as_bytes()).await;
                let _ = stdin.shutdown().await;
            })),
            _ => None,
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

        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + self.heartbeat_interval,
            self.heartbeat_interval,
        );
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let wait = child.wait();
        tokio::pin!(wait);
        let status: std::result::Result<std::process::ExitStatus, std::io::Error> = loop {
            tokio::select! {
                result = &mut wait => break result,
                _ = heartbeat.tick() => {
                    let _ = self.data_store.touch_forge_task_heartbeat(id);
                }
            }
        };
        if let Some(stdin_task) = stdin_task {
            let _ = stdin_task.await;
        }
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
                if exit_status.success() {
                    let _ = self.data_store.update_forge_task_status(
                        id,
                        text_status,
                        Some(code),
                        status_message.as_deref(),
                    );
                    self.publish_task_status(id);
                } else if let Some(message) = status_message {
                    self.record_terminal_failure(id, Some(code), &message);
                }
            }
            Err(e) => {
                let message = format!("Failed while waiting for process completion: {e}");
                self.record_terminal_failure(id, Some(-1), &message);
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

    fn append_system_log(&self, id: i64, message: &str) {
        if let Ok(log) = self.data_store.append_forge_task_log(id, "system", message) {
            self.publish_task_log(&log);
        }
    }

    fn record_terminal_failure(&self, id: i64, exit_code: Option<i32>, message: &str) {
        let _ = self
            .data_store
            .update_forge_task_status(id, "Failed", exit_code, Some(message));
        self.append_system_log(id, message);
        self.publish_task_status(id);
        self.active_tasks.lock().unwrap().remove(&id);
    }

    fn resolve_env_bindings(
        &self,
        required_tokens: &[String],
    ) -> std::result::Result<HashMap<String, String>, String> {
        if required_tokens.is_empty() {
            return Ok(HashMap::new());
        }

        let mut envs = HashMap::new();
        let mut missing = Vec::new();

        for provider in required_tokens
            .iter()
            .map(|provider| provider.trim())
            .filter(|provider| !provider.is_empty())
        {
            let env_key = provider_env_var(provider);

            // 1. Check system environment variable first
            if let Ok(value) = std::env::var(&env_key) {
                if !value.is_empty() {
                    envs.insert(env_key, value);
                    continue;
                }
            }

            // 2. Fall back to Vault
            let Some(cipher) = self.vault_cipher.as_ref() else {
                missing.push(provider.to_string());
                continue;
            };

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
            envs.insert(env_key, value);
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
            heartbeat_interval: TASK_HEARTBEAT_INTERVAL,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn passes_stdin_text_to_spawned_process() -> Result<()> {
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
        let engine = Arc::new(TaskEngine::new(store.clone(), EventBus::new()));

        let task_id = store.insert_forge_task(
            "Echo stdin",
            test_shell(),
            &test_shell_args(stdin_echo_expression())?,
            None,
            Some("hello via stdin\n"),
            r#"[]"#,
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

        let logs = store.list_forge_task_logs(task_id)?;
        assert!(logs.iter().any(|log| log.line.contains("hello via stdin")));

        Ok(())
    }

    #[test]
    fn applies_working_dir_to_spawned_process() -> Result<()> {
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
        let engine = Arc::new(TaskEngine::new(store.clone(), EventBus::new()));
        let working_dir = create_test_working_dir("forge-working-dir")?;
        let working_dir_text = working_dir.to_string_lossy().to_string();

        let task_id = store.insert_forge_task(
            "Print cwd",
            test_shell(),
            &test_shell_args(current_dir_expression())?,
            Some(&working_dir_text),
            None,
            r#"[]"#,
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

        let expected = normalize_logged_path(&working_dir);
        let logs = store.list_forge_task_logs(task_id)?;
        assert!(logs
            .iter()
            .any(|log| normalize_path_text(&log.line) == expected));

        let _ = fs::remove_dir_all(&working_dir);

        Ok(())
    }

    #[test]
    fn failed_process_exit_is_visible_in_system_log() -> Result<()> {
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
        let engine = Arc::new(TaskEngine::new(store.clone(), EventBus::new()));

        let task_id = store.insert_forge_task(
            "Fail",
            test_shell(),
            &test_shell_args(failing_expression())?,
            None,
            None,
            r#"[]"#,
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
        assert_eq!(task.status, "Failed");
        assert_eq!(
            task.status_message.as_deref(),
            Some("Process exited with code 7")
        );

        let logs = store.list_forge_task_logs(task_id)?;
        assert!(logs
            .iter()
            .any(|log| log.stream == "system" && log.line.contains("Process exited with code 7")));

        Ok(())
    }

    #[test]
    fn silent_running_process_refreshes_storage_backed_heartbeat() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0004_create_plugin_forge_task_logs",
                sql: include_str!("../../../migrations/0004_create_plugin_forge_task_logs.sql"),
            },
        ]))?;
        let engine = Arc::new(TaskEngine {
            data_store: store.clone(),
            event_bus: EventBus::new(),
            vault_cipher: None,
            active_tasks: Mutex::new(std::collections::HashMap::new()),
            heartbeat_interval: Duration::from_millis(50),
        });

        let task_id = store.insert_forge_task(
            "Quiet wait",
            test_shell(),
            &test_shell_args(quiet_wait_expression())?,
            None,
            None,
            r#"[]"#,
            "{}",
        )?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            engine.spawn_task(task_id)?;
            let initial_heartbeat = wait_for_heartbeat_async(&store, task_id).await?;
            let refreshed_heartbeat =
                wait_for_heartbeat_change_async(&store, task_id, &initial_heartbeat).await?;
            assert_ne!(refreshed_heartbeat, initial_heartbeat);
            engine.cancel_task(task_id)?;
            wait_for_terminal_status_async(&store, task_id).await
        })?;

        let task = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        assert_eq!(task.status, "Cancelled");
        assert!(task.heartbeat_at.is_some());

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

    async fn wait_for_heartbeat_async(store: &DataStore, task_id: i64) -> Result<String> {
        for _ in 0..1_000 {
            let task = store
                .get_forge_task(task_id)?
                .expect("task should remain queryable");
            if let Some(heartbeat_at) = task.heartbeat_at {
                return Ok(heartbeat_at);
            }
            tokio::task::yield_now().await;
            thread::sleep(Duration::from_millis(5));
        }

        Err(anyhow!("task {task_id} did not record a heartbeat in time"))
    }

    async fn wait_for_heartbeat_change_async(
        store: &DataStore,
        task_id: i64,
        previous_heartbeat: &str,
    ) -> Result<String> {
        for _ in 0..1_000 {
            let task = store
                .get_forge_task(task_id)?
                .expect("task should remain queryable");
            if let Some(heartbeat_at) = task.heartbeat_at {
                if heartbeat_at != previous_heartbeat {
                    return Ok(heartbeat_at);
                }
            }
            tokio::task::yield_now().await;
            thread::sleep(Duration::from_millis(5));
        }

        Err(anyhow!(
            "task {task_id} heartbeat did not advance beyond `{previous_heartbeat}` in time"
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
    fn stdin_echo_expression() -> &'static str {
        "more"
    }

    #[cfg(not(target_os = "windows"))]
    fn stdin_echo_expression() -> &'static str {
        "cat"
    }

    #[cfg(target_os = "windows")]
    fn current_dir_expression() -> &'static str {
        "cd"
    }

    #[cfg(not(target_os = "windows"))]
    fn current_dir_expression() -> &'static str {
        "pwd"
    }

    #[cfg(target_os = "windows")]
    fn env_echo_expression() -> &'static str {
        "echo %OPENAI_API_KEY%"
    }

    #[cfg(not(target_os = "windows"))]
    fn env_echo_expression() -> &'static str {
        "printf '%s\\n' \"$OPENAI_API_KEY\""
    }

    #[cfg(target_os = "windows")]
    fn failing_expression() -> &'static str {
        "exit /b 7"
    }

    #[cfg(not(target_os = "windows"))]
    fn failing_expression() -> &'static str {
        "exit 7"
    }

    #[cfg(target_os = "windows")]
    fn quiet_wait_expression() -> &'static str {
        "ping -n 6 127.0.0.1 > nul"
    }

    #[cfg(not(target_os = "windows"))]
    fn quiet_wait_expression() -> &'static str {
        "sleep 2"
    }

    fn create_test_working_dir(prefix: &str) -> Result<PathBuf> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| anyhow!("system clock error: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{suffix}"));
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    fn normalize_logged_path(path: &Path) -> String {
        normalize_path_text(&path.to_string_lossy())
    }

    fn normalize_path_text(value: &str) -> String {
        #[cfg(target_os = "windows")]
        {
            value
                .trim()
                .trim_start_matches("\\\\?\\")
                .replace('/', "\\")
                .to_ascii_lowercase()
        }

        #[cfg(not(target_os = "windows"))]
        {
            value.trim().to_string()
        }
    }
}
