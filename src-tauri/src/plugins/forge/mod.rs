pub mod commands;
pub mod engine;

use std::sync::Arc;
use anyhow::Result;
use crate::{
    core::data_store::{DataStore, MigrationStep, StoredForgeTask},
    core::event_bus::EventBus,
    plugins::{AppContext, Event, Manifest, McpToolDefinition, Plugin, TauriCommandDefinition},
};
use engine::TaskEngine;

const MANIFEST: Manifest = Manifest {
    name: "forge",
    version: env!("CARGO_PKG_VERSION"),
    description: "Agent task management and execution engine.",
};

const MIGRATIONS: [MigrationStep; 1] = [MigrationStep {
    name: "0002_create_plugin_forge_tasks",
    sql: include_str!("../../../migrations/0002_create_plugin_forge_tasks.sql"),
}];

pub fn migrations() -> &'static [MigrationStep] {
    &MIGRATIONS
}

#[derive(Clone)]
pub struct ForgePlugin {
    manifest: Manifest,
    data_store: DataStore,
    engine: Arc<TaskEngine>,
}

impl ForgePlugin {
    pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
        Self {
            manifest: MANIFEST,
            data_store: data_store.clone(),
            engine: Arc::new(TaskEngine::new(data_store, event_bus)),
        }
    }

    pub fn create_task(&self, name: &str, command: &str, args: &str) -> Result<i64> {
        self.data_store.insert_forge_task(name, command, args)
    }

    pub fn list_tasks(&self) -> Result<Vec<StoredForgeTask>> {
        self.data_store.list_forge_tasks()
    }

    pub fn get_task(&self, id: i64) -> Result<Option<StoredForgeTask>> {
        self.data_store.get_forge_task(id)
    }

    pub fn cancel_task(&self, id: i64) -> Result<()> {
        self.engine.cancel_task(id)
    }

    pub fn engine(&self) -> Arc<TaskEngine> {
        self.engine.clone()
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
                name: "forge.cancel_task",
                description: "Cancel a running forge task",
            },
        ]
    }

    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
