use serde::Serialize;

use crate::core::nota_runtime::{
    NotaAntiZenoProjection, NotaCheckpointRecord, NotaCheckpointRequest, NotaRuntimeFinalize,
    NotaRuntimeIntegrate, NotaRuntimeNextStep, NotaRuntimeReview,
};

#[derive(Clone, Serialize)]
pub(crate) struct NotaFrontDoorProjection {
    posture: String,
    summary: String,
    next_action_label: String,
    next_action_detail: String,
    dashboard_hook: String,
    progress_tracks: Vec<NotaFrontDoorProgressTrack>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotaFrontDoorProgressTrack {
    id: String,
    label: String,
    value: u8,
    tone: String,
    summary: String,
}

pub(crate) fn build_nota_front_door_projection(
    current_checkpoint: Option<&NotaCheckpointRecord>,
    decision_count: usize,
    transaction_count: usize,
    allocation_count: usize,
    receipt_count: usize,
    anti_zeno: &NotaAntiZenoProjection,
    recommended_checkpoint: Option<&NotaCheckpointRequest>,
    review: Option<&NotaRuntimeReview>,
    integrate: Option<&NotaRuntimeIntegrate>,
    finalize: Option<&NotaRuntimeFinalize>,
    next_step: Option<&NotaRuntimeNextStep>,
) -> NotaFrontDoorProjection {
    let posture = if current_checkpoint.is_some() {
        "Checkpoint-backed native front door".to_string()
    } else {
        "Native front door waiting for first checkpoint".to_string()
    };

    let summary = if let Some(checkpoint) = current_checkpoint {
        checkpoint.cadence_object.summary.clone()
    } else if let Some(checkpoint) = recommended_checkpoint {
        checkpoint.stable_level.clone()
    } else {
        "Write the first checkpoint so the GUI can resume from runtime truth instead of terminal recap."
            .to_string()
    };

    let (next_action_label, next_action_detail) = if let Some(step) = next_step {
        (
            "Next runtime move".to_string(),
            describe_nota_front_door_next_step(step),
        )
    } else if let Some(checkpoint) = current_checkpoint {
        (
            "Current slice".to_string(),
            checkpoint
                .payload
                .selected_trunk
                .clone()
                .unwrap_or_else(|| checkpoint.cadence_object.title.clone()),
        )
    } else if let Some(checkpoint) = recommended_checkpoint {
        (
            "Suggested checkpoint".to_string(),
            checkpoint.remaining.first().cloned().unwrap_or_else(|| {
                checkpoint
                    .selected_trunk
                    .clone()
                    .unwrap_or_else(|| "Checkpoint the current closure boundary.".to_string())
            }),
        )
    } else if let Some(finalize) = finalize {
        (
            "Latest closure".to_string(),
            finalize.summary.clone().unwrap_or_else(|| {
                format!(
                    "Finalize closed allocation {} on lineage {}.",
                    finalize.allocation_id, finalize.lineage_ref
                )
            }),
        )
    } else if let Some(integrate) = integrate {
        (
            "Latest integration".to_string(),
            integrate.summary.clone().unwrap_or_else(|| {
                format!(
                    "Integration recorded {} on allocation {}.",
                    integrate
                        .outcome
                        .clone()
                        .unwrap_or_else(|| integrate.state.clone()),
                    integrate.allocation_id
                )
            }),
        )
    } else if let Some(review) = review {
        (
            "Latest review".to_string(),
            review.summary.clone().unwrap_or_else(|| {
                format!(
                    "Review is tracking allocation {} on lineage {}.",
                    review.allocation_id, review.lineage_ref
                )
            }),
        )
    } else {
        (
            "Current slice".to_string(),
            "No active checkpoint is recorded yet.".to_string(),
        )
    };

    let truth_spine_value = front_door_truth_spine_value(
        current_checkpoint.is_some(),
        decision_count,
        transaction_count,
        allocation_count,
        receipt_count,
    );
    let shell_reach_value = if current_checkpoint.is_some() { 82 } else { 72 };
    let relay_relief_summary = current_checkpoint
        .map(|checkpoint| checkpoint.payload.human_continuity_bus.clone())
        .or_else(|| {
            recommended_checkpoint.map(|checkpoint| checkpoint.human_continuity_bus.clone())
        })
        .unwrap_or_else(|| {
            "Human relay is still heavy because no checkpoint is active yet.".to_string()
        });
    let relay_relief_value = front_door_relay_relief_value(
        relay_relief_summary.as_str(),
        next_step.is_some(),
        recommended_checkpoint.is_some(),
    );

    NotaFrontDoorProjection {
        posture,
        summary,
        next_action_label,
        next_action_detail,
        dashboard_hook:
            "Dashboard now reads the same runtime truth plane as Chat, with acceptance-backed anti-Zeno progress and bounded continuity detail."
                .to_string(),
        progress_tracks: vec![
            NotaFrontDoorProgressTrack {
                id: "truth-spine".to_string(),
                label: "Grounded in truth".to_string(),
                value: truth_spine_value,
                tone: if truth_spine_value >= 80 {
                    "steady".to_string()
                } else {
                    "warming".to_string()
                },
                summary:
                    "Checkpoint, decision, transaction, and receipt reads are all coming from the NOTA runtime."
                        .to_string(),
            },
            NotaFrontDoorProgressTrack {
                id: "front-door-slice".to_string(),
                label: "Front-door reach".to_string(),
                value: shell_reach_value,
                tone: "active".to_string(),
                summary:
                    "This build exposes a Chat-first shell, a live state rail, mission progress, and a real import entry."
                        .to_string(),
            },
            NotaFrontDoorProgressTrack {
                id: "anti-zeno".to_string(),
                label: "Anti-Zeno progress".to_string(),
                value: anti_zeno.value,
                tone: if anti_zeno.fully_settled {
                    "steady".to_string()
                } else if anti_zeno.acceptance_present {
                    "active".to_string()
                } else {
                    "caution".to_string()
                },
                summary: anti_zeno.summary.clone(),
            },
            NotaFrontDoorProgressTrack {
                id: "relay-relief".to_string(),
                label: "Human relay relief".to_string(),
                value: relay_relief_value,
                tone: if relay_relief_value >= 70 {
                    "steady".to_string()
                } else {
                    "caution".to_string()
                },
                summary: relay_relief_summary,
            },
        ],
    }
}

fn describe_nota_front_door_next_step(step: &NotaRuntimeNextStep) -> String {
    if step.allocation_id == 0 {
        let checkpoint_ref = step.target_ref.as_str();
        let action = match step.step.as_str() {
            "clarify" => "Resolve the current clarification boundary",
            "ask_unblock" => "Resolve the current unblock ask",
            "ask_decide" => "Resolve the current decision ask",
            "ask_replace" => "Resolve the current replace ask",
            "ask_override" => "Resolve the current override ask",
            other => return format!("Follow `{other}` on checkpoint {checkpoint_ref}."),
        };

        return format!(
            "{action} on checkpoint {checkpoint_ref} via runtime transaction {}.",
            step.transaction_id
        );
    }

    let action = match step.step.as_str() {
        "review" => "Review the returned boundary",
        "integrate" => "Record the integration result",
        "finalize" => "Close the integrated boundary",
        other => return format!("Follow `{other}` for allocation {}.", step.allocation_id),
    };

    format!(
        "{action} for allocation {} on lineage {}.",
        step.allocation_id, step.lineage_ref
    )
}

fn front_door_truth_spine_value(
    has_checkpoint: bool,
    decision_count: usize,
    transaction_count: usize,
    allocation_count: usize,
    receipt_count: usize,
) -> u8 {
    let mut value = 18_u8;
    if has_checkpoint {
        value = value.saturating_add(30);
    }
    if decision_count > 0 {
        value = value.saturating_add(14);
    }
    if transaction_count > 0 {
        value = value.saturating_add(14);
    }
    if allocation_count > 0 {
        value = value.saturating_add(12);
    }
    if receipt_count > 0 {
        value = value.saturating_add(12);
    }
    value.min(100)
}

fn front_door_relay_relief_value(
    human_continuity_bus: &str,
    has_next_step: bool,
    has_recommended_checkpoint: bool,
) -> u8 {
    let normalized = human_continuity_bus.to_ascii_lowercase();
    let mut value: u8 = if normalized.contains("further reduced") {
        78
    } else if normalized.contains("reduced") {
        64
    } else {
        42
    };

    if has_next_step {
        value = value.saturating_sub(8);
    }
    if has_recommended_checkpoint {
        value = value.saturating_sub(6);
    }

    value
}
