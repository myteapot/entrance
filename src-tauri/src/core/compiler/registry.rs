use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::{
    action::{
        ActionEffectKind, ActionObjectKind, ActionPrimitive, ActionRoom, ActorRole,
        AttentionStateCode, ControlPolicyCode, FlowPhaseCode, GatePolicyCode, IntegrityOverlayCode,
        RoutePolicyCode, SandboxPolicyCode, WriterPolicyCode,
    },
    data_store::DataStore,
    supervision::SupervisionScope,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub primitive: ActionPrimitive,
    pub object_kind: ActionObjectKind,
    pub flow_phase: FlowPhaseCode,
    pub attention_state: AttentionStateCode,
    pub integrity_overlay: Option<IntegrityOverlayCode>,
    pub control_policy: ControlPolicyCode,
    pub writer_policy: WriterPolicyCode,
    pub route_policy: RoutePolicyCode,
    pub gate_policy: GatePolicyCode,
    pub sandbox_policy: SandboxPolicyCode,
    pub effect_kind: ActionEffectKind,
    pub supervision_scope: Option<SupervisionScope>,
    pub allowed_roles: Vec<ActorRole>,
    pub allowed_rooms: Vec<ActionRoom>,
}

const ALL_PRIMITIVES: [ActionPrimitive; 15] = [
    ActionPrimitive::Chat,
    ActionPrimitive::Learn,
    ActionPrimitive::Shape,
    ActionPrimitive::Split,
    ActionPrimitive::Assign,
    ActionPrimitive::Prepare,
    ActionPrimitive::Dispatch,
    ActionPrimitive::Make,
    ActionPrimitive::Review,
    ActionPrimitive::Integrate,
    ActionPrimitive::Update,
    ActionPrimitive::Escalate,
    ActionPrimitive::Repair,
    ActionPrimitive::Read,
    ActionPrimitive::Report,
];

fn build_entry(primitive: ActionPrimitive) -> RegistryEntry {
    RegistryEntry {
        primitive,
        object_kind: primitive.object_kind(),
        flow_phase: primitive.flow_phase(),
        attention_state: primitive.attention_state(),
        integrity_overlay: primitive.integrity_overlay(),
        control_policy: primitive.control_policy_code(),
        writer_policy: primitive.writer_policy_code(),
        route_policy: primitive.route_policy_code(),
        gate_policy: primitive.gate_policy_code(),
        sandbox_policy: primitive.sandbox_policy_code(),
        effect_kind: primitive.effect_kind(),
        supervision_scope: primitive.supervision_scope(),
        allowed_roles: primitive.allowed_roles().to_vec(),
        allowed_rooms: primitive.allowed_rooms().to_vec(),
    }
}

pub static REGISTRY: std::sync::LazyLock<Vec<RegistryEntry>> =
    std::sync::LazyLock::new(|| ALL_PRIMITIVES.into_iter().map(build_entry).collect());

pub fn registry() -> &'static [RegistryEntry] {
    REGISTRY.as_slice()
}

pub fn seed_registry_snapshot(data_store: &DataStore) -> Result<usize> {
    data_store.seed_compiler_registry_snapshot(registry())
}

#[cfg(test)]
mod tests {
    use super::{registry, seed_registry_snapshot, REGISTRY};
    use anyhow::Result;

    use crate::{
        core::{
            action::{ActionRecord, KnowledgeLayer},
            data_store::{DataStore, MigrationPlan},
        },
        plugins,
    };

    #[test]
    fn registry_has_all_15_primitives() {
        assert_eq!(REGISTRY.len(), 15);
    }

    #[test]
    fn registry_entries_match_action_lower() -> Result<()> {
        for entry in registry() {
            let record = ActionRecord::new(
                entry.allowed_roles[0],
                entry.primitive,
                entry.allowed_rooms[0],
                KnowledgeLayer::Cold,
            )
            .expect("registry seed should always describe a valid action record");
            let lowered = record.lower();

            assert_eq!(entry.object_kind, lowered.object_kind);
            assert_eq!(entry.flow_phase, lowered.flow_phase);
            assert_eq!(entry.attention_state, lowered.attention_state);
            assert_eq!(entry.integrity_overlay, lowered.integrity_overlay);
            assert_eq!(entry.control_policy, lowered.control_policy_code);
            assert_eq!(entry.writer_policy, lowered.writer_policy_code);
            assert_eq!(entry.route_policy, lowered.route_policy_code);
            assert_eq!(entry.gate_policy, lowered.gate_policy_code);
            assert_eq!(entry.sandbox_policy, lowered.sandbox_policy_code);
            assert_eq!(entry.effect_kind, lowered.effect_kind);
            assert_eq!(entry.supervision_scope, lowered.supervision_scope);
        }

        Ok(())
    }

    #[test]
    fn registry_snapshot_round_trip() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(plugins::forge::migrations()))?;

        let seeded = seed_registry_snapshot(&store)?;
        let snapshot = store.list_compiler_registry_snapshot()?;

        assert_eq!(seeded, REGISTRY.len());
        assert_eq!(snapshot, *REGISTRY);

        Ok(())
    }
}
