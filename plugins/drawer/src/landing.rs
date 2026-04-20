use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::storage::DrawerStorage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingImportReport {
    pub id: i64,
    pub source_path: String,
    pub tags: Vec<String>,
}

pub fn import_path(
    storage: &DrawerStorage,
    source: PathBuf,
    tags: Vec<String>,
) -> Result<LandingImportReport> {
    let source_path = source.display().to_string();
    let id = storage.import_path(source, "import", tags.clone(), false)?;
    Ok(LandingImportReport {
        id,
        source_path,
        tags,
    })
}
