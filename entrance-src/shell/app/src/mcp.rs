use anyhow::{Context, Result};
use entrance_hive::{
    issue_advance_next_action, HiveLoopCreateRequest, IssueAdvanceRequest, IssueCard,
    IssueClaimRequest, IssueCommentRequest, IssueDecisionRequest, IssueRunRequest,
    OperatorConfirmationActor, OperatorConfirmationClient, OperatorConfirmationReceipt,
    OPERATOR_ACTION_CONFIRMATION_ARG, OPERATOR_ACTION_POLICY_SCHEMA_VERSION,
    OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    app::AppServices,
    mcp_resources::{resource_spec, resource_templates},
};

const MCP_LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const MCP_FALLBACK_PROTOCOL_VERSION: &str = "2025-06-18";
const MCP_ISSUE_CONTROL_SCHEMA_VERSION: &str = "entrance.mcp.issue_control.v1";
const MCP_LOOP_CONTROL_SCHEMA_VERSION: &str = "entrance.mcp.loop_control.v1";
const MCP_PERMISSION_POLICY_SCHEMA_VERSION: &str = "entrance.mcp.permission_policy.v1";
const MCP_TOOL_PERMISSION_SCHEMA_VERSION: &str = "entrance.mcp.tool_permission.v1";
const MCP_TOOL_PERMISSION_REGISTRY_SCHEMA_VERSION: &str =
    "entrance.mcp.tool_permission_registry.v1";
const MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION: &str = "entrance.mcp.actor_identity_policy.v1";

const MCP_TOOL_NAMES: &[&str] = &[
    "entrance_issue_create",
    "entrance_issue_list",
    "entrance_issue_show",
    "entrance_issue_claim",
    "entrance_issue_comment",
    "entrance_issue_run",
    "entrance_issue_advance",
    "entrance_issue_review",
    "entrance_issue_retry",
    "entrance_issue_decide",
    "entrance_issue_control",
    "entrance_loop_create",
    "entrance_loop_control",
    "entrance_review_queue",
];

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

#[derive(Debug, Default, Clone)]
struct McpSession {
    client: Option<McpClientIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct McpClientIdentity {
    name: String,
    version: Option<String>,
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
    let mut session = McpSession::default();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_message(&services, &mut session, &line) {
            stdout
                .write_all(format!("{}\n", serde_json::to_string(&response)?).as_bytes())
                .await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

fn handle_message(
    services: &AppServices,
    session: &mut McpSession,
    line: &str,
) -> Option<JsonRpcResponse> {
    let request = match serde_json::from_str::<JsonRpcRequest>(line) {
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
    match handle_request(services, session, request) {
        Ok(RequestOutcome::Result(result)) => id.map(|id| success_response(id, result)),
        Ok(RequestOutcome::Notification) => None,
        Ok(RequestOutcome::Error(error)) => Some(error_response(
            id.unwrap_or(serde_json::Value::Null),
            error.code,
            &error.message,
            error.data,
        )),
        Err(error) => Some(error_response(
            id.unwrap_or(serde_json::Value::Null),
            -32603,
            "Internal error",
            Some(serde_json::json!({ "error": error.to_string() })),
        )),
    }
}

fn handle_request(
    services: &AppServices,
    session: &mut McpSession,
    request: JsonRpcRequest,
) -> Result<RequestOutcome> {
    match request.method.as_str() {
        "initialize" => {
            session.client = mcp_client_identity(&request.params);
            Ok(RequestOutcome::Result(initialize_result(&request.params)))
        }
        "notifications/initialized" => Ok(RequestOutcome::Notification),
        "ping" => Ok(RequestOutcome::Result(serde_json::json!({}))),
        "tools/list" => Ok(RequestOutcome::Result(
            serde_json::json!({ "tools": tool_specs() }),
        )),
        "tools/call" => match call_tool(services, session, &request.params) {
            Ok(value) => Ok(RequestOutcome::Result(value)),
            Err(error) => Ok(RequestOutcome::Error(json_rpc_error(
                -32602,
                "Invalid params",
                Some(serde_json::json!({ "error": error.to_string() })),
            ))),
        },
        "prompts/list" => Ok(RequestOutcome::Result(
            serde_json::json!({ "prompts": prompt_specs() }),
        )),
        "prompts/get" => Ok(RequestOutcome::Result(prompt_get(
            services,
            &request.params,
        )?)),
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
        "instructions": "Entrance exposes a local MCP-native issue workbench. Use issue tools, issue control resources, and review queue resources; remote synchronization surfaces are intentionally absent."
    })
}

fn mcp_client_identity(params: &serde_json::Value) -> Option<McpClientIdentity> {
    let info = params
        .get("clientInfo")
        .or_else(|| params.get("client_info"))?;
    let name = info
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let version = info
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    Some(McpClientIdentity {
        name: name.to_string(),
        version,
    })
}

fn prompt_specs() -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "name": "entrance_issue_review",
        "title": "Review an Entrance issue",
        "description": "Review one local issue from recorded control evidence. Do not implement in Reviewer mode.",
        "arguments": [{
            "name": "issue_id",
            "description": "Entrance issue id.",
            "required": true
        }]
    })]
}

