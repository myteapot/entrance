use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::hosts::desktop::hotkey::DEFAULT_LAUNCHER_HOTKEY;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntranceConfig {
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub shell: ShellConfig,
}

impl Default for EntranceConfig {
    fn default() -> Self {
        Self {
            core: CoreConfig::default(),
            paths: PathsConfig::default(),
            plugins: PluginsConfig::default(),
            shell: ShellConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathsConfig {
    #[serde(default = "default_runtime_db_path")]
    pub runtime_db: String,
    #[serde(default = "default_logs_path")]
    pub logs: String,
    #[serde(default = "default_cache_path")]
    pub cache: String,
    #[serde(default = "default_exports_path")]
    pub exports: String,
    #[serde(default = "default_snapshots_path")]
    pub snapshots: String,
    #[serde(default = "default_worktrees_path")]
    pub worktrees: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            runtime_db: default_runtime_db_path(),
            logs: default_logs_path(),
            cache: default_cache_path(),
            exports: default_exports_path(),
            snapshots: default_snapshots_path(),
            worktrees: default_worktrees_path(),
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
            vault: TogglePluginConfig::default_enabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub scan_paths: Vec<String>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_forge_http_port")]
    pub http_port: u16,
    #[serde(default)]
    pub project_dir: String,
    #[serde(default)]
    pub agent_command: String,
}

impl Default for ForgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            http_port: default_forge_http_port(),
            project_dir: String::new(),
            agent_command: String::new(),
        }
    }
}

impl ForgeConfig {
    pub fn project_dir_option(&self) -> Option<String> {
        normalize_optional_string(&self.project_dir)
    }

