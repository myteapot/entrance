use serde::Serialize;

use crate::core::action::{ActionPrimitive, ActionRecord, ActionRoom, ActorRole, KnowledgeLayer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolPermission {
    pub actor_role: ActorRole,
    pub primitive: ActionPrimitive,
    pub room: ActionRoom,
    pub target_layer: KnowledgeLayer,
}

impl McpToolPermission {
    pub const fn new(
        actor_role: ActorRole,
        primitive: ActionPrimitive,
        room: ActionRoom,
        target_layer: KnowledgeLayer,
    ) -> Self {
        Self {
            actor_role,
            primitive,
            room,
            target_layer,
        }
    }

    pub fn action_record(self) -> Result<ActionRecord, &'static str> {
        ActionRecord::new(
            self.actor_role,
            self.primitive,
            self.room,
            self.target_layer,
        )
    }
}

const HOT_PREP_DEV: McpToolPermission = McpToolPermission::new(
    ActorRole::Dev,
    ActionPrimitive::Prepare,
    ActionRoom::Prep,
    KnowledgeLayer::Hot,
);

const HOT_DISPATCH_DEV: McpToolPermission = McpToolPermission::new(
    ActorRole::Dev,
    ActionPrimitive::Dispatch,
    ActionRoom::Prep,
    KnowledgeLayer::Hot,
);

const HOT_ASSIGN_ARCH: McpToolPermission = McpToolPermission::new(
    ActorRole::Arch,
    ActionPrimitive::Assign,
    ActionRoom::Strategy,
    KnowledgeLayer::Hot,
);

const HOT_ASSIGN_NOTA: McpToolPermission = McpToolPermission::new(
    ActorRole::Nota,
    ActionPrimitive::Assign,
    ActionRoom::Strategy,
    KnowledgeLayer::Hot,
);

const HOT_DO_NOTA: McpToolPermission = McpToolPermission::new(
    ActorRole::Nota,
    ActionPrimitive::Assign,
    ActionRoom::Strategy,
    KnowledgeLayer::Hot,
);

const COLD_CHAT_NOTA: McpToolPermission = McpToolPermission::new(
    ActorRole::Nota,
    ActionPrimitive::Chat,
    ActionRoom::Surface,
    KnowledgeLayer::Cold,
);

const COLD_LEARN_NOTA: McpToolPermission = McpToolPermission::new(
    ActorRole::Nota,
    ActionPrimitive::Learn,
    ActionRoom::Memory,
    KnowledgeLayer::Cold,
);

