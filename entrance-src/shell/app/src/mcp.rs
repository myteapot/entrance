use anyhow::{Context, Result};
use entrance_hive::{
    HiveLoopCreateRequest, IssueCard, IssueCommentRequest, IssueDecisionRequest, IssueRunRequest,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::app::AppServices;

const MCP_LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const MCP_FALLBACK_PROTOCOL_VERSION: &str = "2025-06-18";
const MCP_SCHEMA_VERSION: &str = "entrance.mcp.v1";

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

enum RequestOutcome {
    Result(serde_json::Value),
    Error(JsonRpcError),
    Notification,
}

pub async fn run_stdio(services: AppServices) -> Result<()> {
    let mut stdout = tokio::io::stdout();
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        if let Some(response) = handle_message(&services, &line) {
            stdout
                .write_all(format!("{}\n", serde_json::to_string(&response)?).as_bytes())
                .await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}

fn handle_message(services: &AppServices, line: &str) -> Option<JsonRpcResponse> {
    let parsed = serde_json::from_str::<JsonRpcRequest>(line);
    let request = match parsed {
        Ok(request) => request,
        Err(error) => {
            return Some(error_response(
                serde_json::Value::Null,
                -32700,
                "Parse error",
                Some(serde_json::json!({ "error": error.to_string() })),
            ));
        }
    };

    let id = request.id.clone();
    if request.jsonrpc.as_deref() != Some("2.0") {
        return Some(error_response(
            id.unwrap_or(serde_json::Value::Null),
            -32600,
            "Invalid Request",
            Some(serde_json::json!({ "reason": "jsonrpc must be 2.0" })),
        ));
    }

    match handle_request(services, request) {
        Ok(RequestOutcome::Result(result)) => id.map(|id| success_response(id, result)),
        Ok(RequestOutcome::Error(error)) => Some(error_response(
            id.unwrap_or(serde_json::Value::Null),
            error.code,
            &error.message,
            error.data,
        )),
        Ok(RequestOutcome::Notification) => None,
        Err(error) => Some(error_response(
            id.unwrap_or(serde_json::Value::Null),
            -32603,
            "Internal error",
            Some(serde_json::json!({ "error": error.to_string() })),
        )),
    }
}

fn handle_request(services: &AppServices, request: JsonRpcRequest) -> Result<RequestOutcome> {
    match request.method.as_str() {
        "initialize" => Ok(RequestOutcome::Result(initialize_result(&request.params))),
        "notifications/initialized" => Ok(RequestOutcome::Notification),
        "ping" => Ok(RequestOutcome::Result(serde_json::json!({}))),
        "tools/list" => Ok(RequestOutcome::Result(serde_json::json!({
            "tools": tool_specs()
        }))),
        "tools/call" => Ok(RequestOutcome::Result(call_tool(services, &request.params))),
        "prompts/list" => Ok(RequestOutcome::Result(serde_json::json!({
            "prompts": prompt_specs()
        }))),
        "prompts/get" => match get_prompt(services, &request.params) {
            Ok(value) => Ok(RequestOutcome::Result(value)),
            Err(error) => Ok(RequestOutcome::Error(json_rpc_error(
                -32602,
                "Invalid params",
                Some(serde_json::json!({ "error": error.to_string() })),
            ))),
        },
        "resources/list" => Ok(RequestOutcome::Result(list_resources(services)?)),
        "resources/read" => Ok(RequestOutcome::Result(read_resource(
            services,
            &request.params,
        )?)),
        "resources/templates/list" => Ok(RequestOutcome::Result(resource_templates())),
        _ => Ok(RequestOutcome::Error(json_rpc_error(
            -32601,
            "Method not found",
            Some(serde_json::json!({ "method": request.method })),
        ))),
    }
}

fn initialize_result(params: &serde_json::Value) -> serde_json::Value {
    let requested = params
        .get("protocolVersion")
        .and_then(|value| value.as_str())
        .unwrap_or(MCP_LATEST_PROTOCOL_VERSION);
    let protocol_version = if matches!(
        requested,
        MCP_LATEST_PROTOCOL_VERSION | MCP_FALLBACK_PROTOCOL_VERSION
    ) {
        requested
    } else {
        MCP_LATEST_PROTOCOL_VERSION
    };

    serde_json::json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": { "listChanged": false },
            "prompts": { "listChanged": false },
            "resources": { "listChanged": false }
        },
        "serverInfo": {
            "name": "entrance",
            "title": "Entrance",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Entrance exposes a Linear-like local issue/status/comment kernel. Start from prompts to preserve the loop contract, use tools to create/comment/run/retry/decide issues, and read resources for status, evidence, blockers, and verdicts."
    })
}

