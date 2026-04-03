use serde::{Deserialize, Serialize};

use super::registry::RegistryEntry;
use crate::core::action::{
    ActionEffectKind, GatePolicyCode, IntegrityOverlayCode, RoutePolicyCode, SandboxPolicyCode,
    WriterPolicyCode,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveControlSemantics {
    pub requires_admission_gate: bool,
    pub writes_truth: bool,
    pub requires_supervision: bool,
    pub sandbox_requirement: SandboxRequirement,
    pub hot_projection_allowed: bool,
    pub requires_human_approval: bool,
    pub is_read_only: bool,
    pub routing_constraint: RoutingConstraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxRequirement {
    None,
    WorktreeScoped,
    AdminOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingConstraint {
    Local,
    UpwardOnly,
    HumanBoundary,
    RuntimeInternal,
}

pub fn resolve_effective_semantics(entry: &RegistryEntry) -> EffectiveControlSemantics {
    EffectiveControlSemantics {
        requires_admission_gate: !matches!(entry.gate_policy, GatePolicyCode::None),
        writes_truth: matches!(entry.writer_policy, WriterPolicyCode::NotaBoundaryOnly),
        requires_supervision: entry.supervision_scope.is_some(),
        sandbox_requirement: resolve_sandbox_requirement(entry.sandbox_policy),
        // Registry entries do not encode layer-specific projection policies.
        // Observe primitives are the only entries that never project to hot.
        hot_projection_allowed: !matches!(entry.effect_kind, ActionEffectKind::Observe),
        requires_human_approval: matches!(
            entry.integrity_overlay,
            Some(IntegrityOverlayCode::AdminHold)
        ),
        is_read_only: matches!(entry.effect_kind, ActionEffectKind::Observe),
        routing_constraint: resolve_routing_constraint(entry.route_policy),
    }
}

impl RegistryEntry {
    pub fn effective_semantics(&self) -> EffectiveControlSemantics {
        resolve_effective_semantics(self)
    }
}

fn resolve_sandbox_requirement(policy: SandboxPolicyCode) -> SandboxRequirement {
    match policy {
        SandboxPolicyCode::None => SandboxRequirement::None,
        SandboxPolicyCode::WorktreeRwAllowlist => SandboxRequirement::WorktreeScoped,
        SandboxPolicyCode::RuntimeAdminOnly => SandboxRequirement::AdminOnly,
    }
}

fn resolve_routing_constraint(policy: RoutePolicyCode) -> RoutingConstraint {
    match policy {
        RoutePolicyCode::LocalOnly => RoutingConstraint::Local,
        RoutePolicyCode::UpwardOnly => RoutingConstraint::UpwardOnly,
        RoutePolicyCode::HumanNotaBoundary => RoutingConstraint::HumanBoundary,
        RoutePolicyCode::RuntimeInternal => RoutingConstraint::RuntimeInternal,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_effective_semantics, EffectiveControlSemantics, RoutingConstraint,
        SandboxRequirement,
    };
    use crate::core::{
        action::ActionPrimitive,
        compiler::registry::{lookup_primitive, registry},
    };

    #[test]
    fn chat_is_read_only_no_gate_no_supervision() {
        let entry = lookup_primitive(ActionPrimitive::Chat)
            .expect("chat primitive should always be present in the registry");
        let semantics = resolve_effective_semantics(entry);

        assert_eq!(
            semantics,
            EffectiveControlSemantics {
                requires_admission_gate: false,
                writes_truth: false,
                requires_supervision: false,
                sandbox_requirement: SandboxRequirement::None,
                hot_projection_allowed: false,
                requires_human_approval: false,
                is_read_only: true,
                routing_constraint: RoutingConstraint::Local,
            }
        );
    }

    #[test]
    fn dispatch_requires_supervision_and_sandbox() {
        let entry = lookup_primitive(ActionPrimitive::Dispatch)
            .expect("dispatch primitive should always be present in the registry");
        let semantics = entry.effective_semantics();

        assert!(semantics.requires_supervision);
        assert_eq!(
            semantics.sandbox_requirement,
            SandboxRequirement::WorktreeScoped
        );
        assert_eq!(semantics.routing_constraint, RoutingConstraint::UpwardOnly);
    }

    #[test]
    fn learn_writes_truth() {
        let entry = lookup_primitive(ActionPrimitive::Learn)
            .expect("learn primitive should always be present in the registry");
        let semantics = entry.effective_semantics();

        assert!(semantics.writes_truth);
        assert!(semantics.hot_projection_allowed);
        assert_eq!(
            semantics.routing_constraint,
            RoutingConstraint::HumanBoundary
        );
    }

    #[test]
    fn escalate_requires_human_approval() {
        let entry = lookup_primitive(ActionPrimitive::Escalate)
            .expect("escalate primitive should always be present in the registry");
        let semantics = entry.effective_semantics();

        assert!(semantics.requires_human_approval);
        assert!(!semantics.is_read_only);
    }

    #[test]
    fn all_registry_entries_resolve_without_panic() {
        let resolved = registry()
            .iter()
            .map(resolve_effective_semantics)
            .collect::<Vec<_>>();

        assert_eq!(resolved.len(), 15);
    }
}