pub fn permission_for_mcp_tool(name: &str) -> Option<McpToolPermission> {
    match name {
        "nota_runtime_overview"
        | "nota_runtime_status"
        | "nota_runtime_allocations"
        | "nota_runtime_receipts" => Some(COLD_CHAT_NOTA),
        "nota_do" | "nota_dev" => Some(HOT_DO_NOTA),
        "nota_write_checkpoint" => Some(COLD_LEARN_NOTA),
        "forge_prepare_dispatch"
        | "forge_verify_dispatch"
        | "forge_prepare_agent_dispatch"
        | "forge_verify_agent_dispatch" => Some(HOT_PREP_DEV),
        "forge_dispatch_agent" => Some(HOT_DISPATCH_DEV),
        "forge_bootstrap_mcp_cycle" => Some(HOT_ASSIGN_NOTA),
        "forge_prepare_dev_dispatch" | "forge_verify_dev_dispatch" | "forge_dispatch_dev" => {
            Some(HOT_ASSIGN_ARCH)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{permission_for_mcp_tool, ActionPrimitive, ActionRoom, ActorRole, KnowledgeLayer};

    #[test]
    fn current_dispatch_tools_are_mapped_into_valid_action_records() {
        for name in [
            "nota_runtime_overview",
            "nota_runtime_status",
            "nota_runtime_allocations",
            "nota_runtime_receipts",
            "nota_do",
            "nota_dev",
            "nota_write_checkpoint",
            "forge_prepare_dispatch",
            "forge_verify_dispatch",
            "forge_prepare_agent_dispatch",
            "forge_verify_agent_dispatch",
            "forge_dispatch_agent",
            "forge_bootstrap_mcp_cycle",
            "forge_prepare_dev_dispatch",
            "forge_verify_dev_dispatch",
            "forge_dispatch_dev",
        ] {
            let permission = permission_for_mcp_tool(name)
                .unwrap_or_else(|| panic!("expected permission mapping for `{name}`"));
            permission
                .action_record()
                .unwrap_or_else(|error| panic!("invalid permission mapping for `{name}`: {error}"));
        }
    }

    #[test]
    fn agent_lane_tools_stay_dev_owned_prep_and_dispatch_surface() {
        assert_eq!(
            permission_for_mcp_tool("forge_prepare_dispatch"),
            Some(super::McpToolPermission::new(
                ActorRole::Dev,
                ActionPrimitive::Prepare,
                ActionRoom::Prep,
                KnowledgeLayer::Hot,
            ))
        );
        assert_eq!(
            permission_for_mcp_tool("forge_dispatch_agent"),
            Some(super::McpToolPermission::new(
                ActorRole::Dev,
                ActionPrimitive::Dispatch,
                ActionRoom::Prep,
                KnowledgeLayer::Hot,
            ))
        );
    }

    #[test]
    fn dev_lane_bootstrap_tools_are_currently_arch_owned_assignment_surface() {
        assert_eq!(
            permission_for_mcp_tool("forge_prepare_dev_dispatch"),
            Some(super::McpToolPermission::new(
                ActorRole::Arch,
                ActionPrimitive::Assign,
                ActionRoom::Strategy,
                KnowledgeLayer::Hot,
            ))
        );
        assert_eq!(
            permission_for_mcp_tool("forge_dispatch_dev"),
            Some(super::McpToolPermission::new(
                ActorRole::Arch,
                ActionPrimitive::Assign,
                ActionRoom::Strategy,
                KnowledgeLayer::Hot,
            ))
        );
    }

    #[test]
    fn bootstrap_allocator_tool_is_currently_nota_owned_assignment_surface() {
        assert_eq!(
            permission_for_mcp_tool("forge_bootstrap_mcp_cycle"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Assign,
                ActionRoom::Strategy,
                KnowledgeLayer::Hot,
            ))
        );
    }

    #[test]
    fn nota_runtime_overview_is_nota_owned_cold_surface_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_runtime_overview"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Chat,
                ActionRoom::Surface,
                KnowledgeLayer::Cold,
            ))
        );
    }

    #[test]
    fn nota_runtime_status_is_nota_owned_cold_surface_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_runtime_status"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Chat,
                ActionRoom::Surface,
                KnowledgeLayer::Cold,
            ))
        );
    }

    #[test]
    fn nota_runtime_allocations_is_nota_owned_cold_surface_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_runtime_allocations"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Chat,
                ActionRoom::Surface,
                KnowledgeLayer::Cold,
            ))
        );
    }

    #[test]
    fn nota_runtime_receipts_is_nota_owned_cold_surface_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_runtime_receipts"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Chat,
                ActionRoom::Surface,
                KnowledgeLayer::Cold,
            ))
        );
    }

    #[test]
    fn nota_write_checkpoint_is_nota_owned_cold_memory_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_write_checkpoint"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Learn,
                ActionRoom::Memory,
                KnowledgeLayer::Cold,
            ))
        );
    }

    #[test]
    fn nota_do_is_nota_owned_hot_strategy_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_do"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Assign,
                ActionRoom::Strategy,
                KnowledgeLayer::Hot,
            ))
        );
    }

    #[test]
    fn nota_dev_is_nota_owned_hot_strategy_tool() {
        assert_eq!(
            permission_for_mcp_tool("nota_dev"),
            Some(super::McpToolPermission::new(
                ActorRole::Nota,
                ActionPrimitive::Assign,
                ActionRoom::Strategy,
                KnowledgeLayer::Hot,
            ))
        );
    }

    #[test]
    fn unmapped_tools_remain_permission_neutral_for_now() {
        assert_eq!(permission_for_mcp_tool("forge_status"), None);
        assert_eq!(permission_for_mcp_tool("vault_get_token"), None);
    }
}
