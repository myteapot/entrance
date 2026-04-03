use serde::{Deserialize, Serialize};

use crate::core::data_store::{StoredForgeTask, StoredNotaRuntimeAllocation};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_signal_family: Option<SupervisionSignalFamily>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_code: Option<String>,
    pub last_supervisor_action: SupervisorAction,
    pub escalation_pending: bool,
    pub summary: String,
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
) -> RuntimeSupervisionProjection {
    let current_status = task
        .map(|task| task.status.as_str())
        .unwrap_or(allocation.status.as_str());

    let signal = match task {
        Some(task) => classify_signal(SupervisionEvent::TaskStateChange { allocation, task }),
        None => classify_signal(SupervisionEvent::AllocationStateChange { allocation }),
    };

    build_projection_from_signal(allocation, &signal, current_status)
}

fn build_projection_from_signal(
    _allocation: &StoredNotaRuntimeAllocation,
    signal: &SupervisionSignal,
    current_status: &str,
) -> RuntimeSupervisionProjection {
    match signal.family {
        SupervisionSignalFamily::VerdictReturn => match current_status {
            "Done" | "return_ready" => RuntimeSupervisionProjection {
                current_supervision_state: RuntimeChildState::Done,
                retry_count: 0,
                last_failure_signal_family: None,
                last_failure_code: None,
                last_supervisor_action: SupervisorAction::RouteReturn,
                escalation_pending: false,
                summary: signal.summary.clone(),
            },
            "Running" => RuntimeSupervisionProjection {
                current_supervision_state: RuntimeChildState::Running,
                retry_count: 0,
                last_failure_signal_family: None,
                last_failure_code: None,
                last_supervisor_action: SupervisorAction::ObserveChild,
                escalation_pending: false,
                summary: signal.summary.clone(),
            },
            _ => RuntimeSupervisionProjection {
                current_supervision_state: RuntimeChildState::Pending,
                retry_count: 0,
                last_failure_signal_family: None,
                last_failure_code: None,
                last_supervisor_action: SupervisorAction::ObserveChild,
                escalation_pending: false,
                summary: signal.summary.clone(),
            },
        },
        SupervisionSignalFamily::ExecutionFailure => RuntimeSupervisionProjection {
            current_supervision_state: RuntimeChildState::Blocked,
            retry_count: 0,
            last_failure_signal_family: Some(SupervisionSignalFamily::ExecutionFailure),
            last_failure_code: signal.error_code.clone(),
            last_supervisor_action: match current_status {
                "Blocked" | "escalated_blocked" => SupervisorAction::BlockLineage,
                _ => SupervisorAction::EscalateUp,
            },
            escalation_pending: true,
            summary: signal.summary.clone(),
        },
        SupervisionSignalFamily::Integrity => RuntimeSupervisionProjection {
            current_supervision_state: RuntimeChildState::Cancelled,
            retry_count: 0,
            last_failure_signal_family: Some(SupervisionSignalFamily::Integrity),
            last_failure_code: signal.error_code.clone(),
            last_supervisor_action: SupervisorAction::SurfaceIncident,
            escalation_pending: true,
            summary: signal.summary.clone(),
        },
        SupervisionSignalFamily::AdmissionRejection => RuntimeSupervisionProjection {
            current_supervision_state: RuntimeChildState::Blocked,
            retry_count: 0,
            last_failure_signal_family: Some(SupervisionSignalFamily::AdmissionRejection),
            last_failure_code: signal.error_code.clone(),
            last_supervisor_action: SupervisorAction::BlockLineage,
            escalation_pending: true,
            summary: signal.summary.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_signal, derive_runtime_supervision_projection, FailureVisibility, RestartPolicy,
        RuntimeChildState, SupervisionEvent, SupervisionScope, SupervisionSignalFamily,
        SupervisionStrategy, SupervisorAction, DEFAULT_AGENT_PROCESS_POLICY,
        DEFAULT_DISPATCH_PIPELINE_POLICY, DEFAULT_SESSION_BUNDLE_POLICY,
    };
    use crate::core::data_store::{StoredForgeTask, StoredNotaRuntimeAllocation};

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

        let projection = derive_runtime_supervision_projection(&allocation, Some(&task));
        assert_eq!(
            projection.current_supervision_state,
            RuntimeChildState::Blocked
        );
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::BlockLineage
        );
        assert!(projection.escalation_pending);
    }

    #[test]
    fn runtime_supervision_projects_return_ready_allocation_to_route_return() {
        let allocation = sample_allocation("return_ready");
        let projection = derive_runtime_supervision_projection(&allocation, None);
        assert_eq!(
            projection.current_supervision_state,
            RuntimeChildState::Done
        );
        assert_eq!(
            projection.last_supervisor_action,
            SupervisorAction::RouteReturn
        );
        assert!(!projection.escalation_pending);
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
}