fn prompt_get(services: &AppServices, params: &serde_json::Value) -> Result<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if name != "entrance_issue_review" {
        anyhow::bail!("unknown prompt `{name}`");
    }
    let args = params.get("arguments").cloned().unwrap_or_default();
    let issue_id = required_i64(&args, "issue_id")?;
    let packet = issue_control_packet(services, issue_id)?;
    Ok(serde_json::json!({
        "description": "Review one Entrance issue from recorded local evidence.",
        "messages": [{
            "role": "user",
            "content": {
                "type": "text",
                "text": format!("Read entrance://issues/{issue_id}/control and decide keep/reject/blocked from the recorded evidence only. Developer implements; Reviewer only evaluates.")
            }
        }, {
            "role": "user",
            "content": {
                "type": "resource",
                "resource": {
                    "uri": format!("entrance://issues/{issue_id}/control"),
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&packet)?
                }
            }
        }]
    }))
}

fn tool_specs() -> Vec<serde_json::Value> {
    MCP_TOOL_NAMES.iter().map(|name| tool_spec(name)).collect()
}

fn tool_spec(name: &str) -> serde_json::Value {
    let (title, description, schema) = match name {
        "entrance_issue_create" | "entrance_loop_create" => (
            "Create Entrance issue",
            "Create a local issue-bound loop.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "goal": { "type": "string" },
                    "runtime": { "type": "string", "enum": ["local", "codex"] }
                },
                "required": ["title"]
            }),
        ),
        "entrance_issue_show" | "entrance_issue_control" => (
            "Read Entrance issue",
            "Read one local issue or its control packet.",
            id_schema("issue_id"),
        ),
        "entrance_loop_control" => (
            "Read Entrance loop control",
            "Read one Reviewer-ready loop control packet.",
            id_schema("loop_id"),
        ),
        "entrance_issue_claim" => (
            "Claim Entrance issue",
            "Record a local claim comment with agent and role.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "agent": { "type": "string" },
                    "role": { "type": "string", "enum": ["developer", "reviewer"] }
                },
                "required": ["issue_id", "agent"]
            }),
        ),
        "entrance_issue_comment" => (
            "Comment on Entrance issue",
            "Append a local issue comment.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "body": { "type": "string" },
                    "author": { "type": "string" }
                },
                "required": ["issue_id", "body"]
            }),
        ),
        "entrance_issue_run" | "entrance_issue_retry" => (
            "Run Entrance issue",
            "Run or retry a local Developer -> Reviewer issue loop.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "runtime": { "type": "string", "enum": ["local", "codex"] },
                    "human_confirmed": { "type": "boolean" },
                    "body": { "type": "string" }
                },
                "required": ["issue_id"]
            }),
        ),
        "entrance_issue_advance" => (
            "Advance Entrance issue",
            "Advance a local issue through the kernel until one step or wait state.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "mode": { "type": "string", "enum": ["one_step", "until_wait"] },
                    "runtime": { "type": "string", "enum": ["local", "codex"] },
                    "max_steps": { "type": "integer", "minimum": 1 },
                    "worker_timeout_secs": { "type": "integer", "minimum": 1 },
                    "worker_attempts": { "type": "integer", "minimum": 1 }
                },
                "required": ["issue_id"]
            }),
        ),
        "entrance_issue_review" => (
            "Review Entrance issue",
            "Record a Reviewer decision as local issue review context.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "decision": { "type": "string", "enum": ["keep", "reject", "blocked"] },
                    "body": { "type": "string" },
                    "author": { "type": "string" }
                },
                "required": ["issue_id", "decision"]
            }),
        ),
        "entrance_issue_decide" => (
            "Decide Entrance issue",
            "Apply a human retry/request-review/cancel decision with confirmation.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer" },
                    "action": { "type": "string", "enum": ["retry", "request-review", "cancel"] },
                    "human_confirmed": { "type": "boolean" },
                    "body": { "type": "string" },
                    "author": { "type": "string" }
                },
                "required": ["issue_id", "action", "human_confirmed"]
            }),
        ),
        "entrance_issue_list" | "entrance_review_queue" => (
            "List Entrance issues",
            "List local issues or the Blocked/Needs Review queue.",
            serde_json::json!({ "type": "object", "properties": {} }),
        ),
        _ => (
            "Entrance tool",
            "Entrance local issue tool.",
            serde_json::json!({ "type": "object" }),
        ),
    };
    serde_json::json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": schema,
        "annotations": mcp_tool_permission(name)
    })
}

