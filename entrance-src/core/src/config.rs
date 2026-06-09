use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub persona: String,
    #[serde(default)]
    pub drawer: DrawerConfig,
    #[serde(default)]
    pub hive: HiveConfig,
    #[serde(default)]
    pub launcher: LauncherConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            persona: "operator".to_string(),
            drawer: DrawerConfig::default(),
            hive: HiveConfig::default(),
            launcher: LauncherConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerConfig {
    #[serde(default = "default_drawer_mode")]
    pub mode: String,
    #[serde(default = "default_drawer_root")]
    pub root: String,
}

impl Default for DrawerConfig {
    fn default() -> Self {
        Self {
            mode: default_drawer_mode(),
            root: default_drawer_root(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveConfig {
    #[serde(default = "default_hive_http_port")]
    pub http_port: u16,
}

impl Default for HiveConfig {
    fn default() -> Self {
        Self {
            http_port: default_hive_http_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default = "default_launcher_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub scan_paths: Vec<String>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            hotkey: default_launcher_hotkey(),
            scan_paths: vec![],
        }
    }
}

impl AppConfig {
    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            let content = fs::read_to_string(path)?;
            return toml::from_str(&content)
                .with_context(|| format!("failed to parse config at {}", path.display()));
        }

        let config = Self::default();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(&config)?)?;
        Ok(config)
    }

    pub fn drawer_root(&self, app_root: &Path) -> PathBuf {
        app_root.join(&self.drawer.root)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::AppConfig;

    #[test]
    fn invalid_config_fails_instead_of_defaulting() {
        let path = std::env::temp_dir().join(format!(
            "entrance-invalid-config-{}.toml",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, "persona = [").unwrap();

        let result = AppConfig::load_or_create(&path);

        assert!(result.is_err());
        let _ = fs::remove_file(path);
    }
}

fn default_drawer_mode() -> String {
    "filesystem".to_string()
}

fn default_drawer_root() -> String {
    "drawer".to_string()
}

fn default_hive_http_port() -> u16 {
    9720
}

fn default_launcher_hotkey() -> String {
    "Ctrl+Space".to_string()
}
