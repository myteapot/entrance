use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{PersistedCommand, Store};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub id: Option<i64>,
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Bus {
    store: Option<Store>,
    topics: Arc<Mutex<HashMap<String, broadcast::Sender<BusEvent>>>>,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self::with_store(None)
    }

    pub fn with_store(store: Option<Store>) -> Self {
        Self {
            store,
            topics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn notify(&self, topic: impl Into<String>, payload: serde_json::Value) {
        let topic = topic.into();
        let sender = self.sender_for(&topic);
        let _ = sender.send(BusEvent { topic, payload });
    }

    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<BusEvent> {
        self.sender_for(topic).subscribe()
    }

    pub fn dispatch(
        &self,
        topic: impl Into<String>,
        payload: serde_json::Value,
    ) -> Result<CommandEnvelope> {
        let topic = topic.into();
        if let Some(store) = &self.store {
            let command = store.enqueue_command(&topic, &payload)?;
            Ok(CommandEnvelope {
                id: Some(command.id),
                topic,
                payload,
            })
        } else {
            Ok(CommandEnvelope {
                id: None,
                topic,
                payload,
            })
        }
    }

    pub fn recover_pending(&self, topic: Option<&str>) -> Result<Vec<CommandEnvelope>> {
        let Some(store) = &self.store else {
            return Ok(Vec::new());
        };

        let commands = store.list_pending_commands(topic)?;
        Ok(commands.into_iter().map(map_command).collect())
    }

    pub fn acknowledge(&self, command_id: i64) -> Result<()> {
        let Some(store) = &self.store else {
            return Ok(());
        };

        store.update_command_status(command_id, "done")
    }

    fn sender_for(&self, topic: &str) -> broadcast::Sender<BusEvent> {
        let mut topics = self.topics.lock().expect("bus mutex poisoned");
        topics
            .entry(topic.to_string())
            .or_insert_with(|| {
                let (sender, _) = broadcast::channel(64);
                sender
            })
            .clone()
    }
}

fn map_command(command: PersistedCommand) -> CommandEnvelope {
    let raw_payload = command.payload_json.clone();
    let payload = serde_json::from_str(&command.payload_json)
        .unwrap_or_else(|_| serde_json::json!({ "raw": raw_payload }));

    CommandEnvelope {
        id: Some(command.id),
        topic: command.topic,
        payload,
    }
}
