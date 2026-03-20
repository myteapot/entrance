use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::task::JoinHandle;

use crate::core::data_store::DataStore;
use crate::core::event_bus::EventBus;

pub struct TaskEngine {
    data_store: DataStore,
    event_bus: EventBus,
    active_tasks: Mutex<HashMap<i64, JoinHandle<()>>>,
}

impl TaskEngine {
    pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
        Self {
            data_store,
            event_bus,
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
        let command = task_record.command.clone();

        self.data_store
            .update_forge_task_status(id, "Running", None)?;
        let _ = self.event_bus.publish(
            "forge:task_status",
            format!(r#"{{"id":{},"status":"Running"}}"#, id),
        );

        let engine_clone = self.clone();

        let handle = tokio::spawn(async move {
            engine_clone.run_process(id, command, args).await;
        });

        self.active_tasks.lock().unwrap().insert(id, handle);

        Ok(())
    }

    pub fn cancel_task(&self, id: i64) -> Result<()> {
        let mut tasks = self.active_tasks.lock().unwrap();
        if let Some(handle) = tasks.remove(&id) {
            handle.abort();
            self.data_store
                .update_forge_task_status(id, "Cancelled", None)?;
            let _ = self.event_bus.publish(
                "forge:task_status",
                format!(r#"{{"id":{},"status":"Cancelled"}}"#, id),
            );
            Ok(())
        } else {
            Err(anyhow!("Task {id} is not running or doesn't exist"))
        }
    }

    async fn run_process(&self, id: i64, command: String, args: Vec<String>) {
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = self
                    .data_store
                    .update_forge_task_status(id, "Failed", Some(-1));
                let _ = self.event_bus.publish(
                    "forge:task_status",
                    format!(r#"{{"id":{},"status":"Failed","error":"{}"}}"#, id, e),
                );
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
                let log_id = match store_out.append_forge_task_log(id, "stdout", &line) {
                    Ok(log_id) => log_id,
                    Err(_) => continue,
                };
                let encoded_line = serde_json::to_string(&line).unwrap_or_default();
                let _ = bus_out.publish(
                    "forge:task_output",
                    format!(
                        r#"{{"id":{},"task_id":{},"stream":"stdout","line":{},"created_at":null}}"#,
                        log_id, id, encoded_line
                    ),
                );
            }
        });

        let bus_err = self.event_bus.clone();
        let store_err = self.data_store.clone();
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let log_id = match store_err.append_forge_task_log(id, "stderr", &line) {
                    Ok(log_id) => log_id,
                    Err(_) => continue,
                };
                let encoded_line = serde_json::to_string(&line).unwrap_or_default();
                let _ = bus_err.publish(
                    "forge:task_output",
                    format!(
                        r#"{{"id":{},"task_id":{},"stream":"stderr","line":{},"created_at":null}}"#,
                        log_id, id, encoded_line
                    ),
                );
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
                let _ = self
                    .data_store
                    .update_forge_task_status(id, text_status, Some(code));
                let _ = self.event_bus.publish(
                    "forge:task_status",
                    format!(
                        r#"{{"id":{},"status":"{}","exit_code":{}}}"#,
                        id, text_status, code
                    ),
                );
            }
            Err(e) => {
                let _ = self
                    .data_store
                    .update_forge_task_status(id, "Failed", Some(-1));
                let _ = self.event_bus.publish(
                    "forge:task_status",
                    format!(r#"{{"id":{},"status":"Failed","error":"{}"}}"#, id, e),
                );
            }
        }
    }
}
