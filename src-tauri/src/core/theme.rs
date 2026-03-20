use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

use super::config_store::ConfigStore;

pub const THEME_CHANGED_EVENT: &str = "theme:changed";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ThemeChangedPayload {
    pub theme: String,
}

#[derive(Debug, Clone)]
pub struct ThemeSystem {
    config_store: Arc<Mutex<ConfigStore>>,
    current_theme: Arc<Mutex<String>>,
}

impl ThemeSystem {
    pub fn new(config_store: ConfigStore) -> Self {
        let theme = config_store.theme().to_string();
        Self {
            config_store: Arc::new(Mutex::new(config_store)),
            current_theme: Arc::new(Mutex::new(theme)),
        }
    }

    pub fn current_theme(&self) -> Result<String> {
        Ok(self
            .current_theme
            .lock()
            .map_err(|_| anyhow!("theme state lock poisoned"))?
            .clone())
    }

    pub fn emit_current_theme<R: Runtime>(&self, app: &AppHandle<R>) -> Result<()> {
        let theme = self.current_theme()?;
        self.emit_theme_changed(app, &theme)
    }

    pub fn set_theme<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        theme: impl Into<String>,
    ) -> Result<String> {
        let theme = normalize_theme(theme.into())?;

        {
            let mut config_store = self
                .config_store
                .lock()
                .map_err(|_| anyhow!("config store lock poisoned"))?;
            config_store.set_theme(theme.clone())?;
        }

        {
            let mut current_theme = self
                .current_theme
                .lock()
                .map_err(|_| anyhow!("theme state lock poisoned"))?;
            *current_theme = theme.clone();
        }

        self.emit_theme_changed(app, &theme)?;
        tracing::info!(theme = %theme, "theme changed");

        Ok(theme)
    }

    fn emit_theme_changed<R: Runtime>(&self, app: &AppHandle<R>, theme: &str) -> Result<()> {
        app.emit(
            THEME_CHANGED_EVENT,
            ThemeChangedPayload {
                theme: theme.to_string(),
            },
        )?;
        Ok(())
    }
}

#[tauri::command]
pub fn get_theme(state: State<'_, ThemeSystem>) -> std::result::Result<String, String> {
    state.current_theme().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_theme(
    app: AppHandle,
    state: State<'_, ThemeSystem>,
    theme: String,
) -> std::result::Result<String, String> {
    state
        .set_theme(&app, theme)
        .map_err(|error| error.to_string())
}

fn normalize_theme(theme: String) -> Result<String> {
    let normalized = theme.trim();
    if normalized.is_empty() {
        return Err(anyhow!("theme must not be empty"));
    }

    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::core::config_store::ConfigStore;

    use super::ThemeSystem;

    #[test]
    fn creating_theme_system_uses_config_theme() -> anyhow::Result<()> {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("entrance-theme-{suffix}"));
        fs::create_dir_all(&temp_dir)?;
        let config_path = temp_dir.join("entrance.toml");
        let config_store = ConfigStore::load_or_create(&config_path)?;

        let theme_system = ThemeSystem::new(config_store);

        assert_eq!(theme_system.current_theme()?, "dark");
        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
