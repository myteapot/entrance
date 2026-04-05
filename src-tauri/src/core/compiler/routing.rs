use super::{admission::AdmittedDispatch, semantics::RoutingConstraint};

/// Terminal task status categories that determine routing boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalStatus {
    Done,
    Blocked,
    Failed,
    Cancelled,
}

impl TerminalStatus {
    pub fn from_task_status(status: &str) -> Option<TerminalStatus> {
        match status {
            "Done" => Some(Self::Done),
            "Blocked" => Some(Self::Blocked),
            "Failed" => Some(Self::Failed),
            "Cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    fn boundary(self) -> ReturnBoundary {
        match self {
            Self::Done => ReturnBoundary::Return,
            Self::Blocked | Self::Failed | Self::Cancelled => ReturnBoundary::Escalation,
        }
    }
}

/// Which boundary the return crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnBoundary {
    Return,
    Escalation,
}

/// A resolved return route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnRoute {
    pub boundary: ReturnBoundary,
    pub target_kind: String,
    pub target_ref: String,
    pub source_lineage_ref: String,
    pub terminal_status: TerminalStatus,
    pub source_instance_id: Option<String>,
}

/// Routing constraint violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingViolation {
    /// A Local-scoped dispatch tried to escalate upward
    LocalCannotEscalate,
    /// An UpwardOnly dispatch tried to return to a non-upward target
    UpwardReturnToLocal,
}

pub fn resolve_return_route(
    admitted: &AdmittedDispatch,
    terminal_status: TerminalStatus,
) -> Result<ReturnRoute, RoutingViolation> {
    let boundary = terminal_status.boundary();
    let lineage = admitted.lineage();
    let (target_kind, target_ref) = match boundary {
        ReturnBoundary::Return => (&lineage.return_target_kind, &lineage.return_target_ref),
        ReturnBoundary::Escalation => (
            &lineage.escalation_target_kind,
            &lineage.escalation_target_ref,
        ),
    };

    validate_routing_constraint(admitted.routing().constraint, boundary, target_kind)?;

    Ok(ReturnRoute {
        boundary,
        target_kind: target_kind.clone(),
        target_ref: target_ref.clone(),
        source_lineage_ref: lineage.lineage_ref.clone(),
        terminal_status,
        source_instance_id: lineage.target_instance_id.clone(),
    })
}

fn validate_routing_constraint(
    constraint: RoutingConstraint,
    boundary: ReturnBoundary,
    target_kind: &str,
) -> Result<(), RoutingViolation> {
    match (constraint, boundary) {
        (RoutingConstraint::Local, ReturnBoundary::Escalation) => {
            Err(RoutingViolation::LocalCannotEscalate)
        }
        (RoutingConstraint::UpwardOnly, ReturnBoundary::Return)
            if !is_upward_target_kind(target_kind) =>
        {
            Err(RoutingViolation::UpwardReturnToLocal)
        }
        _ => Ok(()),
    }
}

fn is_upward_target_kind(target_kind: &str) -> bool {
    target_kind == "human" || target_kind.starts_with("nota_runtime_")
}

#[cfg(test)]
mod tests {
    use super::{resolve_return_route, ReturnBoundary, RoutingViolation, TerminalStatus};
    use crate::core::{
        action::{ActionPrimitive, ActionRecord, KnowledgeLayer},
        compiler::{
            admission::{
                admit_dispatch, admit_dispatch_with_context, AdmissionContext, AdmittedDispatch,
            },
            lowering::{DispatchLineage, DispatchRouting, LoweredDispatch, SandboxConfig},
            packet::TypedActionPacket,
            registry::lookup_primitive,
            semantics::{RoutingConstraint, SandboxRequirement},
        },
    };

    fn compile_packet(primitive: ActionPrimitive) -> TypedActionPacket {
        let entry = lookup_primitive(primitive)
            .expect("all test primitives must be present in the compiler registry");
        let record = ActionRecord::new(
            entry.allowed_roles[0],
            primitive,
            entry.allowed_rooms[0],
            KnowledgeLayer::Cold,
        )
        .expect("test packet should satisfy role and room constraints");

        TypedActionPacket::compile(record)
    }

    fn admitted_dispatch(
        constraint: RoutingConstraint,
        return_target_kind: &str,
        return_target_ref: &str,
        escalation_target_kind: &str,
        escalation_target_ref: &str,
    ) -> AdmittedDispatch {
        admitted_dispatch_with_instance(
            constraint,
            return_target_kind,
            return_target_ref,
            escalation_target_kind,
            escalation_target_ref,
            None,
        )
    }

