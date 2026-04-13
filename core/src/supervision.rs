use serde::{Deserialize, Serialize};

use crate::core::data_store::{
    BudgetLedgerEntry, DataStore, StoredForgeTask, StoredNotaRuntimeAllocation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionStrategy {
    OneForOne,
    RestForOne,
    OneForAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    Permanent,
    Transient,
    Temporary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureVisibility {
    StatusOnly,
    StatusAndLog,
    StatusLogAndEscalation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionScope {
    AgentProcess,
    DispatchPipeline,
    SessionBundle,
    ConnectorWorker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum RuntimeChildState {
    Pending,
    Running,
    Retrying,
    Degraded,
    Blocked,
    Failed,
    Cancelled,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionSignalFamily {
    ExecutionFailure,
    AdmissionRejection,
    VerdictReturn,
    Integrity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorAction {
    ObserveChild,
    RouteReturn,
    RestartChild,
    ReplaceChild,
    BlockLineage,
    QuarantineLineage,
    EscalateUp,
    SurfaceIncident,
}

impl SupervisionSignalFamily {
    pub const fn ledger_key(self) -> &'static str {
        match self {
            Self::ExecutionFailure => "execution_failure",
            Self::AdmissionRejection => "admission_rejection",
            Self::VerdictReturn => "verdict_return",
            Self::Integrity => "integrity",
        }
    }
}

impl SupervisorAction {
    pub const fn ledger_key(self) -> &'static str {
        match self {
            Self::ObserveChild => "observe_child",
            Self::RouteReturn => "route_return",
            Self::RestartChild => "restart_child",
            Self::ReplaceChild => "replace_child",
            Self::BlockLineage => "block_lineage",
            Self::QuarantineLineage => "quarantine_lineage",
            Self::EscalateUp => "escalate_up",
            Self::SurfaceIncident => "surface_incident",
        }
    }

    pub fn from_ledger_key(value: &str) -> Option<Self> {
        match value {
            "observe_child" => Some(Self::ObserveChild),
            "route_return" => Some(Self::RouteReturn),
            "restart_child" => Some(Self::RestartChild),
            "replace_child" => Some(Self::ReplaceChild),
            "block_lineage" => Some(Self::BlockLineage),
            "quarantine_lineage" => Some(Self::QuarantineLineage),
            "escalate_up" => Some(Self::EscalateUp),
            "surface_incident" => Some(Self::SurfaceIncident),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SupervisorActionResolution {
    pub action: SupervisorAction,
    pub budget_exhausted: bool,
    pub attempt_number: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupervisionSignal {
    pub family: SupervisionSignalFamily,
    pub source_allocation_id: Option<i64>,
    pub source_task_id: Option<i64>,
    pub error_code: Option<String>,
    pub summary: String,
    pub timestamp: String,
}

impl SupervisionSignal {
    pub fn family(&self) -> SupervisionSignalFamily {
        self.family
    }
}

pub enum SupervisionEvent<'a> {
    TaskStateChange {
        allocation: &'a StoredNotaRuntimeAllocation,
        task: &'a StoredForgeTask,
    },
    AllocationStateChange {
        allocation: &'a StoredNotaRuntimeAllocation,
    },
    // AdmissionResult { ... }, -- M6.1 backfill
    // VerdictResult { ... },   -- M6.2 backfill
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSupervisionProjection {
    pub current_supervision_state: RuntimeChildState,
    pub retry_count: u8,
    pub max_restarts: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_signal_family: Option<SupervisionSignalFamily>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_code: Option<String>,
    pub last_supervisor_action: SupervisorAction,
    pub escalation_pending: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSupervisionIncidentSummary {
    pub retry_count: u8,
    pub max_restarts: u8,
    pub last_supervisor_action: SupervisorAction,
    pub budget_exhausted: bool,
    pub ledger_entry_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_budget_entry: Option<BudgetLedgerEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RetryBudget {
    pub max_restarts: u8,
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SupervisionPolicy {
    pub scope: SupervisionScope,
    pub strategy: SupervisionStrategy,
    pub restart_policy: RestartPolicy,
    pub retry_budget: RetryBudget,
    pub failure_visibility: FailureVisibility,
}

impl SupervisionPolicy {
    pub const fn requires_escalation_after(self, retry_count: u8) -> bool {
        retry_count >= self.retry_budget.max_restarts
    }

    pub const fn visible_failure(self) -> bool {
        match self.failure_visibility {
            FailureVisibility::StatusOnly
            | FailureVisibility::StatusAndLog
            | FailureVisibility::StatusLogAndEscalation => true,
        }
    }
}

pub const DEFAULT_AGENT_PROCESS_POLICY: SupervisionPolicy = SupervisionPolicy {
    scope: SupervisionScope::AgentProcess,
    strategy: SupervisionStrategy::OneForOne,
    restart_policy: RestartPolicy::Transient,
    retry_budget: RetryBudget {
        max_restarts: 3,
        window_seconds: 300,
    },
    failure_visibility: FailureVisibility::StatusLogAndEscalation,
};

pub const DEFAULT_DISPATCH_PIPELINE_POLICY: SupervisionPolicy = SupervisionPolicy {
    scope: SupervisionScope::DispatchPipeline,
    strategy: SupervisionStrategy::RestForOne,
    restart_policy: RestartPolicy::Transient,
    retry_budget: RetryBudget {
        max_restarts: 2,
        window_seconds: 120,
    },
    failure_visibility: FailureVisibility::StatusLogAndEscalation,
};

pub const DEFAULT_SESSION_BUNDLE_POLICY: SupervisionPolicy = SupervisionPolicy {
    scope: SupervisionScope::SessionBundle,
    strategy: SupervisionStrategy::OneForAll,
    restart_policy: RestartPolicy::Transient,
    retry_budget: RetryBudget {
        max_restarts: 1,
        window_seconds: 60,
    },
    failure_visibility: FailureVisibility::StatusLogAndEscalation,
};

pub fn classify_signal(event: SupervisionEvent<'_>) -> SupervisionSignal {
    let timestamp = chrono::Utc::now().to_rfc3339();

    match event {
        SupervisionEvent::TaskStateChange { allocation, task } => {
            let status = task.status.as_str();
            match status {
                "Done" | "return_ready" => SupervisionSignal {
                    family: SupervisionSignalFamily::VerdictReturn,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: Some(task.id),
                    error_code: None,
                    summary: format!(
                        "Task {} completed, allocation {} ready for return routing.",
                        task.id, allocation.id
                    ),
                    timestamp,
                },
                "Failed" | "escalated_failed" => SupervisionSignal {
                    family: SupervisionSignalFamily::ExecutionFailure,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: Some(task.id),
                    error_code: task
                        .status_message
                        .clone()
                        .or_else(|| Some(allocation.status.clone())),
                    summary: format!(
                        "Task {} failed under allocation {}.",
                        task.id, allocation.id
                    ),
                    timestamp,
                },
                "Blocked" | "escalated_blocked" => SupervisionSignal {
                    family: SupervisionSignalFamily::ExecutionFailure,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: Some(task.id),
                    error_code: task
                        .status_message
                        .clone()
                        .or_else(|| Some(allocation.status.clone())),
                    summary: format!(
                        "Task {} blocked under allocation {}.",
                        task.id, allocation.id
                    ),
                    timestamp,
                },
                "Cancelled" | "escalated_cancelled" => SupervisionSignal {
                    family: SupervisionSignalFamily::Integrity,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: Some(task.id),
                    error_code: task
                        .status_message
                        .clone()
                        .or_else(|| Some(allocation.status.clone())),
                    summary: format!(
                        "Task {} cancelled under allocation {}.",
                        task.id, allocation.id
                    ),
                    timestamp,
                },
                _ => SupervisionSignal {
                    family: SupervisionSignalFamily::VerdictReturn,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: Some(task.id),
                    error_code: None,
                    summary: format!(
                        "Task {} in state '{}' under allocation {}.",
                        task.id, status, allocation.id
                    ),
                    timestamp,
                },
            }
        }
        SupervisionEvent::AllocationStateChange { allocation } => {
            let status = allocation.status.as_str();
            match status {
                "return_ready" | "Done" => SupervisionSignal {
                    family: SupervisionSignalFamily::VerdictReturn,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: None,
                    error_code: None,
                    summary: format!("Allocation {} ready for return.", allocation.id),
                    timestamp,
                },
                "Failed" | "escalated_failed" | "Blocked" | "escalated_blocked" => {
                    SupervisionSignal {
                        family: SupervisionSignalFamily::ExecutionFailure,
                        source_allocation_id: Some(allocation.id),
                        source_task_id: None,
                        error_code: Some(allocation.status.clone()),
                        summary: format!(
                            "Allocation {} entered failure state '{}'.",
                            allocation.id, status
                        ),
                        timestamp,
                    }
                }
                "Cancelled" | "escalated_cancelled" => SupervisionSignal {
                    family: SupervisionSignalFamily::Integrity,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: None,
                    error_code: Some(allocation.status.clone()),
                    summary: format!("Allocation {} cancelled.", allocation.id),
                    timestamp,
                },
                _ => SupervisionSignal {
                    family: SupervisionSignalFamily::VerdictReturn,
                    source_allocation_id: Some(allocation.id),
                    source_task_id: None,
                    error_code: None,
                    summary: format!("Allocation {} in state '{}'.", allocation.id, status),
                    timestamp,
                },
            }
        }
    }
}

pub fn derive_runtime_supervision_projection(
    allocation: &StoredNotaRuntimeAllocation,
    task: Option<&StoredForgeTask>,
    consumed_attempts: u8,
) -> RuntimeSupervisionProjection {
    derive_runtime_supervision_projection_with_attempts(allocation, task, consumed_attempts)
}

pub fn derive_runtime_supervision_projection_with_budget(
    allocation: &StoredNotaRuntimeAllocation,
    task: Option<&StoredForgeTask>,
    data_store: &DataStore,
) -> RuntimeSupervisionProjection {
    let signal = match task {
        Some(task) => classify_signal(SupervisionEvent::TaskStateChange { allocation, task }),
        None => classify_signal(SupervisionEvent::AllocationStateChange { allocation }),
    };
    let budget_signal_family = budget_consumption_signal_family(signal.family());
    let consumed_attempts = data_store
        .get_budget_consumption_count(allocation.id, budget_signal_family.ledger_key())
        .unwrap_or_default();

    derive_runtime_supervision_projection_with_attempts(allocation, task, consumed_attempts)
}

fn derive_runtime_supervision_projection_with_attempts(
    allocation: &StoredNotaRuntimeAllocation,
    task: Option<&StoredForgeTask>,
    consumed_attempts: u8,
) -> RuntimeSupervisionProjection {
    let current_status = task
        .map(|task| task.status.as_str())
        .unwrap_or(allocation.status.as_str());

    let signal = match task {
        Some(task) => classify_signal(SupervisionEvent::TaskStateChange { allocation, task }),
        None => classify_signal(SupervisionEvent::AllocationStateChange { allocation }),
    };

    build_projection_from_signal(
        &signal,
        supervision_policy_for_allocation(allocation),
        consumed_attempts,
        current_status,
    )
}

pub fn resolve_supervisor_action(
    signal: &SupervisionSignal,
    policy: &SupervisionPolicy,
    consumed_attempts: u8,
) -> SupervisorActionResolution {
    match signal.family {
        SupervisionSignalFamily::VerdictReturn => SupervisorActionResolution {
            action: SupervisorAction::RouteReturn,
            budget_exhausted: false,
            attempt_number: 0,
        },
        SupervisionSignalFamily::ExecutionFailure => {
            let next_attempt = consumed_attempts.saturating_add(1);
            if next_attempt > policy.retry_budget.max_restarts {
                SupervisorActionResolution {
                    action: SupervisorAction::EscalateUp,
                    budget_exhausted: true,
                    attempt_number: next_attempt,
                }
            } else {
                let action = match policy.restart_policy {
                    RestartPolicy::Permanent | RestartPolicy::Transient => {
                        SupervisorAction::RestartChild
                    }
                    RestartPolicy::Temporary => SupervisorAction::EscalateUp,
                };
                SupervisorActionResolution {
                    action,
                    budget_exhausted: false,
                    attempt_number: next_attempt,
                }
            }
        }
        SupervisionSignalFamily::AdmissionRejection => SupervisorActionResolution {
            action: SupervisorAction::BlockLineage,
            budget_exhausted: false,
            attempt_number: 0,
        },
        SupervisionSignalFamily::Integrity => SupervisorActionResolution {
            action: SupervisorAction::SurfaceIncident,
            budget_exhausted: true,
            attempt_number: 1,
        },
    }
}

fn build_projection_from_signal(
    signal: &SupervisionSignal,
    policy: SupervisionPolicy,
    consumed_attempts: u8,
    current_status: &str,
) -> RuntimeSupervisionProjection {
    let resolution = resolve_supervisor_action(signal, &policy, consumed_attempts);
    let retry_count = projected_retry_count(signal.family, consumed_attempts, resolution);
    let last_supervisor_action = projected_last_supervisor_action(
        signal.family,
        current_status,
        consumed_attempts,
        resolution.action,
    );

    match signal.family {
        SupervisionSignalFamily::VerdictReturn => match current_status {
            "Done" | "return_ready" => RuntimeSupervisionProjection {
                current_supervision_state: RuntimeChildState::Done,
                retry_count,
                max_restarts: policy.retry_budget.max_restarts,
                last_failure_signal_family: None,
                last_failure_code: None,
                last_supervisor_action,
                escalation_pending: supervisor_action_requires_escalation(resolution.action),
                summary: signal.summary.clone(),
            },
            "Running" => RuntimeSupervisionProjection {
                current_supervision_state: RuntimeChildState::Running,
                retry_count,
                max_restarts: policy.retry_budget.max_restarts,
                last_failure_signal_family: None,
                last_failure_code: None,
                last_supervisor_action,
                escalation_pending: supervisor_action_requires_escalation(resolution.action),
                summary: signal.summary.clone(),
            },
            _ => RuntimeSupervisionProjection {
                current_supervision_state: RuntimeChildState::Pending,
                retry_count,
                max_restarts: policy.retry_budget.max_restarts,
                last_failure_signal_family: None,
                last_failure_code: None,
                last_supervisor_action,
                escalation_pending: supervisor_action_requires_escalation(resolution.action),
                summary: signal.summary.clone(),
            },
        },
        SupervisionSignalFamily::ExecutionFailure => RuntimeSupervisionProjection {
            current_supervision_state: execution_failure_state(current_status, resolution.action),
            retry_count,
            max_restarts: policy.retry_budget.max_restarts,
            last_failure_signal_family: Some(SupervisionSignalFamily::ExecutionFailure),
            last_failure_code: signal.error_code.clone(),
            last_supervisor_action,
            escalation_pending: supervisor_action_requires_escalation(resolution.action),
            summary: signal.summary.clone(),
        },
        SupervisionSignalFamily::Integrity => RuntimeSupervisionProjection {
            current_supervision_state: RuntimeChildState::Cancelled,
            retry_count,
            max_restarts: policy.retry_budget.max_restarts,
            last_failure_signal_family: Some(SupervisionSignalFamily::Integrity),
            last_failure_code: signal.error_code.clone(),
            last_supervisor_action,
            escalation_pending: supervisor_action_requires_escalation(resolution.action),
            summary: signal.summary.clone(),
        },
        SupervisionSignalFamily::AdmissionRejection => RuntimeSupervisionProjection {
            current_supervision_state: RuntimeChildState::Blocked,
            retry_count,
            max_restarts: policy.retry_budget.max_restarts,
            last_failure_signal_family: Some(SupervisionSignalFamily::AdmissionRejection),
            last_failure_code: signal.error_code.clone(),
            last_supervisor_action,
            escalation_pending: supervisor_action_requires_escalation(resolution.action),
            summary: signal.summary.clone(),
        },
    }
}

pub fn build_runtime_supervision_incident_summary(
    projection: &RuntimeSupervisionProjection,
    budget_ledger: &[BudgetLedgerEntry],
) -> Option<RuntimeSupervisionIncidentSummary> {
    if projection.retry_count == 0 && !projection.escalation_pending {
        return None;
    }

    let last_budget_entry = budget_ledger.last().cloned();
    let last_supervisor_action = last_budget_entry
        .as_ref()
        .and_then(|entry| SupervisorAction::from_ledger_key(entry.action_taken.as_str()))
        .unwrap_or(projection.last_supervisor_action);
    let budget_exhausted = last_budget_entry
        .as_ref()
        .map(|entry| entry.exhausted)
        .unwrap_or(projection.escalation_pending);

    Some(RuntimeSupervisionIncidentSummary {
        retry_count: projection.retry_count,
        max_restarts: projection.max_restarts,
        last_supervisor_action,
        budget_exhausted,
        ledger_entry_count: budget_ledger.len(),
        last_budget_entry,
    })
}

fn execution_failure_state(current_status: &str, action: SupervisorAction) -> RuntimeChildState {
    match action {
        SupervisorAction::RestartChild | SupervisorAction::ReplaceChild => {
            RuntimeChildState::Retrying
        }
        SupervisorAction::BlockLineage => RuntimeChildState::Blocked,
        SupervisorAction::EscalateUp => match current_status {
            "Blocked" | "escalated_blocked" => RuntimeChildState::Blocked,
            _ => RuntimeChildState::Failed,
        },
        _ => RuntimeChildState::Failed,
    }
}

fn supervisor_action_requires_escalation(action: SupervisorAction) -> bool {
    matches!(
        action,
        SupervisorAction::EscalateUp | SupervisorAction::SurfaceIncident
    )
}

fn budget_consumption_signal_family(
    signal_family: SupervisionSignalFamily,
) -> SupervisionSignalFamily {
    match signal_family {
        SupervisionSignalFamily::VerdictReturn | SupervisionSignalFamily::ExecutionFailure => {
            SupervisionSignalFamily::ExecutionFailure
        }
        SupervisionSignalFamily::AdmissionRejection => SupervisionSignalFamily::AdmissionRejection,
        SupervisionSignalFamily::Integrity => SupervisionSignalFamily::Integrity,
    }
}

fn projected_retry_count(
    signal_family: SupervisionSignalFamily,
    consumed_attempts: u8,
    resolution: SupervisorActionResolution,
) -> u8 {
    match signal_family {
        SupervisionSignalFamily::VerdictReturn if consumed_attempts > 0 => {
            consumed_attempts.saturating_add(1)
        }
        _ => resolution.attempt_number,
    }
}

fn projected_last_supervisor_action(
    signal_family: SupervisionSignalFamily,
    current_status: &str,
    consumed_attempts: u8,
    resolved_action: SupervisorAction,
) -> SupervisorAction {
    if matches!(signal_family, SupervisionSignalFamily::VerdictReturn)
        && consumed_attempts > 0
        && !matches!(current_status, "Done" | "return_ready")
    {
        SupervisorAction::RestartChild
    } else {
        resolved_action
    }
}

pub(crate) fn supervision_policy_for_allocation(
    allocation: &StoredNotaRuntimeAllocation,
) -> SupervisionPolicy {
    match allocation.allocation_kind.as_str() {
        "forge_dev_dispatch" => DEFAULT_DISPATCH_PIPELINE_POLICY,
        _ => DEFAULT_AGENT_PROCESS_POLICY,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_signal, derive_runtime_supervision_projection,
        derive_runtime_supervision_projection_with_budget, resolve_supervisor_action,
        FailureVisibility, RestartPolicy, RuntimeChildState, SupervisionEvent, SupervisionScope,
        SupervisionSignal, SupervisionSignalFamily, SupervisionStrategy, SupervisorAction,
        DEFAULT_AGENT_PROCESS_POLICY, DEFAULT_DISPATCH_PIPELINE_POLICY,
        DEFAULT_SESSION_BUNDLE_POLICY,
    };
    use crate::core::data_store::{
        DataStore, MigrationPlan, NewNotaRuntimeAllocation, NewNotaRuntimeTransaction,
        StoredForgeTask, StoredNotaRuntimeAllocation,
    };
    use crate::core::nota::{sync_runtime_truth, NotaDoAllocationPayload};

    #[test]
    fn default_agent_process_policy_matches_otp_style_intent() {
        assert_eq!(
            DEFAULT_AGENT_PROCESS_POLICY.strategy,
            SupervisionStrategy::OneForOne
        );
        assert_eq!(
            DEFAULT_AGENT_PROCESS_POLICY.restart_policy,
            RestartPolicy::Transient
        );
        assert_eq!(DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts, 3);
        assert!(DEFAULT_AGENT_PROCESS_POLICY.visible_failure());
    }

    #[test]
    fn dispatch_pipeline_prefers_rest_for_one() {
        assert_eq!(
            DEFAULT_DISPATCH_PIPELINE_POLICY.scope,
            SupervisionScope::DispatchPipeline
        );
        assert_eq!(
            DEFAULT_DISPATCH_PIPELINE_POLICY.strategy,
            SupervisionStrategy::RestForOne
        );
    }

    #[test]
    fn session_bundle_prefers_one_for_all() {
        assert_eq!(
            DEFAULT_SESSION_BUNDLE_POLICY.scope,
            SupervisionScope::SessionBundle
        );
        assert_eq!(
            DEFAULT_SESSION_BUNDLE_POLICY.strategy,
            SupervisionStrategy::OneForAll
        );
    }

    #[test]
    fn retry_budget_forces_escalation_after_threshold() {
        assert!(!DEFAULT_AGENT_PROCESS_POLICY.requires_escalation_after(2));
        assert!(DEFAULT_AGENT_PROCESS_POLICY.requires_escalation_after(3));
    }

    #[test]
    fn runtime_child_state_exposes_retrying_and_degraded() {
        assert_eq!(RuntimeChildState::Retrying, RuntimeChildState::Retrying);
        assert_eq!(RuntimeChildState::Degraded, RuntimeChildState::Degraded);
    }

    #[test]
    fn visibility_model_has_no_silent_failure_variant() {
        let options = [
            FailureVisibility::StatusOnly,
            FailureVisibility::StatusAndLog,
            FailureVisibility::StatusLogAndEscalation,
        ];
        assert_eq!(options.len(), 3);
    }

    #[test]
    fn runtime_supervision_projects_blocked_allocation_to_blocked_state() {
        let allocation = sample_allocation("escalated_blocked");
        let task = sample_task("Blocked", Some("missing token"));

        let projection = derive_runtime_supervision_projection(&allocation, Some(&task), 0);
        assert_eq!(
            projection.current_supervision_state,
            RuntimeChildState::Retrying
        );
        assert_eq!(projection.retry_count, 1);
        assert_eq!(projection.max_restarts, 3);
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::RestartChild
        );
        assert!(!projection.escalation_pending);
    }

    #[test]
    fn runtime_supervision_projects_return_ready_allocation_to_route_return() {
        let allocation = sample_allocation("return_ready");
        let projection = derive_runtime_supervision_projection(&allocation, None, 0);
        assert_eq!(
            projection.current_supervision_state,
            RuntimeChildState::Done
        );
        assert_eq!(projection.max_restarts, 3);
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::RouteReturn
        );
        assert!(!projection.escalation_pending);
    }

    #[test]
    fn projection_with_budget_uses_real_count() -> anyhow::Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(
            crate::hosts::plugins::forge::migrations(),
        ))?;
        let allocation = insert_allocation(&store, "task_created")?;
        store.record_budget_consumption(
            allocation.id,
            SupervisionSignalFamily::ExecutionFailure.ledger_key(),
            1,
            SupervisorAction::RestartChild.ledger_key(),
            DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts,
            2,
            false,
            Some("first restart"),
        )?;
        store.record_budget_consumption(
            allocation.id,
            SupervisionSignalFamily::ExecutionFailure.ledger_key(),
            2,
            SupervisorAction::RestartChild.ledger_key(),
            DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts,
            1,
            false,
            Some("second restart"),
        )?;

        let task = sample_task("Blocked", Some("flaky integration"));
        let projection =
            derive_runtime_supervision_projection_with_budget(&allocation, Some(&task), &store);

        assert_eq!(projection.retry_count, 3);
        assert_eq!(projection.max_restarts, 3);
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::RestartChild
        );
        assert!(!projection.escalation_pending);

        Ok(())
    }

    #[test]
    fn projection_with_zero_budget_matches_default() -> anyhow::Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(
            crate::hosts::plugins::forge::migrations(),
        ))?;
        let allocation = insert_allocation(&store, "task_created")?;
        let task = sample_task("Blocked", Some("flaky integration"));

        let baseline = derive_runtime_supervision_projection(&allocation, Some(&task), 0);
        let with_budget =
            derive_runtime_supervision_projection_with_budget(&allocation, Some(&task), &store);

        assert_eq!(
            with_budget.current_supervision_state,
            baseline.current_supervision_state
        );
        assert_eq!(with_budget.retry_count, baseline.retry_count);
        assert_eq!(with_budget.max_restarts, baseline.max_restarts);
        assert_eq!(
            with_budget.last_supervisor_action,
            baseline.last_supervisor_action
        );
        assert_eq!(with_budget.escalation_pending, baseline.escalation_pending);

        Ok(())
    }

    #[test]
    fn escalation_pending_when_budget_exhausted() -> anyhow::Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(
            crate::hosts::plugins::forge::migrations(),
        ))?;
        let allocation = insert_allocation(&store, "task_created")?;
        for attempt in 1..=DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts {
            store.record_budget_consumption(
                allocation.id,
                SupervisionSignalFamily::ExecutionFailure.ledger_key(),
                attempt,
                SupervisorAction::RestartChild.ledger_key(),
                DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts,
                DEFAULT_AGENT_PROCESS_POLICY
                    .retry_budget
                    .max_restarts
                    .saturating_sub(attempt),
                false,
                Some("retry consumed"),
            )?;
        }

        let task = sample_task("Failed", Some("still failing"));
        let projection =
            derive_runtime_supervision_projection_with_budget(&allocation, Some(&task), &store);

        assert!(projection.escalation_pending);
        assert_eq!(projection.retry_count, 4);
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::EscalateUp
        );

        Ok(())
    }

    #[test]
    fn restart_child_creates_new_task_and_consumes_budget() -> anyhow::Result<()> {
        let _guard = crate::test_env_guard();
        let store = DataStore::in_memory(MigrationPlan::new(
            crate::hosts::plugins::forge::migrations(),
        ))?;
        let (transaction_id, allocation, failed_task_id) =
            insert_terminal_runtime_allocation(&store, "Failed", Some("agent crashed"))?;

        sync_runtime_truth(&store, Some(transaction_id))?;

        let failed_task = store
            .get_forge_task(failed_task_id)?
            .expect("original failed task should remain addressable");
        assert_eq!(failed_task.status, "Cancelled");

        let tasks = store.list_forge_tasks()?;
        assert_eq!(tasks.len(), 2);
        let restarted_task = tasks
            .into_iter()
            .find(|task| task.id != failed_task_id)
            .expect("restarted child task should exist");
        assert_eq!(restarted_task.status, "Pending");
        assert_eq!(restarted_task.command, failed_task.command);
        assert_eq!(restarted_task.args, failed_task.args);
        assert_eq!(restarted_task.working_dir, failed_task.working_dir);
        assert_eq!(restarted_task.stdin_text, failed_task.stdin_text);
        assert_eq!(restarted_task.required_tokens, failed_task.required_tokens);
        assert_eq!(restarted_task.metadata, failed_task.metadata);

        let stored_allocation = store
            .list_nota_runtime_allocations()?
            .into_iter()
            .find(|candidate| candidate.id == allocation.id)
            .expect("allocation should still exist after restart");
        assert_eq!(stored_allocation.status, "task_created");
        assert_eq!(
            stored_allocation.child_execution_ref,
            restarted_task.id.to_string()
        );
        let payload: NotaDoAllocationPayload =
            serde_json::from_str(&stored_allocation.payload_json)?;
        assert!(payload.terminal_outcome.is_none());
        assert_eq!(payload.repair_of_allocation_id, Some(allocation.id));

        let transaction = store
            .get_nota_runtime_transaction(transaction_id)?
            .expect("source transaction should still exist");
        assert_eq!(transaction.forge_task_id, Some(restarted_task.id));

        let ledger = store.list_budget_ledger(allocation.id)?;
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].signal_family, "execution_failure");
        assert_eq!(ledger[0].attempt_number, 1);
        assert_eq!(ledger[0].action_taken, "restart_child");
        assert_eq!(
            ledger[0].budget_max,
            DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts
        );
        assert_eq!(ledger[0].budget_remaining, 2);
        assert!(!ledger[0].exhausted);

        Ok(())
    }

    #[test]
    fn restart_child_respects_budget_exhaustion() -> anyhow::Result<()> {
        let _guard = crate::test_env_guard();
        let store = DataStore::in_memory(MigrationPlan::new(
            crate::hosts::plugins::forge::migrations(),
        ))?;
        let (transaction_id, allocation, failed_task_id) =
            insert_terminal_runtime_allocation(&store, "Failed", Some("still failing"))?;
        for attempt in 1..=DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts {
            store.record_budget_consumption(
                allocation.id,
                SupervisionSignalFamily::ExecutionFailure.ledger_key(),
                attempt,
                SupervisorAction::RestartChild.ledger_key(),
                DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts,
                DEFAULT_AGENT_PROCESS_POLICY
                    .retry_budget
                    .max_restarts
                    .saturating_sub(attempt),
                false,
                Some("retry consumed"),
            )?;
        }

        sync_runtime_truth(&store, Some(transaction_id))?;

        let tasks = store.list_forge_tasks()?;
        assert_eq!(tasks.len(), 1);
        let failed_task = store
            .get_forge_task(failed_task_id)?
            .expect("failed task should still exist");
        assert_eq!(failed_task.status, "Failed");

        let stored_allocation = store
            .list_nota_runtime_allocations()?
            .into_iter()
            .find(|candidate| candidate.id == allocation.id)
            .expect("allocation should still exist after escalation");
        assert_eq!(stored_allocation.status, "escalated_failed");
        assert_eq!(
            stored_allocation.child_execution_ref,
            failed_task_id.to_string()
        );
        let payload: NotaDoAllocationPayload =
            serde_json::from_str(&stored_allocation.payload_json)?;
        let outcome = payload
            .terminal_outcome
            .expect("terminal outcome should persist when budget is exhausted");
        assert_eq!(outcome.child_execution_status, "Failed");
        assert_eq!(
            outcome.child_execution_status_message.as_deref(),
            Some("still failing")
        );

        let projection = derive_runtime_supervision_projection_with_budget(
            &stored_allocation,
            Some(&failed_task),
            &store,
        );
        assert!(projection.escalation_pending);
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::EscalateUp
        );

        let ledger = store.list_budget_ledger(allocation.id)?;
        assert_eq!(
            ledger.len() as u8,
            DEFAULT_AGENT_PROCESS_POLICY.retry_budget.max_restarts
        );

        Ok(())
    }

    #[test]
    fn classify_signal_failed_task_produces_execution_failure() {
        let allocation = sample_allocation("escalated_failed");
        let task = sample_task("Failed", Some("exit code 1"));

        let signal = classify_signal(SupervisionEvent::TaskStateChange {
            allocation: &allocation,
            task: &task,
        });

        assert_eq!(signal.family(), SupervisionSignalFamily::ExecutionFailure);
        assert_eq!(signal.source_allocation_id, Some(allocation.id));
        assert_eq!(signal.source_task_id, Some(task.id));
    }

    #[test]
    fn classify_signal_blocked_task_produces_execution_failure() {
        let allocation = sample_allocation("escalated_blocked");
        let task = sample_task("Blocked", Some("missing token"));

        let signal = classify_signal(SupervisionEvent::TaskStateChange {
            allocation: &allocation,
            task: &task,
        });

        assert_eq!(signal.family(), SupervisionSignalFamily::ExecutionFailure);
        assert_eq!(signal.source_allocation_id, Some(allocation.id));
        assert_eq!(signal.source_task_id, Some(task.id));
    }

    #[test]
    fn classify_signal_cancelled_allocation_produces_integrity() {
        let allocation = sample_allocation("Cancelled");

        let signal = classify_signal(SupervisionEvent::AllocationStateChange {
            allocation: &allocation,
        });

        assert_eq!(signal.family(), SupervisionSignalFamily::Integrity);
        assert_eq!(signal.source_allocation_id, Some(allocation.id));
        assert_eq!(signal.source_task_id, None);
    }

    #[test]
    fn classify_signal_done_task_produces_verdict_return() {
        let allocation = sample_allocation("return_ready");
        let task = sample_task("Done", None);

        let signal = classify_signal(SupervisionEvent::TaskStateChange {
            allocation: &allocation,
            task: &task,
        });

        assert_eq!(signal.family(), SupervisionSignalFamily::VerdictReturn);
        assert_eq!(signal.source_allocation_id, Some(allocation.id));
        assert_eq!(signal.source_task_id, Some(task.id));
    }

    #[test]
    fn classify_signal_running_task_produces_verdict_return() {
        let allocation = sample_allocation("Running");
        let task = sample_task("Running", None);

        let signal = classify_signal(SupervisionEvent::TaskStateChange {
            allocation: &allocation,
            task: &task,
        });

        assert_eq!(signal.family(), SupervisionSignalFamily::VerdictReturn);
        assert_eq!(signal.source_allocation_id, Some(allocation.id));
        assert_eq!(signal.source_task_id, Some(task.id));
        assert_eq!(signal.error_code, None);
    }

    #[test]
    fn resolve_action_verdict_return_routes() {
        let signal = sample_signal(SupervisionSignalFamily::VerdictReturn);
        let resolution = resolve_supervisor_action(&signal, &DEFAULT_AGENT_PROCESS_POLICY, 0);

        assert_eq!(resolution.action, SupervisorAction::RouteReturn);
        assert!(!resolution.budget_exhausted);
        assert_eq!(resolution.attempt_number, 0);
    }

    #[test]
    fn resolve_action_execution_failure_within_budget_restarts() {
        let signal = sample_signal(SupervisionSignalFamily::ExecutionFailure);
        let resolution = resolve_supervisor_action(&signal, &DEFAULT_AGENT_PROCESS_POLICY, 0);

        assert_eq!(resolution.action, SupervisorAction::RestartChild);
        assert!(!resolution.budget_exhausted);
        assert_eq!(resolution.attempt_number, 1);
    }

    #[test]
    fn resolve_action_execution_failure_budget_exhausted_escalates() {
        let signal = sample_signal(SupervisionSignalFamily::ExecutionFailure);
        let resolution = resolve_supervisor_action(&signal, &DEFAULT_AGENT_PROCESS_POLICY, 3);

        assert_eq!(resolution.action, SupervisorAction::EscalateUp);
        assert!(resolution.budget_exhausted);
        assert_eq!(resolution.attempt_number, 4);
    }

    #[test]
    fn resolve_action_integrity_always_escalates() {
        let signal = sample_signal(SupervisionSignalFamily::Integrity);
        let resolution = resolve_supervisor_action(&signal, &DEFAULT_AGENT_PROCESS_POLICY, 0);

        assert_eq!(resolution.action, SupervisorAction::SurfaceIncident);
        assert!(resolution.budget_exhausted);
        assert_eq!(resolution.attempt_number, 1);
    }

    fn sample_allocation(status: &str) -> StoredNotaRuntimeAllocation {
        StoredNotaRuntimeAllocation {
            id: 7,
            allocator_role: "nota".to_string(),
            allocator_surface: "do".to_string(),
            allocation_kind: "forge_do_dispatch".to_string(),
            source_transaction_id: 11,
            lineage_ref: "nota/do/transaction/11/forge-task/7".to_string(),
            child_execution_kind: "forge_task".to_string(),
            child_execution_ref: "7".to_string(),
            return_target_kind: "nota_runtime_transaction".to_string(),
            return_target_ref: "11".to_string(),
            escalation_target_kind: "nota_runtime_transaction".to_string(),
            escalation_target_ref: "11".to_string(),
            status: status.to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-25T00:00:00Z".to_string(),
            updated_at: "2026-03-25T00:00:00Z".to_string(),
        }
    }

    fn sample_task(status: &str, status_message: Option<&str>) -> StoredForgeTask {
        StoredForgeTask {
            id: 7,
            name: "blocked".to_string(),
            command: "echo".to_string(),
            args: "[]".to_string(),
            working_dir: None,
            stdin_text: None,
            required_tokens: "[]".to_string(),
            metadata: "{}".to_string(),
            status: status.to_string(),
            status_message: status_message.map(str::to_string),
            exit_code: None,
            created_at: "2026-03-25T00:00:00Z".to_string(),
            heartbeat_at: None,
            finished_at: None,
        }
    }

    fn sample_signal(family: SupervisionSignalFamily) -> SupervisionSignal {
        SupervisionSignal {
            family,
            source_allocation_id: Some(7),
            source_task_id: Some(7),
            error_code: Some("sample".to_string()),
            summary: "sample".to_string(),
            timestamp: "2026-03-25T00:00:00Z".to_string(),
        }
    }

    fn insert_allocation(
        store: &DataStore,
        status: &str,
    ) -> anyhow::Result<StoredNotaRuntimeAllocation> {
        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: "supervision-test",
            payload_json: "{}",
            status: "opened",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;

        store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: "nota/do/transaction/1/forge-task/7",
            child_execution_kind: "forge_task",
            child_execution_ref: "7",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: "1",
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: "1",
            status,
            payload_json: "{}",
        })
    }

    fn insert_terminal_runtime_allocation(
        store: &DataStore,
        task_status: &str,
        task_message: Option<&str>,
    ) -> anyhow::Result<(i64, StoredNotaRuntimeAllocation, i64)> {
        let task_id = store.insert_forge_task(
            "restartable child",
            "cargo",
            r#"["check"]"#,
            Some("A:/Publish/entrance"),
            Some("resume"),
            r#"["openai"]"#,
            r#"{"owner":"supervision"}"#,
        )?;
        store.update_forge_task_status(task_id, task_status, Some(1), task_message)?;

        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "nota",
            surface_action: "do",
            transaction_kind: "forge_agent_dispatch",
            title: "runtime supervision restart",
            payload_json: "{}",
            status: "checkpointed",
            forge_task_id: Some(task_id),
            cadence_checkpoint_id: None,
        })?;
        let payload_json = serde_json::to_string(&runtime_allocation_payload())?;
        let lineage_ref = format!(
            "nota/do/transaction/{}/forge-task/{task_id}",
            transaction.id
        );
        let allocation = store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "nota",
            allocator_surface: "nota_do",
            allocation_kind: "forge_agent_dispatch",
            source_transaction_id: transaction.id,
            lineage_ref: &lineage_ref,
            child_execution_kind: "forge_task",
            child_execution_ref: &task_id.to_string(),
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &transaction.id.to_string(),
            escalation_target_kind: "nota_runtime_transaction",
            escalation_target_ref: &transaction.id.to_string(),
            status: "dispatched",
            payload_json: &payload_json,
        })?;

        Ok((transaction.id, allocation, task_id))
    }

    fn runtime_allocation_payload() -> NotaDoAllocationPayload {
        NotaDoAllocationPayload {
            issue_id: "MYT-1C".to_string(),
            issue_status: "Todo".to_string(),
            issue_status_source: "test".to_string(),
            issue_title: Some("supervision runtime restart".to_string()),
            project_root: "A:/Publish/entrance".to_string(),
            worktree_path: "A:/Publish/entrance/worktrees/feat-1c-supervision".to_string(),
            prompt_source: "test".to_string(),
            model: "codex".to_string(),
            agent_command: None,
            repair_of_allocation_id: None,
            repair_of_transaction_id: None,
            repair_of_lineage_ref: None,
            execution_host: "in_process".to_string(),
            child_dispatch_role: "agent".to_string(),
            child_dispatch_tool_name: "forge_dispatch_agent".to_string(),
            terminal_outcome: None,
        }
    }
}