fn id_schema(name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": { name: { "type": "integer" } },
        "required": [name]
    })
}

fn call_tool(
    services: &AppServices,
    session: &McpSession,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .context("tools/call requires params.name")?;
    let args = params.get("arguments").cloned().unwrap_or_default();
    let value = match name {
        "entrance_issue_create" | "entrance_loop_create" => {
            let title = required_string(&args, "title")?;
            let goal = optional_string(&args, "goal").unwrap_or_else(|| title.clone());
            serde_json::to_value(services.hive.loop_create(HiveLoopCreateRequest {
                title,
                goal,
                boundary: "Local MCP-native issue workbench boundary.".to_string(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: "local-hive-panel".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: optional_string(&args, "runtime").unwrap_or_else(|| "local".to_string()),
            })?)?
        }
        "entrance_issue_list" => serde_json::to_value(services.hive.panel()?)?,
        "entrance_issue_show" => serde_json::to_value(
            services
                .hive
                .issue_report(required_i64(&args, "issue_id")?)?,
        )?,
        "entrance_issue_control" => {
            issue_control_packet(services, required_i64(&args, "issue_id")?)?
        }
        "entrance_loop_control" => loop_control_packet(services, required_i64(&args, "loop_id")?)?,
        "entrance_issue_claim" => {
            let issue_id = required_i64(&args, "issue_id")?;
            let agent = required_string(&args, "agent")?;
            let role = optional_string(&args, "role").unwrap_or_else(|| "developer".to_string());
            serde_json::to_value(services.hive.issue_claim(IssueClaimRequest {
                issue_id,
                agent,
                role: Some(role),
                source: Some("mcp".to_string()),
            })?)?
        }
        "entrance_issue_comment" => {
            serde_json::to_value(services.hive.issue_comment(IssueCommentRequest {
                issue_id: required_i64(&args, "issue_id")?,
                author: optional_string(&args, "author").unwrap_or_else(|| "mcp".to_string()),
                body: required_string(&args, "body")?,
            })?)?
        }
        "entrance_issue_run" | "entrance_issue_retry" => {
            let retry = name == "entrance_issue_retry";
            let author = optional_string(&args, "author").unwrap_or_else(|| "mcp".to_string());
            serde_json::to_value(
                services.hive.issue_run(IssueRunRequest {
                    issue_id: required_i64(&args, "issue_id")?,
                    runtime: optional_string(&args, "runtime"),
                    decision: None,
                    worker_timeout_secs: None,
                    worker_attempts: None,
                    retry,
                    author: author.clone(),
                    body: optional_string(&args, "body"),
                    confirmation_receipt: retry
                        .then(|| {
                            mcp_human_confirmation_receipt(
                                "retry",
                                &author,
                                human_confirmed(&args),
                                session.client.as_ref(),
                            )
                        })
                        .flatten(),
                })?,
            )?
        }
        "entrance_issue_advance" => serde_json::to_value(
            services.hive.issue_advance(IssueAdvanceRequest {
                issue_id: required_i64(&args, "issue_id")?,
                mode: optional_string(&args, "mode"),
                runtime: optional_string(&args, "runtime"),
                max_steps: args
                    .get("max_steps")
                    .and_then(|value| value.as_u64())
                    .map(|value| value as usize),
                worker_timeout_secs: args
                    .get("worker_timeout_secs")
                    .and_then(|value| value.as_u64()),
                worker_attempts: args.get("worker_attempts").and_then(|value| value.as_u64()),
            })?,
        )?,
        "entrance_issue_review" => {
            let decision = required_string(&args, "decision")?;
            let action = match decision.as_str() {
                "keep" | "reject" | "blocked" => "request-review",
                _ => anyhow::bail!("unsupported review decision `{decision}`"),
            };
            let author = optional_string(&args, "author").unwrap_or_else(|| "reviewer".to_string());
            serde_json::to_value(
                services.hive.issue_decide(IssueDecisionRequest {
                    issue_id: required_i64(&args, "issue_id")?,
                    action: action.to_string(),
                    author: author.clone(),
                    body: optional_string(&args, "body")
                        .or_else(|| Some(format!("Reviewer decision: {decision}."))),
                    confirmation_receipt: mcp_human_confirmation_receipt(
                        action,
                        &author,
                        true,
                        session.client.as_ref(),
                    ),
                })?,
            )?
        }
        "entrance_issue_decide" => {
            let action = required_string(&args, "action")?;
            let author = optional_string(&args, "author").unwrap_or_else(|| "mcp".to_string());
            serde_json::to_value(services.hive.issue_decide(IssueDecisionRequest {
                issue_id: required_i64(&args, "issue_id")?,
                action: action.clone(),
                author: author.clone(),
                body: optional_string(&args, "body"),
                confirmation_receipt: mcp_human_confirmation_receipt(
                    &action,
                    &author,
                    human_confirmed(&args),
                    session.client.as_ref(),
                ),
            })?)?
        }
        "entrance_review_queue" => review_queue(services)?,
        _ => anyhow::bail!("unknown tool `{name}`"),
    };
    Ok(tool_text(value))
}

fn list_resources(services: &AppServices) -> Result<serde_json::Value> {
    let mut resources = vec![
        resource_spec("entrance://status", "Entrance status", "Local app status."),
        resource_spec("entrance://issues", "Entrance issues", "Local issue board."),
        resource_spec(
            "entrance://review-queue",
            "Entrance review queue",
            "Blocked and Needs Review issues.",
        ),
        resource_spec(
            "entrance://policy/issue-transitions",
            "Entrance issue transition policy",
            "Local issue transition policy.",
        ),
        resource_spec(
            "entrance://policy/mcp-permissions",
            "Entrance MCP permissions",
            "Human-confirmation requirements for tools.",
        ),
        resource_spec(
            "entrance://policy/actor-identity",
            "Entrance actor identity policy",
            "Self-reported MCP actor identity policy.",
        ),
    ];
    for card in services.hive.panel()? {
        resources.push(resource_spec(
            &format!("entrance://issues/{}", card.issue.id),
            &format!("Issue #{}", card.issue.id),
            "One local issue card.",
        ));
        resources.push(resource_spec(
            &format!("entrance://issues/{}/control", card.issue.id),
            &format!("Issue #{} control", card.issue.id),
            "One local issue control packet.",
        ));
        resources.push(resource_spec(
            &format!("entrance://issues/{}/timeline", card.issue.id),
            &format!("Issue #{} timeline", card.issue.id),
            "One local issue timeline.",
        ));
        resources.push(resource_spec(
            &format!("entrance://issues/{}/transition-policy", card.issue.id),
            &format!("Issue #{} transition policy", card.issue.id),
            "One local issue transition policy.",
        ));
        if let Some(loop_id) = card.issue.loop_id {
            resources.push(resource_spec(
                &format!("entrance://loops/{loop_id}/control"),
                &format!("Loop #{loop_id} control"),
                "One Reviewer-ready loop control packet.",
            ));
            resources.push(resource_spec(
                &format!("entrance://loops/{loop_id}/dashboard"),
                &format!("Loop #{loop_id} dashboard"),
                "One local loop dashboard.",
            ));
            resources.push(resource_spec(
                &format!("entrance://loops/{loop_id}/evidence-drilldown"),
                &format!("Loop #{loop_id} evidence drilldown"),
                "One local loop evidence drilldown.",
            ));
            resources.push(resource_spec(
                &format!("entrance://loops/{loop_id}/evidence-manifest"),
                &format!("Loop #{loop_id} evidence manifest"),
                "One local loop evidence manifest.",
            ));
            resources.push(resource_spec(
                &format!("entrance://loops/{loop_id}/runtime-preflight"),
                &format!("Loop #{loop_id} runtime preflight"),
                "One local loop runtime preflight.",
            ));
            resources.push(resource_spec(
                &format!("entrance://loops/{loop_id}/worker-lifecycle"),
                &format!("Loop #{loop_id} worker lifecycle"),
                "One local loop worker lifecycle.",
            ));
        }
    }
    Ok(serde_json::json!({ "resources": resources }))
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
        "entrance://issues" => serde_json::to_value(services.hive.panel()?)?,
        "entrance://review-queue" => review_queue(services)?,
        "entrance://policy/issue-transitions" => {
            serde_json::to_value(services.hive.policy_registry().issue_transitions)?
        }
        "entrance://policy/mcp-permissions" => mcp_permission_policy(),
        "entrance://policy/actor-identity" => mcp_actor_identity_policy(),
        value if value.starts_with("entrance://issues/") && value.ends_with("/control") => {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .trim_end_matches("/control")
                .parse::<i64>()?;
            issue_control_packet(services, issue_id)?
        }
        value if value.starts_with("entrance://issues/") && value.ends_with("/timeline") => {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .trim_end_matches("/timeline")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.issue_timeline(issue_id)?)?
        }
        value
            if value.starts_with("entrance://issues/") && value.ends_with("/transition-policy") =>
        {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .trim_end_matches("/transition-policy")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.issue_transition_policy(issue_id)?)?
        }
        value if value.starts_with("entrance://issues/") => {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.issue_report(issue_id)?)?
        }
        value if value.starts_with("entrance://loops/") && value.ends_with("/dashboard") => {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/dashboard")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.loop_dashboard(loop_id)?)?
        }
        value
            if value.starts_with("entrance://loops/") && value.ends_with("/evidence-drilldown") =>
        {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/evidence-drilldown")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.loop_evidence_drilldown(loop_id)?)?
        }
        value
            if value.starts_with("entrance://loops/") && value.ends_with("/evidence-manifest") =>
        {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/evidence-manifest")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.loop_evidence_manifest(loop_id)?)?
        }
        value
            if value.starts_with("entrance://loops/") && value.ends_with("/runtime-preflight") =>
        {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/runtime-preflight")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.loop_runtime_preflight(loop_id)?)?
        }
        value if value.starts_with("entrance://loops/") && value.ends_with("/worker-lifecycle") => {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/worker-lifecycle")
                .parse::<i64>()?;
            serde_json::to_value(services.hive.loop_worker_lifecycle(loop_id)?)?
        }
        value if value.starts_with("entrance://loops/") && value.ends_with("/control") => {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/control")
                .parse::<i64>()?;
            loop_control_packet(services, loop_id)?
        }
        _ => anyhow::bail!("unknown resource URI `{uri}`"),
    };
    Ok(serde_json::json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": serde_json::to_string_pretty(&value)?
        }]
    }))
}

