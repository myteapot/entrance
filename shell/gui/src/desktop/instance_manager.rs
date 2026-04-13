use std::str::FromStr;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::core::{
    data_store::{DataStore, NewAgentInstance, StoredAgentInstance},
    event_bus::EventBus,
    system_heartbeat::AgentTier,
};

/// Instance 角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceRole {
    Nota,
    Arch,
    Dev,
    Agent,
}

impl InstanceRole {
    /// 该角色的下一级子角色。
    pub fn child_role(&self) -> Option<InstanceRole> {
        match self {
            Self::Nota => Some(Self::Arch),
            Self::Arch => Some(Self::Dev),
            Self::Dev => Some(Self::Agent),
            Self::Agent => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Nota => "nota",
            Self::Arch => "arch",
            Self::Dev => "dev",
            Self::Agent => "agent",
        }
    }
}

impl FromStr for InstanceRole {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match normalize_enum_key(value).as_str() {
            "nota" => Ok(Self::Nota),
            "arch" => Ok(Self::Arch),
            "dev" => Ok(Self::Dev),
            "agent" => Ok(Self::Agent),
            _ => Err(anyhow!("unknown instance role `{value}`")),
        }
    }
}

pub struct InstanceManager {
    data_store: DataStore,
    event_bus: EventBus,
}

impl InstanceManager {
    pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
        Self {
            data_store,
            event_bus,
        }
    }

    /// 创建新实例并持久化。
    pub fn create_instance(
        &self,
        role: InstanceRole,
        parent_id: Option<i64>,
        display_name: &str,
        config_json: &str,
        workspace_path: Option<&str>,
        tier: AgentTier,
    ) -> Result<StoredAgentInstance> {
        let instance = self.data_store.insert_agent_instance(NewAgentInstance {
            role: role.as_str(),
            parent_instance_id: parent_id,
            agent_tier: tier.as_str(),
            display_name,
            config_json,
            workspace_path,
        })?;
        self.event_bus
            .publish("instance:created", serde_json::to_string(&instance)?)?;
        Ok(instance)
    }

    /// 自动 spawn 子实例。返回创建的子实例列表。
    pub fn spawn_children(&self, parent_id: i64, count: usize) -> Result<Vec<StoredAgentInstance>> {
        let parent = self
            .data_store
            .get_agent_instance(parent_id)?
            .ok_or_else(|| anyhow!("Instance {parent_id} not found"))?;
        let parent_role: InstanceRole = parent.role.parse()?;
        let child_role = parent_role
            .child_role()
            .ok_or_else(|| anyhow!("{} instances cannot spawn children", parent.role))?;
        let tier: AgentTier = parent.agent_tier.parse()?;

        let mut children = Vec::with_capacity(count);
        for index in 0..count {
            let name = format!("{}-{}-{}", child_role.as_str(), parent_id, index + 1);
            let child =
                self.create_instance(child_role, Some(parent_id), &name, "{}", None, tier)?;
            children.push(child);
        }

        Ok(children)
    }

    /// 递归 stop：先 stop 所有子实例，再 stop 自己。
    pub fn stop_instance(&self, id: i64) -> Result<()> {
        let children = self.data_store.list_child_instances(id)?;
        for child in children {
            if child.status != "Stopped" {
                self.stop_instance(child.id)?;
            }
        }

        self.data_store
            .update_agent_instance_status(id, "Stopped")?;
        self.event_bus
            .publish("instance:stopped", serde_json::to_string(&id)?)?;
        Ok(())
    }

    /// 获取实例树（根 -> 叶）。
    pub fn instance_tree(&self) -> Result<Vec<StoredAgentInstance>> {
        self.data_store.list_agent_instances()
    }

    /// 更新实例心跳。
    pub fn heartbeat(&self, id: i64) -> Result<()> {
        self.data_store.update_agent_instance_heartbeat(id)
    }

    /// 将实例标记为 Busy。
    pub fn mark_busy(&self, id: i64) -> Result<()> {
        self.data_store.update_agent_instance_status(id, "Busy")?;
        self.event_bus
            .publish("instance:busy", serde_json::to_string(&id)?)?;
        Ok(())
    }

    /// 将实例标记为 Idle。
    pub fn mark_idle(&self, id: i64) -> Result<()> {
        self.data_store.update_agent_instance_status(id, "Idle")?;
        self.event_bus
            .publish("instance:idle", serde_json::to_string(&id)?)?;
        Ok(())
    }
}

fn normalize_enum_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;
    use crate::core::{
        data_store::MigrationPlan, event_bus::EventBus, system_heartbeat::AgentTier,
    };

    fn test_store() -> Result<DataStore> {
        DataStore::in_memory(MigrationPlan::new(&[]))
    }

    #[test]
    fn spawn_children_creates_correct_roles() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        let manager = InstanceManager::new(store.clone(), EventBus::new());
        let arch = manager.create_instance(
            InstanceRole::Arch,
            None,
            "arch-root",
            "{}",
            None,
            AgentTier::ArchNota,
        )?;

        let children = manager.spawn_children(arch.id, 2)?;

        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|child| child.role == "dev"));
        assert!(children
            .iter()
            .all(|child| child.parent_instance_id == Some(arch.id)));

        Ok(())
    }

    #[test]
    fn spawn_children_from_agent_fails() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        let manager = InstanceManager::new(store, EventBus::new());
        let agent = manager.create_instance(
            InstanceRole::Agent,
            None,
            "agent-root",
            "{}",
            None,
            AgentTier::ArchNota,
        )?;

        let error = manager
            .spawn_children(agent.id, 1)
            .expect_err("agent instances should not spawn children");

        assert!(error
            .to_string()
            .contains("agent instances cannot spawn children"));

        Ok(())
    }

    #[test]
    fn stop_instance_cascades_to_children() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        let manager = InstanceManager::new(store.clone(), EventBus::new());
        let arch = manager.create_instance(
            InstanceRole::Arch,
            None,
            "arch-root",
            "{}",
            None,
            AgentTier::ArchNota,
        )?;
        let devs = manager.spawn_children(arch.id, 2)?;
        for dev in &devs {
            manager.spawn_children(dev.id, 1)?;
        }

        manager.stop_instance(arch.id)?;

        let instances = manager.instance_tree()?;
        assert!(!instances.is_empty());
        assert!(instances
            .iter()
            .all(|instance| instance.status == "Stopped"));

        Ok(())
    }

    #[test]
    fn child_role_mapping() {
        let _guard = crate::test_env_guard();

        assert_eq!(InstanceRole::Nota.child_role(), Some(InstanceRole::Arch));
        assert_eq!(InstanceRole::Arch.child_role(), Some(InstanceRole::Dev));
        assert_eq!(InstanceRole::Dev.child_role(), Some(InstanceRole::Agent));
        assert_eq!(InstanceRole::Agent.child_role(), None);
    }
}