fn prompt_specs() -> Vec<serde_json::Value> {
    vec![
        prompt_spec(
            "entrance_loop_contract",
            "Compile an Entrance loop contract",
            "Turn a human goal into an issue-bound Entrance loop contract without running it.",
            vec![
                prompt_arg("goal", "Human goal to compile into a loop contract.", true),
                prompt_arg("boundary", "Optional safety, scope, runtime, or file boundary.", false),
                prompt_arg("runtime", "Optional runtime override such as local or codex.", false),
            ],
        ),
        prompt_spec(
            "entrance_issue_advance",
            "Advance an Entrance issue",
            "Read an issue, preserve role boundaries, and advance it through Developer/Reviewer only when the status allows it.",
            vec![
                prompt_arg("issue_id", "Entrance issue id.", true),
                prompt_arg("runtime", "Optional runtime override such as local or codex.", false),
            ],
        ),
        prompt_spec(
            "entrance_blocker_decision",
            "Prepare a blocked issue decision",
            "Summarize a Blocked or Needs Review issue into human options before retry/review/cancel.",
            vec![prompt_arg("issue_id", "Entrance issue id.", true)],
        ),
    ]
}

fn prompt_spec(
    name: &str,
    title: &str,
    description: &str,
    arguments: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "title": title,
        "description": description,
        "arguments": arguments
    })
}

fn prompt_arg(name: &str, description: &str, required: bool) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "required": required
    })
}

fn get_prompt(services: &AppServices, params: &serde_json::Value) -> Result<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .context("prompts/get requires params.name")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    match name {
        "entrance_loop_contract" => prompt_loop_contract(&args),
        "entrance_issue_advance" => prompt_issue_advance(services, &args),
        "entrance_blocker_decision" => prompt_blocker_decision(services, &args),
        other => anyhow::bail!("unknown Entrance prompt `{other}`"),
    }
}

fn prompt_loop_contract(args: &serde_json::Value) -> Result<serde_json::Value> {
    let goal = string_arg(args, "goal")?;
    let boundary = optional_string_arg(args, "boundary").unwrap_or_else(|| {
        "No explicit boundary supplied; ask before expanding scope.".to_string()
    });
    let runtime = optional_string_arg(args, "runtime").unwrap_or_else(|| "local".to_string());
    let text = format!(
        "You are the Entrance Explorer. Compile the human goal into a typed issue-bound loop contract before any implementation.\n\nGoal: {goal}\nBoundary: {boundary}\nRuntime: {runtime}\n\nRules:\n1. Do not implement directly from this prompt.\n2. Produce approach_space and eval_space as concrete arrays.\n3. Use entrance_loop_create with review_surface=local-hive-panel and runtime={runtime}.\n4. After creation, read the returned issue id and report the next issue action.\n5. Keep all state on issue/status/comment/evidence surfaces so Developer and Reviewer can be audited."
    );
    Ok(prompt_result(
        "Compile a human goal into an Entrance loop contract.",
        vec![prompt_text_message("user", text)],
    ))
}

