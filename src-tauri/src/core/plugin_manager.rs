use std::sync::Arc;

use anyhow::Result;

use crate::plugins::{AppContext, Manifest, Plugin};

#[derive(Default)]
pub struct PluginManager {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginManager {
    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn init_all(&self, ctx: &AppContext) -> Result<()> {
        for plugin in &self.plugins {
            let _ = plugin.manifest();
            let _ = plugin.register_commands();
            let _ = plugin.mcp_tools();
            plugin.init(ctx)?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn shutdown_all(&self) -> Result<()> {
        for plugin in &self.plugins {
            plugin.shutdown()?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn manifests(&self) -> Vec<Manifest> {
        self.plugins
            .iter()
            .map(|plugin| plugin.manifest().clone())
            .collect()
    }
}
