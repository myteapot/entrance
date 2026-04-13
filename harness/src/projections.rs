use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::StartupState;
use entrance_core::{
    cold_docs_runtime::{export_cold_docs_to_repo, NotaColdDocExportReport},
    data_store::DataStore,
    invariant_runtime::refresh_runtime_invariants,
    overview::{build_nota_runtime_status, build_projection_truth_revision, NotaRuntimeStatus},
    projection_runtime::{
        record_projection_failure, record_projection_success, ProjectionTargetSpec,
        ProjectionTruthRevision, HOT_ROOT_PROJECTION_CLASS, OPTIONAL_PROJECTION_POLICY,
        ORACLE_PROJECTION_CLASS, REQUIRED_PROJECTION_POLICY,
    },
};

#[derive(Clone, Serialize)]
pub struct HotRootProjectionWriteReport {
    pub export_root: String,
    pub files_written: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirrored_repo_specs_dir: Option<String>,
    pub truth_revision: ProjectionTruthRevision,
}

#[derive(Clone, Serialize)]
pub struct ProjectionRebuildReport {
    pub status: String,
    pub truth_revision: ProjectionTruthRevision,
    pub hot_root: HotRootProjectionWriteReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cold_docs: Option<NotaColdDocExportReport>,
    pub required_targets_fresh: bool,
    pub dirty_required_target_count: usize,
    pub repair_lane_open_count: usize,
}

pub fn write_hot_root_projection(
    startup: &StartupState,
    mirror_project_dir: Option<&str>,
) -> Result<HotRootProjectionWriteReport> {
    let status = build_nota_runtime_status(&startup.data_store())?;
    let hot_root_dir = startup.paths().exports_dir().join("hot-root");
    let truth_revision = build_projection_truth_revision(
        status.current_checkpoint_id,
        status
            .current_human_round
            .as_ref()
            .map(|round| round.cadence_object.id),
        status
            .current_acceptance_bundle
            .as_ref()
            .map(|bundle| bundle.cadence_object.id),
    );
    let hot_root_dir_display = hot_root_dir.to_string_lossy().replace('\\', "/");
    let oracle_readme_path = hot_root_dir
        .join("README.md")
        .to_string_lossy()
        .replace('\\', "/");
    if let Err(error) = fs::create_dir_all(&hot_root_dir).with_context(|| {
        format!(
            "failed to create hot-root export directory at {}",
            hot_root_dir.display()
        )
    }) {
        record_hot_root_projection_failure(
            startup,
            &truth_revision,
            &hot_root_dir_display,
            &oracle_readme_path,
            &error.to_string(),
        )?;
        refresh_runtime_invariant_truth(&startup.data_store())?;
        return Err(error);
    }

    let files = render_hot_root_files(startup, &status);
    let mut files_written = Vec::new();
    for (name, content) in &files {
        let path = hot_root_dir.join(name);
        if let Err(error) = fs::write(&path, content)
            .with_context(|| format!("failed to write hot-root export at {}", path.display()))
        {
            record_hot_root_projection_failure(
                startup,
                &truth_revision,
                &hot_root_dir_display,
                &oracle_readme_path,
                &error.to_string(),
            )?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            return Err(error);
        }
        files_written.push(path.display().to_string());
    }
    record_hot_root_projection_success(
        startup,
        &truth_revision,
        &hot_root_dir_display,
        &oracle_readme_path,
    )?;

    let mirrored_repo_specs_dir = mirror_project_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(resolve_mirror_project_dir)
        .map(|project_dir| project_dir.join("notes").join("agents").join("specs"));
    if let Some(specs_dir) = mirrored_repo_specs_dir.as_ref() {
        let repo_specs_dir_display = specs_dir.to_string_lossy().replace('\\', "/");
        let mirror_target_key = "mirror/notes/agents/specs";
        if let Err(error) = fs::create_dir_all(specs_dir).with_context(|| {
            format!(
                "failed to create mirrored repo specs directory at {}",
                specs_dir.display()
            )
        }) {
            record_projection_failure(
                &startup.data_store(),
                ProjectionTargetSpec {
                    projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                    target_key: mirror_target_key.into(),
                    title: "Mirrored repo hot root".into(),
                    target_path: repo_specs_dir_display.as_str().into(),
                    source_scope: "runtime:Entrance".into(),
                    repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                    projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                    is_required: false,
                },
                &truth_revision,
                "hot_root_export",
                "failed to create mirrored repo specs directory",
                &error.to_string(),
            )?;
            refresh_runtime_invariant_truth(&startup.data_store())?;
            return Err(error);
        }
        for (name, content) in &files {
            let path = specs_dir.join(name);
            if let Err(error) = fs::write(&path, content).with_context(|| {
                format!(
                    "failed to write mirrored hot-root projection at {}",
                    path.display()
                )
            }) {
                record_projection_failure(
                    &startup.data_store(),
                    ProjectionTargetSpec {
                        projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                        target_key: mirror_target_key.into(),
                        title: "Mirrored repo hot root".into(),
                        target_path: repo_specs_dir_display.as_str().into(),
                        source_scope: "runtime:Entrance".into(),
                        repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                        projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                        is_required: false,
                    },
                    &truth_revision,
                    "hot_root_export",
                    "failed to write mirrored repo hot root",
                    &error.to_string(),
                )?;
                refresh_runtime_invariant_truth(&startup.data_store())?;
                return Err(error);
            }
        }
        record_projection_success(
            &startup.data_store(),
            ProjectionTargetSpec {
                projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
                target_key: mirror_target_key.into(),
                title: "Mirrored repo hot root".into(),
                target_path: repo_specs_dir_display.as_str().into(),
                source_scope: "runtime:Entrance".into(),
                repair_action: "entrance nota export-hot-root --project-dir <path>".into(),
                projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
                is_required: false,
            },
            &truth_revision,
            "hot_root_export",
            "Mirrored repo hot root is current with runtime truth.",
        )?;
    }

    refresh_runtime_invariant_truth(&startup.data_store())?;

    Ok(HotRootProjectionWriteReport {
        export_root: hot_root_dir.display().to_string(),
        files_written,
        mirrored_repo_specs_dir: mirrored_repo_specs_dir.map(|path| path.display().to_string()),
        truth_revision,
    })
}