fn prompt_issue_advance(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let runtime = optional_string_arg(args, "runtime")
        .unwrap_or_else(|| "stored contract runtime".to_string());
    let issue_resource = issue_prompt_resource(services, issue_id)?;
    let text = format!(
        "You are an Entrance loop operator for issue #{issue_id}. Read the attached issue resource before acting.\n\nRules:\n1. If status is Todo, use entrance_issue_run with runtime={runtime} and then read entrance://issues/{issue_id} again.\n2. If status is Blocked or Needs Review, do not continue automatically unless a human decision explicitly asks for retry; prepare options instead.\n3. Preserve role boundaries: Explorer understands context, Developer implements accepted work, Reviewer decides keep/reject/needs-review/blocked from gates, score vector, and evidence.\n4. If Reviewer returns reject at or after the 3-round budget, treat Blocked as the correct fallback and surface human options.\n5. Summarize only status, decision, evidence, missing receipts, blockers, and next actions. Do not invent unrecorded success."
    );
    Ok(prompt_result(
        "Advance one Entrance issue through the transparent loop contract.",
        vec![prompt_text_message("user", text), issue_resource],
    ))
}

fn prompt_blocker_decision(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let issue_resource = issue_prompt_resource(services, issue_id)?;
    let text = format!(
        "You are preparing a human decision for Entrance issue #{issue_id}. Read entrance://review-queue, entrance://policy/mcp-permissions, and the attached issue resource, then do not choose for the human.\n\nPresent:\nA. retry with the smallest changed assumption or boundary;\nB. request-review when preference, scope, or external data is needed;\nC. cancel when the candidate is no longer valuable.\n\nInclude the current status, Reviewer decision/reason, failed gates, missing receipts, evidence links, and the exact Entrance tool call to execute only after the human chooses. Add human_confirmed=true only after the human has chosen that option."
    );
    Ok(prompt_result(
        "Prepare human retry/review/cancel options for a blocked Entrance issue.",
        vec![prompt_text_message("user", text), issue_resource],
    ))
}

fn issue_prompt_resource(services: &AppServices, issue_id: i64) -> Result<serde_json::Value> {
    let uri = format!("entrance://issues/{issue_id}");
    let report = services.hive.issue_report(issue_id)?;
    Ok(serde_json::json!({
        "role": "user",
        "content": {
            "type": "resource",
            "resource": {
                "uri": uri,
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&report)?
            }
        }
    }))
}

fn prompt_result(description: &str, messages: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "description": description,
        "messages": messages
    })
}

fn prompt_text_message(role: &str, text: String) -> serde_json::Value {
    serde_json::json!({
        "role": role,
        "content": {
            "type": "text",
            "text": text
        }
    })
}

