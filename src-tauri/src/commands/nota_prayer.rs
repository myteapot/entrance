use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PendingPrayer {
    pub allocation_id: i64,
    pub title: String,
    pub detail: String,
    pub dispatch_role: String,
    pub created_at: String,
}

// NOTE: These commands are defined but NOT registered in invoke_handler yet.
// Registration will happen in a follow-up MR once runtime state wiring is confirmed.
// Registering todo!() commands would cause runtime panics.

#[allow(dead_code)]
pub async fn nota_prayer_list() -> Result<Vec<PendingPrayer>, String> {
    // Query DB for allocations with status = 'pending_approval'
    todo!("G1-Skeleton: implement after runtime state wiring")
}

#[allow(dead_code)]
pub async fn nota_approve_prayer(_allocation_id: i64) -> Result<String, String> {
    // Call admission pipeline → approve, emit graph:update NodeStateChanged
    todo!("G1-Skeleton: implement after runtime state wiring")
}

#[allow(dead_code)]
pub async fn nota_reject_prayer(_allocation_id: i64, _reason: String) -> Result<String, String> {
    // Record rejection receipt, emit graph:update NodeStateChanged
    todo!("G1-Skeleton: implement after runtime state wiring")
}
