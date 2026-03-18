use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use anyhow::{anyhow, bail, Result};

use crate::plugins::{AppContext, Event, MCPToolDefinition, Plugin, TauriCommand};

#[derive(Clone)]
struct PluginEntry {
    plugin: Arc<dyn Plugin>,
    enabled: bool,
    initialized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginState {
    pub name: String,
    pub enabled: bool,
    pub initialized: bool,
}

pub struct PluginManager {
    ctx: AppContext,
    plugins: RwLock<BTreeMap<String, PluginEntry>>,
}

impl PluginManager {
    pub fn new(ctx: AppContext) -> Self {
        Self {
            ctx,
            plugins: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn context(&self) -> &AppContext {
        &self.ctx
    }

    pub fn register(&self, plugin: Arc<dyn Plugin>) -> Result<()> {
        let name = plugin.manifest().name.clone();
        let mut plugins = self.write_plugins()?;

        if plugins.contains_key(&name) {
            bail!("plugin `{name}` is already registered");
        }

        plugins.insert(
            name,
            PluginEntry {
                enabled: plugin.manifest().enabled_by_default,
                initialized: false,
                plugin,
            },
        );

        Ok(())
    }

    pub fn init_all(&self) -> Result<()> {
        let pending = {
            let plugins = self.read_plugins()?;
            plugins
                .iter()
                .filter(|(_, entry)| entry.enabled && !entry.initialized)
                .map(|(name, entry)| (name.clone(), Arc::clone(&entry.plugin)))
                .collect::<Vec<_>>()
        };

        for (name, plugin) in pending {
            plugin
                .init(&self.ctx)
                .map_err(|source| anyhow!("failed to initialize plugin `{name}`: {source}"))?;
            if let Some(entry) = self.write_plugins()?.get_mut(&name) {
                entry.initialized = true;
            }
        }

        Ok(())
    }

    pub fn enable(&self, name: &str) -> Result<()> {
        let should_init = {
            let mut plugins = self.write_plugins()?;
            let entry = plugins
                .get_mut(name)
                .ok_or_else(|| anyhow!("plugin `{name}` is not registered"))?;
            entry.enabled = true;
            !entry.initialized
        };

        if should_init {
            let plugin = {
                let plugins = self.read_plugins()?;
                Arc::clone(&plugins[name].plugin)
            };
            plugin
                .init(&self.ctx)
                .map_err(|source| anyhow!("failed to initialize plugin `{name}`: {source}"))?;
            if let Some(entry) = self.write_plugins()?.get_mut(name) {
                entry.initialized = true;
            }
        }

        Ok(())
    }

    pub fn disable(&self, name: &str) -> Result<()> {
        let plugin = {
            let plugins = self.read_plugins()?;
            let entry = plugins
                .get(name)
                .ok_or_else(|| anyhow!("plugin `{name}` is not registered"))?;

            if !entry.enabled || !entry.initialized {
                None
            } else {
                Some(Arc::clone(&entry.plugin))
            }
        };

        if let Some(plugin) = plugin {
            plugin
                .shutdown()
                .map_err(|source| anyhow!("failed to shutdown plugin `{name}`: {source}"))?;
        }

        if let Some(entry) = self.write_plugins()?.get_mut(name) {
            entry.enabled = false;
            entry.initialized = false;
        }

        Ok(())
    }

    pub fn dispatch_event(&self, event: &Event) -> Result<()> {
        let plugins = self.active_plugins()?;
        for (name, plugin) in plugins {
            plugin
                .on_event(event)
                .map_err(|source| anyhow!("plugin `{name}` failed to handle event: {source}"))?;
        }

        Ok(())
    }

    pub fn tauri_commands(&self) -> Result<Vec<TauriCommand>> {
        let plugins = self.active_plugins()?;
        Ok(plugins
            .into_iter()
            .flat_map(|(_, plugin)| plugin.register_commands())
            .collect())
    }

    pub fn mcp_tools(&self) -> Result<Vec<MCPToolDefinition>> {
        let plugins = self.active_plugins()?;
        Ok(plugins
            .into_iter()
            .flat_map(|(_, plugin)| plugin.mcp_tools())
            .collect())
    }

    pub fn states(&self) -> Result<Vec<PluginState>> {
        let plugins = self.read_plugins()?;
        Ok(plugins
            .iter()
            .map(|(name, entry)| PluginState {
                name: name.clone(),
                enabled: entry.enabled,
                initialized: entry.initialized,
            })
            .collect())
    }

    fn active_plugins(&self) -> Result<Vec<(String, Arc<dyn Plugin>)>> {
        let plugins = self.read_plugins()?;
        Ok(plugins
            .iter()
            .filter(|(_, entry)| entry.enabled && entry.initialized)
            .map(|(name, entry)| (name.clone(), Arc::clone(&entry.plugin)))
            .collect())
    }

    fn read_plugins(&self) -> Result<RwLockReadGuard<'_, BTreeMap<String, PluginEntry>>> {
        self.plugins
            .read()
            .map_err(|_| anyhow!("plugin manager lock poisoned"))
    }

    fn write_plugins(&self) -> Result<RwLockWriteGuard<'_, BTreeMap<String, PluginEntry>>> {
        self.plugins
            .write()
            .map_err(|_| anyhow!("plugin manager lock poisoned"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::core::config_store::ConfigStore;
    use crate::core::data_store::DataStore;
    use crate::plugins::{IntegrationDepth, Manifest, PermissionLevel};
    use tempfile::tempdir;

    struct CountingPlugin {
        manifest: Manifest,
        init_calls: AtomicUsize,
        shutdown_calls: AtomicUsize,
    }

    impl CountingPlugin {
        fn new() -> Self {
            Self {
                manifest: Manifest::new(
                    "counting",
                    "0.1.0",
                    PermissionLevel::Read,
                    IntegrationDepth::Internal,
                ),
                init_calls: AtomicUsize::new(0),
                shutdown_calls: AtomicUsize::new(0),
            }
        }
    }

    impl Plugin for CountingPlugin {
        fn manifest(&self) -> &Manifest {
            &self.manifest
        }

        fn init(&self, _ctx: &AppContext) -> Result<()> {
            self.init_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn shutdown(&self) -> Result<()> {
            self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn registers_and_toggles_plugins() -> Result<()> {
        let dir = tempdir()?;
        let config = Arc::new(ConfigStore::load(dir.path().join("entrance.toml"))?);
        let data_store = Arc::new(DataStore::connect(dir.path().join("entrance.db")).await?);
        let ctx = AppContext::new(dir.path(), config, data_store);
        let manager = PluginManager::new(ctx);
        let plugin = Arc::new(CountingPlugin::new());

        manager.register(plugin.clone())?;
        manager.init_all()?;
        manager.disable("counting")?;
        manager.enable("counting")?;

        assert_eq!(plugin.init_calls.load(Ordering::SeqCst), 2);
        assert_eq!(plugin.shutdown_calls.load(Ordering::SeqCst), 1);

        Ok(())
    }
}