pub fn rebuild_nota_projections(
    startup: &StartupState,
    project_dir: Option<&str>,
) -> Result<ProjectionRebuildReport> {
    let before = build_nota_runtime_status(&startup.data_store())?;
    let truth_revision = before.projections.current_truth_revision.clone();
    let hot_root = write_hot_root_projection(startup, project_dir)?;
    let cold_docs = if let Some(project_dir) = project_dir {
        Some(export_cold_docs_to_repo(
            &startup.data_store(),
            project_dir,
            &truth_revision,
        )?)
    } else {
        None
    };
    refresh_runtime_invariant_truth(&startup.data_store())?;
    let after = build_nota_runtime_status(&startup.data_store())?;

    Ok(ProjectionRebuildReport {
        status: if after.projections.required_targets_fresh {
            "rebuilt".to_string()
        } else {
            "repair_required".to_string()
        },
        truth_revision,
        hot_root,
        cold_docs,
        required_targets_fresh: after.projections.required_targets_fresh,
        dirty_required_target_count: after.projections.dirty_required_target_count,
        repair_lane_open_count: after.repair_lane.open_count,
    })
}

fn resolve_mirror_project_dir(raw_project_dir: &str) -> PathBuf {
    let normalized = raw_project_dir.trim().replace('\\', "/");
    if cfg!(windows) {
        return PathBuf::from(normalized);
    }

    let bytes = normalized.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        let drive = (bytes[0] as char).to_ascii_lowercase().to_string();
        let suffix = normalized[3..].trim_start_matches('/');
        let mut converted = PathBuf::from("/mnt");
        converted.push(drive);
        if !suffix.is_empty() {
            converted.push(suffix);
        }
        return converted;
    }

    PathBuf::from(normalized)
}

fn refresh_runtime_invariant_truth(data_store: &DataStore) -> Result<()> {
    refresh_runtime_invariants(data_store).map(|_| ())
}

