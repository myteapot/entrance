use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{memory_tag, storage::DrawerStorage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImportReport {
    pub id: i64,
    pub title: String,
}

pub fn import_memory(
    storage: &DrawerStorage,
    title: String,
    body: String,
    mut tags: Vec<String>,
) -> Result<MemoryImportReport> {
    if !tags.iter().any(|tag| tag == memory_tag()) {
        tags.push(memory_tag().to_string());
    }

    let id = storage.create_record(title.clone(), body, "memory".to_string(), tags, false)?;
    Ok(MemoryImportReport { id, title })
}
