use anyhow::Result;
use serde::Serialize;

use crate::core::data_store::{DataStore, NewAntiZenoEvent, StoredAntiZenoEvent};

const SEMANTIC_BUDGET_LIMIT: i64 = 6;
const REPAIR_BUDGET_LIMIT: i64 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct AntiZenoBudgetReport {
    pub semantic_budget_limit: i64,
    pub repair_budget_limit: i64,
    pub semantic_event_count: i64,
    pub boundary_anchor_count: i64,
    pub acceptance_event_count: i64,
    pub closure_event_count: i64,
    pub repair_event_count: i64,
    pub projection_debt_count: usize,
    pub budget_exhausted: bool,
    pub state: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forced_action: Option<String>,
    pub recent_events: Vec<StoredAntiZenoEvent>,
}

pub fn build_anti_zeno_budget_report(
    data_store: &DataStore,
    current_checkpoint_id: Option<i64>,
    current_acceptance_bundle_id: Option<i64>,
    acceptance_present: bool,
    fully_settled: bool,
    next_step_open: bool,
    projection_debt_count: usize,
) -> Result<AntiZenoBudgetReport> {
    let scoped_events = data_store
        .list_anti_zeno_events()?
        .into_iter()
        .filter(|event| {
            anti_zeno_event_matches_current_round(
                event,
                current_checkpoint_id,
                current_acceptance_bundle_id,
            )
        })
        .collect::<Vec<_>>();

    let semantic_event_count = scoped_events
        .iter()
        .filter(|event| event.budget_axis == "semantic")
        .map(|event| event.event_weight)
        .sum::<i64>();
    let repair_event_count = scoped_events
        .iter()
        .filter(|event| event.budget_axis == "repair")
        .map(|event| event.event_weight)
        .sum::<i64>();
    let boundary_anchor_count = scoped_events
        .iter()
        .filter(|event| event.event_kind == "checkpoint_written")
        .map(|event| event.event_weight)
        .sum::<i64>();
    let acceptance_event_count = scoped_events
        .iter()
        .filter(|event| event.event_kind == "acceptance_recorded")
        .map(|event| event.event_weight)
        .sum::<i64>();
    let closure_event_count = scoped_events
        .iter()
        .filter(|event| event.event_kind == "closure_recorded")
        .map(|event| event.event_weight)
        .sum::<i64>();
    let total_pressure = semantic_event_count + repair_event_count + projection_debt_count as i64;

    let (state, summary, forced_action) = if fully_settled && projection_debt_count == 0 {
        (
            "settled".to_string(),
            format!(
                "The current round is fully settled with anchors={} acceptance={} closure={} and no outstanding anti-Zeno pressure.",
                boundary_anchor_count, acceptance_event_count, closure_event_count
            ),
            None,
        )
    } else if repair_event_count >= REPAIR_BUDGET_LIMIT || total_pressure >= SEMANTIC_BUDGET_LIMIT {
        (
            "budget_exhausted".to_string(),
            format!(
                "Anti-Zeno budget is exhausted at semantic={} anchor={} acceptance={} closure={} repair={} projection_debt={}.",
                semantic_event_count,
                boundary_anchor_count,
                acceptance_event_count,
                closure_event_count,
                repair_event_count,
                projection_debt_count
            ),
            Some(
                "Force bounded closure, repair, or explicit human decision before opening another recursive cut."
                    .to_string(),
            ),
        )
    } else if acceptance_present && closure_event_count == 0 && !next_step_open {
        (
            "closure_required".to_string(),
            format!(
                "Acceptance is recorded with anchors={} acceptance={}, but no closure event has carried the round forward yet.",
                boundary_anchor_count, acceptance_event_count
            ),
            Some(
                "Write the closure checkpoint that carries the accepted boundary forward before reopening the cycle."
                    .to_string(),
            ),
        )
    } else if repair_event_count > 0 || projection_debt_count > 0 {
        (
            "repair_required".to_string(),
            format!(
                "Anti-Zeno pressure is currently repair-weighted with repair={} projection_debt={} and closure={}.",
                repair_event_count, projection_debt_count, closure_event_count
            ),
            Some("Close the repair lane or refresh dirty projections before deepening the cycle.".to_string()),
        )
    } else if next_step_open {
        (
            "bounded_followup".to_string(),
            format!(
                "A bounded next-step is open within the current anti-Zeno envelope; anchors={} acceptance={} closure={}.",
                boundary_anchor_count, acceptance_event_count, closure_event_count
            ),
            None,
        )
    } else if acceptance_present {
        (
            "accepted_waiting_closure".to_string(),
            format!(
                "Acceptance is present and the budget is still healthy, but closure has not fully settled yet; anchors={} acceptance={} closure={}.",
                boundary_anchor_count, acceptance_event_count, closure_event_count
            ),
            None,
        )
    } else if current_checkpoint_id.is_some() {
        (
            "checkpointed".to_string(),
            format!(
                "A checkpoint exists and anti-Zeno tracking is live with anchors={}, but acceptance has not landed yet.",
                boundary_anchor_count
            ),
            None,
        )
    } else {
        (
            "uncheckpointed".to_string(),
            "No checkpoint anchors the current round yet, so the anti-Zeno budget cannot constrain replay effectively.".to_string(),
            None,
        )
    };

    Ok(AntiZenoBudgetReport {
        semantic_budget_limit: SEMANTIC_BUDGET_LIMIT,
        repair_budget_limit: REPAIR_BUDGET_LIMIT,
        semantic_event_count,
        boundary_anchor_count,
        acceptance_event_count,
        closure_event_count,
        repair_event_count,
        projection_debt_count,
        budget_exhausted: state == "budget_exhausted",
        state,
        summary,
        forced_action,
        recent_events: scoped_events.into_iter().take(10).collect(),
    })
}

