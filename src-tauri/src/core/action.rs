use serde::{Deserialize, Serialize};

use crate::core::supervision::SupervisionScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeLayer {
    Cold,
    Hot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernancePrinciple {
    ColdHotDualTrack,
}

impl GovernancePrinciple {
    pub fn slug(self) -> &'static str {
        match self {
            Self::ColdHotDualTrack => "cold_hot_dual_track",
        }
    }
}

pub const FIRST_GUIDING_PRINCIPLE: GovernancePrinciple = GovernancePrinciple::ColdHotDualTrack;

pub const CANONICAL_LAYER_WRITE_ORDER: [KnowledgeLayer; 2] =
    [KnowledgeLayer::Cold, KnowledgeLayer::Hot];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerOrderViolation {
    pub offending_index: usize,
}

pub fn validate_layer_write_order(layers: &[KnowledgeLayer]) -> Result<(), LayerOrderViolation> {
    let mut seen_hot = false;

    for (index, layer) in layers.iter().enumerate() {
        match layer {
            KnowledgeLayer::Cold if seen_hot => {
                return Err(LayerOrderViolation {
                    offending_index: index,
                });
            }
            KnowledgeLayer::Cold => {}
            KnowledgeLayer::Hot => {
                seen_hot = true;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorRole {
    Nota,
    Arch,
    Dev,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotaSurfaceAction {
    Chat,
    Learn,
    Do,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPrimitive {
    Chat,
    Learn,
    Shape,
    Split,
    Assign,
    Prepare,
    Dispatch,
    Make,
    Review,
    Integrate,
    Update,
    Escalate,
    Repair,
    Read,
    Report,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionRoom {
    Surface,
    Memory,
    Strategy,
    Prep,
    Work,
    Review,
    Integration,
    Approval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionObjectKind {
    RuntimeQuery,
    CadenceCheckpoint,
    ControlDecision,
    RuntimeDispatch,
    AgentWorkArtifact,
    ReturnReview,
    ReturnIntegration,
    RuntimeContinuation,
    RepairFollowup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowPhaseCode {
    In,
    Cycle,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionStateCode {
    Ready,
    Running,
    Waiting,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityOverlayCode {
    LineageBlocked,
    AdminHold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlPolicyCode {
    RuntimeQueryLocal,
    NotaBoundaryWrite,
    StrategyShape,
    DispatchPrep,
    AgentWorkExecution,
    ReviewReturn,
    IntegrateReturn,
    RuntimeContinuation,
    RepairFollowup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriterPolicyCode {
    OwnerAppend,
    RuntimeAppend,
    NotaBoundaryOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutePolicyCode {
    HumanNotaBoundary,
    LocalOnly,
    UpwardOnly,
    RuntimeInternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatePolicyCode {
    None,
    ReviewReady,
    IntegrationReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicyCode {
    None,
    WorktreeRwAllowlist,
    RuntimeAdminOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionPolicyCode {
    StorageAlways,
    StorageAndColdAlways,
    StorageColdHotOnAttention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionPolicyCode {
    HotNever,
    HotActiveOnly,
    HotOnAttentionOrReject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionEffectKind {
    Observe,
    TruthWrite,
    StrategyWrite,
    DispatchWrite,
    ArtifactWrite,
    ReviewWrite,
    IntegrateWrite,
    ContinuationWrite,
    RepairWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledActionPlan {
    pub object_kind: ActionObjectKind,
    pub flow_phase: FlowPhaseCode,
    pub attention_state: AttentionStateCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity_overlay: Option<IntegrityOverlayCode>,
    pub control_policy_code: ControlPolicyCode,
    pub writer_policy_code: WriterPolicyCode,
    pub route_policy_code: RoutePolicyCode,
    pub gate_policy_code: GatePolicyCode,
    pub sandbox_policy_code: SandboxPolicyCode,
    pub admission_policy_code: AdmissionPolicyCode,
    pub projection_policy_code: ProjectionPolicyCode,
    pub effect_kind: ActionEffectKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supervision_scope: Option<SupervisionScope>,
}

const NOTA_SURFACE_ACTIONS: [NotaSurfaceAction; 3] = [
    NotaSurfaceAction::Chat,
    NotaSurfaceAction::Learn,
    NotaSurfaceAction::Do,
];

const NOTA_INTERNAL_PRIMITIVES: [ActionPrimitive; 5] = [
    ActionPrimitive::Chat,
    ActionPrimitive::Learn,
    ActionPrimitive::Assign,
    ActionPrimitive::Update,
    ActionPrimitive::Escalate,
];

const ARCH_PRIMITIVES: [ActionPrimitive; 5] = [
    ActionPrimitive::Shape,
    ActionPrimitive::Split,
    ActionPrimitive::Assign,
    ActionPrimitive::Update,
    ActionPrimitive::Escalate,
];

const DEV_PRIMITIVES: [ActionPrimitive; 5] = [
    ActionPrimitive::Prepare,
    ActionPrimitive::Dispatch,
    ActionPrimitive::Review,
    ActionPrimitive::Integrate,
    ActionPrimitive::Repair,
];

const AGENT_PRIMITIVES: [ActionPrimitive; 3] = [
    ActionPrimitive::Read,
    ActionPrimitive::Make,
    ActionPrimitive::Report,
];

const ROOM_SURFACE: [ActionRoom; 1] = [ActionRoom::Surface];
const ROOM_MEMORY: [ActionRoom; 1] = [ActionRoom::Memory];
const ROOM_STRATEGY: [ActionRoom; 1] = [ActionRoom::Strategy];
const ROOM_PREP: [ActionRoom; 1] = [ActionRoom::Prep];
const ROOM_WORK: [ActionRoom; 1] = [ActionRoom::Work];
const ROOM_REVIEW: [ActionRoom; 1] = [ActionRoom::Review];
const ROOM_INTEGRATION: [ActionRoom; 1] = [ActionRoom::Integration];
const ROOM_APPROVAL: [ActionRoom; 1] = [ActionRoom::Approval];
const ROOM_STRATEGY_OR_INTEGRATION: [ActionRoom; 2] =
    [ActionRoom::Strategy, ActionRoom::Integration];
const ROOM_WORK_OR_REVIEW: [ActionRoom; 2] = [ActionRoom::Work, ActionRoom::Review];

impl ActorRole {
    pub fn nota_surface_actions(self) -> &'static [NotaSurfaceAction] {
        match self {
            Self::Nota => &NOTA_SURFACE_ACTIONS,
            Self::Arch | Self::Dev | Self::Agent => &[],
        }
    }

    pub fn primitives(self) -> &'static [ActionPrimitive] {
        match self {
            Self::Nota => &NOTA_INTERNAL_PRIMITIVES,
            Self::Arch => &ARCH_PRIMITIVES,
            Self::Dev => &DEV_PRIMITIVES,
            Self::Agent => &AGENT_PRIMITIVES,
        }
    }
}

impl ActionPrimitive {
    pub fn allowed_roles(self) -> &'static [ActorRole] {
        match self {
            Self::Chat | Self::Learn => &[ActorRole::Nota],
            Self::Shape | Self::Split => &[ActorRole::Arch],
            Self::Assign | Self::Update | Self::Escalate => &[ActorRole::Nota, ActorRole::Arch],
            Self::Prepare | Self::Dispatch | Self::Review | Self::Integrate | Self::Repair => {
                &[ActorRole::Dev]
            }
            Self::Read | Self::Make | Self::Report => &[ActorRole::Agent],
        }
    }

    pub fn allowed_rooms(self) -> &'static [ActionRoom] {
        match self {
            Self::Chat => &ROOM_SURFACE,
            Self::Learn => &ROOM_MEMORY,
            Self::Shape | Self::Split | Self::Assign => &ROOM_STRATEGY,
            Self::Prepare | Self::Dispatch => &ROOM_PREP,
            Self::Make | Self::Read => &ROOM_WORK,
            Self::Review => &ROOM_REVIEW,
            Self::Integrate => &ROOM_INTEGRATION,
            Self::Update => &ROOM_STRATEGY_OR_INTEGRATION,
            Self::Escalate => &ROOM_APPROVAL,
            Self::Repair => &ROOM_WORK_OR_REVIEW,
            Self::Report => &ROOM_SURFACE,
        }
    }

    pub(crate) fn object_kind(self) -> ActionObjectKind {
        match self {
            Self::Chat | Self::Read | Self::Report => ActionObjectKind::RuntimeQuery,
            Self::Learn => ActionObjectKind::CadenceCheckpoint,
            Self::Shape | Self::Split | Self::Assign => ActionObjectKind::ControlDecision,
            Self::Prepare | Self::Dispatch => ActionObjectKind::RuntimeDispatch,
            Self::Make => ActionObjectKind::AgentWorkArtifact,
            Self::Review => ActionObjectKind::ReturnReview,
            Self::Integrate => ActionObjectKind::ReturnIntegration,
            Self::Update | Self::Escalate => ActionObjectKind::RuntimeContinuation,
            Self::Repair => ActionObjectKind::RepairFollowup,
        }
    }

    pub(crate) fn flow_phase(self) -> FlowPhaseCode {
        match self {
            Self::Chat | Self::Read | Self::Report => FlowPhaseCode::In,
            Self::Review | Self::Integrate | Self::Escalate => FlowPhaseCode::Out,
            _ => FlowPhaseCode::Cycle,
        }
    }

    pub(crate) fn attention_state(self) -> AttentionStateCode {
        match self {
            Self::Prepare | Self::Dispatch | Self::Make | Self::Review | Self::Integrate => {
                AttentionStateCode::Running
            }
            Self::Repair | Self::Escalate => AttentionStateCode::Waiting,
            Self::Report => AttentionStateCode::Stopped,
            _ => AttentionStateCode::Ready,
        }
    }

    pub(crate) fn integrity_overlay(self) -> Option<IntegrityOverlayCode> {
        match self {
            Self::Escalate => Some(IntegrityOverlayCode::AdminHold),
            Self::Repair => Some(IntegrityOverlayCode::LineageBlocked),
            _ => None,
        }
    }

    pub(crate) fn control_policy_code(self) -> ControlPolicyCode {
        match self {
            Self::Chat | Self::Read | Self::Report => ControlPolicyCode::RuntimeQueryLocal,
            Self::Learn => ControlPolicyCode::NotaBoundaryWrite,
            Self::Shape | Self::Split | Self::Assign => ControlPolicyCode::StrategyShape,
            Self::Prepare | Self::Dispatch => ControlPolicyCode::DispatchPrep,
            Self::Make => ControlPolicyCode::AgentWorkExecution,
            Self::Review => ControlPolicyCode::ReviewReturn,
            Self::Integrate => ControlPolicyCode::IntegrateReturn,
            Self::Update | Self::Escalate => ControlPolicyCode::RuntimeContinuation,
            Self::Repair => ControlPolicyCode::RepairFollowup,
        }
    }

    pub(crate) fn writer_policy_code(self) -> WriterPolicyCode {
        match self {
            Self::Learn => WriterPolicyCode::NotaBoundaryOnly,
            Self::Report => WriterPolicyCode::RuntimeAppend,
            _ => WriterPolicyCode::OwnerAppend,
        }
    }

    pub(crate) fn route_policy_code(self) -> RoutePolicyCode {
        match self {
            Self::Learn => RoutePolicyCode::HumanNotaBoundary,
            Self::Dispatch | Self::Make | Self::Escalate => RoutePolicyCode::UpwardOnly,
            Self::Report => RoutePolicyCode::RuntimeInternal,
            _ => RoutePolicyCode::LocalOnly,
        }
    }

    pub(crate) fn gate_policy_code(self) -> GatePolicyCode {
        match self {
            Self::Review => GatePolicyCode::ReviewReady,
            Self::Integrate => GatePolicyCode::IntegrationReady,
            _ => GatePolicyCode::None,
        }
    }

    pub(crate) fn sandbox_policy_code(self) -> SandboxPolicyCode {
        match self {
            Self::Dispatch | Self::Make | Self::Repair => SandboxPolicyCode::WorktreeRwAllowlist,
            Self::Report => SandboxPolicyCode::RuntimeAdminOnly,
            _ => SandboxPolicyCode::None,
        }
    }

    pub(crate) fn effect_kind(self) -> ActionEffectKind {
        match self {
            Self::Chat | Self::Read | Self::Report => ActionEffectKind::Observe,
            Self::Learn => ActionEffectKind::TruthWrite,
            Self::Shape | Self::Split | Self::Assign => ActionEffectKind::StrategyWrite,
            Self::Prepare | Self::Dispatch => ActionEffectKind::DispatchWrite,
            Self::Make => ActionEffectKind::ArtifactWrite,
            Self::Review => ActionEffectKind::ReviewWrite,
            Self::Integrate => ActionEffectKind::IntegrateWrite,
            Self::Update | Self::Escalate => ActionEffectKind::ContinuationWrite,
            Self::Repair => ActionEffectKind::RepairWrite,
        }
    }

    pub(crate) fn supervision_scope(self) -> Option<SupervisionScope> {
        match self {
            Self::Dispatch => Some(SupervisionScope::DispatchPipeline),
            Self::Make => Some(SupervisionScope::AgentProcess),
            Self::Review | Self::Integrate | Self::Repair => Some(SupervisionScope::SessionBundle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub verb: ActionPrimitive,
    pub actor_role: ActorRole,
    pub room: ActionRoom,
    pub target_layer: KnowledgeLayer,
}

impl ActionRecord {
    pub fn new(
        actor_role: ActorRole,
        verb: ActionPrimitive,
        room: ActionRoom,
        target_layer: KnowledgeLayer,
    ) -> Result<Self, &'static str> {
        if !verb.allowed_roles().contains(&actor_role) {
            return Err("action primitive is not allowed for actor role");
        }

        if !verb.allowed_rooms().contains(&room) {
            return Err("action primitive is not allowed in the selected room");
        }

        Ok(Self {
            verb,
            actor_role,
            room,
            target_layer,
        })
    }

    pub fn lower(&self) -> CompiledActionPlan {
        let (admission_policy_code, projection_policy_code) = match (self.target_layer, self.verb) {
            (_, ActionPrimitive::Chat | ActionPrimitive::Read | ActionPrimitive::Report) => (
                AdmissionPolicyCode::StorageAlways,
                ProjectionPolicyCode::HotNever,
            ),
            (KnowledgeLayer::Cold, _) => (
                AdmissionPolicyCode::StorageAndColdAlways,
                ProjectionPolicyCode::HotActiveOnly,
            ),
            (KnowledgeLayer::Hot, _) => (
                AdmissionPolicyCode::StorageColdHotOnAttention,
                ProjectionPolicyCode::HotOnAttentionOrReject,
            ),
        };

        CompiledActionPlan {
            object_kind: self.verb.object_kind(),
            flow_phase: self.verb.flow_phase(),
            attention_state: self.verb.attention_state(),
            integrity_overlay: self.verb.integrity_overlay(),
            control_policy_code: self.verb.control_policy_code(),
            writer_policy_code: self.verb.writer_policy_code(),
            route_policy_code: self.verb.route_policy_code(),
            gate_policy_code: self.verb.gate_policy_code(),
            sandbox_policy_code: self.verb.sandbox_policy_code(),
            admission_policy_code,
            projection_policy_code,
            effect_kind: self.verb.effect_kind(),
            supervision_scope: self.verb.supervision_scope(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_layer_write_order, ActionEffectKind, ActionObjectKind, ActionPrimitive,
        ActionRecord, ActionRoom, ActorRole, ControlPolicyCode, GovernancePrinciple,
        KnowledgeLayer, NotaSurfaceAction, ProjectionPolicyCode, RoutePolicyCode, SupervisionScope,
        CANONICAL_LAYER_WRITE_ORDER, FIRST_GUIDING_PRINCIPLE,
    };

    #[test]
    fn cold_hot_dual_track_is_the_first_guiding_principle() {
        assert_eq!(
            FIRST_GUIDING_PRINCIPLE,
            GovernancePrinciple::ColdHotDualTrack
        );
        assert_eq!(FIRST_GUIDING_PRINCIPLE.slug(), "cold_hot_dual_track");
        assert_eq!(
            CANONICAL_LAYER_WRITE_ORDER,
            [KnowledgeLayer::Cold, KnowledgeLayer::Hot]
        );
    }

    #[test]
    fn layer_write_order_rejects_hot_before_cold() {
        let error = validate_layer_write_order(&[KnowledgeLayer::Hot, KnowledgeLayer::Cold])
            .expect_err("expected hot -> cold to violate canonical order");
        assert_eq!(error.offending_index, 1);
    }

    #[test]
    fn nota_surface_actions_are_fixed() {
        assert_eq!(
            ActorRole::Nota.nota_surface_actions(),
            &[
                NotaSurfaceAction::Chat,
                NotaSurfaceAction::Learn,
                NotaSurfaceAction::Do,
            ]
        );
    }

    #[test]
    fn role_primitive_sets_match_the_current_compiler_contract() {
        assert_eq!(
            ActorRole::Arch.primitives(),
            &[
                ActionPrimitive::Shape,
                ActionPrimitive::Split,
                ActionPrimitive::Assign,
                ActionPrimitive::Update,
                ActionPrimitive::Escalate,
            ]
        );
        assert_eq!(
            ActorRole::Dev.primitives(),
            &[
                ActionPrimitive::Prepare,
                ActionPrimitive::Dispatch,
                ActionPrimitive::Review,
                ActionPrimitive::Integrate,
                ActionPrimitive::Repair,
            ]
        );
        assert_eq!(
            ActorRole::Agent.primitives(),
            &[
                ActionPrimitive::Read,
                ActionPrimitive::Make,
                ActionPrimitive::Report,
            ]
        );
    }

    #[test]
    fn action_records_enforce_role_and_room_boundaries() {
        let valid = ActionRecord::new(
            ActorRole::Arch,
            ActionPrimitive::Assign,
            ActionRoom::Strategy,
            KnowledgeLayer::Hot,
        );
        assert!(valid.is_ok());

        let wrong_role = ActionRecord::new(
            ActorRole::Agent,
            ActionPrimitive::Assign,
            ActionRoom::Strategy,
            KnowledgeLayer::Hot,
        );
        assert_eq!(
            wrong_role,
            Err("action primitive is not allowed for actor role")
        );

        let wrong_room = ActionRecord::new(
            ActorRole::Dev,
            ActionPrimitive::Review,
            ActionRoom::Work,
            KnowledgeLayer::Hot,
        );
        assert_eq!(
            wrong_room,
            Err("action primitive is not allowed in the selected room")
        );
    }

    #[test]
    fn lowered_checkpoint_write_surfaces_boundary_compiler_plan() {
        let record = ActionRecord::new(
            ActorRole::Nota,
            ActionPrimitive::Learn,
            ActionRoom::Memory,
            KnowledgeLayer::Cold,
        )
        .expect("checkpoint write should compile");

        let lowered = record.lower();
        assert_eq!(lowered.object_kind, ActionObjectKind::CadenceCheckpoint);
        assert_eq!(
            lowered.control_policy_code,
            ControlPolicyCode::NotaBoundaryWrite
        );
        assert_eq!(lowered.effect_kind, ActionEffectKind::TruthWrite);
        assert_eq!(
            lowered.projection_policy_code,
            ProjectionPolicyCode::HotActiveOnly
        );
        assert_eq!(lowered.supervision_scope, None);
    }

    #[test]
    fn lowered_dispatch_and_review_keep_runtime_policies_explicit() {
        let dispatch = ActionRecord::new(
            ActorRole::Dev,
            ActionPrimitive::Dispatch,
            ActionRoom::Prep,
            KnowledgeLayer::Hot,
        )
        .expect("dispatch should compile");
        let dispatch_lowered = dispatch.lower();
        assert_eq!(
            dispatch_lowered.object_kind,
            ActionObjectKind::RuntimeDispatch
        );
        assert_eq!(
            dispatch_lowered.control_policy_code,
            ControlPolicyCode::DispatchPrep
        );
        assert_eq!(
            dispatch_lowered.route_policy_code,
            RoutePolicyCode::UpwardOnly
        );
        assert_eq!(
            dispatch_lowered.supervision_scope,
            Some(SupervisionScope::DispatchPipeline)
        );

        let review = ActionRecord::new(
            ActorRole::Dev,
            ActionPrimitive::Review,
            ActionRoom::Review,
            KnowledgeLayer::Hot,
        )
        .expect("review should compile");
        let review_lowered = review.lower();
        assert_eq!(review_lowered.object_kind, ActionObjectKind::ReturnReview);
        assert_eq!(
            review_lowered.control_policy_code,
            ControlPolicyCode::ReviewReturn
        );
        assert_eq!(review_lowered.effect_kind, ActionEffectKind::ReviewWrite);
        assert_eq!(
            review_lowered.supervision_scope,
            Some(SupervisionScope::SessionBundle)
        );
    }
}
