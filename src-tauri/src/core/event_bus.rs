use std::sync::{Arc, OnceLock};

use anyhow::Result;
use serde_json;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::broadcast;

use crate::core::graph_events::{GraphUpdateEvent, NotaDialogEvent};

#[derive(Debug, Clone)]
pub struct EventPayload {
    pub topic: String,
    pub payload: String,
}

#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<EventPayload>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }
}

type GraphEmitter = Arc<dyn Fn(&GraphUpdateEvent) + Send + Sync>;
type DialogEmitter = Arc<dyn Fn(&NotaDialogEvent) + Send + Sync>;

static GRAPH_EMITTER: OnceLock<GraphEmitter> = OnceLock::new();
static DIALOG_EMITTER: OnceLock<DialogEmitter> = OnceLock::new();

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventPayload> {
        self.sender.subscribe()
    }

    pub fn publish(&self, topic: impl Into<String>, payload: impl Into<String>) -> Result<usize> {
        let event = EventPayload {
            topic: topic.into(),
            payload: payload.into(),
        };
        // ignore SendError if no receivers are currently active
        let count = self.sender.send(event).unwrap_or(0);
        Ok(count)
    }

    pub fn emit_launcher_toggle<R: Runtime>(&self, app: &AppHandle<R>) -> Result<()> {
        app.emit("launcher:toggle", ())?;
        Ok(())
    }

    pub fn emit_graph_update<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        event: &GraphUpdateEvent,
    ) -> Result<()> {
        let json = serde_json::to_string(event)?;
        app.emit("graph:update", json)?;
        Ok(())
    }

    pub fn emit_nota_dialog<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        event: &NotaDialogEvent,
    ) -> Result<()> {
        let json = serde_json::to_string(event)?;
        app.emit("nota:dialog", json)?;
        Ok(())
    }
}

pub fn install_runtime_emitters<R: Runtime + 'static>(event_bus: EventBus, app: AppHandle<R>) {
    let graph_bus = event_bus.clone();
    let graph_app = app.clone();
    let _ = GRAPH_EMITTER.set(Arc::new(move |event: &GraphUpdateEvent| {
        if let Err(error) = graph_bus.emit_graph_update(&graph_app, event) {
            tracing::warn!(?error, "failed to emit graph update");
        }
    }));

    let dialog_bus = event_bus;
    let dialog_app = app;
    let _ = DIALOG_EMITTER.set(Arc::new(move |event: &NotaDialogEvent| {
        if let Err(error) = dialog_bus.emit_nota_dialog(&dialog_app, event) {
            tracing::warn!(?error, "failed to emit nota dialog");
        }
    }));
}

pub fn emit_graph_update_runtime(event: &GraphUpdateEvent) {
    if let Some(emitter) = GRAPH_EMITTER.get() {
        emitter(event);
    }
}

pub fn emit_nota_dialog_runtime(event: &NotaDialogEvent) {
    if let Some(emitter) = DIALOG_EMITTER.get() {
        emitter(event);
    }
}

/// Helper to match topics with wildcards (e.g., "forge:*")
pub fn match_topic(pattern: &str, topic: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        topic.starts_with(prefix)
    } else {
        pattern == topic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_topic() {
        assert!(match_topic("forge:*", "forge:started"));
        assert!(match_topic("forge:*", "forge:"));
        assert!(match_topic("system:*", "system:pulse"));
        assert!(match_topic("system:*", "system:attention"));
        assert!(!match_topic("forge:*", "vault:unlocked"));
        assert!(match_topic("system:ready", "system:ready"));
        assert!(!match_topic("system:ready", "system:ready:yes"));
    }
}
