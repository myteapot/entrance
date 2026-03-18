use anyhow::Result;
use tauri::{plugin::TauriPlugin, Runtime};
use tauri_plugin_global_shortcut::ShortcutState;

use super::event_bus::EventBus;

pub const DEFAULT_LAUNCHER_HOTKEY: &str = "Alt+Space";

pub fn plugin<R: Runtime>() -> Result<TauriPlugin<R>> {
    let event_bus = EventBus;

    Ok(tauri_plugin_global_shortcut::Builder::new()
        .with_shortcut(DEFAULT_LAUNCHER_HOTKEY)?
        .with_handler(move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = event_bus.emit_launcher_toggle(app);
            }
        })
        .build())
}
