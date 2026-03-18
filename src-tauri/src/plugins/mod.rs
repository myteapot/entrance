pub mod launcher;

use anyhow::Result;
use serde::Serialize;

use crate::core::data_store::DataStore;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub data_store: DataStore,
}

impl AppContext {
    pub fn new(data_store: DataStore) -> Self {
        Self { data_store }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub enum Event {
    LauncherToggleRequested,
}

#[derive(Debug, Clone, Serialize)]
pub struct TauriCommandDefinition {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
}

#[allow(dead_code)]
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn init(&self, ctx: &AppContext) -> Result<()>;
    fn on_event(&self, event: &Event) -> Result<()>;
    fn register_commands(&self) -> Vec<TauriCommandDefinition>;
    fn mcp_tools(&self) -> Vec<McpToolDefinition>;
    fn shutdown(&self) -> Result<()>;
}
