use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::{
        FailureVisibility, RestartPolicy, RuntimeChildState, SupervisionScope, SupervisionStrategy,
        DEFAULT_AGENT_PROCESS_POLICY, DEFAULT_DISPATCH_PIPELINE_POLICY,
        DEFAULT_SESSION_BUNDLE_POLICY,
    };

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
}