pub fn loop_control_packet(services: &AppServices, loop_id: i64) -> Result<serde_json::Value> {
    let dashboard = services.hive.loop_dashboard(loop_id)?;
    let evidence = services.hive.loop_evidence_drilldown(loop_id)?;
    let manifest = services.hive.loop_evidence_manifest(loop_id)?;
    let preflight = services.hive.loop_runtime_preflight(loop_id)?;
    let lifecycle = services.hive.loop_worker_lifecycle(loop_id)?;
    let report = services.hive.loop_report(loop_id)?;
    Ok(serde_json::json!({
        "schema_version": MCP_LOOP_CONTROL_SCHEMA_VERSION,
        "loop": {
            "id": report.contract.id,
            "title": report.contract.title,
            "status": report.contract.status,
            "active_phase": report.contract.active_phase,
            "current_round": report.contract.current_round,
            "runtime": report.contract.runtime
        },
        "issue": report.issues.first().map(compact_issue_card),
        "dashboard": dashboard,
        "evidence_drilldown": evidence,
        "evidence_manifest": manifest,
        "runtime_preflight": preflight,
        "worker_lifecycle": lifecycle,
        "resources": {
            "review_queue": "entrance://review-queue",
            "policy": "entrance://policy/issue-transitions"
        }
    }))
}