fn anti_zeno_event_matches_current_round(
    event: &StoredAntiZenoEvent,
    current_checkpoint_id: Option<i64>,
    current_acceptance_bundle_id: Option<i64>,
) -> bool {
    match current_checkpoint_id {
        Some(checkpoint_id) => {
            event.checkpoint_id == Some(checkpoint_id)
                || current_acceptance_bundle_id
                    .map(|acceptance_bundle_id| {
                        event.acceptance_bundle_id == Some(acceptance_bundle_id)
                    })
                    .unwrap_or(false)
        }
        None => {
            event.checkpoint_id.is_none()
                && current_acceptance_bundle_id
                    .map(|acceptance_bundle_id| {
                        event.acceptance_bundle_id == Some(acceptance_bundle_id)
                    })
                    .unwrap_or(true)
        }
    }
}

pub fn record_checkpoint_written_event(
    data_store: &DataStore,
    checkpoint_id: i64,
    summary: &str,
) -> Result<StoredAntiZenoEvent> {
    data_store.insert_anti_zeno_event(NewAntiZenoEvent {
        checkpoint_id: Some(checkpoint_id),
        acceptance_bundle_id: None,
        event_kind: "checkpoint_written",
        boundary_ref: "checkpoint",
        budget_axis: "semantic",
        event_weight: 1,
        summary,
    })
}

pub fn record_acceptance_recorded_event(
    data_store: &DataStore,
    checkpoint_id: i64,
    acceptance_bundle_id: i64,
    boundary_ref: &str,
    summary: &str,
) -> Result<StoredAntiZenoEvent> {
    data_store.insert_anti_zeno_event(NewAntiZenoEvent {
        checkpoint_id: Some(checkpoint_id),
        acceptance_bundle_id: Some(acceptance_bundle_id),
        event_kind: "acceptance_recorded",
        boundary_ref,
        budget_axis: "semantic",
        event_weight: 1,
        summary,
    })
}

pub fn record_closure_recorded_event(
    data_store: &DataStore,
    checkpoint_id: i64,
    acceptance_bundle_id: i64,
    boundary_ref: &str,
    summary: &str,
) -> Result<StoredAntiZenoEvent> {
    data_store.insert_anti_zeno_event(NewAntiZenoEvent {
        checkpoint_id: Some(checkpoint_id),
        acceptance_bundle_id: Some(acceptance_bundle_id),
        event_kind: "closure_recorded",
        boundary_ref,
        budget_axis: "semantic",
        event_weight: 1,
        summary,
    })
}

