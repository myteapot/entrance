use std::borrow::Cow;
use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::core::data_store::{
    DataStore, NewProjectionRun, StoredProjectionRun, StoredProjectionTarget,
    UpsertProjectionTarget,
};

pub const ORACLE_PROJECTION_CLASS: &str = "oracle_projection";
pub const HOT_ROOT_PROJECTION_CLASS: &str = "hot_root_projection";
pub const COLD_DOC_PROJECTION_CLASS: &str = "cold_doc_projection";
pub const UI_PROJECTION_CLASS: &str = "ui_projection";

pub const REQUIRED_PROJECTION_POLICY: &str = "required";
pub const OPTIONAL_PROJECTION_POLICY: &str = "optional";

const PROJECTION_RUN_SUCCEEDED: &str = "succeeded";
const PROJECTION_RUN_FAILED: &str = "failed";
const PROJECTION_RUN_SKIPPED: &str = "skipped";
const FRESHNESS_FRESH: &str = "fresh";
const FRESHNESS_FAILED: &str = "failed";
const FRESHNESS_SKIPPED: &str = "skipped";

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ProjectionTruthRevision {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_round_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_bundle_id: Option<i64>,
}

impl ProjectionTruthRevision {
    pub fn describe(&self) -> String {
        format!(
            "checkpoint {:?}, human_round {:?}, acceptance {:?}",
            self.checkpoint_id, self.human_round_id, self.acceptance_bundle_id
        )
    }

