use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{slugify, storage::DrawerStorage};
use entrance_core::{DrawerFilter, FileSystem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawerActionKind {
    Move,
    Create,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawerAction {
    pub kind: DrawerActionKind,
    pub entry_id: Option<i64>,
    pub summary: String,
    pub destination: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorganizationPlan {
    pub actions: Vec<DrawerAction>,
}

pub fn plan(storage: &DrawerStorage) -> Result<ReorganizationPlan> {
    let mut actions = Vec::new();
    for entry in storage.list(DrawerFilter::default())? {
        if entry.kind == "note" {
            continue;
        }

        if let Some(storage_path) = entry.storage_path {
            let path = PathBuf::from(&storage_path);
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("item");
            let destination = storage
                .root()
                .join(entry.kind.clone())
                .join(format!("{}.{}", slugify(&entry.title), extension));
            if destination != path {
                actions.push(DrawerAction {
                    kind: DrawerActionKind::Move,
                    entry_id: Some(entry.id),
                    summary: format!("move `{}` into {}", entry.title, entry.kind),
                    destination: Some(destination.display().to_string()),
                });
            }
        }
    }

    Ok(ReorganizationPlan { actions })
}

pub fn apply(storage: &DrawerStorage, fs: &FileSystem, plan: ReorganizationPlan) -> Result<usize> {
    let mut applied = 0usize;
    for action in plan.actions {
        match action.kind {
            DrawerActionKind::Move => {
                if let (Some(entry_id), Some(destination)) = (action.entry_id, action.destination) {
                    let destination = PathBuf::from(destination);
                    if let Some(parent) = destination.parent() {
                        fs.create_dir_all(parent)?;
                    }
                    storage.relocate(entry_id, destination)?;
                    applied += 1;
                }
            }
            DrawerActionKind::Create => {}
        }
    }
    Ok(applied)
}