    fn admitted_dispatch_with_instance(
        constraint: RoutingConstraint,
        return_target_kind: &str,
        return_target_ref: &str,
        escalation_target_kind: &str,
        escalation_target_ref: &str,
        target_instance_id: Option<&str>,
    ) -> AdmittedDispatch {
        let lowered = LoweredDispatch {
            packet: compile_packet(ActionPrimitive::Dispatch),
            lineage: DispatchLineage {
                lineage_ref: "nota/dev/transaction/11/forge-task/7".to_string(),
                child_execution_kind: "forge_task".to_string(),
                child_execution_ref: "7".to_string(),
                return_target_kind: return_target_kind.to_string(),
                return_target_ref: return_target_ref.to_string(),
                escalation_target_kind: escalation_target_kind.to_string(),
                escalation_target_ref: escalation_target_ref.to_string(),
                target_instance_id: target_instance_id.map(str::to_string),
            },
            sandbox: SandboxConfig {
                requirement: SandboxRequirement::None,
                working_dir: None,
            },
            routing: DispatchRouting {
                constraint,
                allocator_role: "nota".to_string(),
                allocation_kind: "forge_dev_dispatch".to_string(),
            },
        };

        match target_instance_id {
            Some(target_instance_id) => {
                let context = AdmissionContext {
                    budget_remaining: None,
                    dedup_key: None,
                    available_instances: Some(vec![target_instance_id.to_string()]),
                };

                admit_dispatch_with_context(lowered, None, Some(&context))
                    .expect("synthetic lowered dispatch should admit for a known instance")
            }
            None => admit_dispatch(lowered, None).expect("synthetic lowered dispatch should admit"),
        }
    }

    #[test]
    fn done_routes_to_return_target() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let route = resolve_return_route(&admitted, TerminalStatus::Done)
            .expect("done routes should resolve");

        assert_eq!(route.boundary, ReturnBoundary::Return);
        assert_eq!(route.target_kind, "nota_runtime_transaction");
        assert_eq!(route.target_ref, "11");
        assert_eq!(route.terminal_status, TerminalStatus::Done);
        assert_eq!(route.source_instance_id, None);
    }

    #[test]
    fn blocked_routes_to_escalation_target() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let route = resolve_return_route(&admitted, TerminalStatus::Blocked)
            .expect("blocked routes should resolve");

        assert_eq!(route.boundary, ReturnBoundary::Escalation);
        assert_eq!(route.target_kind, "human");
        assert_eq!(route.target_ref, "review");
        assert_eq!(route.terminal_status, TerminalStatus::Blocked);
    }

    #[test]
    fn failed_routes_to_escalation_target() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let route = resolve_return_route(&admitted, TerminalStatus::Failed)
            .expect("failed routes should resolve");

        assert_eq!(route.boundary, ReturnBoundary::Escalation);
        assert_eq!(route.target_kind, "human");
        assert_eq!(route.target_ref, "review");
        assert_eq!(route.terminal_status, TerminalStatus::Failed);
    }

    #[test]
    fn cancelled_routes_to_escalation_target() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let route = resolve_return_route(&admitted, TerminalStatus::Cancelled)
            .expect("cancelled routes should resolve");

        assert_eq!(route.boundary, ReturnBoundary::Escalation);
        assert_eq!(route.target_kind, "human");
        assert_eq!(route.target_ref, "review");
        assert_eq!(route.terminal_status, TerminalStatus::Cancelled);
    }

    #[test]
    fn return_route_preserves_lineage_ref() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let route = resolve_return_route(&admitted, TerminalStatus::Done)
            .expect("done routes should resolve");

        assert_eq!(route.source_lineage_ref, admitted.lineage().lineage_ref);
    }

    #[test]
    fn local_constraint_blocks_escalation() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::Local,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let violation = resolve_return_route(&admitted, TerminalStatus::Blocked)
            .expect_err("local constraint should block escalation");

        assert_eq!(violation, RoutingViolation::LocalCannotEscalate);
    }

    #[test]
    fn upward_only_allows_both_return_and_escalation() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
        );

        let return_route = resolve_return_route(&admitted, TerminalStatus::Done)
            .expect("upward-only should allow return");
        let escalation_route = resolve_return_route(&admitted, TerminalStatus::Failed)
            .expect("upward-only should allow escalation");

        assert_eq!(return_route.boundary, ReturnBoundary::Return);
        assert_eq!(escalation_route.boundary, ReturnBoundary::Escalation);
    }

    #[test]
    fn terminal_status_from_task_status_parsing() {
        let _guard = crate::test_env_guard();
        assert_eq!(
            TerminalStatus::from_task_status("Done"),
            Some(TerminalStatus::Done)
        );
        assert_eq!(
            TerminalStatus::from_task_status("Blocked"),
            Some(TerminalStatus::Blocked)
        );
        assert_eq!(
            TerminalStatus::from_task_status("Failed"),
            Some(TerminalStatus::Failed)
        );
        assert_eq!(
            TerminalStatus::from_task_status("Cancelled"),
            Some(TerminalStatus::Cancelled)
        );
        assert_eq!(TerminalStatus::from_task_status("Pending"), None);
        assert_eq!(TerminalStatus::from_task_status("Running"), None);
    }

    #[test]
    fn upward_only_return_to_local_target_is_rejected() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch(
            RoutingConstraint::UpwardOnly,
            "forge_task",
            "7",
            "human",
            "review",
        );

        let violation = resolve_return_route(&admitted, TerminalStatus::Done)
            .expect_err("upward-only should reject local return targets");

        assert_eq!(violation, RoutingViolation::UpwardReturnToLocal);
    }

    #[test]
    fn cross_agent_dispatch_route_carries_source_instance_id() {
        let _guard = crate::test_env_guard();
        let admitted = admitted_dispatch_with_instance(
            RoutingConstraint::UpwardOnly,
            "nota_runtime_transaction",
            "11",
            "human",
            "review",
            Some("slot-1"),
        );

        let route = resolve_return_route(&admitted, TerminalStatus::Done)
            .expect("done routes should resolve for an explicit target instance");

        assert_eq!(route.source_instance_id.as_deref(), Some("slot-1"));
    }
}
