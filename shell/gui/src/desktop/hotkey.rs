use anyhow::Result;
use tauri::{plugin::TauriPlugin, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_LAUNCHER_HOTKEY: &str = "Alt+Space";

pub fn plugin<R: Runtime>() -> Result<TauriPlugin<R>> {
    Ok(tauri_plugin_global_shortcut::Builder::new().build())
}

pub fn register_launcher_shortcut<R: Runtime, M: Manager<R>>(
    manager: &M,
    shortcut: &str,
) -> Result<()> {
    Ok(manager
        .global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = app.emit("launcher:toggle", ());
            }
        })?)
}
