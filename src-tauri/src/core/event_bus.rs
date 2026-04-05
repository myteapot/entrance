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
        assert!(!match_topic("forge:*", "vault:unlocked"));
        assert!(match_topic("system:ready", "system:ready"));
        assert!(!match_topic("system:ready", "system:ready:yes"));
    }
}
