use serde::{Deserialize, Serialize};

use super::{
    packet::TypedActionPacket,
    semantics::{RoutingConstraint, SandboxRequirement},
};
use crate::core::action::ActionPrimitive;

pub const DISPATCH_SCOPE_PRIMITIVES: [ActionPrimitive; 4] = [
    ActionPrimitive::Prepare,
    ActionPrimitive::Dispatch,
    ActionPrimitive::Make,
    ActionPrimitive::Repair,
];

/// Context provided by the caller (nota runtime), not fetched from DB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringContext {
    pub transaction_id: i64,
    pub task_id: i64,
    pub dispatch_lane: String,
    pub allocator_surface: String,
    pub project_root: String,
    /// Restart attempts already consumed for this allocation.
    pub consumed_restart_attempts: u8,
    /// Maximum restart attempts allowed by supervision policy.
    pub max_restart_attempts: u8,
    /// Whether this transaction already has an active allocation.
    pub has_active_allocation: bool,
}

/// Resolved lineage for an allocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchLineage {
    pub lineage_ref: String,
    pub child_execution_kind: String,
    pub child_execution_ref: String,
    pub return_target_kind: String,
    pub return_target_ref: String,
    pub escalation_target_kind: String,
    pub escalation_target_ref: String,
}

/// Sandbox configuration derived from packet semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub requirement: SandboxRequirement,
    pub working_dir: Option<String>,
}

/// Routing derived from packet semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchRouting {
    pub constraint: RoutingConstraint,
    pub allocator_role: String,
    pub allocation_kind: String,
}

/// The output of lowering. Ready for admission (M6.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweredDispatch {
    pub packet: TypedActionPacket,
    pub lineage: DispatchLineage,
    pub sandbox: SandboxConfig,
    pub routing: DispatchRouting,
}

/// Errors that can occur during lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringError {
    NotDispatchable,
    MissingSupervision,
    BudgetExhausted,
    DuplicateDispatch,
}

pub fn lower_dispatch(
    packet: &TypedActionPacket,
    ctx: &LoweringContext,
) -> Result<LoweredDispatch, LoweringError> {
    if !is_dispatch_scope_primitive(packet.record().verb) {
        return Err(LoweringError::NotDispatchable);
    }

    if packet.requires_supervision() && packet.plan().supervision_scope.is_none() {
        return Err(LoweringError::MissingSupervision);
    }

    if ctx.consumed_restart_attempts >= ctx.max_restart_attempts {
        return Err(LoweringError::BudgetExhausted);
    }

    if ctx.has_active_allocation {
        return Err(LoweringError::DuplicateDispatch);
    }

    Ok(LoweredDispatch {
        packet: packet.clone(),
        lineage: resolve_lineage(ctx),
        sandbox: resolve_sandbox(packet.semantics().sandbox_requirement, ctx),
        routing: resolve_routing(packet, ctx),
    })
}

fn is_dispatch_scope_primitive(primitive: ActionPrimitive) -> bool {
    DISPATCH_SCOPE_PRIMITIVES.contains(&primitive)
}

fn resolve_lineage(ctx: &LoweringContext) -> DispatchLineage {
    let transaction_ref = ctx.transaction_id.to_string();
    let task_ref = ctx.task_id.to_string();
    let lineage_lane = resolve_lineage_lane(ctx);

    DispatchLineage {
        lineage_ref: format!("{lineage_lane}/transaction/{transaction_ref}/forge-task/{task_ref}"),
        child_execution_kind: "forge_task".to_string(),
        child_execution_ref: task_ref.clone(),
        return_target_kind: "nota_runtime_transaction".to_string(),
        return_target_ref: transaction_ref.clone(),
        escalation_target_kind: "nota_runtime_transaction".to_string(),
        escalation_target_ref: transaction_ref,
    }
}

