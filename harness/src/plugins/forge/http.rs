use std::{convert::Infallible, time::Duration};

use async_stream::stream;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    build_agent_task_request, CreateTaskRequest, ForgePlugin, ForgeTaskDetails,
    ForgeTaskStatusEvent,
};

#[derive(Clone)]
struct ForgeHttpState {
    forge: ForgePlugin,
}

#[derive(Debug, Deserialize)]
struct RunTaskRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default, alias = "workingDir")]
    working_dir: Option<String>,
    #[serde(default)]
    stdin: Option<String>,
    #[serde(default)]
    required_tokens: Vec<String>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(default, alias = "issueId")]
    issue_id: Option<String>,
    #[serde(default, alias = "worktreePath")]
    worktree_path: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatedTaskResponse {
    id: i64,
    status: String,
    message: Option<String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "error": self.message,
            })),
        )
            .into_response()
    }
}

pub fn router(forge: ForgePlugin) -> Router {
    let state = ForgeHttpState { forge };

    Router::new()
        .route("/api/forge/run", post(run_task))
        .route("/api/forge/tasks", get(list_tasks))
        .route("/api/forge/tasks/:id", get(get_task))
        .route("/api/forge/tasks/:id/cancel", post(cancel_task))
        .route("/api/forge/tasks/:id/stream", get(stream_task))
        .with_state(state)
}

async fn run_task(
    State(state): State<ForgeHttpState>,
    Json(payload): Json<RunTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let request = if let (Some(issue_id), Some(worktree_path), Some(model), Some(prompt)) = (
        payload.issue_id,
        payload.worktree_path,
        payload.model,
        payload.prompt,
    ) {
        build_agent_task_request(
            issue_id,
            worktree_path,
            model,
            prompt,
            payload.required_tokens,
            None,
        )
        .map_err(ApiError::bad_request)?
    } else {
        let command = payload
            .command
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if command.is_empty() {
            return Err(ApiError::bad_request(
                "`command` must not be empty when agent dispatch fields are omitted",
            ));
        }

        let name = payload
            .name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| command.to_string());
        let args = serde_json::to_string(&payload.args)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        let required_tokens = serde_json::to_string(&payload.required_tokens)
            .map_err(|error| ApiError::internal(error.to_string()))?;
        let metadata = serde_json::to_string(
            &payload
                .metadata
                .unwrap_or_else(|| Value::Object(serde_json::Map::new())),
        )
        .map_err(|error| ApiError::internal(error.to_string()))?;

        CreateTaskRequest {
            name,
            command: command.to_string(),
            args,
            working_dir: payload.working_dir.filter(|value| !value.trim().is_empty()),
            stdin_text: payload.stdin.filter(|value| !value.trim().is_empty()),
            required_tokens,
            metadata,
            dispatch_receipt: None,
        }
    };

    let id = state
        .forge
        .create_task(request)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state
        .forge
        .engine()
        .spawn_task(id)
        .map_err(|error| ApiError::internal(error.to_string()))?;

    let task = state
        .forge
        .get_task(id)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .ok_or_else(|| {
            ApiError::internal(format!("forge task `{id}` disappeared after creation"))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedTaskResponse {
            id,
            status: task.status,
            message: task.status_message,
        }),
    ))
}

async fn list_tasks(
    State(state): State<ForgeHttpState>,
) -> Result<Json<Vec<crate::core::data_store::StoredForgeTask>>, ApiError> {
    state
        .forge
        .list_tasks()
        .map(Json)
        .map_err(|error| ApiError::internal(error.to_string()))
}

async fn get_task(
    Path(id): Path<i64>,
    State(state): State<ForgeHttpState>,
) -> Result<Json<ForgeTaskDetails>, ApiError> {
    state
        .forge
        .get_task_details(id)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("forge task `{id}` not found")))
}

async fn cancel_task(
    Path(id): Path<i64>,
    State(state): State<ForgeHttpState>,
) -> Result<Json<ForgeTaskDetails>, ApiError> {
    let Some(task) = state
        .forge
        .get_task(id)
        .map_err(|error| ApiError::internal(error.to_string()))?
    else {
        return Err(ApiError::not_found(format!("forge task `{id}` not found")));
    };

    if task.status != "Running" {
        return Err(ApiError::conflict(format!(
            "forge task `{id}` is `{}` and cannot be cancelled",
            task.status
        )));
    }

    state
        .forge
        .cancel_task(id)
        .map_err(|error| ApiError::conflict(error.to_string()))?;

    state
        .forge
        .get_task_details(id)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .map(Json)
        .ok_or_else(|| {
            ApiError::not_found(format!("forge task `{id}` not found after cancellation"))
        })
}

async fn stream_task(
    Path(id): Path<i64>,
    State(state): State<ForgeHttpState>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let Some(task) = state
        .forge
        .get_task(id)
        .map_err(|error| ApiError::internal(error.to_string()))?
    else {
        return Err(ApiError::not_found(format!("forge task `{id}` not found")));
    };

    let logs = state
        .forge
        .list_task_logs(id)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let mut receiver = state.forge.subscribe_events();

    let event_stream = stream! {
        for log in logs {
            yield Ok(SseEvent::default().event("log").data(serde_json::to_string(&log).unwrap_or_default()));
        }

        yield Ok(SseEvent::default().event("status").data(
            serde_json::to_string(&ForgeTaskStatusEvent::from(&task)).unwrap_or_default(),
        ));

        if is_terminal_status(&task.status) {
            return;
        }

        loop {
            match receiver.recv().await {
                Ok(message) => {
                    if let Some((event, is_terminal)) = sse_event_for_task(id, &message.topic, &message.payload) {
                        yield Ok(event);
                        if is_terminal {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(event_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

fn sse_event_for_task(task_id: i64, topic: &str, payload: &str) -> Option<(SseEvent, bool)> {
    let value: Value = serde_json::from_str(payload).ok()?;
    match topic {
        "forge:task_output" => {
            let payload_task_id = value.get("task_id")?.as_i64()?;
            if payload_task_id != task_id {
                return None;
            }

            Some((
                SseEvent::default().event("log").data(payload.to_string()),
                false,
            ))
        }
        "forge:task_status" => {
            let payload_task_id = value.get("id")?.as_i64()?;
            if payload_task_id != task_id {
                return None;
            }

            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Some((
                SseEvent::default()
                    .event("status")
                    .data(payload.to_string()),
                is_terminal_status(status),
            ))
        }
        _ => None,
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "Done" | "Failed" | "Cancelled" | "Blocked")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_statuses_are_detected() {
        assert!(is_terminal_status("Done"));
        assert!(is_terminal_status("Failed"));
        assert!(is_terminal_status("Cancelled"));
        assert!(is_terminal_status("Blocked"));
        assert!(!is_terminal_status("Running"));
    }

    #[test]
    fn sse_mapper_filters_other_tasks() {
        let event = sse_event_for_task(7, "forge:task_status", r#"{"id":8,"status":"Done"}"#);
        assert!(event.is_none());
    }
}
