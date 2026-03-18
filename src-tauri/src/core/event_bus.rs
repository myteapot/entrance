use anyhow::Result;
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Debug, Clone, Default)]
pub struct EventBus;

impl EventBus {
    pub fn emit_launcher_toggle<R: Runtime>(&self, app: &AppHandle<R>) -> Result<()> {
        app.emit("launcher:toggle", ())?;
        Ok(())
    }
}