fn tool_specs() -> Vec<serde_json::Value> {
    vec![
        tool_spec(
            "entrance_issue_list",
            "List Entrance issues",
            "List local Hive issue/status/comment cards from the Entrance kernel.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Optional issue status filter such as Todo, Doing, Blocked, Needs Review, Done, or Canceled."
                    }
                }
            }),
        ),
        tool_spec(
            "entrance_issue_show",
            "Show an Entrance issue",
            "Read one issue card with comments, trace, doctor, and evidence summaries.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer", "description": "Issue id." }
                },
                "required": ["issue_id"]
            }),
        ),
        tool_spec(
            "entrance_review_queue",
            "List Entrance review queue",
            "List Blocked and Needs Review issues with decision options, blockers, and evidence summaries.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["Blocked", "Needs Review"],
                        "description": "Optional queue status filter."
                    }
                }
            }),
        ),
        tool_spec(
            "entrance_issue_comment",
            "Comment on an Entrance issue",
            "Add an issue comment and mirror it into loop evidence when the issue is loop-bound.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "body": { "type": "string" },
                    "author": { "type": "string", "description": "Optional author label. Defaults to mcp-agent." }
                },
                "required": ["issue_id", "body"]
            }),
        ),
        tool_spec(
            "entrance_loop_create",
            "Create an Entrance loop issue",
            "Create a local issue-bound Explorer -> Developer -> Reviewer loop contract.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "goal": { "type": "string" },
                    "boundary": { "type": "string" },
                    "runtime": { "type": "string", "description": "local or codex. Defaults to local." },
                    "review_surface": { "type": "string", "description": "Defaults to local-hive-panel." },
                    "approach_space": { "type": "array", "items": { "type": "string" } },
                    "eval_space": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["title", "goal"]
            }),
        ),
        tool_spec(
            "entrance_issue_run",
            "Run an Entrance issue",
            "Run a Todo issue through Explorer -> Developer -> Reviewer.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "runtime": { "type": "string" },
                    "decision": { "type": "string", "enum": ["keep", "reject", "needs-review", "blocked"] },
                    "worker_timeout_secs": { "type": "integer" },
                    "worker_attempts": { "type": "integer" }
                },
                "required": ["issue_id"]
            }),
        ),
        tool_spec(
            "entrance_issue_retry",
            "Retry an Entrance issue",
            "Record a human-confirmed retry decision, advance the loop round, and run the issue again.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "body": { "type": "string" },
                    "human_confirmed": {
                        "type": "boolean",
                        "description": "Must be true because retry is a human decision boundary."
                    },
                    "author": { "type": "string", "description": "Optional author label. Defaults to mcp-agent." },
                    "runtime": { "type": "string" },
                    "decision": { "type": "string", "enum": ["keep", "reject", "needs-review", "blocked"] },
                    "worker_timeout_secs": { "type": "integer" },
                    "worker_attempts": { "type": "integer" }
                },
                "required": ["issue_id", "body", "human_confirmed"]
            }),
        ),
        tool_spec(
            "entrance_issue_decide",
            "Decide an Entrance issue",
            "Move an issue through a human-confirmed decision action: retry, request-review, or cancel.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "action": { "type": "string", "enum": ["retry", "request-review", "cancel"] },
                    "body": { "type": "string" },
                    "human_confirmed": {
                        "type": "boolean",
                        "description": "Must be true because retry/review/cancel are human decision boundaries."
                    },
                    "author": { "type": "string", "description": "Optional author label. Defaults to mcp-agent." }
                },
                "required": ["issue_id", "action", "human_confirmed"]
            }),
        ),
    ]
}

fn tool_spec(
    name: &str,
    title: &str,
    description: &str,
    input_schema: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema
    })
}

fn call_tool(services: &AppServices, params: &serde_json::Value) -> serde_json::Value {
    let name = params.get("name").and_then(|value| value.as_str());
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let result = match name {
        Some("entrance_issue_list") => tool_issue_list(services, &args),
        Some("entrance_issue_show") => tool_issue_show(services, &args),
        Some("entrance_review_queue") => tool_review_queue(services, &args),
        Some("entrance_issue_comment") => tool_issue_comment(services, &args),
        Some("entrance_loop_create") => tool_loop_create(services, &args),
        Some("entrance_issue_run") => tool_issue_run(services, &args, false),
        Some("entrance_issue_retry") => tool_issue_run(services, &args, true),
        Some("entrance_issue_decide") => tool_issue_decide(services, &args),
        Some(other) => Err(anyhow::anyhow!("unknown tool `{other}`")),
        None => Err(anyhow::anyhow!("tools/call requires params.name")),
    };

    match result {
        Ok(value) => tool_result(false, tool_summary(&value), value),
        Err(error) => tool_result(
            true,
            error.to_string(),
            serde_json::json!({
                "schema_version": MCP_SCHEMA_VERSION,
                "error": error.to_string()
            }),
        ),
    }
}

fn tool_issue_list(services: &AppServices, args: &serde_json::Value) -> Result<serde_json::Value> {
    let status = args.get("status").and_then(|value| value.as_str());
    let issues = services
        .hive
        .panel()?
        .into_iter()
        .filter(|card| status.is_none_or(|status| card.issue.status == status))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.issue_list.v1",
        "count": issues.len(),
        "issues": issues
    }))
}

fn tool_issue_show(services: &AppServices, args: &serde_json::Value) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.issue_show.v1",
        "issue": services.hive.issue_report(issue_id)?
    }))
}

fn tool_review_queue(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let status = args.get("status").and_then(|value| value.as_str());
    review_queue(services, status)
}

