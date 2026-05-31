use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use entrance_core::LauncherQuery;
use entrance_drawer::VaultSecret;
use entrance_hive::{
    HiveCallbackRequest, HiveDispatchRequest, HiveLoopCreateRequest, HiveLoopRunRequest,
    IssueCommentRequest, IssueDecisionRequest, IssueRunRequest, ReviewDecision,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::app::AppServices;

#[derive(Debug, Clone)]
struct DaemonState {
    services: AppServices,
}

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    id: String,
    command: String,
    #[serde(default)]
    args: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct InvokeResponse {
    kind: &'static str,
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub async fn run_stdio(services: AppServices) -> Result<()> {
    let state = Arc::new(DaemonState { services });
    let mut stdout = tokio::io::stdout();
    stdout.write_all(br#"{"kind":"ready"}"#).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let request: InvokeRequest = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                let response = InvokeResponse {
                    kind: "response",
                    id: "parse-error".to_string(),
                    ok: false,
                    result: None,
                    error: Some(error.to_string()),
                };
                stdout
                    .write_all(format!("{}\n", serde_json::to_string(&response)?).as_bytes())
                    .await?;
                stdout.flush().await?;
                continue;
            }
        };

        let response = match handle_invoke(state.as_ref(), request.command, request.args).await {
            Ok(result) => InvokeResponse {
                kind: "response",
                id: request.id,
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => InvokeResponse {
                kind: "response",
                id: request.id,
                ok: false,
                result: None,
                error: Some(error.to_string()),
            },
        };

        stdout
            .write_all(format!("{}\n", serde_json::to_string(&response)?).as_bytes())
            .await?;
        stdout.flush().await?;
    }

    Ok(())
}

pub async fn run_http(services: AppServices) -> Result<()> {
    let port = services.kernel.config.hive.http_port;
    let state = Arc::new(DaemonState { services });
    let router = Router::new()
        .route("/health", get(http_health))
        .route("/invoke", post(http_invoke).options(http_options))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("failed to bind daemon HTTP server on port {port}"))?;
    axum::serve(listener, router).await?;
    Ok(())
}

async fn http_health(State(state): State<Arc<DaemonState>>) -> impl IntoResponse {
    (
        cors_headers(),
        Json(
            state
                .services
                .kernel
                .store
                .app_status(&state.services.kernel.root)
                .unwrap(),
        ),
    )
}

async fn http_options() -> impl IntoResponse {
    cors_headers()
}

async fn http_invoke(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<InvokeRequest>,
) -> impl IntoResponse {
    match handle_invoke(state.as_ref(), request.command, request.args).await {
        Ok(result) => (
            cors_headers(),
            Json(serde_json::json!({
                "ok": true,
                "id": request.id,
                "result": result
            })),
        ),
        Err(error) => (
            cors_headers(),
            Json(serde_json::json!({
                "ok": false,
                "id": request.id,
                "error": error.to_string()
            })),
        ),
    }
}

fn cors_headers() -> [(&'static str, &'static str); 3] {
    [
        ("access-control-allow-origin", "*"),
        ("access-control-allow-methods", "GET, POST, OPTIONS"),
        ("access-control-allow-headers", "content-type"),
    ]
}

