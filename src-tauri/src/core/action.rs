use serde::{Deserialize, Serialize};

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
}

#[cfg(test)]
mod tests {
    use super::{
        validate_layer_write_order, ActionPrimitive, ActionRecord, ActionRoom, ActorRole,
        GovernancePrinciple, KnowledgeLayer, NotaSurfaceAction, CANONICAL_LAYER_WRITE_ORDER,
        FIRST_GUIDING_PRINCIPLE,
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
}
