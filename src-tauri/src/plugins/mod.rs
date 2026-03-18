use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::config_store::ConfigStore;
use crate::core::data_store::DataStore;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    Read,
    ReadWrite,
    Admin,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationDepth {
    Internal,
    External,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub permission_level: PermissionLevel,
    pub integration_depth: IntegrationDepth,
    #[serde(default = "enabled_by_default")]
    pub enabled_by_default: bool,
}

impl Manifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        permission_level: PermissionLevel,
        integration_depth: IntegrationDepth,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: None,
            permission_level,
            integration_depth,
            enabled_by_default: true,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn disabled_by_default(mut self) -> Self {
        self.enabled_by_default = false;
        self
    }
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub topic: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl Event {
    pub fn new(topic: impl Into<String>, payload: impl Into<serde_json::Value>) -> Self {
        Self {
            topic: topic.into(),
            payload: payload.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TauriCommand {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl TauriCommand {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MCPToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl MCPToolDefinition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Clone)]
pub struct AppContext {
    app_dir: PathBuf,
    config_store: Arc<ConfigStore>,
    data_store: Arc<DataStore>,
}

impl AppContext {
    pub fn new(
        app_dir: impl Into<PathBuf>,
        config_store: Arc<ConfigStore>,
        data_store: Arc<DataStore>,
    ) -> Self {
        Self {
            app_dir: app_dir.into(),
            config_store,
            data_store,
        }
    }

    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    pub fn config_store(&self) -> Arc<ConfigStore> {
        Arc::clone(&self.config_store)
    }

    pub fn data_store(&self) -> Arc<DataStore> {
        Arc::clone(&self.data_store)
    }
}

pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;

    fn init(&self, _ctx: &AppContext) -> Result<()> {
        Ok(())
    }

    fn on_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }

    fn register_commands(&self) -> Vec<TauriCommand> {
        Vec::new()
    }

    fn mcp_tools(&self) -> Vec<MCPToolDefinition> {
        Vec::new()
    }

    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyPlugin {
        manifest: Manifest,
    }

    impl EmptyPlugin {
        fn new() -> Self {
            Self {
                manifest: Manifest::new(
                    "empty",
                    "0.1.0",
                    PermissionLevel::Read,
                    IntegrationDepth::Internal,
                ),
            }
        }
    }

    impl Plugin for EmptyPlugin {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }
    }

    #[test]
    fn plugin_trait_supports_minimal_implementations() {
        let plugin = EmptyPlugin::new();

        assert_eq!(plugin.manifest().name, "empty");
        assert!(plugin.register_commands().is_empty());
        assert!(plugin.mcp_tools().is_empty());
    }
}
