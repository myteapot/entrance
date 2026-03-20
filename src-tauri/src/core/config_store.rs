use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::hotkey::DEFAULT_LAUNCHER_HOTKEY;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntranceConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
}

impl Default for EntranceConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig::default(),
            plugins: PluginsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_true")]
    pub mcp_enabled: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            log_level: default_log_level(),
            mcp_enabled: default_true(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginsConfig {
    #[serde(default)]
    pub launcher: LauncherConfig,
    #[serde(default)]
    pub forge: ForgeConfig,
    #[serde(default)]
    pub vault: TogglePluginConfig,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            launcher: LauncherConfig::default(),
            forge: ForgeConfig::default(),
            vault: TogglePluginConfig::default_disabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_launcher_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub scan_paths: Vec<String>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: default_launcher_hotkey(),
            scan_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_forge_http_port")]
    pub http_port: u16,
    #[serde(default)]
    pub project_dir: Option<String>,
    /// Custom agent command path. When set, overrides the default CLI name.
    /// e.g. "C:\\Scoop\\apps\\nodejs\\current\\bin\\codex.cmd"
    #[serde(default)]
    pub agent_command: Option<String>,
}

impl Default for ForgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            http_port: default_forge_http_port(),
            project_dir: None,
            agent_command: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TogglePluginConfig {
    #[serde(default)]
    pub enabled: bool,
}

impl TogglePluginConfig {
    fn default_disabled() -> Self {
        Self { enabled: false }
    }
}

impl Default for TogglePluginConfig {
    fn default() -> Self {
        Self::default_disabled()
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
    config: EntranceConfig,
}

impl ConfigStore {
    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory at {}", parent.display())
            })?;
        }

        let config = if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config file at {}", path.display()))?;
            toml::from_str::<EntranceConfig>(&content)
                .with_context(|| format!("failed to parse config file at {}", path.display()))?
        } else {
            let default_config = EntranceConfig::default();
            write_config_file(&path, &default_config)?;
            default_config
        };

        Ok(Self { path, config })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config(&self) -> &EntranceConfig {
        &self.config
    }

    pub fn theme(&self) -> &str {
        &self.config.core.theme
    }

    pub fn log_level(&self) -> &str {
        &self.config.core.log_level
    }

    pub fn mcp_enabled(&self) -> bool {
        self.config.core.mcp_enabled
    }

    pub fn set_theme(&mut self, theme: impl Into<String>) -> Result<()> {
        self.config.core.theme = theme.into();
        write_config_file(&self.path, &self.config)
    }
}

fn write_config_file(path: &Path, config: &EntranceConfig) -> Result<()> {
    let content = render_config(config)?;
    fs::write(path, content)
        .with_context(|| format!("failed to write config file at {}", path.display()))?;
    Ok(())
}

pub fn render_config(config: &EntranceConfig) -> Result<String> {
    toml::to_string_pretty(config).context("failed to render default config")
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_true() -> bool {
    true
}

fn default_launcher_hotkey() -> String {
    DEFAULT_LAUNCHER_HOTKEY.to_string()
}

fn default_forge_http_port() -> u16 {
    9721
}