fn record_hot_root_projection_success(
    startup: &StartupState,
    truth_revision: &ProjectionTruthRevision,
    hot_root_dir: &str,
    oracle_readme_path: &str,
) -> Result<()> {
    record_projection_success(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root".into(),
            title: "Hot-root export".into(),
            target_path: hot_root_dir.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Hot-root export is current with runtime truth.",
    )?;
    record_projection_success(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: ORACLE_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root/README.md".into(),
            title: "Oracle README export".into(),
            target_path: oracle_readme_path.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Oracle README projection is current with runtime truth.",
    )?;
    Ok(())
}

fn record_hot_root_projection_failure(
    startup: &StartupState,
    truth_revision: &ProjectionTruthRevision,
    hot_root_dir: &str,
    oracle_readme_path: &str,
    error_message: &str,
) -> Result<()> {
    record_projection_failure(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: HOT_ROOT_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root".into(),
            title: "Hot-root export".into(),
            target_path: hot_root_dir.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Hot-root export failed.",
        error_message,
    )?;
    record_projection_failure(
        &startup.data_store(),
        ProjectionTargetSpec {
            projection_class: ORACLE_PROJECTION_CLASS.into(),
            target_key: "exports/hot-root/README.md".into(),
            title: "Oracle README export".into(),
            target_path: oracle_readme_path.into(),
            source_scope: "runtime:Entrance".into(),
            repair_action: "entrance nota export-hot-root".into(),
            projection_policy: REQUIRED_PROJECTION_POLICY.into(),
            is_required: true,
        },
        truth_revision,
        "hot_root_export",
        "Oracle README export failed.",
        error_message,
    )?;
    Ok(())
}

