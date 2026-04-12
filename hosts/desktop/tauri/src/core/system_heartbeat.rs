use std::{str::FromStr, time::Duration};

use anyhow::{anyhow, Result};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};

use crate::core::{data_store::DataStore, event_bus::EventBus};

/// 代理层级，决定心跳行为和 NOTA 载体。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgentTier {
    /// Level 1: Human only. 心跳禁用。
    Solo = 1,
    /// Level 2: Dev(NOTA) -> Agent. Dev 自带 NOTA 特质。
    DevNota = 2,
    /// Level 3: Arch(NOTA) -> Dev -> Agent. Arch 自带 NOTA 特质。默认。
    ArchNota = 3,
    /// Level 4: NOTA -> Arch -> Dev -> Agent. NOTA 独立身份。实验性。
    FullNota = 4,
}

impl Default for AgentTier {
    fn default() -> Self {
        Self::ArchNota
    }
}

impl AgentTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::DevNota => "DevNota",
            Self::ArchNota => "ArchNota",
            Self::FullNota => "FullNota",
        }
    }
}

impl FromStr for AgentTier {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match normalize_enum_key(value).as_str() {
            "solo" => Ok(Self::Solo),
            "devnota" => Ok(Self::DevNota),
            "archnota" => Ok(Self::ArchNota),
            "fullnota" => Ok(Self::FullNota),
            _ => Err(anyhow!("unknown agent tier `{value}`")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub agent_tier: AgentTier,
    /// tick 间隔（秒）。Settings 可调。
    pub tick_interval_secs: u64,
    /// 连续 miss N 次 tick 才报 stale。Advanced Settings 可调。
    pub stale_threshold_multiplier: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            agent_tier: AgentTier::default(),
            tick_interval_secs: 30,
            stale_threshold_multiplier: 3,
        }
    }
}

impl HeartbeatConfig {
    pub fn is_enabled(&self) -> bool {
        self.agent_tier >= AgentTier::DevNota
    }

    pub fn tick_interval(&self) -> Duration {
        Duration::from_secs(self.tick_interval_secs)
    }

    pub fn stale_duration(&self) -> Duration {
        self.tick_interval() * self.stale_threshold_multiplier
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SystemHealth {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemPulse {
    pub timestamp: String,
    pub agent_tier: AgentTier,
    pub active_tasks: u32,
    pub stale_tasks: u32,
    pub pending_approvals: u32,
    pub pending_work: u32,
    pub total_instances: u32,
    pub active_instances: u32,
    pub stale_instances: u32,
    pub stopped_instances: u32,
    pub health: SystemHealth,
    pub tick_interval_secs: u64,
    pub stale_threshold_multiplier: u32,
}

pub async fn run_system_heartbeat(
    data_store: DataStore,
    event_bus: EventBus,
    config: HeartbeatConfig,
) {
    if !config.is_enabled() {
        return;
    }

    let mut interval = tokio::time::interval(config.tick_interval());
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        let pulse = match compute_pulse(&data_store, &config) {
            Ok(pulse) => pulse,
            Err(error) => {
                tracing::warn!(?error, "failed to compute system heartbeat pulse");
                continue;
            }
        };

        publish_pulse(&event_bus, "system:pulse", &pulse);

        if should_notify_human(&pulse, &config) {
            publish_pulse(&event_bus, "system:attention", &pulse);
        }
    }
}

fn publish_pulse(event_bus: &EventBus, topic: &str, pulse: &SystemPulse) {
    let payload = match serde_json::to_string(pulse) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(?error, topic, "failed to serialize system heartbeat pulse");
            return;
        }
    };

    if let Err(error) = event_bus.publish(topic, payload) {
        tracing::warn!(?error, topic, "failed to publish system heartbeat pulse");
    }
}

fn should_notify_human(pulse: &SystemPulse, config: &HeartbeatConfig) -> bool {
    match config.agent_tier {
        AgentTier::Solo => false,
        AgentTier::DevNota => pulse.stale_tasks > 0 || pulse.stale_instances > 0,
        AgentTier::ArchNota => {
            pulse.stale_tasks > 0 || pulse.stale_instances > 0 || pulse.pending_approvals > 0
        }
        AgentTier::FullNota => {
            !matches!(pulse.health, SystemHealth::Green)
                || pulse.pending_approvals > 0
                || pulse.pending_work > 0
        }
    }
}

