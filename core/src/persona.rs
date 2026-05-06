use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaProfile {
    pub key: String,
    pub label: String,
}

impl PersonaProfile {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub active: PersonaProfile,
}