fn render_hot_root_files(
    startup: &StartupState,
    status: &NotaRuntimeStatus,
) -> Vec<(&'static str, String)> {
    let checkpoint_label = status
        .current_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.cadence_object.title.clone())
        .unwrap_or_else(|| "No active checkpoint".to_string());
    let checkpoint_level = status
        .current_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.payload.stable_level.clone())
        .unwrap_or_else(|| {
            "Checkpoint the current human round before relying on exported views.".to_string()
        });
    let human_round_line = status
        .current_human_round
        .as_ref()
        .map(|round| {
            format!(
                "{} / {} on checkpoint {}",
                round.payload.round_state,
                round
                    .payload
                    .detail_round_state
                    .as_deref()
                    .unwrap_or("unknown"),
                round.payload.checkpoint_id
            )
        })
        .unwrap_or_else(|| "No current human round is materialized yet.".to_string());
    let acceptance_line = status
        .current_acceptance_bundle
        .as_ref()
        .map(|bundle| {
            if bundle.payload.allocation_id == 0 {
                format!(
                    "{} on checkpoint {} ({})",
                    bundle.payload.acceptance_kind,
                    bundle.payload.checkpoint_id,
                    bundle.payload.round_state
                )
            } else {
                format!(
                    "{} on allocation {} ({})",
                    bundle.payload.acceptance_kind,
                    bundle.payload.allocation_id,
                    bundle.payload.round_state
                )
            }
        })
        .unwrap_or_else(|| "No formal acceptance bundle is current.".to_string());
    let next_step_line = status
        .next_step
        .as_ref()
        .map(|step| {
            if step.allocation_id == 0 {
                format!("{} on checkpoint {}", step.step, step.target_ref)
            } else {
                format!("{} for allocation {}", step.step, step.allocation_id)
            }
        })
        .unwrap_or_else(|| "No follow-on runtime step is currently open.".to_string());
    let round_state_line = format!(
        "{} / {} ({})",
        status.round_state.state, status.round_state.detail_state, status.round_state.summary
    );
    let projection_line = format!(
        "{}/{} required projections fresh",
        status.projections.fresh_required_target_count, status.projections.required_target_count
    );
    let invariant_line = format!(
        "{} passed, {} repairable, {} blocked",
        status.invariants.passed_count,
        status.invariants.repairable_count,
        status.invariants.blocked_count
    );
    let repair_lane_line = format!(
        "{} open, {} resolved",
        status.repair_lane.open_count, status.repair_lane.resolved_count
    );
    let recovery_line = format!("{} ({})", status.recovery.mode, status.recovery.summary);
    let supervision_line = match (
        status.current_supervision.as_ref(),
        status.current_supervision_incident.as_ref(),
    ) {
        (Some(projection), Some(incident)) => {
            let exhaustion_suffix = if incident.budget_exhausted {
                ", budget exhausted"
            } else {
                ""
            };
            format!(
                "{:?} (attempt {}/{}, action: {:?}{})",
                projection.current_supervision_state,
                incident.retry_count,
                incident.max_restarts,
                incident.last_supervisor_action,
                exhaustion_suffix,
            )
        }
        (Some(projection), None) => format!(
            "{:?} via {:?}",
            projection.current_supervision_state, projection.last_supervisor_action
        ),
        (None, _) => "No supervised allocation is currently active.".to_string(),
    };
    let owner_root = startup.paths().app_data_dir().display().to_string();
    let config_path = startup.paths().config_path().display().to_string();
    let db_path = startup.paths().db_path().display().to_string();
    let host_line = status
        .host
        .as_ref()
        .map(|host| format!("{} on {}", host.host_label, host.os_family))
        .unwrap_or_else(|| "No host snapshot has been recorded yet.".to_string());

    let readme = format!(
        "# Top Layer\n\n> Status: exported hot root from DB-first runtime truth\n\nThe top layer is a retained projection, not an authoring authority.\n\nActive hot-root files:\n\n- [machine.md](./machine.md)\n- [control.md](./control.md)\n- [truth.md](./truth.md)\n- [phase-todo.md](./phase-todo.md)\n- [pending.md](./pending.md)\n\nCurrent owner root:\n\n- `{owner_root}`\n- host: {host_line}\n- config: `{config_path}`\n- runtime DB: `{db_path}`\n- exported hot root: `{}`\n- observed worktrees: {}\n\nCurrent round:\n\n- human round: {human_round_line}\n- round state: {round_state_line}\n- checkpoint: {checkpoint_label}\n- stable level: {checkpoint_level}\n- acceptance: {acceptance_line}\n- anti-Zeno: {} / {} ({})\n- anti-Zeno budget: {} ({})\n- supervision: {supervision_line}\n- invariants: {invariant_line}\n- repair lane: {repair_lane_line}\n- recovery: {recovery_line}\n- next step: {next_step_line}\n- projection freshness: {projection_line}\n\nProjection law:\n\n- DB is the only canonical writer.\n- README, hot root, cold docs, GUI, CLI, and MCP are projections from DB truth.\n- `passed human round = acceptance`.\n- canonical state and detail state are both derived from DB truth rather than hand-authored status prose.\n- `fully settled round = acceptance + no next_step + checkpoint carry-forward`.\n",
        startup.paths().exports_dir().join("hot-root").display(),
        status.worktree_count,
        status.anti_zeno.state,
        status.anti_zeno.detail_state,
        status.anti_zeno.summary,
        status.anti_zeno_budget.state,
        status.anti_zeno_budget.summary,
    );
    let machine = format!(
        "# Machine\n\n\
Checkpoint: {checkpoint_label}\n\n\
Level: {checkpoint_level}\n\n\
Round state: {round_state_line}\n\n\
Human round: {human_round_line}\n\n\
Acceptance: {acceptance_line}\n\n\
Next step: {next_step_line}\n"
    );
    let control = format!(
        "# Control\n\n\
Projections: {projection_line}\n\n\
Invariants: {invariant_line}\n\n\
Repair lane: {repair_lane_line}\n\n\
Supervision: {supervision_line}\n\n\
Recovery: {recovery_line}\n"
    );
    let truth = format!(
        "# Truth\n\n\
The current runtime truth is retained in `{db_path}`.\n\n\
Checkpoint: {checkpoint_label}\n\n\
Stable level: {checkpoint_level}\n\n\
Acceptance: {acceptance_line}\n"
    );
    let phase_todo = format!(
        "# Phase Todo\n\n\
- Keep required projections fresh (`{projection_line}`)\n\
- Close repair lane debt (`{repair_lane_line}`)\n\
- Advance next step: {next_step_line}\n"
    );
    let pending = format!(
        "# Pending\n\n\
- Human round: {human_round_line}\n\
- Acceptance: {acceptance_line}\n\
- Recovery lane: {recovery_line}\n"
    );

    vec![
        ("README.md", readme),
        ("machine.md", machine),
        ("control.md", control),
        ("truth.md", truth),
        ("phase-todo.md", phase_todo),
        ("pending.md", pending),
    ]
}
