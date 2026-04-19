use anyhow::Result;
use entrance_core::{
    HiveRun, HiveRunCreate, Plugin, PluginContext, RoundState, Scheduler, Store, Supervision,
    TaskState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveDispatchRequest {
    pub title: String,
    pub project_dir: Option<String>,
    pub summary: Option<String>,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveDispatchReport {
    pub run: HiveRun,
    pub round: RoundState,
}

#[derive(Debug, Clone)]
pub struct HivePlugin {
    store: Store,
    scheduler: Scheduler,
    supervision: Supervision,
}

impl HivePlugin {
    pub fn new(ctx: &PluginContext) -> Self {
        Self {
            store: ctx.store(),
            scheduler: ctx.scheduler(),
            supervision: ctx.supervision(),
        }
    }

    pub fn dispatch(&self, request: HiveDispatchRequest) -> Result<HiveDispatchReport> {
        let mut round = self.scheduler.start();
        round = self
            .scheduler
            .checkpoint(round, TaskState::Running, "dispatch created");

        let id = self.store.insert_hive_run(HiveRunCreate {
            title: request.title,
            mode: "dispatch".to_string(),
            status: "running".to_string(),
            project_dir: request.project_dir,
            summary: request.summary,
            payload_json: request.payload_json,
        })?;

        self.store
            .update_hive_run_status(id, "ready", Some("queued in microkernel scheduler"))?;

        round = self
            .scheduler
            .checkpoint(round, TaskState::Done, "dispatch persisted");

        let run = self
            .list()?
            .into_iter()
            .find(|value| value.id == id)
            .expect("newly created run should exist");

        let _ = self.supervision.retry_policy();

        Ok(HiveDispatchReport { run, round })
    }

    pub fn list(&self) -> Result<Vec<HiveRun>> {
        self.store.list_hive_runs()
    }
}

impl Plugin for HivePlugin {
    fn name(&self) -> &'static str {
        "hive"
    }
}
