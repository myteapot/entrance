use serde::{Deserialize, Serialize};

use crate::core::data_store::DataStore;

/// Global concurrency budget for agent-backed Forge tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParallelBudgetConfig {
    /// Hard cap for simultaneously running agent tasks.
    pub max_concurrent_agents: u32,
    /// Behavior when the hard cap has been reached.
    pub capacity_mode: CapacityMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CapacityMode {
    /// Reject new work once the hard cap has been reached.
    Reject,
    /// Keep new work pending until a slot becomes available.
    Queue,
}

impl Default for ParallelBudgetConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 3,
            capacity_mode: CapacityMode::Queue,
        }
    }
}

/// Future budget slots reserved for follow-up enforcement work.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BudgetSlots {
    /// Per-task token budget (input + output)
    pub task_token_limit: Option<u64>,
    /// Per-task wall-clock time budget (seconds)
    pub task_time_limit_secs: Option<u64>,
    /// Per-transaction total token budget
    pub transaction_token_limit: Option<u64>,
    /// Per-session total token budget
    pub session_token_limit: Option<u64>,
}

/// Result of checking the current global parallel budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetCheckResult {
    /// Dispatch can start immediately.
    Allowed,
    /// Dispatch should remain queued until a slot opens.
    Queued { position: usize },
    /// Dispatch cannot start because the hard cap is exhausted.
    Rejected { running: u32, limit: u32 },
}

pub fn check_parallel_budget(
    data_store: &DataStore,
    config: &ParallelBudgetConfig,
) -> anyhow::Result<BudgetCheckResult> {
    let running = data_store.count_running_forge_tasks()?;
    if running < config.max_concurrent_agents {
        return Ok(BudgetCheckResult::Allowed);
    }

    match config.capacity_mode {
        CapacityMode::Reject => Ok(BudgetCheckResult::Rejected {
            running,
            limit: config.max_concurrent_agents,
        }),
        CapacityMode::Queue => {
            let pending = data_store
                .list_forge_tasks()?
                .into_iter()
                .filter(|task| task.status == "Pending")
                .count();
            Ok(BudgetCheckResult::Queued {
                position: pending + 1,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{
        check_parallel_budget, BudgetCheckResult, BudgetSlots, CapacityMode, ParallelBudgetConfig,
    };
    use crate::core::data_store::{DataStore, MigrationPlan};

    fn test_store() -> Result<DataStore> {
        DataStore::in_memory(MigrationPlan::new(crate::plugins::forge::migrations()))
    }

    fn insert_task_with_status(store: &DataStore, name: &str, status: &str) -> Result<i64> {
        let task_id =
            store.insert_forge_task(name, "echo", r#"["hello"]"#, None, None, "[]", "{}")?;
        if status != "Pending" {
            store.update_forge_task_status(task_id, status, None, None)?;
        }
        Ok(task_id)
    }

    #[test]
    fn parallel_budget_allows_under_limit() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        insert_task_with_status(&store, "running-1", "Running")?;
        let config = ParallelBudgetConfig {
            max_concurrent_agents: 3,
            capacity_mode: CapacityMode::Queue,
        };

        assert_eq!(
            check_parallel_budget(&store, &config)?,
            BudgetCheckResult::Allowed
        );

        Ok(())
    }

    #[test]
    fn parallel_budget_rejects_at_limit() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        insert_task_with_status(&store, "running-1", "Running")?;
        insert_task_with_status(&store, "running-2", "Running")?;
        insert_task_with_status(&store, "running-3", "Running")?;
        let config = ParallelBudgetConfig {
            max_concurrent_agents: 3,
            capacity_mode: CapacityMode::Reject,
        };

        assert_eq!(
            check_parallel_budget(&store, &config)?,
            BudgetCheckResult::Rejected {
                running: 3,
                limit: 3,
            }
        );

        Ok(())
    }

    #[test]
    fn parallel_budget_queues_at_limit() -> Result<()> {
        let _guard = crate::test_env_guard();
        let store = test_store()?;
        insert_task_with_status(&store, "running-1", "Running")?;
        insert_task_with_status(&store, "running-2", "Running")?;
        insert_task_with_status(&store, "running-3", "Running")?;
        insert_task_with_status(&store, "pending-1", "Pending")?;
        let config = ParallelBudgetConfig {
            max_concurrent_agents: 3,
            capacity_mode: CapacityMode::Queue,
        };

        assert_eq!(
            check_parallel_budget(&store, &config)?,
            BudgetCheckResult::Queued { position: 2 }
        );

        Ok(())
    }

    #[test]
    fn budget_slots_default_is_all_none() {
        let _guard = crate::test_env_guard();
        let slots = BudgetSlots::default();

        assert!(slots.task_token_limit.is_none());
        assert!(slots.task_time_limit_secs.is_none());
        assert!(slots.transaction_token_limit.is_none());
        assert!(slots.session_token_limit.is_none());
    }
}