pub fn issue_control_packet(services: &AppServices, issue_id: i64) -> Result<serde_json::Value> {
    let card = services.hive.issue_report(issue_id)?;
    let transition = services.hive.issue_transition_policy(issue_id).ok();
    let timeline = services.hive.issue_timeline(issue_id).ok();
    let loop_control = card
        .issue
        .loop_id
        .and_then(|loop_id| loop_control_packet(services, loop_id).ok());
    Ok(serde_json::json!({
        "schema_version": MCP_ISSUE_CONTROL_SCHEMA_VERSION,
        "issue": compact_issue_detail(&card),
        "transition_policy": transition,
        "timeline": timeline,
        "loop_control": loop_control,
        "advance_next_action": issue_advance_next_action(&card),
        "permissions": mcp_permission_policy(),
        "actor_identity": mcp_actor_identity_policy(),
        "resources": {
            "issue": format!("entrance://issues/{issue_id}"),
            "issue_control": format!("entrance://issues/{issue_id}/control"),
            "issue_timeline": format!("entrance://issues/{issue_id}/timeline"),
            "transition_policy": format!("entrance://issues/{issue_id}/transition-policy"),
            "review_queue": "entrance://review-queue",
            "policy": "entrance://policy/issue-transitions",
            "permissions": "entrance://policy/mcp-permissions"
        }
    }))
}

