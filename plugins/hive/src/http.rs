use anyhow::{Context, Result};
use entrance_core::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveCallbackRequest {
    pub run_id: i64,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveCallback {
    pub run_id: i64,
    pub status: String,
    pub summary: Option<String>,
}

pub fn record_callback(store: &Store, request: HiveCallbackRequest) -> Result<HiveCallback> {
    store
        .get_hive_run(request.run_id)?
        .with_context(|| format!("unknown hive run `{}`", request.run_id))?;
    store.update_hive_run_status(
        request.run_id,
        &request.status,
        request.summary.as_deref(),
    )?;

    Ok(HiveCallback {
        run_id: request.run_id,
        status: request.status,
        summary: request.summary,
    })
}
