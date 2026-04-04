use anyhow::Result;
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Default)]
pub struct WindowManager;

impl WindowManager {
    /// Show and focus the main Dashboard window.
    pub fn show_main<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
        if let Some(window) = app.get_webview_window("main") {
            window.show()?;
            window.set_focus()?;
        }
        Ok(())
    }

    /// Show and focus the launcher floating window.
    pub fn show_launcher<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
        if let Some(window) = app.get_webview_window("launcher") {
            window.show()?;
            window.set_focus()?;
        }
        Ok(())
    }

    /// Hide the launcher floating window.
    pub fn hide_launcher<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
        if let Some(window) = app.get_webview_window("launcher") {
            window.hide()?;
        }
        Ok(())
    }

    /// Toggle the visibility of the launcher floating window.
    pub fn toggle_launcher<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
        if let Some(window) = app.get_webview_window("launcher") {
            if window.is_visible().unwrap_or(false) {
                window.hide()?;
            } else {
                window.show()?;
                window.set_focus()?;
            }
        }
        Ok(())
    }
}
