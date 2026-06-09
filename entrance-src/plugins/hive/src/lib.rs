mod advance;
mod audit;
mod claim;
mod dispatch;
mod engine;
mod evidence;
mod http;
mod kernel;
mod loop_control;
mod model;
mod policy;
mod preset;
mod review;
mod reviewer_semantics;
mod runner;
mod timeline;
mod view;
mod worker;

use anyhow::Result;
use entrance_core::{
    Bus, HiveRun, HiveRunCreate, Plugin, PluginContext, Scheduler, Store, Supervision,
};
use serde::{Deserialize, Serialize};

pub use advance::*;
pub use audit::*;
pub use claim::*;
pub use dispatch::DispatchSummary;
pub use engine::{EngineEvent, EngineReport};
pub use evidence::*;
pub use http::{HiveCallback, HiveCallbackRequest};
pub use model::*;
pub use policy::*;
pub use preset::{HivePreset, SoftwareEngPreset};
pub use review::{ReviewDecision, ReviewRecord};
pub use timeline::*;
pub use view::*;
pub use worker::*;

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

    pub fn loop_create(&self, request: HiveLoopCreateRequest) -> Result<HiveLoopReport> {
        kernel::create(&self.store, request)
    }

    pub fn loop_run(&self, request: HiveLoopRunRequest) -> Result<HiveLoopReport> {
        runner::run(&self.store, request)
    }

    pub fn loop_report(&self, id: i64) -> Result<HiveLoopReport> {
        kernel::report(&self.store, id)
    }

    pub fn loop_list(&self) -> Result<Vec<entrance_core::HiveLoopContract>> {
        kernel::list(&self.store)
    }

    pub fn policy_registry(&self) -> PolicyRegistryReport {
        policy::policy_registry()
    }

    pub fn loop_policies(&self, id: i64) -> Result<HiveLoopPolicyReport> {
        loop_control::policies(&self.store, id)
    }

    pub fn loop_trace(&self, id: i64) -> Result<HiveLoopTraceReport> {
        loop_control::trace(&self.store, id)
    }

    pub fn loop_evidence(&self, id: i64) -> Result<HiveLoopEvidenceReport> {
        loop_control::evidence_report(&self.store, id)
    }

    pub fn loop_evidence_drilldown(&self, id: i64) -> Result<HiveLoopEvidenceDrilldownReport> {
        loop_control::evidence_drilldown(&self.store, id)
    }

    pub fn loop_evidence_manifest(&self, id: i64) -> Result<HiveLoopEvidenceManifestReport> {
        loop_control::evidence_manifest(&self.store, id)
    }

    pub fn loop_audit(&self, id: i64) -> Result<HiveLoopAuditReport> {
        loop_control::audit(&self.store, id)
    }

    pub fn loop_doctor(&self, id: i64) -> Result<HiveLoopDoctorReport> {
        loop_control::doctor(&self.store, id)
    }

    pub fn loop_worker_lifecycle(&self, id: i64) -> Result<HiveLoopWorkerLifecycleReport> {
        loop_control::worker_lifecycle(&self.store, id)
    }

    pub fn loop_runtime_preflight(&self, id: i64) -> Result<HiveLoopRuntimePreflightReport> {
        loop_control::runtime_preflight(&self.store, id)
    }

    pub fn loop_dashboard(&self, id: i64) -> Result<HiveLoopDashboardReport> {
        loop_control::dashboard(&self.store, id)
    }

    pub fn panel(&self) -> Result<Vec<IssueCard>> {
        loop_control::panel(&self.store)
    }

    pub fn issue_report(&self, id: i64) -> Result<IssueCard> {
        loop_control::issue(&self.store, id)
    }

    pub fn issue_claim(&self, request: IssueClaimRequest) -> Result<IssueClaimReport> {
        claim::claim_issue(&self.store, request)
    }

    pub fn issue_timeline(&self, id: i64) -> Result<IssueTimelineReport> {
        loop_control::issue_timeline(&self.store, id)
    }

    pub fn issue_timeline_item(&self, id: i64, item_id: &str) -> Result<IssueTimelineItemReport> {
        loop_control::issue_timeline_item(&self.store, id, item_id)
    }

    pub fn issue_transition_policy(&self, id: i64) -> Result<IssueTransitionPolicyReport> {
        loop_control::issue_transition_policy(&self.store, id)
    }

    pub fn issue_comment(&self, request: IssueCommentRequest) -> Result<IssueCard> {
        kernel::add_comment(&self.store, request)
    }

    pub fn issue_decide(&self, request: IssueDecisionRequest) -> Result<IssueCard> {
        kernel::decide_issue(&self.store, request)
    }

    pub fn issue_run(&self, request: IssueRunRequest) -> Result<HiveLoopReport> {
        kernel::run_issue(&self.store, request)
    }

    pub fn issue_advance(&self, request: IssueAdvanceRequest) -> Result<IssueAdvanceReport> {
        advance::advance_issue(&self.store, request)
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