    fn matches_run(&self, run: &StoredProjectionRun) -> bool {
        self.checkpoint_id == run.truth_checkpoint_id
            && self.human_round_id == run.truth_human_round_id
            && self.acceptance_bundle_id == run.truth_acceptance_bundle_id
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionTargetSpec<'a> {
    pub projection_class: Cow<'a, str>,
    pub target_key: Cow<'a, str>,
    pub title: Cow<'a, str>,
    pub target_path: Cow<'a, str>,
    pub source_scope: Cow<'a, str>,
    pub repair_action: Cow<'a, str>,
    pub projection_policy: Cow<'a, str>,
    pub is_required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectionTargetStatus {
    pub target: StoredProjectionTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<StoredProjectionRun>,
    pub state: String,
    pub fresh: bool,
    pub dirty: bool,
    pub required: bool,
    pub summary: String,
    pub repair_action: String,
    pub current_truth_match: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectionStatusReport {
    pub target_count: usize,
    pub required_target_count: usize,
    pub fresh_required_target_count: usize,
    pub dirty_required_target_count: usize,
    pub failed_required_target_count: usize,
    pub required_targets_fresh: bool,
    pub current_truth_revision: ProjectionTruthRevision,
    pub targets: Vec<ProjectionTargetStatus>,
}

pub fn record_projection_success(
    data_store: &DataStore,
    spec: ProjectionTargetSpec<'_>,
    truth_revision: &ProjectionTruthRevision,
    trigger_kind: &str,
    summary: &str,
) -> Result<StoredProjectionRun> {
    record_projection_run(
        data_store,
        spec,
        truth_revision,
        trigger_kind,
        PROJECTION_RUN_SUCCEEDED,
        FRESHNESS_FRESH,
        summary,
        None,
        None,
    )
}

pub fn record_projection_failure(
    data_store: &DataStore,
    spec: ProjectionTargetSpec<'_>,
    truth_revision: &ProjectionTruthRevision,
    trigger_kind: &str,
    summary: &str,
    error_message: &str,
) -> Result<StoredProjectionRun> {
    let repair_action = spec.repair_action.clone();
    record_projection_run(
        data_store,
        spec,
        truth_revision,
        trigger_kind,
        PROJECTION_RUN_FAILED,
        FRESHNESS_FAILED,
        summary,
        Some(error_message),
        Some(repair_action.as_ref()),
    )
}

pub fn record_projection_skipped(
    data_store: &DataStore,
    spec: ProjectionTargetSpec<'_>,
    truth_revision: &ProjectionTruthRevision,
    trigger_kind: &str,
    summary: &str,
) -> Result<StoredProjectionRun> {
    let repair_action = spec.repair_action.clone();
    record_projection_run(
        data_store,
        spec,
        truth_revision,
        trigger_kind,
        PROJECTION_RUN_SKIPPED,
        FRESHNESS_SKIPPED,
        summary,
        None,
        Some(repair_action.as_ref()),
    )
}

pub fn build_projection_status_report(
    data_store: &DataStore,
    current_truth_revision: ProjectionTruthRevision,
) -> Result<ProjectionStatusReport> {
    let targets = data_store.list_projection_targets()?;
    let runs = data_store.list_projection_runs()?;
    let mut latest_runs = HashMap::new();
    for run in runs {
        latest_runs.entry(run.target_id).or_insert(run);
    }

    let mut target_statuses = Vec::with_capacity(targets.len());
    let mut required_target_count = 0usize;
    let mut fresh_required_target_count = 0usize;
    let mut dirty_required_target_count = 0usize;
    let mut failed_required_target_count = 0usize;

    for target in targets {
        if target.is_required {
            required_target_count += 1;
        }

        let latest_run = latest_runs.get(&target.id).cloned();
        let current_truth_match = latest_run
            .as_ref()
            .map(|run| current_truth_revision.matches_run(run))
            .unwrap_or(false);

        let (state, fresh, dirty, summary) = match latest_run.as_ref() {
            None => (
                "unprojected".to_string(),
                false,
                target.is_required,
                format!(
                    "{} has not been projected for the current runtime truth yet.",
                    target.title
                ),
            ),
            Some(run) if run.run_state == PROJECTION_RUN_FAILED => (
                "failed".to_string(),
                false,
                target.is_required,
                run.error_message
                    .clone()
                    .unwrap_or_else(|| run.summary.clone()),
            ),
            Some(run) if run.run_state == PROJECTION_RUN_SKIPPED => {
                ("skipped".to_string(), false, false, run.summary.clone())
            }
            Some(run) if current_truth_match => {
                ("fresh".to_string(), true, false, run.summary.clone())
            }
            Some(run) => (
                "stale".to_string(),
                false,
                target.is_required,
                format!(
                    "{} is older than the current runtime truth revision (latest run: {}).",
                    target.title, run.summary
                ),
            ),
        };

        if target.is_required {
            if fresh {
                fresh_required_target_count += 1;
            }
            if dirty {
                dirty_required_target_count += 1;
            }
            if state == "failed" {
                failed_required_target_count += 1;
            }
        }

        target_statuses.push(ProjectionTargetStatus {
            repair_action: target.repair_action.clone(),
            required: target.is_required,
            target,
            latest_run,
            state,
            fresh,
            dirty,
            summary,
            current_truth_match,
        });
    }

    Ok(ProjectionStatusReport {
        target_count: target_statuses.len(),
        required_target_count,
        fresh_required_target_count,
        dirty_required_target_count,
        failed_required_target_count,
        required_targets_fresh: required_target_count > 0
            && dirty_required_target_count == 0
            && fresh_required_target_count == required_target_count,
        current_truth_revision,
        targets: target_statuses,
    })
}

fn record_projection_run(
    data_store: &DataStore,
    spec: ProjectionTargetSpec<'_>,
    truth_revision: &ProjectionTruthRevision,
    trigger_kind: &str,
    run_state: &str,
    freshness_state: &str,
    summary: &str,
    error_message: Option<&str>,
    repair_hint: Option<&str>,
) -> Result<StoredProjectionRun> {
    let target = data_store.upsert_projection_target(UpsertProjectionTarget {
        projection_class: spec.projection_class.as_ref(),
        target_key: spec.target_key.as_ref(),
        title: spec.title.as_ref(),
        target_path: spec.target_path.as_ref(),
        source_scope: spec.source_scope.as_ref(),
        repair_action: spec.repair_action.as_ref(),
        projection_policy: spec.projection_policy.as_ref(),
        is_required: spec.is_required,
    })?;
    let now = Utc::now().to_rfc3339();

    data_store.insert_projection_run(NewProjectionRun {
        target_id: target.id,
        truth_checkpoint_id: truth_revision.checkpoint_id,
        truth_human_round_id: truth_revision.human_round_id,
        truth_acceptance_bundle_id: truth_revision.acceptance_bundle_id,
        run_state,
        freshness_state,
        trigger_kind,
        summary,
        error_message,
        repair_hint,
        started_at: &now,
        completed_at: &now,
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
        build_projection_status_report, record_projection_failure, record_projection_success,
        ProjectionTargetSpec, ProjectionTruthRevision, HOT_ROOT_PROJECTION_CLASS,
        OPTIONAL_PROJECTION_POLICY, ORACLE_PROJECTION_CLASS, REQUIRED_PROJECTION_POLICY,
    };

    struct TempDbPath {
        root: PathBuf,
        db_path: PathBuf,
    }

    impl TempDbPath {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-projection-runtime-{label}-{}-{suffix}",
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
    fn projection_status_distinguishes_fresh_stale_and_failed_required_targets() -> Result<()> {
        let temp_db = TempDbPath::new("projection-status")?;
        let migration_plan = MigrationPlan::new(crate::hosts::plugins::forge::migrations());
        let store = DataStore::open(temp_db.path(), migration_plan)?;

        let fresh_revision = ProjectionTruthRevision {
            checkpoint_id: Some(11),
            human_round_id: Some(12),
            acceptance_bundle_id: Some(13),
        };
        let stale_revision = ProjectionTruthRevision {
            checkpoint_id: Some(22),
            human_round_id: Some(23),
            acceptance_bundle_id: Some(24),
        };

        record_projection_success(
            &store,
            ProjectionTargetSpec {
                projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                target_key: "exports/hot-root".into(),
                title: "Hot root export".into(),
                target_path: "/tmp/hot-root".into(),
                source_scope: "runtime:Entrance".into(),
                repair_action: "entrance nota export-hot-root".into(),
                projection_policy: REQUIRED_PROJECTION_POLICY.into(),
                is_required: true,
            },
            &fresh_revision,
            "test",
            "hot-root export is current",
        )?;
        record_projection_failure(
            &store,
            ProjectionTargetSpec {
                projection_class: ORACLE_PROJECTION_CLASS.into(),
                target_key: "exports/hot-root/README.md".into(),
                title: "Oracle README export".into(),
                target_path: "/tmp/hot-root/README.md".into(),
                source_scope: "runtime:Entrance".into(),
                repair_action: "entrance nota export-hot-root".into(),
                projection_policy: REQUIRED_PROJECTION_POLICY.into(),
                is_required: true,
            },
            &fresh_revision,
            "test",
            "oracle export failed",
            "permission denied",
        )?;
        record_projection_success(
            &store,
            ProjectionTargetSpec {
                projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                target_key: "mirror/notes/specs/top".into(),
                title: "Mirrored repo hot root".into(),
                target_path: "/tmp/notes/specs/top".into(),
                source_scope: "runtime:Entrance".into(),
                repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                is_required: false,
            },
            &fresh_revision,
            "test",
            "repo mirror written for the previous round",
        )?;

        let report = build_projection_status_report(&store, stale_revision)?;
        assert_eq!(report.target_count, 3);
        assert_eq!(report.required_target_count, 2);
        assert_eq!(report.fresh_required_target_count, 0);
        assert_eq!(report.dirty_required_target_count, 2);
        assert_eq!(report.failed_required_target_count, 1);
        assert!(!report.required_targets_fresh);
        assert_eq!(
            report.current_truth_revision,
            ProjectionTruthRevision {
                checkpoint_id: Some(22),
                human_round_id: Some(23),
                acceptance_bundle_id: Some(24),
            }
        );

        let hot_root = report
            .targets
            .iter()
            .find(|target| target.target.target_key == "exports/hot-root")
            .expect("hot root target should exist");
        assert_eq!(hot_root.state, "stale");
        assert!(hot_root.dirty);
        assert!(!hot_root.fresh);

        let oracle = report
            .targets
            .iter()
            .find(|target| target.target.target_key == "exports/hot-root/README.md")
            .expect("oracle target should exist");
        assert_eq!(oracle.state, "failed");
        assert!(oracle.dirty);
        assert!(oracle.summary.contains("permission denied"));

        let mirror = report
            .targets
            .iter()
            .find(|target| target.target.target_key == "mirror/notes/specs/top")
            .expect("mirror target should exist");
        assert_eq!(mirror.state, "stale");
        assert!(!mirror.required);

        Ok(())
    }
}