fn tool_issue_comment(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let body = string_arg(args, "body")?;
    let author = optional_string_arg(args, "author").unwrap_or_else(|| "mcp-agent".to_string());
    let card = services.hive.issue_comment(IssueCommentRequest {
        issue_id,
        author,
        body,
    })?;
    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.issue_comment.v1",
        "issue": card
    }))
}

fn tool_loop_create(services: &AppServices, args: &serde_json::Value) -> Result<serde_json::Value> {
    let title = string_arg(args, "title")?;
    let goal = string_arg(args, "goal")?;
    let report = services.hive.loop_create(HiveLoopCreateRequest {
        title,
        goal,
        boundary: optional_string_arg(args, "boundary").unwrap_or_default(),
        approach_space: optional_string_array_arg(args, "approach_space"),
        eval_space: optional_string_array_arg(args, "eval_space"),
        review_surface: optional_string_arg(args, "review_surface")
            .unwrap_or_else(|| "local-hive-panel".to_string()),
        autonomy_level: optional_string_arg(args, "autonomy_level")
            .unwrap_or_else(|| "run-approved-candidates".to_string()),
        runtime: optional_string_arg(args, "runtime").unwrap_or_else(|| "local".to_string()),
    })?;
    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.loop_create.v1",
        "loop": report.contract,
        "issues": report.issues
    }))
}

fn tool_issue_run(
    services: &AppServices,
    args: &serde_json::Value,
    retry: bool,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let body = if retry {
        ensure_human_confirmed(args, "retry")?;
        Some(string_arg(args, "body")?)
    } else {
        optional_string_arg(args, "body")
    };
    let report = services.hive.issue_run(IssueRunRequest {
        issue_id,
        runtime: optional_string_arg(args, "runtime"),
        decision: optional_string_arg(args, "decision"),
        worker_timeout_secs: optional_u64_arg(args, "worker_timeout_secs"),
        worker_attempts: optional_u64_arg(args, "worker_attempts"),
        retry,
        author: optional_string_arg(args, "author").unwrap_or_else(|| "mcp-agent".to_string()),
        body,
    })?;
    Ok(serde_json::json!({
        "schema_version": if retry { "entrance.mcp.issue_retry.v1" } else { "entrance.mcp.issue_run.v1" },
        "loop": report.contract,
        "issues": report.issues,
        "verdicts": report.verdicts
    }))
}

fn tool_issue_decide(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let action = string_arg(args, "action")?;
    ensure_human_confirmed(args, &action)?;
    let card = services.hive.issue_decide(IssueDecisionRequest {
        issue_id,
        action,
        author: optional_string_arg(args, "author").unwrap_or_else(|| "mcp-agent".to_string()),
        body: optional_string_arg(args, "body"),
    })?;
    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.issue_decide.v1",
        "issue": card
    }))
}

fn list_resources(services: &AppServices) -> Result<serde_json::Value> {
    let mut resources = vec![
        resource_spec(
            "entrance://status",
            "Entrance status",
            "App root, database path, counts, and schema health.",
        ),
        resource_spec(
            "entrance://issues",
            "Entrance issues",
            "All local issue/status/comment cards.",
        ),
        resource_spec(
            "entrance://review-queue",
            "Entrance review queue",
            "Blocked and Needs Review issue decision surface.",
        ),
        resource_spec(
            "entrance://policy/registry",
            "Entrance policy registry",
            "Active loop, runtime, and connector policy registry.",
        ),
        resource_spec(
            "entrance://policy/mcp-permissions",
            "Entrance MCP permissions",
            "MCP tool permission and human confirmation policy.",
        ),
        resource_spec(
            "entrance://schema/status",
            "Entrance schema status",
            "SQLite schema contract and health report.",
        ),
    ];
    for card in services.hive.panel()? {
        resources.push(resource_spec(
            &format!("entrance://issues/{}", card.issue.id),
            &format!("Issue #{}: {}", card.issue.id, card.issue.title),
            "One issue card with comments, trace, doctor, and evidence.",
        ));
    }
    Ok(serde_json::json!({ "resources": resources }))
}