    pub fn agent_command_option(&self) -> Option<String> {
        normalize_optional_string(&self.agent_command)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TogglePluginConfig {
    #[serde(default)]
    pub enabled: bool,
}

impl TogglePluginConfig {
    fn default_enabled() -> Self {
        Self { enabled: true }
    }
}

impl Default for TogglePluginConfig {
    fn default() -> Self {
        Self::default_enabled()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellConfig {
    #[serde(default)]
    pub cli: CliShellConfig,
    #[serde(default)]
    pub gui: GuiShellConfig,
    #[serde(default)]
    pub mcp: McpShellConfig,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            cli: CliShellConfig::default(),
            gui: GuiShellConfig::default(),
            mcp: McpShellConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliShellConfig {
    #[serde(default = "default_cli_output")]
    pub default_output: String,
}

impl Default for CliShellConfig {
    fn default() -> Self {
        Self {
            default_output: default_cli_output(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuiShellConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_launcher_hotkey")]
    pub global_hotkey: String,
}

impl Default for GuiShellConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            global_hotkey: default_launcher_hotkey(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpShellConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_mcp_mode")]
    pub mode: String,
    #[serde(default = "default_mcp_actor_role")]
    pub default_actor_role: String,
}

impl Default for McpShellConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: default_mcp_mode(),
            default_actor_role: default_mcp_actor_role(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LegacyEntranceConfig {
    #[serde(default)]
    core: LegacyCoreConfig,
    #[serde(default)]
    paths: PathsConfig,
    #[serde(default)]
    plugins: LegacyPluginsConfig,
}

impl Default for LegacyEntranceConfig {
    fn default() -> Self {
        Self {
            core: LegacyCoreConfig::default(),
            paths: PathsConfig::default(),
            plugins: LegacyPluginsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LegacyCoreConfig {
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_log_level")]
    log_level: String,
    #[serde(default = "default_true")]
    mcp_enabled: bool,
}

impl Default for LegacyCoreConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            log_level: default_log_level(),
            mcp_enabled: default_true(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LegacyPluginsConfig {
    #[serde(default)]
    launcher: LegacyLauncherConfig,
    #[serde(default)]
    forge: LegacyForgeConfig,
    #[serde(default)]
    vault: LegacyTogglePluginConfig,
}

impl Default for LegacyPluginsConfig {
    fn default() -> Self {
        Self {
            launcher: LegacyLauncherConfig::default(),
            forge: LegacyForgeConfig::default(),
            vault: LegacyTogglePluginConfig::default_disabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LegacyLauncherConfig {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_launcher_hotkey")]
    hotkey: String,
    #[serde(default)]
    scan_paths: Vec<String>,
}

impl Default for LegacyLauncherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: default_launcher_hotkey(),
            scan_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LegacyForgeConfig {
    #[serde(default = "default_legacy_forge_enabled")]
    enabled: bool,
    #[serde(default = "default_legacy_forge_http_port")]
    http_port: u16,
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    agent_command: Option<String>,
}

impl Default for LegacyForgeConfig {
    fn default() -> Self {
        Self {
            enabled: default_legacy_forge_enabled(),
            http_port: default_legacy_forge_http_port(),
            project_dir: None,
            agent_command: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LegacyTogglePluginConfig {
    #[serde(default)]
    enabled: bool,
}

impl LegacyTogglePluginConfig {
    fn default_disabled() -> Self {
        Self { enabled: false }
    }
}

impl Default for LegacyTogglePluginConfig {
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
            let (config, rewritten) = parse_config_document(&content)
                .with_context(|| format!("failed to parse config file at {}", path.display()))?;
            if rewritten {
                write_config_file(&path, &config)?;
            }
            config
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
        &self.config.shell.gui.theme
    }

    pub fn log_level(&self) -> &str {
        &self.config.core.log_level
    }

    pub fn mcp_enabled(&self) -> bool {
        self.config.shell.mcp.enabled
    }

    pub fn set_theme(&mut self, theme: impl Into<String>) -> Result<()> {
        self.config.shell.gui.theme = theme.into();
        write_config_file(&self.path, &self.config)
    }
}

fn parse_config_document(raw_content: &str) -> Result<(EntranceConfig, bool)> {
    let parsed = toml::from_str::<toml::Value>(raw_content).context("failed to parse TOML")?;
    if contains_legacy_schema(&parsed) {
        let mut legacy = toml::from_str::<LegacyEntranceConfig>(raw_content)
            .context("failed to parse legacy Entrance config schema")?;
        if should_enable_forge_for_legacy_desktop_config(raw_content, &legacy)? {
            legacy.plugins.forge.enabled = true;
        }
        return Ok((legacy_into_current(legacy), true));
    }

    let config = toml::from_str::<EntranceConfig>(raw_content)
        .context("failed to parse Entrance config schema")?;
    Ok((config, false))
}

fn contains_legacy_schema(value: &toml::Value) -> bool {
    has_table_key(value, &["core", "theme"])
        || has_table_key(value, &["core", "mcp_enabled"])
        || has_table_key(value, &["plugins", "launcher", "hotkey"])
}

fn has_table_key(value: &toml::Value, path: &[&str]) -> bool {
    let mut current = value;
    for segment in path {
        let Some(next) = current.get(*segment) else {
            return false;
        };
        current = next;
    }
    true
}

fn legacy_into_current(legacy: LegacyEntranceConfig) -> EntranceConfig {
    EntranceConfig {
        core: CoreConfig {
            log_level: legacy.core.log_level,
        },
        paths: legacy.paths,
        plugins: PluginsConfig {
            launcher: LauncherConfig {
                enabled: legacy.plugins.launcher.enabled,
                scan_paths: legacy.plugins.launcher.scan_paths,
            },
            forge: ForgeConfig {
                enabled: legacy.plugins.forge.enabled,
                http_port: legacy.plugins.forge.http_port,
                project_dir: legacy.plugins.forge.project_dir.unwrap_or_default(),
                agent_command: legacy.plugins.forge.agent_command.unwrap_or_default(),
            },
            vault: TogglePluginConfig {
                enabled: legacy.plugins.vault.enabled,
            },
        },
        shell: ShellConfig {
            cli: CliShellConfig::default(),
            gui: GuiShellConfig {
                theme: legacy.core.theme,
                global_hotkey: legacy.plugins.launcher.hotkey,
            },
            mcp: McpShellConfig {
                enabled: legacy.core.mcp_enabled,
                mode: default_mcp_mode(),
                default_actor_role: default_mcp_actor_role(),
            },
        },
    }
}

fn write_config_file(path: &Path, config: &EntranceConfig) -> Result<()> {
    let content = render_config(config)?;
    fs::write(path, content)
        .with_context(|| format!("failed to write config file at {}", path.display()))?;
    Ok(())
}

pub fn render_config(config: &EntranceConfig) -> Result<String> {
    toml::to_string_pretty(config).context("failed to render Entrance config")
}

fn normalize_optional_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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
    9315
}

fn default_cli_output() -> String {
    "json".to_string()
}

fn default_mcp_mode() -> String {
    "stdio".to_string()
}

fn default_mcp_actor_role() -> String {
    "nota".to_string()
}

fn default_runtime_db_path() -> String {
    "data/entrance.db".to_string()
}

fn default_logs_path() -> String {
    "logs".to_string()
}

fn default_cache_path() -> String {
    "cache".to_string()
}

fn default_exports_path() -> String {
    "exports".to_string()
}

fn default_snapshots_path() -> String {
    "snapshots".to_string()
}

fn default_worktrees_path() -> String {
    "worktrees".to_string()
}

fn default_legacy_forge_enabled() -> bool {
    false
}

fn default_legacy_forge_http_port() -> u16 {
    9721
}

fn should_enable_forge_for_legacy_desktop_config(
    raw_content: &str,
    legacy: &LegacyEntranceConfig,
) -> Result<bool> {
    if legacy.plugins.forge.enabled {
        return Ok(false);
    }

    let normalized = raw_content.replace("\r\n", "\n");
    if !normalized.contains("[plugins.forge]") {
        return Ok(true);
    }

    let legacy_default = toml::to_string_pretty(&legacy_default_config())
        .context("failed to render legacy Entrance config")?
        .replace("\r\n", "\n");
    Ok(normalized == legacy_default)
}

fn legacy_default_config() -> LegacyEntranceConfig {
    LegacyEntranceConfig::default()
}
