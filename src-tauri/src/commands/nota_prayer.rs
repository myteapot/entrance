use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, State, Wry};

use crate::core::{
    data_store::{DataStore, NewNotaRuntimeReceipt, NotaRuntimeAllocationUpdate},
    event_bus::EventBus,
    graph_events::GraphUpdateEvent,
    nota::NotaDoAllocationPayload,
};

#[derive(Debug, Clone, Serialize)]
pub struct PendingPrayer {
    pub allocation_id: i64,
    pub title: String,
    pub detail: String,
    pub dispatch_role: String,
    pub created_at: String,
}

#[allow(dead_code)]
pub async fn nota_prayer_list(
    data_store: State<'_, DataStore>,
) -> Result<Vec<PendingPrayer>, String> {
    data_store
        .list_nota_runtime_allocations()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|allocation| allocation.status == "pending_approval")
        .map(|allocation| {
            let payload: NotaDoAllocationPayload = serde_json::from_str(&allocation.payload_json)
                .map_err(|error| error.to_string())?;
            Ok(PendingPrayer {
                allocation_id: allocation.id,
                title: payload
                    .issue_title
                    .unwrap_or_else(|| format!("Prayer #{}", allocation.id)),
                detail: payload.issue_id,
                dispatch_role: payload.child_dispatch_role,
                created_at: allocation.created_at,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn nota_approve_prayer(
    app: AppHandle<Wry>,
    event_bus: State<'_, EventBus>,
    data_store: State<'_, DataStore>,
    allocation_id: i64,
) -> Result<String, String> {
    let allocation = data_store
        .list_nota_runtime_allocations()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|allocation| allocation.id == allocation_id)
        .ok_or_else(|| format!("allocation {} not found", allocation_id))?;
    let previous_status = allocation.status.clone();
    let updated_allocation = if previous_status == "pending_approval" {
        data_store
            .update_nota_runtime_allocation(
                allocation_id,
                NotaRuntimeAllocationUpdate {
                    status: "approved",
                    payload_json: None,
                    child_execution_ref: None,
                },
            )
            .map_err(|error| error.to_string())?
    } else {
        allocation
    };
    let detail = if previous_status == "pending_approval" {
        "approved".to_string()
    } else {
        format!(
            "approval acknowledged while status remained `{}`",
            updated_allocation.status
        )
    };

    let _ = event_bus.emit_graph_update(
        &app,
        &GraphUpdateEvent::NodeStateChanged {
            id: format!("alloc-{}", updated_allocation.id),
            tone: "steady".to_string(),
            detail,
        },
    );

    Ok(format!("Approved allocation {}", updated_allocation.id))
}

#[tauri::command]
pub async fn nota_reject_prayer(
    app: AppHandle<Wry>,
    event_bus: State<'_, EventBus>,
    data_store: State<'_, DataStore>,
    allocation_id: i64,
    reason: String,
) -> Result<String, String> {
    let allocation = data_store
        .list_nota_runtime_allocations()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|allocation| allocation.id == allocation_id)
        .ok_or_else(|| format!("allocation {} not found", allocation_id))?;
    let previous_status = allocation.status.clone();
    let updated_allocation = if previous_status == "pending_approval" {
        data_store
            .update_nota_runtime_allocation(
                allocation_id,
                NotaRuntimeAllocationUpdate {
                    status: "rejected",
                    payload_json: None,
                    child_execution_ref: None,
                },
            )
            .map_err(|error| error.to_string())?
    } else {
        allocation
    };

    data_store
        .append_nota_runtime_receipt(NewNotaRuntimeReceipt {
            transaction_id: updated_allocation.source_transaction_id,
            receipt_kind: "NOTA_PRAYER_REJECTED",
            payload_json: &serde_json::to_string(&json!({
                "allocation_id": updated_allocation.id,
                "reason": reason,
            }))
            .map_err(|error| error.to_string())?,
            status: "recorded",
        })
        .map_err(|error| error.to_string())?;

    let _ = event_bus.emit_graph_update(
        &app,
        &GraphUpdateEvent::NodeStateChanged {
            id: format!("alloc-{}", updated_allocation.id),
            tone: "caution".to_string(),
            detail: format!("rejected: {}", reason),
        },
    );

    Ok(format!(
        "Rejected allocation {} reason: {}",
        updated_allocation.id, reason
    ))
}
