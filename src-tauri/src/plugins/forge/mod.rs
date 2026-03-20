pub mod commands;
pub mod engine;
pub mod http;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener},
    sync::{Arc, Mutex},
};

use crate::{
    core::{
        data_store::{DataStore, MigrationStep, StoredForgeTask, StoredForgeTaskLog},
        event_bus::EventBus,
    },
    plugins::{AppContext, Event, Manifest, McpToolDefinition, Plugin, TauriCommandDefinition},
};
use anyhow::Result;
use engine::TaskEngine;
use serde::Serialize;
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

    pub fn create_task(
        &self,
        name: &str,
        command: &str,
        args: &str,
        required_tokens: &str,
    ) -> Result<i64> {
        self.data_store
            .insert_forge_task(name, command, args, required_tokens)
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
        let listener = tokio::net::TcpListener::from_std(listener)?;
        let app = http::router(self.clone());

        let handle = tauri::async_runtime::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                tracing::error!(?error, "forge HTTP server stopped unexpectedly");
            }
        });

        *server = Some(handle);
        tracing::info!("forge HTTP API listening on http://127.0.0.1:{port}");
        Ok(())
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