fn resource_templates() -> serde_json::Value {
    serde_json::json!({
        "resourceTemplates": [
            {
                "uriTemplate": "entrance://issues/{issue_id}",
                "name": "Entrance issue by id",
                "description": "Read one issue card with comments, trace, doctor, and evidence.",
                "mimeType": "application/json"
            }
        ]
    })
}

fn resource_spec(uri: &str, name: &str, description: &str) -> serde_json::Value {
    serde_json::json!({
        "uri": uri,
        "name": name,
        "description": description,
        "mimeType": "application/json"
    })
}

fn read_resource(services: &AppServices, params: &serde_json::Value) -> Result<serde_json::Value> {
    let uri = params
        .get("uri")
        .and_then(|value| value.as_str())
        .context("resources/read requires params.uri")?;
    let value = match uri {
        "entrance://status" => {
            serde_json::to_value(services.kernel.store.app_status(&services.kernel.root)?)?
        }
        "entrance://issues" => tool_issue_list(services, &serde_json::json!({}))?,
        "entrance://review-queue" => review_queue(services, None)?,
        "entrance://policy/registry" => serde_json::to_value(services.hive.policy_registry())?,
        "entrance://policy/mcp-permissions" => mcp_permission_policy(),
        "entrance://schema/status" => serde_json::to_value(services.kernel.store.schema_status()?)?,
        value if value.starts_with("entrance://issues/") => {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .parse::<i64>()
                .with_context(|| format!("invalid Entrance issue resource URI `{value}`"))?;
            serde_json::to_value(services.hive.issue_report(issue_id)?)?
        }
        other => anyhow::bail!("unknown resource `{other}`"),
    };
    Ok(serde_json::json!({
        "contents": [
            {
                "uri": uri,
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&value)?
            }
        ]
    }))
}

fn review_queue(services: &AppServices, status: Option<&str>) -> Result<serde_json::Value> {
    if !matches!(status, None | Some("Blocked") | Some("Needs Review")) {
        anyhow::bail!("review queue status must be Blocked or Needs Review");
    }

    let items = services
        .hive
        .panel()?
        .into_iter()
        .filter(|card| {
            matches!(card.issue.status.as_str(), "Blocked" | "Needs Review")
                && status.is_none_or(|status| card.issue.status == status)
        })
        .map(|card| review_queue_item(&card))
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.review_queue.v1",
        "count": items.len(),
        "statuses": ["Blocked", "Needs Review"],
        "items": items
    }))
}

