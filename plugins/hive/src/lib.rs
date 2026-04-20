mod dispatch;
mod engine;
mod http;
mod preset;
mod review;

use anyhow::Result;
use entrance_core::{
    Bus, HiveRun, HiveRunCreate, Plugin, PluginContext, Scheduler, Store, Supervision,
};
use serde::{Deserialize, Serialize};

pub use dispatch::DispatchSummary;
pub use engine::{EngineEvent, EngineReport};
pub use http::{HiveCallback, HiveCallbackRequest};
pub use preset::{HivePreset, SoftwareEngPreset};
pub use review::{ReviewDecision, ReviewRecord};

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
    pub round: entrance_core::RoundState,
    pub preset: String,
    pub engine: EngineReport,
}

#[derive(Debug, Clone)]
pub struct HivePlugin {
    store: Store,
    bus: Bus,
    scheduler: Scheduler,
    supervision: Supervision,
    preset: SoftwareEngPreset,
}

impl HivePlugin {
    pub fn new(ctx: &PluginContext) -> Self {
        Self {
            store: ctx.store(),
            bus: ctx.bus(),
            scheduler: ctx.scheduler(),
            supervision: ctx.supervision(),
            preset: SoftwareEngPreset,
        }
    }

    pub fn dispatch(&self, request: HiveDispatchRequest) -> Result<HiveDispatchReport> {
        dispatch::dispatch(
            &self.store,
            &self.bus,
            &self.scheduler,
            &self.supervision,
            &self.preset,
            request,
        )
    }

    pub fn list(&self) -> Result<Vec<HiveRun>> {
        self.store.list_hive_runs()
    }

    pub fn summary(&self) -> Result<DispatchSummary> {
        dispatch::summary(&self.store)
    }

    pub fn engine_report(&self, id: i64) -> Result<EngineReport> {
        engine::report(&self.store, id)
    }

    pub fn callback(&self, request: HiveCallbackRequest) -> Result<HiveCallback> {
        http::record_callback(&self.store, request)
    }

    pub fn review(&self, id: i64, decision: ReviewDecision) -> Result<ReviewRecord> {
        review::apply(&self.store, id, decision)
    }

    pub fn bootstrap_run(&self, row: HiveRunCreate) -> Result<i64> {
        self.store.insert_hive_run(row)
    }
}

impl Plugin for HivePlugin {
    fn name(&self) -> &'static str {
        "hive"
    }

    fn init(&self, _ctx: &PluginContext) -> Result<()> {
        for command in self.bus.recover_pending(Some("hive:run_task"))? {
            if let Some(id) = command.id {
                self.bus.acknowledge(id)?;
            }
        }
        Ok(())
    }
}
