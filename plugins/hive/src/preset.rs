use serde::{Deserialize, Serialize};

pub trait HivePreset {
    fn name(&self) -> &'static str;
    fn default_summary(&self, title: &str) -> String;
    fn default_payload(&self) -> serde_json::Value;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SoftwareEngPreset;

impl HivePreset for SoftwareEngPreset {
    fn name(&self) -> &'static str {
        "software-eng"
    }

    fn default_summary(&self, title: &str) -> String {
        format!("dispatch `{title}` prepared for software execution")
    }

    fn default_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "preset": self.name(),
            "rounds": 1,
            "review_required": true
        })
    }
}