pub fn compute_pulse(data_store: &DataStore, config: &HeartbeatConfig) -> Result<SystemPulse> {
    let now = Utc::now();
    let stale_cutoff = (now - ChronoDuration::from_std(config.stale_duration())?).to_rfc3339();

    let tasks = data_store.list_forge_tasks()?;
    let active_tasks = tasks.iter().filter(|task| task.status == "Running").count() as u32;
    let stale_tasks = tasks
        .iter()
        .filter(|task| {
            task.status == "Running"
                && match &task.heartbeat_at {
                    Some(heartbeat_at) => heartbeat_at.as_str() < stale_cutoff.as_str(),
                    None => true,
                }
        })
        .count() as u32;
    let pending_work = tasks.iter().filter(|task| task.status == "Pending").count() as u32;
    let failed_unhandled = tasks.iter().filter(|task| task.status == "Failed").count() as u32;
    let pending_approvals = data_store
        .list_nota_runtime_allocations()?
        .into_iter()
        .filter(|allocation| allocation.status == "pending_approval")
        .count() as u32;
    let instances = data_store.list_agent_instances()?;
    let total_instances = instances.len() as u32;
    let active_instances = instances
        .iter()
        .filter(|instance| matches!(instance.status.as_str(), "Idle" | "Busy"))
        .count() as u32;
    let stale_instances = instances
        .iter()
        .filter(|instance| {
            instance.status == "Stale"
                || (matches!(instance.status.as_str(), "Idle" | "Busy")
                    && match &instance.last_heartbeat_at {
                        Some(heartbeat_at) => heartbeat_at.as_str() < stale_cutoff.as_str(),
                        None => false,
                    })
        })
        .count() as u32;
    let stopped_instances = instances
        .iter()
        .filter(|instance| instance.status == "Stopped")
        .count() as u32;

    let has_stale_instances = stale_instances > 0;
    let health = if (stale_tasks > 0 || has_stale_instances) && failed_unhandled > 0 {
        SystemHealth::Red
    } else if stale_tasks > 0 || has_stale_instances || failed_unhandled > 0 {
        SystemHealth::Yellow
    } else {
        SystemHealth::Green
    };

    Ok(SystemPulse {
        timestamp: now.to_rfc3339(),
        agent_tier: config.agent_tier,
        active_tasks,
        stale_tasks,
        pending_approvals,
        pending_work,
        total_instances,
        active_instances,
        stale_instances,
        stopped_instances,
        health,
        tick_interval_secs: config.tick_interval_secs,
        stale_threshold_multiplier: config.stale_threshold_multiplier,
    })
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
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use anyhow::Result;
    use chrono::{Duration as ChronoDuration, Utc};
    use rusqlite::params;

    use super::*;
    use crate::core::data_store::{
        MigrationPlan, NewAgentInstance, NewNotaRuntimeAllocation, NewNotaRuntimeTransaction,
    };

    #[test]
    fn default_tier_is_arch_nota() {
        let _guard = crate::test_env_guard();

        assert_eq!(AgentTier::default(), AgentTier::ArchNota);
    }

    #[test]
    fn solo_disables_heartbeat() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            agent_tier: AgentTier::Solo,
            ..Default::default()
        };
        assert!(!config.is_enabled());
    }

    #[test]
    fn dev_nota_enables_heartbeat() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            agent_tier: AgentTier::DevNota,
            ..Default::default()
        };
        assert!(config.is_enabled());
    }

    #[test]
    fn stale_duration_is_multiplied() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            tick_interval_secs: 30,
            stale_threshold_multiplier: 3,
            ..Default::default()
        };

        assert_eq!(config.stale_duration(), Duration::from_secs(90));
    }

    #[test]
    fn compute_pulse_detects_stale_tasks() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        let task_id = insert_task(&store, "Running")?;
        set_task_heartbeat_at(
            &store,
            task_id,
            (Utc::now() - ChronoDuration::seconds(180))
                .to_rfc3339()
                .as_str(),
        )?;

        let pulse = compute_pulse(&store, &HeartbeatConfig::default())?;

        assert_eq!(pulse.active_tasks, 1);
        assert_eq!(pulse.stale_tasks, 1);
        assert_eq!(pulse.health, SystemHealth::Yellow);

        Ok(())
    }

    #[test]
    fn compute_pulse_green_when_healthy() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        insert_task(&store, "Running")?;

        let pulse = compute_pulse(&store, &HeartbeatConfig::default())?;

        assert_eq!(pulse.active_tasks, 1);
        assert_eq!(pulse.stale_tasks, 0);
        assert_eq!(pulse.health, SystemHealth::Green);

        Ok(())
    }

    #[test]
    fn compute_pulse_red_when_stale_and_failed() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        let stale_task_id = insert_task(&store, "Running")?;
        set_task_heartbeat_at(
            &store,
            stale_task_id,
            (Utc::now() - ChronoDuration::seconds(180))
                .to_rfc3339()
                .as_str(),
        )?;
        insert_task(&store, "Failed")?;

        let pulse = compute_pulse(&store, &HeartbeatConfig::default())?;

        assert_eq!(pulse.active_tasks, 1);
        assert_eq!(pulse.stale_tasks, 1);
        assert_eq!(pulse.health, SystemHealth::Red);

        Ok(())
    }

    #[test]
    fn compute_pulse_includes_instance_counts() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        insert_instance(&store, "Idle")?;
        let busy = insert_instance(&store, "Busy")?;
        insert_instance(&store, "Stopped")?;
        set_instance_heartbeat_at(
            &store,
            busy,
            (Utc::now() - ChronoDuration::seconds(180))
                .to_rfc3339()
                .as_str(),
        )?;

        let pulse = compute_pulse(&store, &HeartbeatConfig::default())?;

        assert_eq!(pulse.total_instances, 3);
        assert_eq!(pulse.active_instances, 2);
        assert_eq!(pulse.stale_instances, 1);
        assert_eq!(pulse.stopped_instances, 1);

        Ok(())
    }

    #[test]
    fn stale_instances_affect_health() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        let busy = insert_instance(&store, "Busy")?;
        set_instance_heartbeat_at(
            &store,
            busy,
            (Utc::now() - ChronoDuration::seconds(180))
                .to_rfc3339()
                .as_str(),
        )?;

        let pulse = compute_pulse(&store, &HeartbeatConfig::default())?;

        assert_eq!(pulse.stale_instances, 1);
        assert_eq!(pulse.health, SystemHealth::Yellow);

        Ok(())
    }

    #[test]
    fn notify_dev_nota_only_on_stale() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            agent_tier: AgentTier::DevNota,
            ..Default::default()
        };
        let healthy = sample_pulse(SystemHealth::Green);
        let stale = SystemPulse {
            stale_tasks: 1,
            health: SystemHealth::Yellow,
            ..sample_pulse(SystemHealth::Yellow)
        };

        assert!(!should_notify_human(&healthy, &config));
        assert!(should_notify_human(&stale, &config));
    }

    #[test]
    fn notify_dev_nota_on_stale_instance() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            agent_tier: AgentTier::DevNota,
            ..Default::default()
        };
        let stale_instance = SystemPulse {
            stale_instances: 1,
            health: SystemHealth::Yellow,
            ..sample_pulse(SystemHealth::Yellow)
        };

        assert!(should_notify_human(&stale_instance, &config));
    }

    #[test]
    fn notify_arch_nota_on_stale_or_approval() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            agent_tier: AgentTier::ArchNota,
            ..Default::default()
        };
        let healthy = sample_pulse(SystemHealth::Green);
        let stale = SystemPulse {
            stale_tasks: 1,
            health: SystemHealth::Yellow,
            ..sample_pulse(SystemHealth::Yellow)
        };
        let approval = SystemPulse {
            pending_approvals: 1,
            ..sample_pulse(SystemHealth::Green)
        };

        assert!(!should_notify_human(&healthy, &config));
        assert!(should_notify_human(&stale, &config));
        assert!(should_notify_human(&approval, &config));
    }

    #[test]
    fn notify_full_nota_on_any_non_green() {
        let _guard = crate::test_env_guard();

        let config = HeartbeatConfig {
            agent_tier: AgentTier::FullNota,
            ..Default::default()
        };
        let healthy = sample_pulse(SystemHealth::Green);
        let warning = sample_pulse(SystemHealth::Yellow);
        let pending = SystemPulse {
            pending_work: 1,
            ..sample_pulse(SystemHealth::Green)
        };

        assert!(!should_notify_human(&healthy, &config));
        assert!(should_notify_human(&warning, &config));
        assert!(should_notify_human(&pending, &config));
    }

    #[test]
    fn tier_ordering() {
        let _guard = crate::test_env_guard();

        assert!(AgentTier::Solo < AgentTier::DevNota);
        assert!(AgentTier::DevNota < AgentTier::ArchNota);
        assert!(AgentTier::ArchNota < AgentTier::FullNota);
    }

    #[test]
    fn compute_pulse_counts_pending_approvals() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        insert_pending_approval_allocation(&store)?;

        let pulse = compute_pulse(&store, &HeartbeatConfig::default())?;

        assert_eq!(pulse.pending_approvals, 1);

        Ok(())
    }

    fn sample_pulse(health: SystemHealth) -> SystemPulse {
        SystemPulse {
            timestamp: "2026-04-05T00:00:00Z".to_string(),
            agent_tier: AgentTier::ArchNota,
            active_tasks: 0,
            stale_tasks: 0,
            pending_approvals: 0,
            pending_work: 0,
            total_instances: 0,
            active_instances: 0,
            stale_instances: 0,
            stopped_instances: 0,
            health,
            tick_interval_secs: 30,
            stale_threshold_multiplier: 3,
        }
    }

    fn test_store() -> Result<DataStore> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let db_path = std::env::temp_dir()
            .join(format!(
                "entrance-heartbeat-{}-{unique}",
                std::process::id()
            ))
            .join("heartbeat.db");
        let store = DataStore::open(
            db_path,
            MigrationPlan::new(crate::hosts::plugins::forge::migrations()),
        )?;
        Ok(store)
    }

    fn insert_task(store: &DataStore, status: &str) -> Result<i64> {
        let task_id =
            store.insert_forge_task("Heartbeat test", "echo", "[]", None, None, "[]", "{}")?;
        if status != "Pending" {
            store.update_forge_task_status(task_id, status, None, None)?;
        }
        Ok(task_id)
    }

    fn set_task_heartbeat_at(store: &DataStore, task_id: i64, heartbeat_at: &str) -> Result<()> {
        let connection = rusqlite::Connection::open(store.path())?;
        connection.execute(
            "UPDATE plugin_forge_tasks SET heartbeat_at = ?2 WHERE id = ?1",
            params![task_id, heartbeat_at],
        )?;
        Ok(())
    }

    fn insert_instance(store: &DataStore, status: &str) -> Result<i64> {
        let instance = store.insert_agent_instance(NewAgentInstance {
            role: "agent",
            parent_instance_id: None,
            agent_tier: "ArchNota",
            display_name: "heartbeat-agent",
            config_json: "{}",
            workspace_path: None,
        })?;
        if status != "Idle" {
            store.update_agent_instance_status(instance.id, status)?;
        }
        Ok(instance.id)
    }

    fn set_instance_heartbeat_at(
        store: &DataStore,
        instance_id: i64,
        heartbeat_at: &str,
    ) -> Result<()> {
        let connection = rusqlite::Connection::open(store.path())?;
        connection.execute(
            "UPDATE agent_instances SET last_heartbeat_at = ?2 WHERE id = ?1",
            params![instance_id, heartbeat_at],
        )?;
        Ok(())
    }

    fn insert_pending_approval_allocation(store: &DataStore) -> Result<()> {
        let transaction = store.insert_nota_runtime_transaction(NewNotaRuntimeTransaction {
            actor_role: "arch",
            surface_action: "heartbeat test",
            transaction_kind: "agent_dispatch",
            title: "Heartbeat approval",
            payload_json: "{}",
            status: "task_created",
            forge_task_id: None,
            cadence_checkpoint_id: None,
        })?;
        let return_target_ref = transaction.id.to_string();

        store.insert_nota_runtime_allocation(NewNotaRuntimeAllocation {
            allocator_role: "arch",
            allocator_surface: "heartbeat",
            allocation_kind: "approval",
            source_transaction_id: transaction.id,
            lineage_ref: "heartbeat:test",
            child_execution_kind: "forge_task",
            child_execution_ref: "task:1",
            return_target_kind: "nota_runtime_transaction",
            return_target_ref: &return_target_ref,
            escalation_target_kind: "human",
            escalation_target_ref: "review",
            status: "pending_approval",
            payload_json: "{}",
        })?;

        Ok(())
    }
}
