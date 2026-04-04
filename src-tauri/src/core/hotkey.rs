use anyhow::Result;
use tauri::{plugin::TauriPlugin, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use super::event_bus::EventBus;

pub const DEFAULT_LAUNCHER_HOTKEY: &str = "Alt+Space";

pub fn plugin<R: Runtime>() -> Result<TauriPlugin<R>> {
    Ok(tauri_plugin_global_shortcut::Builder::new().build())
}

pub fn register_launcher_shortcut<R: Runtime, M: Manager<R>>(
    manager: &M,
    shortcut: &str,
) -> Result<()> {
    let event_bus = EventBus::new();

    Ok(manager
        .global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = event_bus.emit_launcher_toggle(app);
            }
        })?)
}