pub fn record_repair_requested_event(
    data_store: &DataStore,
    checkpoint_id: Option<i64>,
    acceptance_bundle_id: Option<i64>,
    boundary_ref: &str,
    summary: &str,
) -> Result<StoredAntiZenoEvent> {
    data_store.insert_anti_zeno_event(NewAntiZenoEvent {
        checkpoint_id,
        acceptance_bundle_id,
        event_kind: "repair_requested",
        boundary_ref,
        budget_axis: "repair",
        event_weight: 1,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::core::data_store::{DataStore, MigrationPlan};

    use super::{
        build_anti_zeno_budget_report, record_acceptance_recorded_event,
        record_checkpoint_written_event, record_closure_recorded_event,
        record_repair_requested_event,
    };

    struct TempDbPath {
        root: PathBuf,
        db_path: PathBuf,
    }

    impl TempDbPath {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-anti-zeno-{label}-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            let db_path = root.join("data").join("entrance.db");
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent)?;
            }
            Ok(Self { root, db_path })
        }

        fn path(&self) -> &Path {
            &self.db_path
        }
    }

    impl Drop for TempDbPath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn anti_zeno_budget_exhausts_after_repair_pressure_and_projection_debt() -> Result<()> {
        let temp_db = TempDbPath::new("budget")?;
        let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
        let store = DataStore::open(temp_db.path(), migration_plan)?;

        record_checkpoint_written_event(&store, 7, "checkpoint written")?;
        record_acceptance_recorded_event(&store, 7, 9, "acceptance", "acceptance recorded")?;
        record_repair_requested_event(&store, Some(7), Some(9), "repair-1", "repair requested")?;
        record_repair_requested_event(&store, Some(7), Some(9), "repair-2", "repair requested")?;
        record_repair_requested_event(&store, Some(7), Some(9), "repair-3", "repair requested")?;

        let report = build_anti_zeno_budget_report(&store, Some(7), Some(9), true, false, true, 1)?;
        assert_eq!(report.state, "budget_exhausted");
        assert!(report.budget_exhausted);
        assert_eq!(report.repair_event_count, 3);
        assert_eq!(report.projection_debt_count, 1);
        assert!(report.forced_action.is_some());

        Ok(())
    }

    #[test]
    fn anti_zeno_budget_scopes_checkpointed_round_without_acceptance_to_current_checkpoint(
    ) -> Result<()> {
        let temp_db = TempDbPath::new("checkpoint-scope")?;
        let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
        let store = DataStore::open(temp_db.path(), migration_plan)?;

        record_checkpoint_written_event(&store, 3, "old checkpoint")?;
        record_repair_requested_event(&store, Some(3), None, "repair-old", "old repair")?;
        record_checkpoint_written_event(&store, 7, "current checkpoint")?;

        let report = build_anti_zeno_budget_report(&store, Some(7), None, false, false, false, 0)?;
        assert_eq!(report.semantic_event_count, 1);
        assert_eq!(report.boundary_anchor_count, 1);
        assert_eq!(report.acceptance_event_count, 0);
        assert_eq!(report.closure_event_count, 0);
        assert_eq!(report.repair_event_count, 0);
        assert_eq!(report.state, "checkpointed");

        Ok(())
    }

    #[test]
    fn anti_zeno_budget_without_checkpoint_ignores_checkpointed_history() -> Result<()> {
        let temp_db = TempDbPath::new("uncheckpointed-scope")?;
        let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
        let store = DataStore::open(temp_db.path(), migration_plan)?;

        record_checkpoint_written_event(&store, 3, "old checkpoint")?;
        record_repair_requested_event(&store, Some(3), None, "repair-old", "old repair")?;
        record_repair_requested_event(&store, None, None, "repair-current", "current repair")?;

        let report = build_anti_zeno_budget_report(&store, None, None, false, false, false, 0)?;
        assert_eq!(report.semantic_event_count, 0);
        assert_eq!(report.repair_event_count, 1);
        assert_eq!(report.state, "repair_required");

        Ok(())
    }

    #[test]
    fn anti_zeno_budget_distinguishes_acceptance_from_closure() -> Result<()> {
        let temp_db = TempDbPath::new("closure-classification")?;
        let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
        let store = DataStore::open(temp_db.path(), migration_plan)?;

        record_checkpoint_written_event(&store, 7, "checkpoint written")?;
        record_acceptance_recorded_event(&store, 7, 9, "acceptance", "acceptance recorded")?;

        let pre_closure =
            build_anti_zeno_budget_report(&store, Some(7), Some(9), true, false, false, 0)?;
        assert_eq!(pre_closure.state, "closure_required");
        assert_eq!(pre_closure.boundary_anchor_count, 1);
        assert_eq!(pre_closure.acceptance_event_count, 1);
        assert_eq!(pre_closure.closure_event_count, 0);
        assert!(pre_closure.forced_action.is_some());

        record_closure_recorded_event(&store, 7, 9, "acceptance", "closure recorded")?;
        let settled =
            build_anti_zeno_budget_report(&store, Some(7), Some(9), true, true, false, 0)?;
        assert_eq!(settled.state, "settled");
        assert_eq!(settled.boundary_anchor_count, 1);
        assert_eq!(settled.acceptance_event_count, 1);
        assert_eq!(settled.closure_event_count, 1);

        Ok(())
    }
}