fn review_queue(services: &AppServices) -> Result<serde_json::Value> {
    let issues = services
        .hive
        .panel()?
        .iter()
        .filter(|card| matches!(card.issue.status.as_str(), "Blocked" | "Needs Review"))
        .map(|card| {
            let mut issue = compact_issue_card(card);
            issue["advance_next_action"] = serde_json::json!(issue_advance_next_action(card));
            issue
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "schema_version": "entrance.mcp.review_queue.v1",
        "count": issues.len(),
        "issues": issues
    }))
}

fn compact_issue_detail(card: &IssueCard) -> serde_json::Value {
    serde_json::json!({
        "issue": compact_issue_card(card),
        "comments": card.comments,
        "actions": card.actions,
        "trace": card.trace,
        "doctor": card.doctor
    })
}

fn compact_issue_card(card: &IssueCard) -> serde_json::Value {
    serde_json::json!({
        "id": card.issue.id,
        "loop_id": card.issue.loop_id,
        "title": card.issue.title,
        "status": card.issue.status,
        "summary": card.issue.summary,
        "assignee": card.issue.assignee,
        "claim_role": card.issue.claim_role,
        "claim_source": card.issue.claim_source,
        "claimed_at": card.issue.claimed_at,
        "round": card.trace.as_ref().map(|trace| trace.current_round),
        "last_decision": card.trace.as_ref().and_then(|trace| trace.last_decision.clone()),
        "evidence_count": card.trace.as_ref().map(|trace| trace.evidence_count).unwrap_or_default(),
        "comment_count": card.comments.len()
    })
}

