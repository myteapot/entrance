use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Bus {
    topics: Arc<Mutex<HashMap<String, broadcast::Sender<BusEvent>>>>,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self {
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
