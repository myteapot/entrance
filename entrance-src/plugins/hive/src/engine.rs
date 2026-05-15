use anyhow::{Context, Result};
use entrance_core::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    pub phase: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineReport {
    pub run_id: i64,
    pub command_id: Option<i64>,
    pub status: String,
    pub events: Vec<EngineEvent>,
}

pub fn build(
    run_id: i64,
    title: &str,
    project_dir: Option<&str>,
    summary: &str,
    command_id: Option<i64>,
) -> EngineReport {
    let mut events = vec![
        EngineEvent {
            phase: "prepare".to_string(),
            summary: format!("dispatch `{title}` admitted into hive engine"),
        },
        EngineEvent {
            phase: "schedule".to_string(),
            summary: "task attached to scheduler round 1".to_string(),
        },
    ];

    if let Some(project_dir) = project_dir {
        events.push(EngineEvent {
            phase: "context".to_string(),
            summary: format!("project context set to {project_dir}"),
        });
    }

    if let Some(command_id) = command_id {
        events.push(EngineEvent {
            phase: "bus".to_string(),
            summary: format!("command persisted on bus as #{command_id}"),
        });
    }

    events.push(EngineEvent {
        phase: "review".to_string(),
        summary: summary.to_string(),
    });

    EngineReport {
        run_id,
        command_id,
        status: "ready".to_string(),
        events,
    }
}

pub fn report(store: &Store, id: i64) -> Result<EngineReport> {
    let run = store
        .get_hive_run(id)?
        .with_context(|| format!("unknown hive run `{id}`"))?;
    Ok(build(
        run.id,
        &run.title,
        run.project_dir.as_deref(),
        run.summary
            .as_deref()
            .unwrap_or("review surface not initialized"),
        None,
    ))
}
