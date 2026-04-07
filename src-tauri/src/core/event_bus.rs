use std::sync::{Arc, OnceLock};

use anyhow::Result;
use serde_json;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::broadcast;

use crate::core::{
    data_store::StoredAgentInstance,
    graph_events::{GraphNodeKind, GraphUpdateEvent, NotaDialogEvent},
};

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
        let topic = topic.into();
        let payload = payload.into();
        if let Some(graph_event) = graph_update_for_instance_event(&topic, &payload) {
            emit_graph_update_runtime(&graph_event);
        }

        let event = EventPayload { topic, payload };
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

fn graph_update_for_instance_event(topic: &str, payload: &str) -> Option<GraphUpdateEvent> {
    match topic {
        "instance:created" => {
            let instance = serde_json::from_str::<StoredAgentInstance>(payload).ok()?;
            Some(GraphUpdateEvent::NodeCreated {
                id: format!("instance-{}", instance.id),
                node_kind: graph_node_kind_for_instance(&instance.role),
                label: instance.display_name,
                parent_id: instance
                    .parent_instance_id
                    .map(|parent_id| format!("instance-{parent_id}")),
                detail: instance.status.clone(),
                tone: graph_tone_for_instance_status(&instance.status).to_string(),
            })
        }
        "instance:stopped" => {
            let id = serde_json::from_str::<i64>(payload).ok()?;
            Some(GraphUpdateEvent::NodeArchived {
                id: format!("instance-{id}"),
            })
        }
        "instance:busy" => {
            let id = serde_json::from_str::<i64>(payload).ok()?;
            Some(GraphUpdateEvent::NodeStateChanged {
                id: format!("instance-{id}"),
                tone: "active".to_string(),
                detail: "Busy".to_string(),
            })
        }
        "instance:idle" => {
            let id = serde_json::from_str::<i64>(payload).ok()?;
            Some(GraphUpdateEvent::NodeStateChanged {
                id: format!("instance-{id}"),
                tone: "steady".to_string(),
                detail: "Idle".to_string(),
            })
        }
        _ => None,
    }
}

fn graph_node_kind_for_instance(role: &str) -> GraphNodeKind {
    match role.trim().to_ascii_lowercase().as_str() {
        "nota" => GraphNodeKind::Nota,
        "arch" => GraphNodeKind::Arch,
        "dev" => GraphNodeKind::Dev,
        _ => GraphNodeKind::Agent,
    }
}

fn graph_tone_for_instance_status(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "busy" => "active",
        "idle" => "steady",
        "stale" => "caution",
        "stopped" => "archived",
        _ => "steady",
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
    use crate::core::data_store::StoredAgentInstance;

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

    #[test]
    fn maps_instance_created_events_to_graph_nodes() {
        let payload = serde_json::to_string(&StoredAgentInstance {
            id: 7,
            role: "arch".to_string(),
            parent_instance_id: Some(3),
            agent_tier: "ArchNota".to_string(),
            status: "Busy".to_string(),
            display_name: "arch-3-1".to_string(),
            config_json: "{}".to_string(),
            workspace_path: None,
            last_heartbeat_at: None,
            created_at: "2026-04-05T00:00:00Z".to_string(),
            updated_at: "2026-04-05T00:00:00Z".to_string(),
        })
        .expect("instance payload should serialize");

        let event = graph_update_for_instance_event("instance:created", &payload);

        assert!(matches!(
            event,
            Some(GraphUpdateEvent::NodeCreated {
                id,
                node_kind: GraphNodeKind::Arch,
                parent_id: Some(parent_id),
                tone,
                ..
            }) if id == "instance-7" && parent_id == "instance-3" && tone == "active"
        ));
    }

    #[test]
    fn maps_instance_status_events_to_graph_state_changes() {
        let busy = graph_update_for_instance_event("instance:busy", "7");
        let idle = graph_update_for_instance_event("instance:idle", "7");
        let stopped = graph_update_for_instance_event("instance:stopped", "7");

        assert!(matches!(
            busy,
            Some(GraphUpdateEvent::NodeStateChanged { id, tone, detail })
                if id == "instance-7" && tone == "active" && detail == "Busy"
        ));
        assert!(matches!(
            idle,
            Some(GraphUpdateEvent::NodeStateChanged { id, tone, detail })
                if id == "instance-7" && tone == "steady" && detail == "Idle"
        ));
        assert!(matches!(
            stopped,
            Some(GraphUpdateEvent::NodeArchived { id }) if id == "instance-7"
        ));
    }
}
