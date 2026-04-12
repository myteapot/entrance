use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::{registry::lookup_primitive, semantics::EffectiveControlSemantics};
use crate::core::action::{ActionRecord, CompiledActionPlan};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedActionPacket {
    record: ActionRecord,
    plan: CompiledActionPlan,
    semantics: EffectiveControlSemantics,
    created_at: String,
}

impl TypedActionPacket {
    pub fn compile(record: ActionRecord) -> Self {
        let plan = record.lower();
        let semantics = lookup_primitive(record.verb)
            .expect("all action primitives must be present in the compiler registry")
            .effective_semantics();

        Self {
            record,
            plan,
            semantics,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    pub fn record(&self) -> &ActionRecord {
        &self.record
    }

    pub fn plan(&self) -> &CompiledActionPlan {
        &self.plan
    }

    pub fn semantics(&self) -> &EffectiveControlSemantics {
        &self.semantics
    }

    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    pub fn is_read_only(&self) -> bool {
        self.semantics.is_read_only
    }

    pub fn requires_gate(&self) -> bool {
        self.semantics.requires_admission_gate
    }

    pub fn requires_supervision(&self) -> bool {
        self.semantics.requires_supervision
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::TypedActionPacket;
    use crate::core::{
        action::{
            ActionPrimitive, ActionRecord, ActionRoom, ActorRole, AdmissionPolicyCode,
            KnowledgeLayer, ProjectionPolicyCode,
        },
        compiler::registry::{lookup_primitive, registry, RegistryEntry},
    };

    fn compile_packet(
        actor_role: ActorRole,
        verb: ActionPrimitive,
        room: ActionRoom,
        target_layer: KnowledgeLayer,
    ) -> TypedActionPacket {
        let record = ActionRecord::new(actor_role, verb, room, target_layer)
            .expect("test action record should satisfy role and room constraints");

        TypedActionPacket::compile(record)
    }

    fn compile_registry_entry(
        entry: &RegistryEntry,
        target_layer: KnowledgeLayer,
    ) -> TypedActionPacket {
        compile_packet(
            entry.allowed_roles[0],
            entry.primitive,
            entry.allowed_rooms[0],
            target_layer,
        )
    }

    #[test]
    fn compile_chat_produces_read_only_packet() {
        let packet = compile_packet(
            ActorRole::Nota,
            ActionPrimitive::Chat,
            ActionRoom::Surface,
            KnowledgeLayer::Cold,
        );

        assert!(packet.is_read_only());
        assert!(!packet.requires_gate());
        assert!(!packet.requires_supervision());
        assert_eq!(
            packet.plan().admission_policy_code,
            AdmissionPolicyCode::StorageAlways
        );
        assert!(DateTime::parse_from_rfc3339(packet.created_at()).is_ok());
    }

    #[test]
    fn compile_dispatch_cold_has_correct_admission() {
        let packet = compile_packet(
            ActorRole::Dev,
            ActionPrimitive::Dispatch,
            ActionRoom::Prep,
            KnowledgeLayer::Cold,
        );

        assert_eq!(
            packet.plan().admission_policy_code,
            AdmissionPolicyCode::StorageAndColdAlways
        );
        assert_eq!(
            packet.plan().projection_policy_code,
            ProjectionPolicyCode::HotActiveOnly
        );
    }

    #[test]
    fn compile_dispatch_hot_has_correct_admission() {
        let packet = compile_packet(
            ActorRole::Dev,
            ActionPrimitive::Dispatch,
            ActionRoom::Prep,
            KnowledgeLayer::Hot,
        );

        assert_eq!(
            packet.plan().admission_policy_code,
            AdmissionPolicyCode::StorageColdHotOnAttention
        );
        assert_eq!(
            packet.plan().projection_policy_code,
            ProjectionPolicyCode::HotOnAttentionOrReject
        );
    }

    #[test]
    fn all_15_primitives_compile_both_layers() {
        let mut packets = Vec::new();

        for entry in registry() {
            for target_layer in [KnowledgeLayer::Cold, KnowledgeLayer::Hot] {
                packets.push(compile_registry_entry(entry, target_layer));
            }
        }

        assert_eq!(packets.len(), 30);
    }

    #[test]
    fn packet_semantics_matches_registry() {
        for entry in registry() {
            let expected = lookup_primitive(entry.primitive)
                .expect("registry entries should be retrievable by primitive")
                .effective_semantics();

            for target_layer in [KnowledgeLayer::Cold, KnowledgeLayer::Hot] {
                let packet = compile_registry_entry(entry, target_layer);

                assert_eq!(packet.semantics(), &expected);
            }
        }
    }
}