fn mcp_permission_policy() -> serde_json::Value {
    serde_json::json!({
        "schema_version": MCP_PERMISSION_POLICY_SCHEMA_VERSION,
        "registry_schema_version": MCP_TOOL_PERMISSION_REGISTRY_SCHEMA_VERSION,
        "tools": MCP_TOOL_NAMES.iter().map(|name| {
            serde_json::json!({
                "name": name,
                "permission": mcp_tool_permission(name)
            })
        }).collect::<Vec<_>>()
    })
}

fn mcp_tool_permission(name: &str) -> serde_json::Value {
    let human_confirmed = matches!(name, "entrance_issue_retry" | "entrance_issue_decide");
    serde_json::json!({
        "schema_version": MCP_TOOL_PERMISSION_SCHEMA_VERSION,
        "local_only": true,
        "remote_sync": false,
        "human_confirmed_required": human_confirmed,
        "confirmation_arg": if human_confirmed { Some("human_confirmed") } else { None::<&str> },
        "destructive": matches!(name, "entrance_issue_decide"),
        "read_only": matches!(name, "entrance_issue_list" | "entrance_issue_show" | "entrance_issue_control" | "entrance_loop_control" | "entrance_review_queue")
    })
}

fn mcp_actor_identity_policy() -> serde_json::Value {
    serde_json::json!({
        "schema_version": MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION,
        "verified": false,
        "summary": "MCP actors are self-reported local audit context, not authenticated operator identity."
    })
}

fn mcp_human_confirmation_receipt(
    action: &str,
    author: &str,
    human_confirmed: bool,
    client: Option<&McpClientIdentity>,
) -> Option<OperatorConfirmationReceipt> {
    human_confirmed.then(|| OperatorConfirmationReceipt {
        schema_version: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
        source: "mcp".to_string(),
        policy_schema_version: OPERATOR_ACTION_POLICY_SCHEMA_VERSION.to_string(),
        confirmation_arg: OPERATOR_ACTION_CONFIRMATION_ARG.to_string(),
        human_confirmed: true,
        action: action.to_string(),
        author: author.to_string(),
        marker: format!(
            "MCP confirmation: human_confirmed=true; action={action}; author={author}; policy={OPERATOR_ACTION_POLICY_SCHEMA_VERSION}"
        ),
        client: client.map(|client| OperatorConfirmationClient {
            name: client.name.clone(),
            version: client.version.clone(),
            source: "mcp.initialize.clientInfo".to_string(),
        }),
        actor: Some(OperatorConfirmationActor {
            id: author.to_string(),
            label: author.to_string(),
            source: "mcp".to_string(),
            trust: "self-reported".to_string(),
            verified: false,
        }),
    })
}

fn human_confirmed(args: &serde_json::Value) -> bool {
    args.get("human_confirmed")
        .or_else(|| args.get("humanConfirmed"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn required_i64(args: &serde_json::Value, name: &str) -> Result<i64> {
    args.get(name)
        .and_then(|value| value.as_i64())
        .with_context(|| format!("requires integer argument `{name}`"))
}

fn required_string(args: &serde_json::Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .with_context(|| format!("requires string argument `{name}`"))
}

fn optional_string(args: &serde_json::Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn tool_text(value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
        }],
        "structuredContent": value
    })
}

fn success_response(id: serde_json::Value, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
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

fn json_rpc_error(code: i64, message: &str, data: Option<serde_json::Value>) -> JsonRpcError {
    JsonRpcError {
        code,
        message: message.to_string(),
        data,
    }
}
