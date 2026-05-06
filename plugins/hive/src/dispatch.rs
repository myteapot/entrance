use anyhow::Result;
use entrance_core::{Bus, Scheduler, Store, Supervision, TaskState};
use serde::{Deserialize, Serialize};

use crate::{engine, preset::HivePreset, HiveDispatchReport, HiveDispatchRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchSummary {
    pub total_runs: usize,
    pub ready_runs: usize,
    pub returned_runs: usize,
}

pub fn dispatch(
    store: &Store,
    bus: &Bus,
    scheduler: &Scheduler,
    supervision: &Supervision,
    preset: &impl HivePreset,
    request: HiveDispatchRequest,
) -> Result<HiveDispatchReport> {
    let mut round = scheduler.start();
    round = scheduler.checkpoint(round, TaskState::Running, "dispatch created");

    let fallback_summary = preset.default_summary(&request.title);
    let payload_json = if request.payload_json.trim().is_empty() || request.payload_json == "{}" {
        serde_json::to_string(&preset.default_payload())?
    } else {
        request.payload_json
    };

    let request_title = request.title.clone();
    let request_project_dir = request.project_dir.clone();
    let request_summary = request.summary.clone();
    let payload_json_owned = payload_json.clone();

    let (id, command) = supervision.run(|| {
        let id = store.insert_hive_run(entrance_core::HiveRunCreate {
            title: request_title.clone(),
            mode: "dispatch".to_string(),
            status: "running".to_string(),
            project_dir: request_project_dir.clone(),
            summary: Some(
                request_summary
                    .clone()
                    .unwrap_or_else(|| fallback_summary.clone()),
            ),
            payload_json: payload_json_owned.clone(),
        })?;

        let command = bus.dispatch(
            "hive:run_task",
            serde_json::json!({
                "run_id": id,
                "title": request_title.clone(),
                "project_dir": request_project_dir.clone(),
                "summary": request_summary.clone().unwrap_or_else(|| fallback_summary.clone()),
            }),
        )?;
        Ok((id, command))
    })?;

    let engine = engine::build(
        id,
        &request.title,
        request.project_dir.as_deref(),
        request
            .summary
            .as_deref()
            .unwrap_or(fallback_summary.as_str()),
        command.id,
    );

    store.update_hive_run_status(id, "ready", Some("queued in microkernel scheduler"))?;
    if let Some(command_id) = command.id {
        bus.acknowledge(command_id)?;
    }

    round = scheduler.checkpoint(round, TaskState::Done, "dispatch persisted");

    let run = store
        .get_hive_run(id)?
        .expect("newly created run should exist");

    Ok(HiveDispatchReport {
        run,
        round,
        preset: preset.name().to_string(),
        engine,
    })
}

pub fn summary(store: &Store) -> Result<DispatchSummary> {
    let runs = store.list_hive_runs()?;
    Ok(DispatchSummary {
        total_runs: runs.len(),
        ready_runs: runs.iter().filter(|run| run.status == "ready").count(),
        returned_runs: runs.iter().filter(|run| run.status == "returned").count(),
    })
}