async fn handle_invoke(
    state: &DaemonState,
    command: String,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    match command.as_str() {
        "status" => Ok(serde_json::to_value(
            state
                .services
                .kernel
                .store
                .app_status(&state.services.kernel.root)?,
        )?),
        "drawer_summary" => Ok(serde_json::to_value(state.services.drawer.summary()?)?),
        "drawer_list" => Ok(serde_json::to_value(
            state
                .services
                .drawer
                .list(serde_json::from_value(args).unwrap_or_default())?,
        )?),
        "drawer_add_note" => {
            let title = args
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("Untitled Note")
                .to_string();
            let body = args
                .get("body")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let id =
                state
                    .services
                    .drawer
                    .add_note(title, body, vec!["ai-generated".to_string()])?;
            Ok(serde_json::json!({ "id": id }))
        }
        "drawer_memory_import" => {
            let title = args
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("Memory")
                .to_string();
            let body = args
                .get("body")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            Ok(serde_json::to_value(state.services.drawer.import_memory(
                title,
                body,
                vec!["ai-generated".to_string()],
            )?)?)
        }
        "drawer_import_path" => {
            let source = args
                .get("source")
                .or_else(|| args.get("path"))
                .and_then(|value| value.as_str())
                .context("drawer_import_path requires `source`")?;
            Ok(serde_json::to_value(
                state.services.drawer.import_path_report(
                    std::path::PathBuf::from(source),
                    vec!["imported".to_string()],
                )?,
            )?)
        }
        "drawer_organize_plan" => Ok(serde_json::to_value(
            state.services.drawer.plan_reorganization()?,
        )?),
        "drawer_organize_apply" => {
            let applied = state
                .services
                .drawer
                .apply_reorganization(state.services.drawer.plan_reorganization()?)?;
            Ok(serde_json::json!({ "applied": applied }))
        }
        "drawer_history" => Ok(serde_json::to_value(state.services.drawer.history(20)?)?),
        "drawer_snapshot" => {
            let summary = args
                .get("summary")
                .and_then(|value| value.as_str())
                .unwrap_or("drawer snapshot");
            Ok(serde_json::to_value(
                state.services.drawer.snapshot(summary)?,
            )?)
        }
        "drawer_rollback" => {
            let target = args
                .get("target")
                .and_then(|value| value.as_str())
                .context("drawer_rollback requires `target`")?;
            state.services.drawer.rollback(target)?;
            Ok(serde_json::json!({ "rolled_back_to": target }))
        }
        "drawer_vault_store" => Ok(serde_json::to_value(
            state.services.drawer.store_secret(VaultSecret {
                title: args
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("secret")
                    .to_string(),
                secret: args
                    .get("secret")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                tags: vec![],
            })?,
        )?),
        "drawer_vault_list" => Ok(serde_json::to_value(state.services.drawer.list_secrets()?)?),
        "hive_list" => Ok(serde_json::to_value(state.services.hive.list()?)?),
        "hive_summary" => Ok(serde_json::to_value(state.services.hive.summary()?)?),
        "hive_dispatch" => {
            let request = HiveDispatchRequest {
                title: args
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Untitled dispatch")
                    .to_string(),
                project_dir: args
                    .get("projectDir")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                summary: args
                    .get("summary")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                payload_json: serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string()),
            };
            Ok(serde_json::to_value(
                state.services.hive.dispatch(request)?,
            )?)
        }
        "hive_engine" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_engine requires `id`")?;
            Ok(serde_json::to_value(
                state.services.hive.engine_report(id)?,
            )?)
        }
        "hive_callback" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_callback requires `id`")?;
            let status = args
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("ready")
                .to_string();
            let summary = args
                .get("summary")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            Ok(serde_json::to_value(state.services.hive.callback(
                HiveCallbackRequest {
                    run_id: id,
                    status,
                    summary,
                },
            )?)?)
        }
        "hive_review" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_review requires `id`")?;
            let decision = match args
                .get("decision")
                .and_then(|value| value.as_str())
                .unwrap_or("approve")
            {
                "approve" => ReviewDecision::Approve,
                "return" => ReviewDecision::Return,
                "integrate" => ReviewDecision::Integrate,
                other => anyhow::bail!("unsupported hive review decision `{other}`"),
            };
            Ok(serde_json::to_value(
                state.services.hive.review(id, decision)?,
            )?)
        }
        "hive_loop_list" => Ok(serde_json::to_value(state.services.hive.loop_list()?)?),
        "hive_loop_show" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_show requires `id`")?;
            Ok(serde_json::to_value(state.services.hive.loop_report(id)?)?)
        }
        "hive_loop_trace" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_trace requires `id`")?;
            Ok(serde_json::to_value(state.services.hive.loop_trace(id)?)?)
        }
        "hive_loop_evidence" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_evidence requires `id`")?;
            Ok(serde_json::to_value(
                state.services.hive.loop_evidence(id)?,
            )?)
        }
        "hive_loop_audit" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_audit requires `id`")?;
            Ok(serde_json::to_value(state.services.hive.loop_audit(id)?)?)
        }
        "hive_loop_doctor" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_doctor requires `id`")?;
            Ok(serde_json::to_value(state.services.hive.loop_doctor(id)?)?)
        }
        "hive_loop_policies" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_policies requires `id`")?;
            Ok(serde_json::to_value(
                state.services.hive.loop_policies(id)?,
            )?)
        }
        "hive_policy_registry" => Ok(serde_json::to_value(state.services.hive.policy_registry())?),
        "hive_loop_create" => {
            let string_list = |key: &str| {
                args.get(key)
                    .and_then(|value| value.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            };
            Ok(serde_json::to_value(
                state.services.hive.loop_create(HiveLoopCreateRequest {
                    title: args
                        .get("title")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Untitled loop")
                        .to_string(),
                    goal: args
                        .get("goal")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Run an Entrance loop")
                        .to_string(),
                    boundary: args
                        .get("boundary")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    approach_space: string_list("approachSpace"),
                    eval_space: string_list("evalSpace"),
                    review_surface: args
                        .get("reviewSurface")
                        .and_then(|value| value.as_str())
                        .unwrap_or("local-hive-panel")
                        .to_string(),
                    autonomy_level: args
                        .get("autonomyLevel")
                        .and_then(|value| value.as_str())
                        .unwrap_or("run-approved-candidates")
                        .to_string(),
                    runtime: args
                        .get("runtime")
                        .and_then(|value| value.as_str())
                        .unwrap_or("local")
                        .to_string(),
                })?,
            )?)
        }
        "hive_loop_run" => {
            let id = args
                .get("id")
                .and_then(|value| value.as_i64())
                .context("hive_loop_run requires `id`")?;
            Ok(serde_json::to_value(
                state.services.hive.loop_run(HiveLoopRunRequest {
                    loop_id: id,
                    runtime: args
                        .get("runtime")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned),
                    decision: args
                        .get("decision")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned),
                    worker_timeout_secs: args
                        .get("workerTimeoutSecs")
                        .or_else(|| args.get("worker_timeout_secs"))
                        .and_then(|value| value.as_u64()),
                    worker_attempts: args
                        .get("workerAttempts")
                        .or_else(|| args.get("worker_attempts"))
                        .and_then(|value| value.as_u64()),
                })?,
            )?)
        }
        "hive_panel" => Ok(serde_json::to_value(state.services.hive.panel()?)?),
        "hive_issue_show" => {
            let issue_id = args
                .get("issueId")
                .or_else(|| args.get("issue_id"))
                .or_else(|| args.get("id"))
                .and_then(|value| value.as_i64())
                .context("hive_issue_show requires `issueId`")?;
            Ok(serde_json::to_value(
                state.services.hive.issue_report(issue_id)?,
            )?)
        }
        "hive_issue_comment" => {
            let issue_id = args
                .get("issueId")
                .or_else(|| args.get("issue_id"))
                .and_then(|value| value.as_i64())
                .context("hive_issue_comment requires `issueId`")?;
            Ok(serde_json::to_value(
                state.services.hive.issue_comment(IssueCommentRequest {
                    issue_id,
                    author: args
                        .get("author")
                        .and_then(|value| value.as_str())
                        .unwrap_or("human")
                        .to_string(),
                    body: args
                        .get("body")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                })?,
            )?)
        }
        "hive_issue_decide" => {
            let issue_id = args
                .get("issueId")
                .or_else(|| args.get("issue_id"))
                .and_then(|value| value.as_i64())
                .context("hive_issue_decide requires `issueId`")?;
            Ok(serde_json::to_value(
                state.services.hive.issue_decide(IssueDecisionRequest {
                    issue_id,
                    action: args
                        .get("action")
                        .and_then(|value| value.as_str())
                        .context("hive_issue_decide requires `action`")?
                        .to_string(),
                    author: args
                        .get("author")
                        .and_then(|value| value.as_str())
                        .unwrap_or("human")
                        .to_string(),
                    body: args
                        .get("body")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned),
                })?,
            )?)
        }
        "hive_issue_run" => {
            let issue_id = args
                .get("issueId")
                .or_else(|| args.get("issue_id"))
                .and_then(|value| value.as_i64())
                .context("hive_issue_run requires `issueId`")?;
            Ok(serde_json::to_value(
                state.services.hive.issue_run(IssueRunRequest {
                    issue_id,
                    runtime: args
                        .get("runtime")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned),
                    decision: args
                        .get("decision")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned),
                    worker_timeout_secs: args
                        .get("workerTimeoutSecs")
                        .or_else(|| args.get("worker_timeout_secs"))
                        .and_then(|value| value.as_u64()),
                    worker_attempts: args
                        .get("workerAttempts")
                        .or_else(|| args.get("worker_attempts"))
                        .and_then(|value| value.as_u64()),
                    retry: args
                        .get("retry")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    author: args
                        .get("author")
                        .and_then(|value| value.as_str())
                        .unwrap_or("human")
                        .to_string(),
                    body: args
                        .get("body")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned),
                })?,
            )?)
        }
        "launcher_hotkey" => Ok(serde_json::json!(state.services.launcher.hotkey())),
        "launcher_refresh" => {
            let scan_paths = args
                .get("scanPaths")
                .and_then(|value| value.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let indexed = state.services.launcher.refresh(&scan_paths)?;
            Ok(serde_json::json!({ "indexed": indexed }))
        }
        "launcher_list" => Ok(serde_json::to_value(state.services.launcher.list()?)?),
        "launcher_search" => {
            let query = LauncherQuery {
                query: args
                    .get("query")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                limit: args
                    .get("limit")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(20) as usize,
            };
            Ok(serde_json::to_value(
                state.services.launcher.search(query)?,
            )?)
        }
        "launcher_launch" => {
            let command = args
                .get("command")
                .or_else(|| args.get("path"))
                .and_then(|value| value.as_str())
                .context("launcher_launch requires `command`")?;
            let arguments = args.get("arguments").and_then(|value| value.as_str());
            let working_dir = args
                .get("workingDir")
                .or_else(|| args.get("working_dir"))
                .and_then(|value| value.as_str());
            state
                .services
                .launcher
                .launch(command, arguments, working_dir)?;
            Ok(serde_json::json!({ "launched": command }))
        }
        "launcher_pin" => {
            let command = args
                .get("command")
                .and_then(|value| value.as_str())
                .context("launcher_pin requires `command`")?;
            let pinned = args
                .get("pinned")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            state.services.launcher.pin(command, pinned)?;
            Ok(serde_json::json!({ "command": command, "pinned": pinned }))
        }
        _ => anyhow::bail!("unsupported daemon command `{command}`"),
    }
}