fn resolve_lineage_lane(ctx: &LoweringContext) -> String {
    if ctx.dispatch_lane.contains('/') {
        return ctx.dispatch_lane.clone();
    }

    if let Some((allocator_role, _)) = ctx.dispatch_lane.split_once('_') {
        return format!("{allocator_role}/{}", resolved_allocator_surface(ctx));
    }

    if ctx.dispatch_lane == resolved_allocator_surface(ctx) {
        return ctx.dispatch_lane.clone();
    }

    format!("{}/{}", ctx.dispatch_lane, resolved_allocator_surface(ctx))
}

fn resolve_sandbox(requirement: SandboxRequirement, ctx: &LoweringContext) -> SandboxConfig {
    SandboxConfig {
        requirement,
        working_dir: matches!(requirement, SandboxRequirement::WorktreeScoped)
            .then(|| ctx.project_root.clone()),
    }
}

fn resolve_routing(packet: &TypedActionPacket, ctx: &LoweringContext) -> DispatchRouting {
    DispatchRouting {
        constraint: packet.semantics().routing_constraint,
        allocator_role: resolve_allocator_role(&ctx.dispatch_lane),
        allocation_kind: resolve_allocation_kind(ctx),
    }
}

fn resolve_allocator_role(dispatch_lane: &str) -> String {
    dispatch_lane
        .split(|character| character == '_' || character == '/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(dispatch_lane)
        .to_string()
}

fn resolve_allocation_kind(ctx: &LoweringContext) -> String {
    match resolved_allocator_surface(ctx) {
        "do" => "forge_agent_dispatch".to_string(),
        "dev" => "forge_dev_dispatch".to_string(),
        surface => format!("forge_{surface}_dispatch"),
    }
}

fn resolved_allocator_surface(ctx: &LoweringContext) -> &str {
    if !ctx.allocator_surface.is_empty() {
        return ctx.allocator_surface.as_str();
    }

    ctx.dispatch_lane
        .rsplit(|character| character == '_' || character == '/')
        .next()
        .filter(|surface| !surface.is_empty())
        .unwrap_or(ctx.dispatch_lane.as_str())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        lower_dispatch, DispatchRouting, LoweringContext, LoweringError, DISPATCH_SCOPE_PRIMITIVES,
    };
    use crate::core::{
        action::{ActionPrimitive, ActionRecord, KnowledgeLayer},
        compiler::{
            packet::TypedActionPacket,
            registry::{lookup_primitive, registry},
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

    fn lowering_context(dispatch_lane: &str, allocator_surface: &str) -> LoweringContext {
        LoweringContext {
            transaction_id: 11,
            task_id: 7,
            dispatch_lane: dispatch_lane.to_string(),
            allocator_surface: allocator_surface.to_string(),
            project_root: "A:/Publish/entrance".to_string(),
            consumed_restart_attempts: 0,
            max_restart_attempts: 3,
            has_active_allocation: false,
        }
    }

    fn lowering_context_for_primitive(primitive: ActionPrimitive) -> LoweringContext {
        match primitive {
            ActionPrimitive::Make => lowering_context("nota_do", "do"),
            ActionPrimitive::Prepare | ActionPrimitive::Dispatch | ActionPrimitive::Repair => {
                lowering_context("nota_dev", "dev")
            }
            _ => panic!("non-dispatch primitive should not request a lowering context"),
        }
    }

    #[test]
    fn lower_dispatch_produces_valid_lineage() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let lowered = lower_dispatch(
            &packet,
            &lowering_context_for_primitive(ActionPrimitive::Dispatch),
        )
        .expect("dispatch should lower successfully");

        assert_eq!(
            lowered.lineage.lineage_ref,
            "nota/dev/transaction/11/forge-task/7"
        );
        assert_eq!(lowered.lineage.child_execution_kind, "forge_task");
        assert_eq!(lowered.lineage.return_target_ref, "11");
        assert_eq!(lowered.lineage.escalation_target_ref, "11");
        assert_eq!(
            lowered.routing,
            DispatchRouting {
                constraint: RoutingConstraint::UpwardOnly,
                allocator_role: "nota".to_string(),
                allocation_kind: "forge_dev_dispatch".to_string(),
            }
        );
    }

    #[test]
    fn lower_chat_is_not_dispatchable() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Chat);

        assert_eq!(
            lower_dispatch(&packet, &lowering_context("nota_do", "do")),
            Err(LoweringError::NotDispatchable)
        );
    }

    #[test]
    fn lower_dispatch_has_worktree_sandbox() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let lowered = lower_dispatch(
            &packet,
            &lowering_context_for_primitive(ActionPrimitive::Dispatch),
        )
        .expect("dispatch should lower successfully");

        assert_eq!(
            lowered.sandbox.requirement,
            SandboxRequirement::WorktreeScoped
        );
        assert_eq!(
            lowered.sandbox.working_dir.as_deref(),
            Some("A:/Publish/entrance")
        );
    }

    #[test]
    fn lower_make_has_worktree_sandbox() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Make);
        let lowered = lower_dispatch(
            &packet,
            &lowering_context_for_primitive(ActionPrimitive::Make),
        )
        .expect("make should lower successfully");

        assert_eq!(
            lowered.sandbox.requirement,
            SandboxRequirement::WorktreeScoped
        );
        assert_eq!(
            lowered.sandbox.working_dir.as_deref(),
            Some("A:/Publish/entrance")
        );
        assert_eq!(lowered.routing.allocation_kind, "forge_agent_dispatch");
    }

    #[test]
    fn all_dispatch_scope_primitives_lower_successfully() {
        let _guard = crate::test_env_guard();
        for primitive in DISPATCH_SCOPE_PRIMITIVES {
            let packet = compile_packet(primitive);
            let lowered = lower_dispatch(&packet, &lowering_context_for_primitive(primitive))
                .expect("dispatch scope primitive should lower successfully");

            assert_eq!(lowered.packet.record().verb, primitive);
            assert_eq!(
                lowered.routing.constraint,
                packet.semantics().routing_constraint
            );
        }
    }

    #[test]
    fn non_dispatch_primitives_are_rejected() {
        let _guard = crate::test_env_guard();
        for primitive in registry()
            .iter()
            .map(|entry| entry.primitive)
            .filter(|primitive| !DISPATCH_SCOPE_PRIMITIVES.contains(primitive))
        {
            let packet = compile_packet(primitive);

            assert_eq!(
                lower_dispatch(&packet, &lowering_context("nota_do", "do")),
                Err(LoweringError::NotDispatchable)
            );
        }
    }

    #[test]
    fn missing_supervision_is_rejected() {
        let _guard = crate::test_env_guard();
        let mut packet_value = serde_json::to_value(compile_packet(ActionPrimitive::Dispatch))
            .expect("packet should serialize for supervision mismatch test");
        packet_value["plan"]["supervisionScope"] = Value::Null;
        let packet: TypedActionPacket = serde_json::from_value(packet_value)
            .expect("packet should deserialize with a missing supervision scope");

        assert_eq!(
            lower_dispatch(
                &packet,
                &lowering_context_for_primitive(ActionPrimitive::Dispatch)
            ),
            Err(LoweringError::MissingSupervision)
        );
    }

    #[test]
    fn budget_exhausted_rejects_dispatch() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let context = LoweringContext {
            consumed_restart_attempts: 3,
            max_restart_attempts: 3,
            ..lowering_context_for_primitive(ActionPrimitive::Dispatch)
        };

        assert_eq!(
            lower_dispatch(&packet, &context),
            Err(LoweringError::BudgetExhausted)
        );
    }

    #[test]
    fn duplicate_dispatch_rejected() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let context = LoweringContext {
            has_active_allocation: true,
            ..lowering_context_for_primitive(ActionPrimitive::Dispatch)
        };

        assert_eq!(
            lower_dispatch(&packet, &context),
            Err(LoweringError::DuplicateDispatch)
        );
    }

    #[test]
    fn within_budget_admits_dispatch() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let context = LoweringContext {
            consumed_restart_attempts: 1,
            max_restart_attempts: 3,
            has_active_allocation: false,
            ..lowering_context_for_primitive(ActionPrimitive::Dispatch)
        };

        let lowered =
            lower_dispatch(&packet, &context).expect("dispatch should lower while budget remains");

        assert_eq!(lowered.packet.record().verb, ActionPrimitive::Dispatch);
    }
}