fn review_queue_item(card: &IssueCard) -> serde_json::Value {
    let latest_comment = card.comments.last();
    let trace = card.trace.as_ref();
    let doctor = card.doctor.as_ref();
    let recent_evidence = trace
        .map(|trace| {
            trace
                .evidence
                .iter()
                .rev()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    serde_json::json!({
        "schema_version": "entrance.mcp.review_queue_item.v1",
        "issue": &card.issue,
        "status": &card.issue.status,
        "decision": trace.and_then(|trace| trace.last_decision.as_deref()),
        "reason_code": trace.and_then(|trace| trace.reason_code.as_deref()),
        "current_round": trace.map(|trace| trace.current_round),
        "human_options": trace.map(|trace| trace.human_options.clone()).unwrap_or_default(),
        "actions": &card.actions,
        "mcp_policy": mcp_issue_policy(&card.actions),
        "blockers": {
            "failed_checks": doctor.map(|doctor| doctor.failed_checks.clone()).unwrap_or_default(),
            "audit_failure_details": doctor.map(|doctor| doctor.audit_failure_details.clone()).unwrap_or_default(),
            "missing_receipts": doctor.map(|doctor| doctor.missing_receipts.clone()).unwrap_or_default(),
            "worker_failures": doctor.map(|doctor| doctor.worker_failures.clone()).unwrap_or_default()
        },
        "doctor": doctor.map(|doctor| serde_json::json!({
            "health": &doctor.health,
            "summary": &doctor.summary,
            "next_actions": &doctor.next_actions
        })),
        "latest_comment": latest_comment,
        "recent_evidence": recent_evidence
    })
}

fn mcp_issue_policy(actions: &[entrance_hive::IssueAction]) -> serde_json::Value {
    let human_confirmed_actions = actions
        .iter()
        .filter(|action| mcp_action_requires_human_confirmation(&action.action))
        .map(|action| action.action.as_str())
        .collect::<Vec<_>>();

    serde_json::json!({
        "schema_version": "entrance.mcp.issue_permission_policy.v1",
        "human_confirmed_actions": human_confirmed_actions,
        "confirmation_arg": "human_confirmed",
        "policy_resource": "entrance://policy/mcp-permissions"
    })
}

fn mcp_permission_policy() -> serde_json::Value {
    serde_json::json!({
        "schema_version": "entrance.mcp.permission_policy.v1",
        "default_actor": "mcp-agent",
        "read_only_tools": [
            "entrance_issue_list",
            "entrance_issue_show",
            "entrance_review_queue"
        ],
        "write_tools": [
            "entrance_issue_comment",
            "entrance_loop_create",
            "entrance_issue_run"
        ],
        "human_decision_tools": [
            "entrance_issue_retry",
            "entrance_issue_decide"
        ],
        "requires_human_confirmation": [
            {
                "tool": "entrance_issue_retry",
                "actions": ["retry"],
                "argument": "human_confirmed",
                "value": true,
                "reason": "Retry advances a blocked/rejected issue into a new loop round."
            },
            {
                "tool": "entrance_issue_decide",
                "actions": ["retry", "request-review", "cancel"],
                "argument": "human_confirmed",
                "value": true,
                "reason": "Retry, review, and cancel are human decision boundaries."
            }
        ],
        "blocked_boundary": {
            "statuses": ["Blocked", "Needs Review"],
            "resource": "entrance://review-queue",
            "prompt": "entrance_blocker_decision"
        }
    })
}

fn mcp_action_requires_human_confirmation(action: &str) -> bool {
    matches!(action, "retry" | "request-review" | "cancel")
}

fn ensure_human_confirmed(args: &serde_json::Value, action: &str) -> Result<()> {
    if args
        .get("human_confirmed")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        return Ok(());
    }

    anyhow::bail!(
        "MCP action `{action}` requires human_confirmed=true; read entrance://review-queue or prompt entrance_blocker_decision before executing human decisions"
    )
}

fn tool_result(is_error: bool, text: String, structured: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": is_error
    })
}

fn tool_summary(value: &serde_json::Value) -> String {
    if let Some(schema) = value.get("schema_version").and_then(|value| value.as_str()) {
        return format!("{schema} completed.");
    }
    "Entrance tool completed.".to_string()
}

fn success_response(id: serde_json::Value, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn json_rpc_error(code: i64, message: &str, data: Option<serde_json::Value>) -> JsonRpcError {
    JsonRpcError {
        code,
        message: message.to_string(),
        data,
    }
}

fn error_response(
    id: serde_json::Value,
    code: i64,
    message: &str,
    data: Option<serde_json::Value>,
) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(json_rpc_error(code, message, data)),
    }
}

fn integer_arg(args: &serde_json::Value, name: &str) -> Result<i64> {
    args.get(name)
        .and_then(|value| value.as_i64())
        .with_context(|| format!("missing integer argument `{name}`"))
}

fn string_arg(args: &serde_json::Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .with_context(|| format!("missing string argument `{name}`"))
}

