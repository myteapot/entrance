use anyhow::{Context, Result};
use entrance_core::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewDecision {
    Approve,
    Return,
    Integrate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub run_id: i64,
    pub decision: String,
    pub status: String,
}

pub fn apply(store: &Store, id: i64, decision: ReviewDecision) -> Result<ReviewRecord> {
    store
        .get_hive_run(id)?
        .with_context(|| format!("unknown hive run `{id}`"))?;

    let (decision_label, status, summary) = match decision {
        ReviewDecision::Approve => ("approve", "approved", "approved by review surface"),
        ReviewDecision::Return => ("return", "returned", "returned for another round"),
        ReviewDecision::Integrate => ("integrate", "integrated", "integrated into main flow"),
    };

    store.update_hive_run_status(id, status, Some(summary))?;
    Ok(ReviewRecord {
        run_id: id,
        decision: decision_label.to_string(),
        status: status.to_string(),
    })
}
