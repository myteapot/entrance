use super::{
    CADENCE_ACCEPTANCE_BUNDLE_KIND, CADENCE_CHECKPOINT_KIND, CADENCE_HANDOUT_KIND,
    CADENCE_HUMAN_ROUND_KIND, CADENCE_POLICY_NOTE_KIND, CADENCE_WAKE_REQUEST_KIND,
};

pub fn admission_policy_for_kind(cadence_kind: &str) -> &'static str {
    match cadence_kind {
        CADENCE_CHECKPOINT_KIND
        | CADENCE_HUMAN_ROUND_KIND
        | CADENCE_ACCEPTANCE_BUNDLE_KIND
        | CADENCE_HANDOUT_KIND
        | CADENCE_WAKE_REQUEST_KIND
        | CADENCE_POLICY_NOTE_KIND => "AP_STORAGE_AND_COLD_ALWAYS",
        _ => "AP_STORAGE_ALWAYS",
    }
}

pub fn projection_policy_for_kind(cadence_kind: &str) -> &'static str {
    match cadence_kind {
        CADENCE_CHECKPOINT_KIND
        | CADENCE_HUMAN_ROUND_KIND
        | CADENCE_ACCEPTANCE_BUNDLE_KIND
        | CADENCE_HANDOUT_KIND => "PP_HOT_ACTIVE_ONLY",
        CADENCE_WAKE_REQUEST_KIND => "PP_HOT_ON_ATTENTION_OR_REJECT",
        CADENCE_POLICY_NOTE_KIND => "PP_HOT_NEVER",
        _ => "PP_HOT_ACTIVE_ONLY",
    }
}