fn optional_string_arg(args: &serde_json::Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn optional_u64_arg(args: &serde_json::Value, name: &str) -> Option<u64> {
    args.get(name).and_then(|value| value.as_u64())
}

fn optional_string_array_arg(args: &serde_json::Value, name: &str) -> Vec<String> {
    args.get(name)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_human_confirmed, initialize_result, mcp_permission_policy, prompt_loop_contract,
        prompt_specs, tool_specs, MCP_FALLBACK_PROTOCOL_VERSION,
    };

    #[test]
    fn initialize_negotiates_supported_protocol_and_capabilities() {
        let result = initialize_result(&serde_json::json!({
            "protocolVersion": MCP_FALLBACK_PROTOCOL_VERSION
        }));

        assert_eq!(
            result
                .get("protocolVersion")
                .and_then(|value| value.as_str()),
            Some(MCP_FALLBACK_PROTOCOL_VERSION)
        );
        assert_eq!(
            result
                .pointer("/capabilities/tools/listChanged")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            result
                .pointer("/capabilities/resources/listChanged")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            result
                .pointer("/capabilities/prompts/listChanged")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn issue_tools_are_exposed_for_agents() {
        let tools = tool_specs();
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(names.contains(&"entrance_issue_list"));
        assert!(names.contains(&"entrance_loop_create"));
        assert!(names.contains(&"entrance_issue_run"));
        assert!(names.contains(&"entrance_issue_retry"));
        assert!(names.contains(&"entrance_review_queue"));
        assert!(names.contains(&"entrance_issue_comment"));
        assert!(names.contains(&"entrance_issue_decide"));
    }

    #[test]
    fn human_decision_tools_expose_confirmation_policy() {
        let tools = tool_specs();
        let retry_tool = tools
            .iter()
            .find(|tool| {
                tool.get("name").and_then(|value| value.as_str()) == Some("entrance_issue_retry")
            })
            .expect("retry tool should exist");
        let decide_tool = tools
            .iter()
            .find(|tool| {
                tool.get("name").and_then(|value| value.as_str()) == Some("entrance_issue_decide")
            })
            .expect("decide tool should exist");

        assert_eq!(
            retry_tool.pointer("/inputSchema/properties/human_confirmed/type"),
            Some(&serde_json::json!("boolean"))
        );
        assert_eq!(
            decide_tool.pointer("/inputSchema/properties/human_confirmed/type"),
            Some(&serde_json::json!("boolean"))
        );
        assert_eq!(
            ensure_human_confirmed(&serde_json::json!({}), "retry")
                .expect_err("missing confirmation should fail")
                .to_string(),
            "MCP action `retry` requires human_confirmed=true; read entrance://review-queue or prompt entrance_blocker_decision before executing human decisions"
        );
        assert!(
            ensure_human_confirmed(&serde_json::json!({ "human_confirmed": true }), "retry")
                .is_ok()
        );
    }

    #[test]
    fn permission_policy_names_human_decision_tools() {
        let policy = mcp_permission_policy();
        let human_tools = policy
            .get("human_decision_tools")
            .and_then(|value| value.as_array())
            .expect("policy should list human decision tools");

        assert!(human_tools
            .iter()
            .any(|value| value.as_str() == Some("entrance_issue_retry")));
        assert!(human_tools
            .iter()
            .any(|value| value.as_str() == Some("entrance_issue_decide")));
        assert_eq!(
            policy
                .pointer("/blocked_boundary/resource")
                .and_then(|value| value.as_str()),
            Some("entrance://review-queue")
        );
    }

    #[test]
    fn loop_prompts_are_exposed_for_agents() {
        let prompts = prompt_specs();
        let names = prompts
            .iter()
            .filter_map(|prompt| prompt.get("name").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(names.contains(&"entrance_loop_contract"));
        assert!(names.contains(&"entrance_issue_advance"));
        assert!(names.contains(&"entrance_blocker_decision"));
    }

    #[test]
    fn contract_prompt_constrains_agents_to_issue_tools() {
        let result = prompt_loop_contract(&serde_json::json!({
            "goal": "Ship an observable issue loop",
            "runtime": "local"
        }))
        .expect("prompt should render");
        let text = result
            .pointer("/messages/0/content/text")
            .and_then(|value| value.as_str())
            .expect("prompt should include text");

        assert!(text.contains("Do not implement directly"));
        assert!(text.contains("entrance_loop_create"));
        assert!(text.contains("issue/status/comment/evidence"));
    }
}
