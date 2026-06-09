use anyhow::{Context, Result};
use entrance_hive::{
    HiveLoopCreateRequest, IssueCard, IssueCommentRequest, IssueDecisionRequest, IssueRunRequest,
    OperatorConfirmationActor, OperatorConfirmationClient, OperatorConfirmationReceipt,
    OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    app::AppServices,
    commands::hive::{
        compact_issue_connector_control, connector_publish_plan_report, connector_queue_report,
        connector_roundtrip_plan_report, execute_connector_publish_plan_with_confirmation,
        execute_connector_roundtrip_plan_with_confirmation,
    },
};

const MCP_LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const MCP_FALLBACK_PROTOCOL_VERSION: &str = "2025-06-18";
const MCP_SCHEMA_VERSION: &str = "entrance.mcp.v1";
const MCP_PERMISSION_POLICY_SCHEMA_VERSION: &str = "entrance.mcp.permission_policy.v1";
const MCP_TOOL_PERMISSION_SCHEMA_VERSION: &str = "entrance.mcp.tool_permission.v1";
const MCP_TOOL_PERMISSION_REGISTRY_SCHEMA_VERSION: &str =
    "entrance.mcp.tool_permission_registry.v1";
const MCP_ISSUE_CONTROL_SCHEMA_VERSION: &str = "entrance.mcp.issue_control.v1";
const MCP_LOOP_CONTROL_SCHEMA_VERSION: &str = "entrance.mcp.loop_control.v1";
const MCP_CONNECTOR_CONTROL_SCHEMA_VERSION: &str = "entrance.mcp.connector_control.v1";
const MCP_WORKER_LIFECYCLE_SUMMARY_SCHEMA_VERSION: &str =
    "entrance.mcp.worker_lifecycle_summary.v1";
const MCP_RUNTIME_PREFLIGHT_SUMMARY_SCHEMA_VERSION: &str =
    "entrance.mcp.runtime_preflight_summary.v1";
const MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION: &str = "entrance.mcp.actor_identity_policy.v1";
const MCP_TOOL_NAMES: &[&str] = &[
    "entrance_issue_list",
    "entrance_issue_show",
    "entrance_issue_control",
    "entrance_loop_control",
    "entrance_connector_queue",
    "entrance_connector_control",
    "entrance_connector_publish_plan",
    "entrance_connector_publish_execute",
    "entrance_connector_roundtrip_plan",
    "entrance_connector_roundtrip_execute",
    "entrance_review_queue",
    "entrance_issue_comment",
    "entrance_loop_create",
    "entrance_issue_run",
    "entrance_issue_retry",
    "entrance_issue_decide",
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

enum RequestOutcome {
    Result(serde_json::Value),
    Error(JsonRpcError),
    Notification,
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

    match handle_request(services, session, request) {
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
        "tools/list" => Ok(RequestOutcome::Result(serde_json::json!({
            "tools": tool_specs()
        }))),
        "tools/call" => Ok(RequestOutcome::Result(call_tool(
            services,
            session,
            &request.params,
        ))),
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
            "entrance_loop_review",
            "Review an Entrance loop",
            "Read one loop control packet and prepare a Reviewer verdict or human decision options from gates, score vector, evidence, and fallback budget.",
            vec![prompt_arg("loop_id", "Entrance loop id.", true)],
        ),
        prompt_spec(
            "entrance_blocker_decision",
            "Prepare a blocked issue decision",
            "Summarize a Blocked or Needs Review issue into human options before retry/review/cancel.",
            vec![prompt_arg("issue_id", "Entrance issue id.", true)],
        ),
        prompt_spec(
            "entrance_connector_decision",
            "Prepare a connector queue decision",
            "Summarize connector queue state and digest-bound plans into human options before publish/roundtrip execution.",
            vec![prompt_arg(
                "provider",
                "Optional provider filter such as remote-fixture, github, linear, or file.",
                false,
            )],
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
        "entrance_loop_review" => prompt_loop_review(services, &args),
        "entrance_blocker_decision" => prompt_blocker_decision(services, &args),
        "entrance_connector_decision" => prompt_connector_decision(services, &args),
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

fn prompt_loop_review(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let loop_id = integer_arg(args, "loop_id")?;
    let loop_resource = loop_control_prompt_resource(services, loop_id)?;
    let text = format!(
        "You are the Entrance Reviewer for loop #{loop_id}. Read entrance://loops/{loop_id}/control, entrance://policy/mcp-permissions, and the attached loop control packet before judging.\n\nRules:\n1. Do not implement, edit files, or advance the Developer lane from this prompt.\n2. Judge only from recorded gate results, score vector, evidence receipts, runtime preflight, worker lifecycle, and target drift signals in the packet.\n3. Produce one recommendation: keep, reject, needs-review, or blocked. Include failed gates, missing receipts, evidence links, and the smallest next action.\n4. If the Reviewer invalid budget is exhausted after 3 rounds and the candidate is still invalid, treat Blocked as the correct fallback and present human options instead of retrying automatically.\n5. Do not set human_confirmed=true unless a human explicitly chooses an option."
    );
    Ok(prompt_result(
        "Review one Entrance loop through the transparent Reviewer contract.",
        vec![prompt_text_message("user", text), loop_resource],
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

fn prompt_connector_decision(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let provider = optional_string_arg(args, "provider");
    let connector_resource = connector_control_prompt_resource(services, provider.as_deref())?;
    let resource_uri = connector_control_resource_uri(provider.as_deref());
    let provider_text = provider.as_deref().unwrap_or("all providers");
    let text = format!(
        "You are preparing a human decision for Entrance connector queue ({provider_text}). Read {resource_uri}, entrance://policy/mcp-permissions, and the attached connector control packet, then do not choose for the human.\n\nPresent:\nA. execute the current publish plan when it is executable and the human wants to push local issue/status/comment state outward;\nB. execute the current roundtrip plan when it is executable and the human wants publish + readback + admission evidence;\nC. wait/block when provider setup, remote target, write plan, readback, admission, or policy evidence is not ready.\n\nInclude current queue counts, plan_id values, can_execute flags, blockers, affected issue ids, and the exact Entrance tool call to execute only after the human chooses. Add human_confirmed=true only after the human has chosen that option, and use the exact current plan_id from the control packet."
    );
    Ok(prompt_result(
        "Prepare human connector publish/roundtrip options.",
        vec![prompt_text_message("user", text), connector_resource],
    ))
}

fn loop_control_prompt_resource(services: &AppServices, loop_id: i64) -> Result<serde_json::Value> {
    let uri = loop_control_resource_uri(loop_id);
    let report = loop_control_packet(services, loop_id)?;
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

fn connector_control_prompt_resource(
    services: &AppServices,
    provider_filter: Option<&str>,
) -> Result<serde_json::Value> {
    let uri = connector_control_resource_uri(provider_filter);
    let report = connector_control_packet(services, provider_filter)?;
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

fn issue_prompt_resource(services: &AppServices, issue_id: i64) -> Result<serde_json::Value> {
    let uri = format!("entrance://issues/{issue_id}/control");
    let card = services.hive.issue_report(issue_id)?;
    let report = issue_control_packet_for_services(services, &card);
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
            "entrance_issue_control",
            "Read an Entrance issue control packet",
            "Read one issue as a Linear-like control packet with status, actions, blockers, evidence, receipts, and human decision boundaries.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer", "description": "Issue id." }
                },
                "required": ["issue_id"]
            }),
        ),
        tool_spec(
            "entrance_loop_control",
            "Read an Entrance loop control packet",
            "Read one loop as a Reviewer-ready control packet with dashboard, evidence, runtime preflight, worker lifecycle, gates, fallback budget, and human decision boundaries.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "loop_id": { "type": "integer", "description": "Loop id." }
                },
                "required": ["loop_id"]
            }),
        ),
        tool_spec(
            "entrance_connector_queue",
            "List Entrance connector queue",
            "Read the external issue/status/comment connector queue with publish requirements, remote write plans, status mappings, blockers, and operator decision surfaces.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Optional provider filter such as local-hive-panel, file, remote-fixture, github, or linear."
                    }
                }
            }),
        ),
        tool_spec(
            "entrance_connector_control",
            "Read connector control packet",
            "Read a Linear-like connector control packet with queue state, digest-bound plans, blockers, human options, resources, and confirmation boundaries.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Optional provider filter such as local-hive-panel, file, remote-fixture, github, or linear."
                    }
                }
            }),
        ),
        tool_spec(
            "entrance_connector_publish_plan",
            "Read connector publish plan",
            "Read the digest-bound connector publish plan for all providers or one provider before executing any external issue/status/comment write.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Optional provider filter such as local-hive-panel, file, remote-fixture, github, or linear."
                    }
                }
            }),
        ),
        tool_spec(
            "entrance_connector_publish_execute",
            "Execute connector publish plan",
            "Execute a digest-bound connector publish plan after human confirmation; refuses stale plan ids and non-executable plans.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {
                        "type": "string",
                        "description": "The current plan_id from entrance_connector_publish_plan or entrance://connectors/publish-plan."
                    },
                    "provider": {
                        "type": "string",
                        "description": "Optional provider filter; must match the plan that produced plan_id."
                    },
                    "human_confirmed": {
                        "type": "boolean",
                        "description": "Must be true because connector writes are a human decision boundary."
                    },
                    "author": {
                        "type": "string",
                        "description": "Optional author label. Defaults to mcp-agent."
                    }
                },
                "required": ["plan_id", "human_confirmed"]
            }),
        ),
        tool_spec(
            "entrance_connector_roundtrip_plan",
            "Read connector roundtrip plan",
            "Read the digest-bound connector publish/readback/admission roundtrip plan for all providers or one provider.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Optional provider filter such as local-hive-panel, file, remote-fixture, github, or linear."
                    }
                }
            }),
        ),
        tool_spec(
            "entrance_connector_roundtrip_execute",
            "Execute connector roundtrip plan",
            "Execute a digest-bound connector roundtrip plan after human confirmation; refuses stale plan ids and non-executable plans.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "plan_id": {
                        "type": "string",
                        "description": "The current plan_id from entrance_connector_roundtrip_plan or entrance://connectors/roundtrip-plan."
                    },
                    "provider": {
                        "type": "string",
                        "description": "Optional provider filter; must match the plan that produced plan_id."
                    },
                    "human_confirmed": {
                        "type": "boolean",
                        "description": "Must be true because connector roundtrips can write comments/evidence and external mirrors."
                    },
                    "author": {
                        "type": "string",
                        "description": "Optional author label. Defaults to mcp-agent."
                    }
                },
                "required": ["plan_id", "human_confirmed"]
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
                        "description": "Must be true because retry is a human decision boundary; Entrance records the confirmation in the operator decision note."
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
                        "description": "Must be true because retry/review/cancel are human decision boundaries; Entrance records the confirmation in the operator decision note."
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
        "inputSchema": input_schema,
        "annotations": {
            "entrance_permission": mcp_tool_permission(name)
        }
    })
}

fn call_tool(
    services: &AppServices,
    session: &McpSession,
    params: &serde_json::Value,
) -> serde_json::Value {
    let name = params.get("name").and_then(|value| value.as_str());
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let result = match name {
        Some("entrance_issue_list") => tool_issue_list(services, &args),
        Some("entrance_issue_show") => tool_issue_show(services, &args),
        Some("entrance_issue_control") => tool_issue_control(services, &args),
        Some("entrance_loop_control") => tool_loop_control(services, &args),
        Some("entrance_connector_queue") => tool_connector_queue(services, &args),
        Some("entrance_connector_control") => tool_connector_control(services, &args),
        Some("entrance_connector_publish_plan") => tool_connector_publish_plan(services, &args),
        Some("entrance_connector_publish_execute") => {
            tool_connector_publish_execute(services, session, &args)
        }
        Some("entrance_connector_roundtrip_plan") => tool_connector_roundtrip_plan(services, &args),
        Some("entrance_connector_roundtrip_execute") => {
            tool_connector_roundtrip_execute(services, session, &args)
        }
        Some("entrance_review_queue") => tool_review_queue(services, &args),
        Some("entrance_issue_comment") => tool_issue_comment(services, &args),
        Some("entrance_loop_create") => tool_loop_create(services, &args),
        Some("entrance_issue_run") => tool_issue_run(services, session, &args, false),
        Some("entrance_issue_retry") => tool_issue_run(services, session, &args, true),
        Some("entrance_issue_decide") => tool_issue_decide(services, session, &args),
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

fn tool_issue_control(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let card = services.hive.issue_report(issue_id)?;
    Ok(issue_control_packet_for_services(services, &card))
}

fn tool_loop_control(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let loop_id = integer_arg(args, "loop_id")?;
    loop_control_packet(services, loop_id)
}

fn tool_connector_queue(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let provider = optional_string_arg(args, "provider");
    connector_queue_report(services, provider.as_deref())
}

fn tool_connector_control(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let provider = optional_string_arg(args, "provider");
    connector_control_packet(services, provider.as_deref())
}

fn tool_connector_publish_plan(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let provider = optional_string_arg(args, "provider");
    connector_publish_plan_report(services, provider.as_deref())
}

fn tool_connector_publish_execute(
    services: &AppServices,
    session: &McpSession,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let plan_id = string_arg(args, "plan_id")?;
    ensure_connector_execute_confirmed(args, "connector_publish_execute")?;
    let author = mcp_author(args);
    let provider = optional_string_arg(args, "provider");
    execute_connector_publish_plan_with_confirmation(
        services,
        provider.as_deref(),
        &plan_id,
        Some(mcp_human_confirmation_receipt(
            "connector_publish_execute",
            &author,
            session,
        )),
    )
}

fn tool_connector_roundtrip_plan(
    services: &AppServices,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let provider = optional_string_arg(args, "provider");
    connector_roundtrip_plan_report(services, provider.as_deref())
}

fn tool_connector_roundtrip_execute(
    services: &AppServices,
    session: &McpSession,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let plan_id = string_arg(args, "plan_id")?;
    ensure_connector_execute_confirmed(args, "connector_roundtrip_execute")?;
    let author = mcp_author(args);
    let provider = optional_string_arg(args, "provider");
    execute_connector_roundtrip_plan_with_confirmation(
        services,
        provider.as_deref(),
        &plan_id,
        Some(mcp_human_confirmation_receipt(
            "connector_roundtrip_execute",
            &author,
            session,
        )),
    )
}

pub(crate) fn loop_control_packet(
    services: &AppServices,
    loop_id: i64,
) -> Result<serde_json::Value> {
    let dashboard = serde_json::to_value(services.hive.loop_dashboard(loop_id)?)?;
    let evidence_drilldown = serde_json::to_value(services.hive.loop_evidence_drilldown(loop_id)?)?;
    let evidence_manifest = serde_json::to_value(services.hive.loop_evidence_manifest(loop_id)?)?;
    let runtime_preflight = serde_json::to_value(services.hive.loop_runtime_preflight(loop_id)?)?;
    let worker_lifecycle = serde_json::to_value(services.hive.loop_worker_lifecycle(loop_id)?)?;
    Ok(loop_control_packet_from_reports(
        loop_id,
        dashboard,
        evidence_drilldown,
        evidence_manifest,
        runtime_preflight,
        worker_lifecycle,
    ))
}

fn loop_control_packet_from_reports(
    loop_id: i64,
    dashboard: serde_json::Value,
    evidence_drilldown: serde_json::Value,
    evidence_manifest: serde_json::Value,
    runtime_preflight: serde_json::Value,
    worker_lifecycle: serde_json::Value,
) -> serde_json::Value {
    let issue_id = json_i64(
        &dashboard,
        &[
            "/issue/id",
            "/issue_id",
            "/state/issue_id",
            "/resources/issue_id",
        ],
    )
    .or_else(|| json_i64(&worker_lifecycle, &["/issue_id"]));
    let issue_status = json_string(
        &dashboard,
        &[
            "/issue/status",
            "/human_decision/issue_status",
            "/issue_status",
        ],
    )
    .or_else(|| json_string(&worker_lifecycle, &["/issue_status"]));
    let current_round = json_i64(&dashboard, &["/current_round"])
        .or_else(|| json_i64(&worker_lifecycle, &["/current_round"]));
    let reviewer_decision = json_string(
        &dashboard,
        &[
            "/reviewer/decision",
            "/rounds/0/decision",
            "/human_decision/decision",
        ],
    )
    .or_else(|| json_string(&worker_lifecycle, &["/current/decision"]));
    let reviewer_reason_code = json_string(&dashboard, &["/reviewer/reason_code"])
        .or_else(|| json_string(&dashboard, &["/rounds/0/reason_code"]));
    let reviewer_invalid_rounds_used =
        json_i64(&dashboard, &["/reviewer/reviewer_invalid_rounds_used"]).or_else(|| {
            json_i64(
                &worker_lifecycle,
                &["/current/reviewer_invalid_rounds_used"],
            )
        });
    let reviewer_invalid_round_budget =
        json_i64(&dashboard, &["/reviewer/reviewer_invalid_round_budget"]).or_else(|| {
            json_i64(
                &worker_lifecycle,
                &["/policy/reviewer_invalid_round_budget"],
            )
        });
    let reviewer_invalid_budget_exhausted =
        json_bool(&dashboard, &["/reviewer/reviewer_invalid_budget_exhausted"])
            .or_else(|| {
                json_bool(
                    &worker_lifecycle,
                    &["/current/reviewer_invalid_budget_exhausted"],
                )
            })
            .unwrap_or(false);
    let needs_human_decision = json_bool(&dashboard, &["/human_decision/required"])
        .unwrap_or_else(|| matches!(issue_status.as_deref(), Some("Blocked" | "Needs Review")));
    let primary_action = loop_control_primary_action(
        issue_status.as_deref(),
        needs_human_decision,
        reviewer_invalid_budget_exhausted,
    );
    let dashboard_resource = format!("entrance://loops/{loop_id}/dashboard");
    let evidence_drilldown_resource = format!("entrance://loops/{loop_id}/evidence-drilldown");
    let evidence_manifest_resource = format!("entrance://loops/{loop_id}/evidence-manifest");
    let runtime_preflight_resource = format!("entrance://loops/{loop_id}/runtime-preflight");
    let worker_lifecycle_resource = format!("entrance://loops/{loop_id}/worker-lifecycle");
    let issue_resource = issue_id.map(|id| format!("entrance://issues/{id}"));
    let issue_control_resource = issue_id.map(|id| format!("entrance://issues/{id}/control"));
    let transition_policy_resource =
        issue_id.map(|id| format!("entrance://issues/{id}/transition-policy"));
    let timeline_resource = issue_id.map(|id| format!("entrance://issues/{id}/timeline"));

    serde_json::json!({
        "schema_version": MCP_LOOP_CONTROL_SCHEMA_VERSION,
        "loop_id": loop_id,
        "state": {
            "issue_id": issue_id,
            "issue_status": issue_status,
            "loop_status": json_string(&dashboard, &["/status"]).or_else(|| json_string(&worker_lifecycle, &["/status"])),
            "active_phase": json_string(&dashboard, &["/active_phase"]).or_else(|| json_string(&worker_lifecycle, &["/active_phase"])),
            "current_round": current_round,
            "dashboard_state": json_string(&dashboard, &["/dashboard_state"]),
            "lifecycle_state": json_string(&worker_lifecycle, &["/lifecycle_state"]),
            "runtime_preflight_state": json_string(&runtime_preflight, &["/preflight_state"]),
            "evidence_manifest_state": json_string(&evidence_manifest, &["/manifest_state"]),
            "reviewer_decision": reviewer_decision,
            "reviewer_reason_code": reviewer_reason_code,
            "reviewer_invalid_rounds_used": reviewer_invalid_rounds_used,
            "reviewer_invalid_round_budget": reviewer_invalid_round_budget,
            "reviewer_invalid_budget_exhausted": reviewer_invalid_budget_exhausted,
            "fallback_status": json_string(&dashboard, &["/reviewer/fallback_status"]).or_else(|| json_string(&worker_lifecycle, &["/policy/fallback_status"])),
            "needs_human_decision": needs_human_decision,
            "primary_action": primary_action
        },
        "reviewer_gate_surface": {
            "role": "Reviewer",
            "allowed_decisions": ["keep", "reject", "needs-review", "blocked"],
            "gates": {
                "runtime_preflight": {
                    "resource": runtime_preflight_resource,
                    "state": json_string(&runtime_preflight, &["/preflight_state"]),
                    "gate": json_string(&runtime_preflight, &["/policy/gate", "/current/gate"]),
                    "passed": json_bool(&runtime_preflight, &["/current/gate_passed"])
                },
                "worker_lifecycle": {
                    "resource": worker_lifecycle_resource,
                    "state": json_string(&worker_lifecycle, &["/lifecycle_state"]),
                    "expected_roles": json_clone(&worker_lifecycle, &["/policy/expected_roles"]),
                    "observed_roles": json_clone(&worker_lifecycle, &["/current/observed_roles"]),
                    "missing_roles": json_clone(&worker_lifecycle, &["/current/missing_roles"]),
                    "failures": json_clone(&worker_lifecycle, &["/failures"])
                },
                "evidence_manifest": {
                    "resource": evidence_manifest_resource,
                    "state": json_string(&evidence_manifest, &["/manifest_state"]),
                    "coverage": json_clone(&evidence_manifest, &["/coverage"])
                }
            },
            "score_vector": json_clone(&dashboard, &["/reviewer/score_vector"]).unwrap_or_else(|| serde_json::json!([])),
            "evidence_links": {
                "dashboard": dashboard_resource,
                "evidence_drilldown": evidence_drilldown_resource,
                "evidence_manifest": evidence_manifest_resource,
                "runtime_preflight": runtime_preflight_resource,
                "worker_lifecycle": worker_lifecycle_resource
            },
            "target_drift_check": {
                "state": "shallow",
                "source": "dashboard.reviewer.reason_code and evidence summaries",
                "note": "Full semantic target-drift scoring is still roadmap work."
            },
            "budget_policy": {
                "invalid_round_budget": reviewer_invalid_round_budget,
                "invalid_rounds_used": reviewer_invalid_rounds_used,
                "exhausted": reviewer_invalid_budget_exhausted,
                "fallback_status": "Blocked"
            }
        },
        "human_decision_boundary": {
            "required": needs_human_decision,
            "issue_status": issue_status,
            "actions": json_clone(&dashboard, &["/human_decision/actions"]).unwrap_or_else(|| serde_json::json!([])),
            "options": json_clone(&dashboard, &["/human_decision/options"]).unwrap_or_else(|| serde_json::json!([])),
            "confirmation_arg": "human_confirmed",
            "policy_resource": "entrance://policy/mcp-permissions",
            "review_queue_resource": "entrance://review-queue",
            "instruction": "Do not set human_confirmed=true until a human chooses retry, request-review, or cancel."
        },
        "operator_decision_surface": {
            "primary_action": primary_action,
            "options": loop_control_decision_options(issue_id, needs_human_decision, reviewer_invalid_budget_exhausted),
            "blocked_fallback": {
                "condition": "Reviewer rejects or keeps finding the candidate invalid after the 3-round invalid budget is exhausted.",
                "status": "Blocked",
                "active": reviewer_invalid_budget_exhausted
            }
        },
        "reports": {
            "dashboard": dashboard,
            "evidence_drilldown": evidence_drilldown,
            "evidence_manifest": evidence_manifest,
            "runtime_preflight": runtime_preflight,
            "worker_lifecycle": worker_lifecycle
        },
        "mcp_policy": {
            "loop_control": mcp_tool_permission("entrance_loop_control"),
            "issue_run": mcp_tool_permission("entrance_issue_run"),
            "issue_retry": mcp_tool_permission("entrance_issue_retry"),
            "issue_decide": mcp_tool_permission("entrance_issue_decide")
        },
        "resources": {
            "loop_control": loop_control_resource_uri(loop_id),
            "dashboard": dashboard_resource,
            "evidence_drilldown": evidence_drilldown_resource,
            "evidence_manifest": evidence_manifest_resource,
            "runtime_preflight": runtime_preflight_resource,
            "worker_lifecycle": worker_lifecycle_resource,
            "issue": issue_resource,
            "issue_control": issue_control_resource,
            "transition_policy": transition_policy_resource,
            "timeline": timeline_resource,
            "review_queue": "entrance://review-queue",
            "permissions": "entrance://policy/mcp-permissions",
            "actor_identity": "entrance://policy/actor-identity"
        }
    })
}

fn loop_control_primary_action(
    issue_status: Option<&str>,
    needs_human_decision: bool,
    reviewer_invalid_budget_exhausted: bool,
) -> &'static str {
    if reviewer_invalid_budget_exhausted || needs_human_decision {
        "human_decision"
    } else if issue_status == Some("Todo") {
        "issue_run"
    } else {
        "inspect"
    }
}

fn loop_control_decision_options(
    issue_id: Option<i64>,
    needs_human_decision: bool,
    reviewer_invalid_budget_exhausted: bool,
) -> serde_json::Value {
    let Some(issue_id) = issue_id else {
        return serde_json::json!([
            {
                "key": "A",
                "label": "inspect loop resources",
                "enabled": true,
                "summary": "No bound issue was found; inspect the loop reports before choosing an issue action."
            }
        ]);
    };
    let human_action_enabled = needs_human_decision || reviewer_invalid_budget_exhausted;
    serde_json::json!([
        {
            "key": "A",
            "label": "retry with changed boundary",
            "enabled": human_action_enabled,
            "summary": "Ask a human to retry only after narrowing the failed assumption or boundary.",
            "tool": "entrance_issue_retry",
            "call": if human_action_enabled {
                serde_json::json!({
                    "name": "entrance_issue_retry",
                    "arguments": {
                        "issue_id": issue_id,
                        "body": "Retry with a narrowed boundary or corrected assumption.",
                        "human_confirmed": true
                    }
                })
            } else {
                serde_json::Value::Null
            }
        },
        {
            "key": "B",
            "label": "request human review",
            "enabled": human_action_enabled,
            "summary": "Use when preference, scope, or external data is needed.",
            "tool": "entrance_issue_decide",
            "call": if human_action_enabled {
                serde_json::json!({
                    "name": "entrance_issue_decide",
                    "arguments": {
                        "issue_id": issue_id,
                        "action": "request-review",
                        "body": "Human review requested by Reviewer.",
                        "human_confirmed": true
                    }
                })
            } else {
                serde_json::Value::Null
            }
        },
        {
            "key": "C",
            "label": "keep blocked",
            "enabled": true,
            "summary": "Do not advance automatically; preserve the Blocked/Needs Review issue surface for a human decision.",
            "resources": {
                "issue_control": format!("entrance://issues/{issue_id}/control"),
                "review_queue": "entrance://review-queue"
            }
        }
    ])
}

fn loop_control_resource_uri(loop_id: i64) -> String {
    format!("entrance://loops/{loop_id}/control")
}

fn json_string(value: &serde_json::Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        value
            .pointer(pointer)
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
    })
}

fn json_i64(value: &serde_json::Value, pointers: &[&str]) -> Option<i64> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(|value| value.as_i64()))
}

fn json_bool(value: &serde_json::Value, pointers: &[&str]) -> Option<bool> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(|value| value.as_bool()))
}

fn json_clone(value: &serde_json::Value, pointers: &[&str]) -> Option<serde_json::Value> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).cloned())
}

fn connector_control_packet(
    services: &AppServices,
    provider_filter: Option<&str>,
) -> Result<serde_json::Value> {
    let queue = connector_queue_report(services, provider_filter)?;
    let publish_plan = connector_publish_plan_report(services, provider_filter)?;
    let roundtrip_plan = connector_roundtrip_plan_report(services, provider_filter)?;
    Ok(connector_control_packet_from_reports(
        provider_filter,
        queue,
        publish_plan,
        roundtrip_plan,
    ))
}

fn connector_control_packet_from_reports(
    provider_filter: Option<&str>,
    queue: serde_json::Value,
    publish_plan: serde_json::Value,
    roundtrip_plan: serde_json::Value,
) -> serde_json::Value {
    let publish_action = connector_execute_action(
        "connector_publish_execute",
        "Publish current issue surface",
        "entrance_connector_publish_execute",
        &publish_plan,
        provider_filter,
    );
    let roundtrip_action = connector_execute_action(
        "connector_roundtrip_execute",
        "Roundtrip current issue surface",
        "entrance_connector_roundtrip_execute",
        &roundtrip_plan,
        provider_filter,
    );
    let actions = [publish_action.clone(), roundtrip_action.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let blocked_actions = [
        connector_blocked_plan_action("connector_publish_execute", &publish_plan),
        connector_blocked_plan_action("connector_roundtrip_execute", &roundtrip_plan),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let decision_options = vec![
        connector_decision_option(
            "A",
            "execute publish plan",
            "Push local issue/status/comment state outward.",
            "entrance_connector_publish_execute",
            &publish_plan,
            publish_action,
            provider_filter,
        ),
        connector_decision_option(
            "B",
            "execute roundtrip plan",
            "Publish, read back, and admit connector evidence.",
            "entrance_connector_roundtrip_execute",
            &roundtrip_plan,
            roundtrip_action,
            provider_filter,
        ),
        serde_json::json!({
            "key": "C",
            "label": "wait or block connector work",
            "enabled": true,
            "reason": "human_or_provider_boundary",
            "summary": "Do not write connector state; inspect blockers, provider setup, remote targets, or policy evidence first.",
            "resources": {
                "connector_control": connector_control_resource_uri(provider_filter),
                "connector_queue": connector_queue_resource_uri(provider_filter),
                "policy": "entrance://policy/mcp-permissions"
            }
        }),
    ];
    let primary_action = if connector_plan_can_execute(&roundtrip_plan) {
        "connector_roundtrip_execute"
    } else if connector_plan_can_execute(&publish_plan) {
        "connector_publish_execute"
    } else {
        "wait_or_block"
    };

    serde_json::json!({
        "schema_version": MCP_CONNECTOR_CONTROL_SCHEMA_VERSION,
        "provider_filter": provider_filter,
        "state": {
            "provider_known": queue.pointer("/provider_known").and_then(|value| value.as_bool()),
            "total": queue.pointer("/total").and_then(|value| value.as_u64()),
            "current_count": queue.pointer("/current_count").and_then(|value| value.as_u64()),
            "publish_required_count": queue.pointer("/publish_required_count").and_then(|value| value.as_u64()),
            "publish_can_execute": connector_plan_can_execute(&publish_plan),
            "roundtrip_can_execute": connector_plan_can_execute(&roundtrip_plan),
            "needs_human_decision": !actions.is_empty(),
            "primary_action": primary_action
        },
        "queue": queue,
        "plans": {
            "publish": publish_plan,
            "roundtrip": roundtrip_plan
        },
        "human_decision_boundary": {
            "required": !actions.is_empty(),
            "actions": actions,
            "blocked_actions": blocked_actions,
            "confirmation_arg": "human_confirmed",
            "plan_id_arg": "plan_id",
            "policy_resource": "entrance://policy/mcp-permissions",
            "actor_identity_resource": "entrance://policy/actor-identity"
        },
        "operator_decision_surface": {
            "primary_action": primary_action,
            "options": decision_options,
            "instruction": "Do not set human_confirmed=true until a human chooses one executable option."
        },
        "mcp_policy": {
            "publish_execute": mcp_tool_permission("entrance_connector_publish_execute"),
            "roundtrip_execute": mcp_tool_permission("entrance_connector_roundtrip_execute")
        },
        "resources": {
            "connector_control": connector_control_resource_uri(provider_filter),
            "connector_queue": connector_queue_resource_uri(provider_filter),
            "connector_publish_plan": connector_publish_plan_resource_uri(provider_filter),
            "connector_roundtrip_plan": connector_roundtrip_plan_resource_uri(provider_filter),
            "permissions": "entrance://policy/mcp-permissions",
            "actor_identity": "entrance://policy/actor-identity"
        }
    })
}

fn connector_execute_action(
    action: &str,
    label: &str,
    tool: &str,
    plan: &serde_json::Value,
    provider_filter: Option<&str>,
) -> Option<serde_json::Value> {
    if !connector_plan_can_execute(plan) {
        return None;
    }
    let plan_id = connector_plan_id(plan)?;
    let mut arguments = serde_json::json!({
        "plan_id": plan_id,
        "human_confirmed": true
    });
    if let Some(provider) = provider_filter {
        arguments["provider"] = serde_json::json!(provider);
    }
    Some(serde_json::json!({
        "action": action,
        "label": label,
        "human_decision": true,
        "tool": tool,
        "plan_id": plan_id,
        "issue_count": plan.pointer("/issue_count").and_then(|value| value.as_u64()),
        "issue_ids": connector_plan_issue_ids(plan),
        "call": {
            "name": tool,
            "arguments": arguments
        },
        "mcp_permission": mcp_tool_permission(tool)
    }))
}

fn connector_blocked_plan_action(
    action: &str,
    plan: &serde_json::Value,
) -> Option<serde_json::Value> {
    if connector_plan_can_execute(plan) {
        return None;
    }
    Some(serde_json::json!({
        "action": action,
        "enabled": false,
        "plan_id": connector_plan_id(plan),
        "reason": plan.pointer("/reason").and_then(|value| value.as_str()).unwrap_or("plan_not_executable"),
        "blockers": plan.pointer("/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "issue_count": plan.pointer("/issue_count").and_then(|value| value.as_u64()),
        "issue_ids": connector_plan_issue_ids(plan)
    }))
}

fn connector_decision_option(
    key: &str,
    label: &str,
    summary: &str,
    tool: &str,
    plan: &serde_json::Value,
    action: Option<serde_json::Value>,
    provider_filter: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "label": label,
        "enabled": action.is_some(),
        "summary": summary,
        "tool": tool,
        "plan_id": connector_plan_id(plan),
        "reason": plan.pointer("/reason").and_then(|value| value.as_str()).unwrap_or("unknown"),
        "blockers": plan.pointer("/blockers").cloned().unwrap_or_else(|| serde_json::json!([])),
        "issue_count": plan.pointer("/issue_count").and_then(|value| value.as_u64()),
        "issue_ids": connector_plan_issue_ids(plan),
        "call": action.and_then(|action| action.pointer("/call").cloned()),
        "resources": {
            "connector_control": connector_control_resource_uri(provider_filter),
            "connector_queue": connector_queue_resource_uri(provider_filter),
            "plan": if tool == "entrance_connector_publish_execute" {
                connector_publish_plan_resource_uri(provider_filter)
            } else {
                connector_roundtrip_plan_resource_uri(provider_filter)
            }
        }
    })
}

fn connector_plan_can_execute(plan: &serde_json::Value) -> bool {
    plan.pointer("/can_execute")
        .and_then(|value| value.as_bool())
        == Some(true)
}

fn connector_plan_id(plan: &serde_json::Value) -> Option<String> {
    plan.pointer("/plan_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn connector_plan_issue_ids(plan: &serde_json::Value) -> Vec<i64> {
    plan.pointer("/issues")
        .and_then(|value| value.as_array())
        .map(|issues| {
            issues
                .iter()
                .filter_map(|issue| issue.pointer("/id").and_then(|value| value.as_i64()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn connector_control_resource_uri(provider_filter: Option<&str>) -> String {
    connector_resource_uri("entrance://connectors/control", provider_filter)
}

fn connector_queue_resource_uri(provider_filter: Option<&str>) -> String {
    connector_resource_uri("entrance://connectors/queue", provider_filter)
}

fn connector_publish_plan_resource_uri(provider_filter: Option<&str>) -> String {
    connector_resource_uri("entrance://connectors/publish-plan", provider_filter)
}

fn connector_roundtrip_plan_resource_uri(provider_filter: Option<&str>) -> String {
    connector_resource_uri("entrance://connectors/roundtrip-plan", provider_filter)
}

fn connector_resource_uri(base: &str, provider_filter: Option<&str>) -> String {
    provider_filter
        .filter(|provider| !provider.trim().is_empty())
        .map(|provider| format!("{base}/{provider}"))
        .unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
fn issue_control_packet(card: &IssueCard) -> serde_json::Value {
    issue_control_packet_with_connector(card, serde_json::Value::Null)
}

fn issue_control_packet_for_services(
    services: &AppServices,
    card: &IssueCard,
) -> serde_json::Value {
    issue_control_packet_with_connector(card, compact_issue_connector_control(services, card))
}

fn issue_control_packet_with_connector(
    card: &IssueCard,
    connector: serde_json::Value,
) -> serde_json::Value {
    let connector_provider = connector
        .pointer("/provider")
        .and_then(|value| value.as_str())
        .filter(|provider| !provider.trim().is_empty())
        .map(ToOwned::to_owned);
    let connector_provider_queue = connector_provider
        .as_ref()
        .map(|provider| format!("entrance://connectors/queue/{provider}"));
    let connector_provider_publish_plan = connector_provider
        .as_ref()
        .map(|provider| format!("entrance://connectors/publish-plan/{provider}"));
    let connector_provider_roundtrip_plan = connector_provider
        .as_ref()
        .map(|provider| format!("entrance://connectors/roundtrip-plan/{provider}"));
    let trace = card.trace.as_ref();
    let doctor = card.doctor.as_ref();
    let recent_evidence = trace
        .map(|trace| {
            trace
                .evidence
                .iter()
                .rev()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let latest_comment = card.comments.last();
    let operator_receipts = card
        .comments
        .iter()
        .filter_map(operator_confirmation_receipt_summary)
        .collect::<Vec<_>>();
    let action_controls = card
        .actions
        .iter()
        .filter_map(|action| issue_action_control(card.issue.id, action))
        .collect::<Vec<_>>();
    let human_decision_actions = action_controls
        .iter()
        .filter(|action| {
            action
                .get("human_decision")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .cloned()
        .collect::<Vec<_>>();

    serde_json::json!({
        "schema_version": MCP_ISSUE_CONTROL_SCHEMA_VERSION,
        "issue": &card.issue,
        "state": {
            "status": &card.issue.status,
            "loop_id": card.issue.loop_id,
            "current_round": trace.map(|trace| trace.current_round),
            "decision": trace.and_then(|trace| trace.last_decision.as_deref()),
            "reason_code": trace.and_then(|trace| trace.reason_code.as_deref()),
            "terminal": matches!(card.issue.status.as_str(), "Done" | "Canceled"),
            "needs_human_decision": matches!(card.issue.status.as_str(), "Blocked" | "Needs Review")
        },
        "actions": action_controls,
        "human_decision_boundary": {
            "required": !human_decision_actions.is_empty(),
            "actions": human_decision_actions,
            "confirmation_arg": "human_confirmed",
            "policy_resource": "entrance://policy/mcp-permissions",
            "review_queue_resource": "entrance://review-queue"
        },
        "blockers": {
            "failed_checks": doctor.map(|doctor| doctor.failed_checks.clone()).unwrap_or_default(),
            "audit_failure_details": doctor.map(|doctor| doctor.audit_failure_details.clone()).unwrap_or_default(),
            "missing_receipts": doctor.map(|doctor| doctor.missing_receipts.clone()).unwrap_or_default(),
            "worker_failures": doctor.map(|doctor| doctor.worker_failures.clone()).unwrap_or_default()
        },
        "connector": connector,
        "runtime_preflight": issue_runtime_preflight_summary(card),
        "worker_lifecycle": issue_worker_lifecycle_summary(card),
        "doctor": doctor.map(|doctor| serde_json::json!({
            "health": &doctor.health,
            "summary": &doctor.summary,
            "next_actions": &doctor.next_actions
        })),
        "comments": {
            "count": card.comments.len(),
            "latest": latest_comment,
            "operator_confirmation_receipts": operator_receipts
        },
        "evidence": {
            "count": trace.map(|trace| trace.evidence_count).unwrap_or_default(),
            "recent": recent_evidence,
            "operator_events": trace.map(|trace| trace.operator_events.clone()).unwrap_or_default(),
            "last_operator_event": trace.and_then(|trace| trace.last_operator_event.clone())
        },
        "mcp_policy": mcp_issue_policy(&card.actions),
        "actor_identity": mcp_actor_identity_policy(),
        "resources": {
            "issue": format!("entrance://issues/{}", card.issue.id),
            "control": format!("entrance://issues/{}/control", card.issue.id),
            "transition_policy": format!("entrance://issues/{}/transition-policy", card.issue.id),
            "timeline": format!("entrance://issues/{}/timeline", card.issue.id),
            "loop_control": card.issue.loop_id.map(loop_control_resource_uri),
            "loop_dashboard": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/dashboard")),
            "evidence_drilldown": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/evidence-drilldown")),
            "evidence_manifest": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/evidence-manifest")),
            "runtime_preflight": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/runtime-preflight")),
            "worker_lifecycle": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/worker-lifecycle")),
            "review_queue": "entrance://review-queue",
            "connector_queue": "entrance://connectors/queue",
            "connector_provider_queue": connector_provider_queue,
            "connector_publish_plan": "entrance://connectors/publish-plan",
            "connector_provider_publish_plan": connector_provider_publish_plan,
            "connector_roundtrip_plan": "entrance://connectors/roundtrip-plan",
            "connector_provider_roundtrip_plan": connector_provider_roundtrip_plan,
            "permissions": "entrance://policy/mcp-permissions",
            "actor_identity": "entrance://policy/actor-identity"
        }
    })
}

fn issue_worker_lifecycle_summary(card: &IssueCard) -> Option<serde_json::Value> {
    let trace = card.trace.as_ref()?;
    let expected_roles = ["explorer", "developer", "reviewer"];
    let mut observed_roles = trace
        .evidence
        .iter()
        .filter(|evidence| {
            evidence.worker_kind.is_some()
                || evidence.worker_ok.is_some()
                || evidence.worker_attempt_count.is_some()
        })
        .filter_map(|evidence| evidence.stage_role.clone())
        .collect::<Vec<_>>();
    observed_roles.sort();
    observed_roles.dedup();
    let missing_roles = expected_roles
        .iter()
        .filter(|role| {
            !observed_roles
                .iter()
                .any(|observed| observed.as_str() == **role)
        })
        .map(|role| (*role).to_string())
        .collect::<Vec<_>>();
    let reviewer_invalid_round_budget = 3_i64;
    let reviewer_invalid_budget_exhausted = trace.reason_code.as_deref()
        == Some("review_budget_exhausted")
        || (trace.last_decision.as_deref() == Some("blocked")
            && trace.current_round >= reviewer_invalid_round_budget);

    Some(serde_json::json!({
        "schema_version": MCP_WORKER_LIFECYCLE_SUMMARY_SCHEMA_VERSION,
        "resource": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/worker-lifecycle")),
        "loop_id": card.issue.loop_id,
        "current_round": trace.current_round,
        "expected_roles": expected_roles,
        "observed_roles": observed_roles,
        "missing_roles": missing_roles,
        "round_worker_count": trace.round_role_worker_count,
        "round_worker_ok_count": trace.round_role_worker_ok_count,
        "round_worker_timeout_count": trace.round_worker_timeout_count,
        "round_worker_retry_exhausted_count": trace.round_worker_retry_exhausted_count,
        "round_worker_duration_ms": trace.round_worker_duration_ms,
        "reviewer_invalid_round_budget": reviewer_invalid_round_budget,
        "reviewer_invalid_budget_exhausted": reviewer_invalid_budget_exhausted,
        "fallback_status": "Blocked"
    }))
}

fn issue_runtime_preflight_summary(card: &IssueCard) -> Option<serde_json::Value> {
    let trace = card.trace.as_ref()?;
    let preflight_gate = trace.last_admission_gate.as_deref() == Some("runtime_policy_ready");
    let state = if preflight_gate && trace.last_admission_passed == Some(false) {
        "blocked"
    } else if preflight_gate && trace.last_admission_passed == Some(true) {
        "admitted"
    } else if card.issue.status == "Todo" {
        "pending"
    } else {
        "unknown"
    };
    let blocker = trace
        .audit_failure_details
        .iter()
        .find(|detail| detail.starts_with("runtime_policy:"))
        .cloned();

    Some(serde_json::json!({
        "schema_version": MCP_RUNTIME_PREFLIGHT_SUMMARY_SCHEMA_VERSION,
        "resource": card.issue.loop_id.map(|loop_id| format!("entrance://loops/{loop_id}/runtime-preflight")),
        "loop_id": card.issue.loop_id,
        "current_round": trace.current_round,
        "state": state,
        "gate": trace.last_admission_gate,
        "gate_passed": trace.last_admission_passed,
        "reason_code": trace.reason_code,
        "failed_checks": trace.audit_failed_checks,
        "audit_failure_details": trace.audit_failure_details,
        "blocker": blocker,
        "route": {
            "from": "kernel",
            "to": "explorer",
            "object_kind": "PREFLIGHT_PACKET"
        }
    }))
}

fn issue_action_control(
    issue_id: i64,
    action: &entrance_hive::IssueAction,
) -> Option<serde_json::Value> {
    let tool = mcp_tool_for_issue_action(&action.action)?;
    let human_decision = mcp_action_requires_human_confirmation(&action.action);
    Some(serde_json::json!({
        "action": &action.action,
        "label": &action.label,
        "tool": tool,
        "human_decision": human_decision,
        "command": &action.command,
        "issue_action_contract": {
            "schema_version": &action.schema_version,
            "source": &action.source,
            "input": &action.input,
            "destructive": action.destructive,
            "runtime": &action.runtime,
            "confirmation_required": action.confirmation_required,
            "confirmation_arg": &action.confirmation_arg,
            "receipt_schema": &action.receipt_schema,
            "policy_schema_version": &action.policy_schema_version
        },
        "mcp_permission": mcp_tool_permission(tool),
        "call": issue_action_call_template(issue_id, action, tool)
    }))
}

fn issue_action_call_template(
    issue_id: i64,
    action: &entrance_hive::IssueAction,
    tool: &str,
) -> serde_json::Value {
    match action.action.as_str() {
        "run" => serde_json::json!({
            "name": tool,
            "arguments": {
                "issue_id": issue_id,
                "runtime": action.runtime
            }
        }),
        "comment" => serde_json::json!({
            "name": tool,
            "arguments": {
                "issue_id": issue_id,
                "body": "<comment>"
            }
        }),
        "retry" => serde_json::json!({
            "name": tool,
            "arguments": {
                "issue_id": issue_id,
                "body": "<human note>",
                "human_confirmed": true,
                "runtime": action.runtime
            }
        }),
        "request-review" | "cancel" => serde_json::json!({
            "name": tool,
            "arguments": {
                "issue_id": issue_id,
                "action": &action.action,
                "body": "<human note>",
                "human_confirmed": true
            }
        }),
        _ => serde_json::json!({
            "name": tool,
            "arguments": {
                "issue_id": issue_id
            }
        }),
    }
}

fn operator_confirmation_receipt_summary(
    comment: &entrance_core::HiveComment,
) -> Option<serde_json::Value> {
    let receipt = comment.payload.get("confirmation_receipt")?;
    Some(serde_json::json!({
        "comment_id": comment.id,
        "author": &comment.author,
        "action": comment.payload.get("action").cloned().unwrap_or_default(),
        "receipt": receipt
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
    session: &McpSession,
    args: &serde_json::Value,
    retry: bool,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let author = mcp_author(args);
    let body = if retry {
        ensure_human_confirmed(args, "retry")?;
        append_human_confirmation_note(Some(string_arg(args, "body")?), "retry", &author)
    } else {
        optional_string_arg(args, "body")
    };
    let confirmation_receipt = if retry {
        Some(mcp_human_confirmation_receipt("retry", &author, session))
    } else {
        None
    };
    let report = services.hive.issue_run(IssueRunRequest {
        issue_id,
        runtime: optional_string_arg(args, "runtime"),
        decision: optional_string_arg(args, "decision"),
        worker_timeout_secs: optional_u64_arg(args, "worker_timeout_secs"),
        worker_attempts: optional_u64_arg(args, "worker_attempts"),
        retry,
        author,
        body,
        confirmation_receipt,
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
    session: &McpSession,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let issue_id = integer_arg(args, "issue_id")?;
    let action = string_arg(args, "action")?;
    ensure_human_confirmed(args, &action)?;
    let author = mcp_author(args);
    let body = append_human_confirmation_note(optional_string_arg(args, "body"), &action, &author);
    let confirmation_receipt = Some(mcp_human_confirmation_receipt(&action, &author, session));
    let card = services.hive.issue_decide(IssueDecisionRequest {
        issue_id,
        action,
        author,
        body,
        confirmation_receipt,
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
            "entrance://connectors/queue",
            "Entrance connector queue",
            "External issue/status/comment connector queue across providers.",
        ),
        resource_spec(
            "entrance://connectors/control",
            "Entrance connector control",
            "Linear-like connector control packet with queue state, plans, blockers, human options, and confirmation boundaries.",
        ),
        resource_spec(
            "entrance://connectors/publish-plan",
            "Entrance connector publish plan",
            "Digest-bound external issue/status/comment publish plan across providers.",
        ),
        resource_spec(
            "entrance://connectors/roundtrip-plan",
            "Entrance connector roundtrip plan",
            "Digest-bound external issue/status/comment publish, readback, and admission plan across providers.",
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
            "entrance://policy/actor-identity",
            "Entrance actor identity policy",
            "Self-reported MCP and local Panel actor identity bindings.",
        ),
        resource_spec(
            "entrance://schema/status",
            "Entrance schema status",
            "SQLite schema contract and health report.",
        ),
    ];
    for provider in services.hive.connector_registry().providers {
        resources.push(resource_spec(
            &format!("entrance://connectors/queue/{}", provider.name),
            &format!("Connector queue: {}", provider.display_name),
            "External issue/status/comment connector queue filtered to one provider.",
        ));
        resources.push(resource_spec(
            &format!("entrance://connectors/control/{}", provider.name),
            &format!("Connector control: {}", provider.display_name),
            "Connector control packet filtered to one provider.",
        ));
        resources.push(resource_spec(
            &format!("entrance://connectors/publish-plan/{}", provider.name),
            &format!("Connector publish plan: {}", provider.display_name),
            "Digest-bound connector publish plan filtered to one provider.",
        ));
        resources.push(resource_spec(
            &format!("entrance://connectors/roundtrip-plan/{}", provider.name),
            &format!("Connector roundtrip plan: {}", provider.display_name),
            "Digest-bound connector roundtrip plan filtered to one provider.",
        ));
    }
    for card in services.hive.panel()? {
        resources.push(resource_spec(
            &format!("entrance://issues/{}", card.issue.id),
            &format!("Issue #{}: {}", card.issue.id, card.issue.title),
            "One issue card with comments, trace, doctor, and evidence.",
        ));
        resources.push(resource_spec(
            &format!("entrance://issues/{}/control", card.issue.id),
            &format!("Issue #{} control: {}", card.issue.id, card.issue.title),
            "One issue control packet with actions, blockers, receipts, and human decision boundaries.",
        ));
        resources.push(resource_spec(
            &format!("entrance://issues/{}/transition-policy", card.issue.id),
            &format!(
                "Issue #{} transition policy: {}",
                card.issue.id, card.issue.title
            ),
            "One issue status transition policy with allowed actions, blocked actions, confirmation requirements, and Reviewer fallback budget.",
        ));
        resources.push(resource_spec(
            &format!("entrance://issues/{}/timeline", card.issue.id),
            &format!("Issue #{} timeline: {}", card.issue.id, card.issue.title),
            "One issue activity timeline with comments, evidence, verdicts, operator decisions, decision receipts, blockers, linked resources, round groups, and human decision surface.",
        ));
        if let Ok(timeline) = services.hive.issue_timeline(card.issue.id) {
            for item in timeline.items {
                resources.push(resource_spec(
                    &item.permalink,
                    &format!("Issue #{} timeline item {}", card.issue.id, item.id),
                    "One permalinked timeline item with context, neighboring items, round group, and linked evidence or receipt.",
                ));
            }
        }
    }
    for contract in services.hive.loop_list()? {
        resources.push(resource_spec(
            &loop_control_resource_uri(contract.id),
            &format!("Loop #{} control", contract.id),
            "One Reviewer-ready loop control packet with dashboard, evidence, runtime preflight, worker lifecycle, gates, fallback budget, and human decision boundaries.",
        ));
        resources.push(resource_spec(
            &format!("entrance://loops/{}/dashboard", contract.id),
            &format!("Loop #{} dashboard", contract.id),
            "One loop dashboard report with issue state, kernel preflight, agents, reviewer verdict, human decision surface, blockers, and next actions.",
        ));
        resources.push(resource_spec(
            &format!("entrance://loops/{}/evidence-drilldown", contract.id),
            &format!("Loop #{} evidence drilldown", contract.id),
            "One loop evidence drilldown report with worker receipts, transcript excerpts, remote connector receipts, artifacts, payload diffs, blockers, and human decision surface.",
        ));
        resources.push(resource_spec(
            &format!("entrance://loops/{}/evidence-manifest", contract.id),
            &format!("Loop #{} evidence manifest", contract.id),
            "One loop evidence manifest report with payload, worker receipt, transcript, artifact, path verification, digest coverage, and next actions.",
        ));
        resources.push(resource_spec(
            &format!("entrance://loops/{}/runtime-preflight", contract.id),
            &format!("Loop #{} runtime preflight", contract.id),
            "One loop runtime preflight report with runtime policy, probe, admission gate, blocker, and next actions.",
        ));
        resources.push(resource_spec(
            &format!("entrance://loops/{}/worker-lifecycle", contract.id),
            &format!("Loop #{} worker lifecycle", contract.id),
            "One loop worker lifecycle report with expected roles, observed workers, receipts, retries, and fallback budget.",
        ));
    }
    Ok(serde_json::json!({ "resources": resources }))
}

fn resource_templates() -> serde_json::Value {
    serde_json::json!({
        "resourceTemplates": [
            {
                "uriTemplate": "entrance://connectors/queue/{provider}",
                "name": "Entrance connector queue by provider",
                "description": "Read external issue/status/comment connector queue filtered to one provider.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://connectors/control/{provider}",
                "name": "Entrance connector control by provider",
                "description": "Read connector control packet filtered to one provider.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://connectors/publish-plan/{provider}",
                "name": "Entrance connector publish plan by provider",
                "description": "Read digest-bound connector publish plan filtered to one provider.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://connectors/roundtrip-plan/{provider}",
                "name": "Entrance connector roundtrip plan by provider",
                "description": "Read digest-bound connector roundtrip plan filtered to one provider.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}",
                "name": "Entrance issue by id",
                "description": "Read one issue card with comments, trace, doctor, and evidence.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/control",
                "name": "Entrance issue control by id",
                "description": "Read one issue control packet with actions, blockers, receipts, and human decision boundaries.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/transition-policy",
                "name": "Entrance issue transition policy by id",
                "description": "Read one issue status transition policy with allowed actions, blocked actions, confirmation requirements, and Reviewer fallback budget.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/timeline",
                "name": "Entrance issue timeline by id",
                "description": "Read one issue activity timeline with comments, evidence, verdicts, operator decisions, decision receipts, blockers, linked resources, round groups, and human decision surface.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://issues/{issue_id}/timeline/items/{item_id}",
                "name": "Entrance issue timeline item by id",
                "description": "Read one permalinked timeline item with context, neighboring items, round group, and linked evidence or receipt.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/control",
                "name": "Entrance loop control by id",
                "description": "Read a Reviewer-ready loop control packet with dashboard, evidence, runtime preflight, worker lifecycle, gates, fallback budget, and human decision boundaries.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/dashboard",
                "name": "Entrance loop dashboard by id",
                "description": "Read loop dashboard with issue state, kernel preflight, agents, reviewer verdict, human decision surface, blockers, and next actions.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/evidence-drilldown",
                "name": "Entrance loop evidence drilldown by id",
                "description": "Read evidence drilldown with worker receipts, transcript excerpts, remote connector receipts, artifacts, payload diffs, blockers, and human decision surface.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/evidence-manifest",
                "name": "Entrance loop evidence manifest by id",
                "description": "Read evidence manifest with payload, receipt, transcript, artifact, path verification, digest coverage, and next actions.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/runtime-preflight",
                "name": "Entrance loop runtime preflight by id",
                "description": "Read runtime preflight with runtime policy, probe, admission gate, blocker, and next actions.",
                "mimeType": "application/json"
            },
            {
                "uriTemplate": "entrance://loops/{loop_id}/worker-lifecycle",
                "name": "Entrance loop worker lifecycle by id",
                "description": "Read worker lifecycle with expected roles, observed workers, receipts, retries, and fallback budget.",
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
        "entrance://connectors/queue" => connector_queue_report(services, None)?,
        "entrance://connectors/control" => connector_control_packet(services, None)?,
        value if value.starts_with("entrance://connectors/queue/") => {
            let provider = value.trim_start_matches("entrance://connectors/queue/");
            if provider.trim().is_empty() {
                anyhow::bail!("invalid Entrance connector queue resource URI `{value}`");
            }
            connector_queue_report(services, Some(provider))?
        }
        value if value.starts_with("entrance://connectors/control/") => {
            let provider = value.trim_start_matches("entrance://connectors/control/");
            if provider.trim().is_empty() {
                anyhow::bail!("invalid Entrance connector control resource URI `{value}`");
            }
            connector_control_packet(services, Some(provider))?
        }
        "entrance://connectors/publish-plan" => connector_publish_plan_report(services, None)?,
        value if value.starts_with("entrance://connectors/publish-plan/") => {
            let provider = value.trim_start_matches("entrance://connectors/publish-plan/");
            if provider.trim().is_empty() {
                anyhow::bail!("invalid Entrance connector publish plan resource URI `{value}`");
            }
            connector_publish_plan_report(services, Some(provider))?
        }
        "entrance://connectors/roundtrip-plan" => connector_roundtrip_plan_report(services, None)?,
        value if value.starts_with("entrance://connectors/roundtrip-plan/") => {
            let provider = value.trim_start_matches("entrance://connectors/roundtrip-plan/");
            if provider.trim().is_empty() {
                anyhow::bail!("invalid Entrance connector roundtrip plan resource URI `{value}`");
            }
            connector_roundtrip_plan_report(services, Some(provider))?
        }
        "entrance://policy/registry" => serde_json::to_value(services.hive.policy_registry())?,
        "entrance://policy/mcp-permissions" => mcp_permission_policy(),
        "entrance://policy/actor-identity" => mcp_actor_identity_policy(),
        "entrance://schema/status" => serde_json::to_value(services.kernel.store.schema_status()?)?,
        value if value.starts_with("entrance://loops/") && value.ends_with("/control") => {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/control")
                .parse::<i64>()
                .with_context(|| format!("invalid Entrance loop control resource URI `{value}`"))?;
            loop_control_packet(services, loop_id)?
        }
        value if value.starts_with("entrance://loops/") && value.ends_with("/dashboard") => {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/dashboard")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance loop dashboard resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.loop_dashboard(loop_id)?)?
        }
        value
            if value.starts_with("entrance://loops/") && value.ends_with("/evidence-drilldown") =>
        {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/evidence-drilldown")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance loop evidence drilldown resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.loop_evidence_drilldown(loop_id)?)?
        }
        value
            if value.starts_with("entrance://loops/") && value.ends_with("/evidence-manifest") =>
        {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/evidence-manifest")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance loop evidence manifest resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.loop_evidence_manifest(loop_id)?)?
        }
        value
            if value.starts_with("entrance://loops/") && value.ends_with("/runtime-preflight") =>
        {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/runtime-preflight")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance loop runtime preflight resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.loop_runtime_preflight(loop_id)?)?
        }
        value if value.starts_with("entrance://loops/") && value.ends_with("/worker-lifecycle") => {
            let loop_id = value
                .trim_start_matches("entrance://loops/")
                .trim_end_matches("/worker-lifecycle")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance loop worker lifecycle resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.loop_worker_lifecycle(loop_id)?)?
        }
        value if value.starts_with("entrance://issues/") && value.ends_with("/control") => {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .trim_end_matches("/control")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance issue control resource URI `{value}`")
                })?;
            let card = services.hive.issue_report(issue_id)?;
            issue_control_packet_for_services(services, &card)
        }
        value
            if value.starts_with("entrance://issues/") && value.ends_with("/transition-policy") =>
        {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .trim_end_matches("/transition-policy")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance issue transition policy resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.issue_transition_policy(issue_id)?)?
        }
        value if value.starts_with("entrance://issues/") && value.contains("/timeline/items/") => {
            let rest = value.trim_start_matches("entrance://issues/");
            let (issue_id, item_id) = rest
                .split_once("/timeline/items/")
                .with_context(|| format!("invalid Entrance issue timeline item URI `{value}`"))?;
            let issue_id = issue_id
                .parse::<i64>()
                .with_context(|| format!("invalid Entrance issue timeline item URI `{value}`"))?;
            serde_json::to_value(services.hive.issue_timeline_item(issue_id, item_id)?)?
        }
        value if value.starts_with("entrance://issues/") && value.ends_with("/timeline") => {
            let issue_id = value
                .trim_start_matches("entrance://issues/")
                .trim_end_matches("/timeline")
                .parse::<i64>()
                .with_context(|| {
                    format!("invalid Entrance issue timeline resource URI `{value}`")
                })?;
            serde_json::to_value(services.hive.issue_timeline(issue_id)?)?
        }
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
    let action_tool_permissions = actions
        .iter()
        .filter_map(|action| {
            let tool = mcp_tool_for_issue_action(&action.action)?;
            Some(serde_json::json!({
                "action": &action.action,
                "tool": tool,
                "permission": mcp_tool_permission(tool)
            }))
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "schema_version": "entrance.mcp.issue_permission_policy.v1",
        "human_confirmed_actions": human_confirmed_actions,
        "action_tool_permissions": action_tool_permissions,
        "confirmation_arg": "human_confirmed",
        "confirmation_receipt": {
            "schema_version": OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
            "recorded_as": [
                "issue_comment.payload.confirmation_receipt",
                "loop_evidence.payload.operator.confirmation_receipt",
                "issue_comment.body",
                "loop_evidence.payload.operator.comment_body"
            ],
            "marker_prefix": "MCP confirmation:",
            "policy_schema_version": MCP_PERMISSION_POLICY_SCHEMA_VERSION,
            "client_identity": {
                "source": "initialize.clientInfo",
                "fields": ["name", "version"],
                "required": false
            },
            "actor_identity": {
                "resource": "entrance://policy/actor-identity",
                "field": "confirmation_receipt.actor",
                "verified": false
            }
        },
        "policy_resource": "entrance://policy/mcp-permissions"
    })
}

fn mcp_tool_permissions() -> Vec<serde_json::Value> {
    MCP_TOOL_NAMES
        .iter()
        .map(|tool| mcp_tool_permission(tool))
        .collect()
}

fn mcp_tool_permission(tool: &str) -> serde_json::Value {
    let (access, operation, reads, writes, actor_arg, confirmation, boundary) = match tool {
        "entrance_issue_list" => (
            "read",
            "issue.list",
            vec!["issue/status/comment"],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_issue_show" => (
            "read",
            "issue.show",
            vec!["issue/status/comment", "loop/trace/evidence/doctor"],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_issue_control" => (
            "read",
            "issue.control",
            vec![
                "issue/status/comment",
                "loop/trace/evidence/doctor",
                "policy/action_permission",
                "operator_confirmation_receipt",
            ],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_loop_control" => (
            "read",
            "loop.control",
            vec![
                "loop/dashboard",
                "loop/evidence_drilldown",
                "loop/evidence_manifest",
                "loop/runtime_preflight",
                "loop/worker_lifecycle",
                "issue/status/comment",
                "reviewer/verdict",
                "policy/action_permission",
            ],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_connector_queue" => (
            "read",
            "connector.queue",
            vec![
                "issue/status/comment",
                "connector/queue",
                "connector/registry",
                "connector/decision_surface",
            ],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_connector_control" => (
            "read",
            "connector.control",
            vec![
                "issue/status/comment",
                "connector/queue",
                "connector/publish_plan",
                "connector/roundtrip_plan",
                "connector/decision_surface",
                "policy/action_permission",
            ],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_connector_publish_plan" => (
            "read",
            "connector.publish_plan",
            vec![
                "connector/queue",
                "connector/publish_plan",
                "connector/registry",
            ],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_connector_publish_execute" => (
            "human_decision",
            "connector.publish_execute",
            vec!["connector/queue", "connector/publish_plan"],
            vec![
                "issue_comment",
                "loop_evidence",
                "operator_confirmation_receipt",
                "connector_mirror",
            ],
            Some("author"),
            Some(vec!["connector_publish_execute"]),
            Vec::new(),
        ),
        "entrance_connector_roundtrip_plan" => (
            "read",
            "connector.roundtrip_plan",
            vec![
                "connector/queue",
                "connector/roundtrip_plan",
                "connector/registry",
            ],
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
        "entrance_connector_roundtrip_execute" => (
            "human_decision",
            "connector.roundtrip_execute",
            vec!["connector/queue", "connector/roundtrip_plan"],
            vec![
                "issue_comment",
                "loop_evidence",
                "operator_confirmation_receipt",
                "connector_mirror",
                "connector_readback",
                "connector_admission",
            ],
            Some("author"),
            Some(vec!["connector_roundtrip_execute"]),
            Vec::new(),
        ),
        "entrance_review_queue" => (
            "read",
            "review_queue.list",
            vec!["blocked_issue/status/comment/evidence/options"],
            Vec::new(),
            None,
            None,
            vec!["Blocked", "Needs Review"],
        ),
        "entrance_issue_comment" => (
            "write",
            "issue.comment",
            vec!["issue/status"],
            vec!["issue_comment", "operator_comment_evidence"],
            Some("author"),
            None,
            Vec::new(),
        ),
        "entrance_loop_create" => (
            "write",
            "loop.create",
            Vec::new(),
            vec!["loop_contract", "issue", "system_comment", "policies"],
            None,
            None,
            Vec::new(),
        ),
        "entrance_issue_run" => (
            "write",
            "issue.run",
            vec!["issue/status", "loop_contract", "runtime_policy"],
            vec![
                "stages",
                "packets",
                "admissions",
                "evidence",
                "verdicts",
                "comments",
            ],
            None,
            None,
            vec!["Todo"],
        ),
        "entrance_issue_retry" => (
            "human_decision",
            "issue.retry_run",
            vec![
                "blocked_issue/status/comment/evidence/options",
                "runtime_policy",
            ],
            vec![
                "operator_decision_comment",
                "operator_confirmation_receipt",
                "operator_decision_evidence",
                "stages",
                "packets",
                "admissions",
                "evidence",
                "verdicts",
            ],
            Some("author"),
            Some(vec!["retry"]),
            vec!["Blocked", "Needs Review"],
        ),
        "entrance_issue_decide" => (
            "human_decision",
            "issue.decide",
            vec!["blocked_issue/status/comment/evidence/options"],
            vec![
                "operator_decision_comment",
                "operator_confirmation_receipt",
                "operator_decision_evidence",
            ],
            Some("author"),
            Some(vec!["retry", "request-review", "cancel"]),
            vec!["Blocked", "Needs Review"],
        ),
        _ => (
            "unknown",
            "unknown",
            Vec::new(),
            Vec::new(),
            None,
            None,
            Vec::new(),
        ),
    };

    let mut permission = serde_json::json!({
        "schema_version": MCP_TOOL_PERMISSION_SCHEMA_VERSION,
        "tool": tool,
        "access": access,
        "operation": operation,
        "reads": reads,
        "writes": writes,
        "policy_resource": "entrance://policy/mcp-permissions"
    });
    if let Some(actor_arg) = actor_arg {
        permission["actor_arg"] = serde_json::json!(actor_arg);
        permission["default_actor"] = serde_json::json!("mcp-agent");
    }
    if !boundary.is_empty() {
        permission["status_boundary"] = serde_json::json!(boundary);
    }
    if let Some(actions) = confirmation {
        permission["confirmation"] = serde_json::json!({
            "required": true,
            "actions": actions,
            "argument": "human_confirmed",
            "value": true,
            "receipt_schema": OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
            "receipt_source": "mcp"
        });
    } else {
        permission["confirmation"] = serde_json::json!({
            "required": false
        });
    }
    permission
}

fn mcp_tools_by_access(permissions: &[serde_json::Value], access: &str) -> Vec<String> {
    permissions
        .iter()
        .filter(|permission| {
            permission.get("access").and_then(|value| value.as_str()) == Some(access)
        })
        .filter_map(|permission| {
            permission
                .get("tool")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn mcp_human_confirmation_requirements(
    permissions: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    permissions
        .iter()
        .filter(|permission| {
            permission
                .pointer("/confirmation/required")
                .and_then(|value| value.as_bool())
                == Some(true)
        })
        .map(|permission| {
            serde_json::json!({
                "tool": permission.get("tool").cloned().unwrap_or_default(),
                "actions": permission.pointer("/confirmation/actions").cloned().unwrap_or_default(),
                "argument": permission.pointer("/confirmation/argument").cloned().unwrap_or_default(),
                "value": permission.pointer("/confirmation/value").cloned().unwrap_or_default(),
                "receipt_schema": permission.pointer("/confirmation/receipt_schema").cloned().unwrap_or_default(),
                "reason": match permission.get("tool").and_then(|value| value.as_str()) {
                    Some("entrance_issue_retry") => "Retry advances a blocked/rejected issue into a new loop round.",
                    Some("entrance_issue_decide") => "Retry, review, and cancel are human decision boundaries.",
                    Some("entrance_connector_publish_execute") => "Connector publish writes issue/status/comment state to an external surface.",
                    Some("entrance_connector_roundtrip_execute") => "Connector roundtrip writes, reads back, and admits external connector evidence.",
                    _ => "Human confirmation is required for this MCP tool."
                }
            })
        })
        .collect()
}

fn mcp_tool_for_issue_action(action: &str) -> Option<&'static str> {
    match action {
        "run" => Some("entrance_issue_run"),
        "comment" => Some("entrance_issue_comment"),
        "retry" => Some("entrance_issue_retry"),
        "request-review" | "cancel" => Some("entrance_issue_decide"),
        _ => None,
    }
}

fn mcp_permission_policy() -> serde_json::Value {
    let tool_permissions = mcp_tool_permissions();
    let read_only_tools = mcp_tools_by_access(&tool_permissions, "read");
    let write_tools = mcp_tools_by_access(&tool_permissions, "write");
    let human_decision_tools = mcp_tools_by_access(&tool_permissions, "human_decision");
    let requires_human_confirmation = mcp_human_confirmation_requirements(&tool_permissions);
    let actor_identity = mcp_actor_identity_policy();
    serde_json::json!({
        "schema_version": MCP_PERMISSION_POLICY_SCHEMA_VERSION,
        "default_actor": "mcp-agent",
        "actor_identity": actor_identity,
        "tool_permission_registry": {
            "schema_version": MCP_TOOL_PERMISSION_REGISTRY_SCHEMA_VERSION,
            "tools": tool_permissions
        },
        "read_only_tools": read_only_tools,
        "write_tools": write_tools,
        "human_decision_tools": human_decision_tools,
        "requires_human_confirmation": requires_human_confirmation,
        "confirmation_receipt": {
            "schema_version": OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
            "recorded_as": [
                "issue_comment.payload.confirmation_receipt",
                "loop_evidence.payload.operator.confirmation_receipt",
                "issue_comment.body",
                "loop_evidence.payload.operator.comment_body"
            ],
            "client_identity": {
                "source": "initialize.clientInfo",
                "fields": ["name", "version"],
                "required": false
            },
            "actor_identity": {
                "resource": "entrance://policy/actor-identity",
                "field": "confirmation_receipt.actor",
                "verified": false
            },
            "source": "issue/status/comment",
            "marker_prefix": "MCP confirmation:",
            "note_template": format!("MCP confirmation: human_confirmed=true; action=<action>; author=<author>; policy={MCP_PERMISSION_POLICY_SCHEMA_VERSION}")
        },
        "blocked_boundary": {
            "statuses": ["Blocked", "Needs Review"],
            "resource": "entrance://review-queue",
            "prompt": "entrance_blocker_decision"
        }
    })
}

fn mcp_actor_identity_policy() -> serde_json::Value {
    serde_json::json!({
        "schema_version": MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION,
        "resource": "entrance://policy/actor-identity",
        "default_actor": {
            "id": "mcp:mcp-agent",
            "label": "mcp-agent",
            "source": "author_arg_default",
            "trust": "self_reported",
            "verified": false
        },
        "bindings": [
            {
                "surface": "mcp",
                "actor_source": "author argument",
                "client_source": "initialize.clientInfo",
                "trust": "self_reported",
                "verified": false,
                "receipt_source": "mcp"
            },
            {
                "surface": "panel",
                "actor_source": "daemon author argument",
                "client_source": "local-hive-panel via daemon.invoke",
                "trust": "local_panel_audit",
                "verified": false,
                "receipt_source": "panel"
            }
        ],
        "verified_identity": {
            "available": false,
            "missing": [
                "authenticated operator session",
                "actor-to-client binding",
                "connector account mapping"
            ]
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

fn ensure_connector_execute_confirmed(args: &serde_json::Value, action: &str) -> Result<()> {
    if args
        .get("human_confirmed")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        return Ok(());
    }

    anyhow::bail!(
        "MCP action `{action}` requires human_confirmed=true; read entrance://connectors/queue and the current connector plan resource before executing connector writes"
    )
}

fn mcp_author(args: &serde_json::Value) -> String {
    optional_string_arg(args, "author").unwrap_or_else(|| "mcp-agent".to_string())
}

fn append_human_confirmation_note(
    body: Option<String>,
    action: &str,
    author: &str,
) -> Option<String> {
    let marker = mcp_confirmation_marker(action, author);
    let body = body
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Some(match body {
        Some(body) => format!("{body}\n\n{marker}"),
        None => marker,
    })
}

fn mcp_human_confirmation_receipt(
    action: &str,
    author: &str,
    session: &McpSession,
) -> OperatorConfirmationReceipt {
    OperatorConfirmationReceipt {
        schema_version: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
        source: "mcp".to_string(),
        policy_schema_version: MCP_PERMISSION_POLICY_SCHEMA_VERSION.to_string(),
        confirmation_arg: "human_confirmed".to_string(),
        human_confirmed: true,
        action: action.to_string(),
        author: author.to_string(),
        marker: mcp_confirmation_marker(action, author),
        client: session
            .client
            .as_ref()
            .map(|client| OperatorConfirmationClient {
                name: client.name.clone(),
                version: client.version.clone(),
                source: "initialize.clientInfo".to_string(),
            }),
        actor: Some(OperatorConfirmationActor {
            id: format!("mcp:{author}"),
            label: author.to_string(),
            source: "author_arg".to_string(),
            trust: "self_reported".to_string(),
            verified: false,
        }),
    }
}

fn mcp_confirmation_marker(action: &str, author: &str) -> String {
    format!(
        "MCP confirmation: human_confirmed=true; action={action}; author={author}; policy={MCP_PERMISSION_POLICY_SCHEMA_VERSION}"
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
        append_human_confirmation_note, connector_control_packet_from_reports,
        ensure_connector_execute_confirmed, ensure_human_confirmed, initialize_result,
        issue_control_packet, issue_control_packet_with_connector,
        loop_control_packet_from_reports, mcp_client_identity, mcp_human_confirmation_receipt,
        mcp_permission_policy, prompt_loop_contract, prompt_specs, resource_templates, tool_specs,
        McpSession, MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION, MCP_CONNECTOR_CONTROL_SCHEMA_VERSION,
        MCP_FALLBACK_PROTOCOL_VERSION, MCP_ISSUE_CONTROL_SCHEMA_VERSION,
        MCP_LOOP_CONTROL_SCHEMA_VERSION, MCP_PERMISSION_POLICY_SCHEMA_VERSION,
        MCP_TOOL_PERMISSION_REGISTRY_SCHEMA_VERSION, MCP_TOOL_PERMISSION_SCHEMA_VERSION,
    };
    use entrance_core::{HiveComment, HiveIssue};
    use entrance_hive::{IssueAction, IssueCard};

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
        assert!(names.contains(&"entrance_issue_control"));
        assert!(names.contains(&"entrance_loop_control"));
        assert!(names.contains(&"entrance_connector_queue"));
        assert!(names.contains(&"entrance_connector_control"));
        assert!(names.contains(&"entrance_connector_publish_plan"));
        assert!(names.contains(&"entrance_connector_publish_execute"));
        assert!(names.contains(&"entrance_connector_roundtrip_plan"));
        assert!(names.contains(&"entrance_connector_roundtrip_execute"));
        assert!(names.contains(&"entrance_loop_create"));
        assert!(names.contains(&"entrance_issue_run"));
        assert!(names.contains(&"entrance_issue_retry"));
        assert!(names.contains(&"entrance_review_queue"));
        assert!(names.contains(&"entrance_issue_comment"));
        assert!(names.contains(&"entrance_issue_decide"));

        for tool in tools {
            let name = tool
                .get("name")
                .and_then(|value| value.as_str())
                .expect("tool should have a name");
            assert_eq!(
                tool.pointer("/annotations/entrance_permission/schema_version")
                    .and_then(|value| value.as_str()),
                Some(MCP_TOOL_PERMISSION_SCHEMA_VERSION)
            );
            assert_eq!(
                tool.pointer("/annotations/entrance_permission/tool")
                    .and_then(|value| value.as_str()),
                Some(name)
            );
        }
    }

    #[test]
    fn issue_control_packet_exposes_actions_blockers_and_receipts() {
        let card = IssueCard {
            issue: HiveIssue {
                id: 42,
                loop_id: Some(7),
                title: "Loop #7: blocked issue".to_string(),
                status: "Blocked".to_string(),
                summary: Some("Needs human decision".to_string()),
                created_at: "2026-06-09T00:00:00Z".to_string(),
                updated_at: "2026-06-09T00:00:01Z".to_string(),
            },
            comments: vec![HiveComment {
                id: 9,
                issue_id: 42,
                author: "human".to_string(),
                body: "Retry with local runtime.\n\nPanel confirmation: action=retry".to_string(),
                payload: serde_json::json!({
                    "schema_version": "entrance.hive.operator_decision.v1",
                    "source": "operator",
                    "action": "retry",
                    "confirmation_receipt": {
                        "schema_version": entrance_hive::OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION,
                        "source": "panel",
                        "policy_schema_version": entrance_hive::OPERATOR_ACTION_POLICY_SCHEMA_VERSION,
                        "confirmation_arg": entrance_hive::OPERATOR_ACTION_CONFIRMATION_ARG,
                        "human_confirmed": true,
                        "action": "retry",
                        "author": "human",
                        "marker": "Panel confirmation: action=retry"
                    }
                }),
                created_at: "2026-06-09T00:00:02Z".to_string(),
            }],
            actions: vec![IssueAction {
                schema_version: "entrance.hive.issue_action.v1".to_string(),
                action: "retry".to_string(),
                label: "Retry".to_string(),
                command:
                    "entrance hive issue retry-run 42 --body <note> --human-confirmed --compact"
                        .to_string(),
                source: "human_options".to_string(),
                input: "note".to_string(),
                destructive: false,
                runtime: Some("local".to_string()),
                confirmation_required: true,
                confirmation_arg: Some(entrance_hive::OPERATOR_ACTION_CONFIRMATION_ARG.to_string()),
                receipt_schema: Some(
                    entrance_hive::OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
                ),
                policy_schema_version: Some(
                    entrance_hive::OPERATOR_ACTION_POLICY_SCHEMA_VERSION.to_string(),
                ),
            }],
            trace: None,
            doctor: None,
        };

        let packet = issue_control_packet(&card);
        assert_eq!(
            packet
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(MCP_ISSUE_CONTROL_SCHEMA_VERSION)
        );
        assert_eq!(
            packet
                .pointer("/state/needs_human_decision")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer("/actions/0/tool")
                .and_then(|value| value.as_str()),
            Some("entrance_issue_retry")
        );
        assert_eq!(
            packet
                .pointer("/actions/0/call/arguments/human_confirmed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer(
                    "/human_decision_boundary/actions/0/issue_action_contract/confirmation_arg"
                )
                .and_then(|value| value.as_str()),
            Some(entrance_hive::OPERATOR_ACTION_CONFIRMATION_ARG)
        );
        assert_eq!(
            packet
                .pointer("/comments/operator_confirmation_receipts/0/receipt/source")
                .and_then(|value| value.as_str()),
            Some("panel")
        );
        assert_eq!(
            packet
                .pointer("/actor_identity/schema_version")
                .and_then(|value| value.as_str()),
            Some(MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION)
        );
        assert_eq!(
            packet
                .pointer("/resources/actor_identity")
                .and_then(|value| value.as_str()),
            Some("entrance://policy/actor-identity")
        );
        assert_eq!(
            packet
                .pointer("/resources/loop_dashboard")
                .and_then(|value| value.as_str()),
            Some("entrance://loops/7/dashboard")
        );
        assert_eq!(
            packet
                .pointer("/resources/timeline")
                .and_then(|value| value.as_str()),
            Some("entrance://issues/42/timeline")
        );
        assert_eq!(
            packet
                .pointer("/resources/transition_policy")
                .and_then(|value| value.as_str()),
            Some("entrance://issues/42/transition-policy")
        );
        assert_eq!(
            packet
                .pointer("/resources/loop_control")
                .and_then(|value| value.as_str()),
            Some("entrance://loops/7/control")
        );
        assert_eq!(
            packet
                .pointer("/resources/runtime_preflight")
                .and_then(|value| value.as_str()),
            Some("entrance://loops/7/runtime-preflight")
        );
        assert_eq!(
            packet
                .pointer("/resources/evidence_manifest")
                .and_then(|value| value.as_str()),
            Some("entrance://loops/7/evidence-manifest")
        );
        assert_eq!(
            packet
                .pointer("/resources/worker_lifecycle")
                .and_then(|value| value.as_str()),
            Some("entrance://loops/7/worker-lifecycle")
        );
        assert!(packet
            .get("runtime_preflight")
            .is_some_and(|value| value.is_null()));
        assert!(packet
            .get("worker_lifecycle")
            .is_some_and(|value| value.is_null()));
    }

    #[test]
    fn loop_control_packet_exposes_reviewer_surface() {
        let dashboard = serde_json::json!({
            "schema_version": "entrance.hive.loop_dashboard.v1",
            "loop_id": 7,
            "issue": {
                "id": 42,
                "status": "Blocked",
                "title": "Loop #7: blocked issue"
            },
            "status": "blocked",
            "active_phase": "review",
            "current_round": 3,
            "runtime": "local",
            "dashboard_state": "needs_human_decision",
            "reviewer": {
                "decision": "blocked",
                "reason_code": "review_budget_exhausted",
                "score_vector": [{
                    "name": "gates",
                    "score": 0.0,
                    "summary": "Reviewer rejected the candidate."
                }],
                "reviewer_invalid_rounds_used": 3,
                "reviewer_invalid_round_budget": 3,
                "reviewer_invalid_budget_exhausted": true,
                "fallback_status": "Blocked"
            },
            "human_decision": {
                "required": true,
                "issue_status": "Blocked",
                "options": ["Retry with narrower scope"],
                "actions": [{
                    "action": "retry",
                    "label": "Retry",
                    "command": "entrance hive issue retry-run 42 --body <note> --human-confirmed --compact"
                }]
            }
        });
        let evidence_drilldown = serde_json::json!({
            "schema_version": "entrance.hive.evidence_drilldown.v1",
            "loop_id": 7,
            "blockers": ["missing reviewer receipt"]
        });
        let evidence_manifest = serde_json::json!({
            "schema_version": "entrance.hive.evidence_manifest.v1",
            "loop_id": 7,
            "manifest_state": "ok",
            "coverage": {
                "evidence_count": 3,
                "receipt_count": 3,
                "digest_count": 6
            }
        });
        let runtime_preflight = serde_json::json!({
            "schema_version": "entrance.hive.runtime_preflight.v1",
            "loop_id": 7,
            "preflight_state": "admitted",
            "policy": {
                "gate": "runtime_policy_ready"
            },
            "current": {
                "gate_passed": true
            }
        });
        let worker_lifecycle = serde_json::json!({
            "schema_version": "entrance.hive.worker_lifecycle.v1",
            "loop_id": 7,
            "issue_id": 42,
            "issue_status": "Blocked",
            "lifecycle_state": "blocked",
            "policy": {
                "expected_roles": ["explorer", "developer", "reviewer"],
                "reviewer_invalid_round_budget": 3,
                "fallback_status": "Blocked"
            },
            "current": {
                "round": 3,
                "decision": "blocked",
                "observed_roles": ["explorer", "developer", "reviewer"],
                "missing_roles": [],
                "reviewer_invalid_rounds_used": 3,
                "reviewer_invalid_budget_exhausted": true
            },
            "failures": []
        });

        let packet = loop_control_packet_from_reports(
            7,
            dashboard,
            evidence_drilldown,
            evidence_manifest,
            runtime_preflight,
            worker_lifecycle,
        );

        assert_eq!(
            packet
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(MCP_LOOP_CONTROL_SCHEMA_VERSION)
        );
        assert_eq!(
            packet
                .pointer("/state/primary_action")
                .and_then(|value| value.as_str()),
            Some("human_decision")
        );
        assert_eq!(
            packet
                .pointer("/state/reviewer_invalid_budget_exhausted")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer("/reviewer_gate_surface/allowed_decisions/0")
                .and_then(|value| value.as_str()),
            Some("keep")
        );
        assert_eq!(
            packet
                .pointer("/reviewer_gate_surface/budget_policy/fallback_status")
                .and_then(|value| value.as_str()),
            Some("Blocked")
        );
        assert_eq!(
            packet
                .pointer("/human_decision_boundary/required")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer("/operator_decision_surface/blocked_fallback/active")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer("/resources/loop_control")
                .and_then(|value| value.as_str()),
            Some("entrance://loops/7/control")
        );
        assert_eq!(
            packet
                .pointer("/mcp_policy/loop_control/operation")
                .and_then(|value| value.as_str()),
            Some("loop.control")
        );
    }

    #[test]
    fn issue_control_packet_exposes_connector_status_mapping() {
        let card = IssueCard {
            issue: HiveIssue {
                id: 43,
                loop_id: Some(8),
                title: "Loop #8: mapped issue".to_string(),
                status: "Blocked".to_string(),
                summary: Some("Needs remote status mapping".to_string()),
                created_at: "2026-06-09T00:00:00Z".to_string(),
                updated_at: "2026-06-09T00:00:01Z".to_string(),
            },
            comments: Vec::new(),
            actions: Vec::new(),
            trace: None,
            doctor: None,
        };
        let connector = serde_json::json!({
            "schema_version": "entrance.hive.issue_connector_control.v1",
            "issue_id": 43,
            "provider": "linear",
            "review_surface": "linear:ENT-43",
            "status_mapping": {
                "hive_status": "Blocked",
                "remote_state": "Blocked",
                "remote_state_id": "state-blocked",
                "remote_status_marker": "Status: Blocked",
                "readback_check": "remote_status"
            },
            "configured_status_mappings": [{
                "hive_status": "Blocked",
                "remote_state": "Blocked",
                "remote_state_id": "state-blocked"
            }]
        });

        let packet = issue_control_packet_with_connector(&card, connector);

        assert_eq!(
            packet
                .pointer("/connector/schema_version")
                .and_then(|value| value.as_str()),
            Some("entrance.hive.issue_connector_control.v1")
        );
        assert_eq!(
            packet
                .pointer("/connector/status_mapping/hive_status")
                .and_then(|value| value.as_str()),
            Some("Blocked")
        );
        assert_eq!(
            packet
                .pointer("/connector/status_mapping/remote_state_id")
                .and_then(|value| value.as_str()),
            Some("state-blocked")
        );
        assert_eq!(
            packet
                .pointer("/connector/configured_status_mappings/0/remote_state_id")
                .and_then(|value| value.as_str()),
            Some("state-blocked")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_queue")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/queue")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_provider_queue")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/queue/linear")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_publish_plan")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/publish-plan")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_provider_publish_plan")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/publish-plan/linear")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_roundtrip_plan")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/roundtrip-plan")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_provider_roundtrip_plan")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/roundtrip-plan/linear")
        );
    }

    #[test]
    fn connector_control_packet_exposes_decision_surface() {
        let queue = serde_json::json!({
            "schema_version": "entrance.hive.connector_queue.v1",
            "provider_filter": "linear",
            "provider_known": true,
            "total": 1,
            "current_count": 0,
            "publish_required_count": 1,
            "issues": [{
                "id": 43,
                "provider": "linear"
            }]
        });
        let publish_plan = serde_json::json!({
            "schema_version": "entrance.hive.connector_publish_plan.v1",
            "plan_id": "publish-plan-1",
            "provider_filter": "linear",
            "provider_known": true,
            "issue_count": 1,
            "can_execute": true,
            "reason": "ready",
            "blockers": [],
            "issues": [{ "id": 43 }]
        });
        let roundtrip_plan = serde_json::json!({
            "schema_version": "entrance.hive.connector_roundtrip_plan.v1",
            "plan_id": "roundtrip-plan-1",
            "provider_filter": "linear",
            "provider_known": true,
            "issue_count": 1,
            "can_execute": false,
            "reason": "plan_blocked",
            "blockers": ["readback_not_supported"],
            "issues": [{ "id": 43 }]
        });

        let packet = connector_control_packet_from_reports(
            Some("linear"),
            queue,
            publish_plan,
            roundtrip_plan,
        );

        assert_eq!(
            packet
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(MCP_CONNECTOR_CONTROL_SCHEMA_VERSION)
        );
        assert_eq!(
            packet
                .pointer("/state/primary_action")
                .and_then(|value| value.as_str()),
            Some("connector_publish_execute")
        );
        assert_eq!(
            packet
                .pointer("/human_decision_boundary/required")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer("/human_decision_boundary/actions/0/call/name")
                .and_then(|value| value.as_str()),
            Some("entrance_connector_publish_execute")
        );
        assert_eq!(
            packet
                .pointer("/human_decision_boundary/actions/0/call/arguments/plan_id")
                .and_then(|value| value.as_str()),
            Some("publish-plan-1")
        );
        assert_eq!(
            packet
                .pointer("/human_decision_boundary/actions/0/call/arguments/human_confirmed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            packet
                .pointer("/human_decision_boundary/blocked_actions/0/action")
                .and_then(|value| value.as_str()),
            Some("connector_roundtrip_execute")
        );
        assert_eq!(
            packet
                .pointer("/operator_decision_surface/options/2/key")
                .and_then(|value| value.as_str()),
            Some("C")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_control")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/control/linear")
        );
        assert_eq!(
            packet
                .pointer("/resources/connector_publish_plan")
                .and_then(|value| value.as_str()),
            Some("entrance://connectors/publish-plan/linear")
        );
    }

    #[test]
    fn resource_templates_expose_loop_observability() {
        let templates = resource_templates();
        let uri_templates = templates
            .get("resourceTemplates")
            .and_then(|value| value.as_array())
            .expect("resource templates should be an array")
            .iter()
            .filter_map(|template| template.get("uriTemplate").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(uri_templates.contains(&"entrance://connectors/queue/{provider}"));
        assert!(uri_templates.contains(&"entrance://connectors/control/{provider}"));
        assert!(uri_templates.contains(&"entrance://connectors/publish-plan/{provider}"));
        assert!(uri_templates.contains(&"entrance://connectors/roundtrip-plan/{provider}"));
        assert!(uri_templates.contains(&"entrance://issues/{issue_id}"));
        assert!(uri_templates.contains(&"entrance://issues/{issue_id}/control"));
        assert!(uri_templates.contains(&"entrance://issues/{issue_id}/transition-policy"));
        assert!(uri_templates.contains(&"entrance://issues/{issue_id}/timeline"));
        assert!(uri_templates.contains(&"entrance://issues/{issue_id}/timeline/items/{item_id}"));
        assert!(uri_templates.contains(&"entrance://loops/{loop_id}/control"));
        assert!(uri_templates.contains(&"entrance://loops/{loop_id}/dashboard"));
        assert!(uri_templates.contains(&"entrance://loops/{loop_id}/evidence-manifest"));
        assert!(uri_templates.contains(&"entrance://loops/{loop_id}/runtime-preflight"));
        assert!(uri_templates.contains(&"entrance://loops/{loop_id}/worker-lifecycle"));
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
        let publish_execute_tool = tools
            .iter()
            .find(|tool| {
                tool.get("name").and_then(|value| value.as_str())
                    == Some("entrance_connector_publish_execute")
            })
            .expect("connector publish execute tool should exist");
        let roundtrip_execute_tool = tools
            .iter()
            .find(|tool| {
                tool.get("name").and_then(|value| value.as_str())
                    == Some("entrance_connector_roundtrip_execute")
            })
            .expect("connector roundtrip execute tool should exist");

        assert_eq!(
            retry_tool.pointer("/inputSchema/properties/human_confirmed/type"),
            Some(&serde_json::json!("boolean"))
        );
        assert_eq!(
            decide_tool.pointer("/inputSchema/properties/human_confirmed/type"),
            Some(&serde_json::json!("boolean"))
        );
        assert_eq!(
            publish_execute_tool.pointer("/inputSchema/properties/human_confirmed/type"),
            Some(&serde_json::json!("boolean"))
        );
        assert_eq!(
            publish_execute_tool.pointer("/inputSchema/required/0"),
            Some(&serde_json::json!("plan_id"))
        );
        assert_eq!(
            roundtrip_execute_tool.pointer("/inputSchema/properties/human_confirmed/type"),
            Some(&serde_json::json!("boolean"))
        );
        assert_eq!(
            retry_tool
                .pointer("/annotations/entrance_permission/access")
                .and_then(|value| value.as_str()),
            Some("human_decision")
        );
        assert_eq!(
            publish_execute_tool
                .pointer("/annotations/entrance_permission/access")
                .and_then(|value| value.as_str()),
            Some("human_decision")
        );
        assert_eq!(
            retry_tool
                .pointer("/annotations/entrance_permission/confirmation/receipt_schema")
                .and_then(|value| value.as_str()),
            Some(entrance_hive::OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION)
        );
        assert_eq!(
            publish_execute_tool
                .pointer("/annotations/entrance_permission/confirmation/actions/0")
                .and_then(|value| value.as_str()),
            Some("connector_publish_execute")
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
        assert_eq!(
            ensure_connector_execute_confirmed(&serde_json::json!({}), "connector_publish_execute")
                .expect_err("missing connector confirmation should fail")
                .to_string(),
            "MCP action `connector_publish_execute` requires human_confirmed=true; read entrance://connectors/queue and the current connector plan resource before executing connector writes"
        );
        assert!(ensure_connector_execute_confirmed(
            &serde_json::json!({ "human_confirmed": true }),
            "connector_publish_execute"
        )
        .is_ok());
    }

    #[test]
    fn human_confirmation_note_records_action_author_and_policy() {
        let note = append_human_confirmation_note(
            Some("Retry with a narrower scope.".to_string()),
            "retry",
            "human-a",
        )
        .expect("confirmed decisions should always return a note");

        assert!(note.contains("Retry with a narrower scope."));
        assert!(note.contains(
            "MCP confirmation: human_confirmed=true; action=retry; author=human-a; policy=entrance.mcp.permission_policy.v1"
        ));

        let marker_only = append_human_confirmation_note(None, "cancel", "mcp-agent")
            .expect("marker-only confirmed decisions should return a note");
        assert_eq!(
            marker_only,
            format!(
                "MCP confirmation: human_confirmed=true; action=cancel; author=mcp-agent; policy={MCP_PERMISSION_POLICY_SCHEMA_VERSION}"
            )
        );

        let session = McpSession {
            client: mcp_client_identity(&serde_json::json!({
                "clientInfo": {
                    "name": "codex-test-client",
                    "version": "0.1.0"
                }
            })),
        };
        let receipt = mcp_human_confirmation_receipt("retry", "human-a", &session);
        assert_eq!(
            receipt.schema_version.as_str(),
            entrance_hive::OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION
        );
        assert_eq!(receipt.source.as_str(), "mcp");
        assert_eq!(
            receipt.policy_schema_version.as_str(),
            MCP_PERMISSION_POLICY_SCHEMA_VERSION
        );
        assert_eq!(receipt.confirmation_arg.as_str(), "human_confirmed");
        assert!(receipt.human_confirmed);
        assert_eq!(receipt.action.as_str(), "retry");
        assert_eq!(receipt.author.as_str(), "human-a");
        assert_eq!(
            receipt.marker.as_str(),
            "MCP confirmation: human_confirmed=true; action=retry; author=human-a; policy=entrance.mcp.permission_policy.v1"
        );
        let client = receipt
            .client
            .as_ref()
            .expect("clientInfo should be copied into receipt");
        assert_eq!(client.name, "codex-test-client");
        assert_eq!(client.version.as_deref(), Some("0.1.0"));
        assert_eq!(client.source, "initialize.clientInfo");
        let actor = receipt.actor.as_ref().expect("receipt should record actor");
        assert_eq!(actor.id, "mcp:human-a");
        assert_eq!(actor.label, "human-a");
        assert_eq!(actor.source, "author_arg");
        assert_eq!(actor.trust, "self_reported");
        assert!(!actor.verified);
    }

    #[test]
    fn client_identity_reads_initialize_client_info() {
        let identity = mcp_client_identity(&serde_json::json!({
            "clientInfo": { "name": "Codex Desktop", "version": "2.3.4" }
        }))
        .expect("client info should parse");
        assert_eq!(identity.name, "Codex Desktop");
        assert_eq!(identity.version.as_deref(), Some("2.3.4"));

        assert!(mcp_client_identity(&serde_json::json!({
            "clientInfo": { "name": "   " }
        }))
        .is_none());
    }

    #[test]
    fn permission_policy_names_human_decision_tools() {
        let policy = mcp_permission_policy();
        assert_eq!(
            policy
                .pointer("/tool_permission_registry/schema_version")
                .and_then(|value| value.as_str()),
            Some(MCP_TOOL_PERMISSION_REGISTRY_SCHEMA_VERSION)
        );
        let human_tools = policy
            .get("human_decision_tools")
            .and_then(|value| value.as_array())
            .expect("policy should list human decision tools");
        let tool_permissions = policy
            .pointer("/tool_permission_registry/tools")
            .and_then(|value| value.as_array())
            .expect("policy should expose per-tool permissions");

        assert!(human_tools
            .iter()
            .any(|value| value.as_str() == Some("entrance_issue_retry")));
        assert!(human_tools
            .iter()
            .any(|value| value.as_str() == Some("entrance_issue_decide")));
        assert!(human_tools
            .iter()
            .any(|value| value.as_str() == Some("entrance_connector_publish_execute")));
        assert!(human_tools
            .iter()
            .any(|value| value.as_str() == Some("entrance_connector_roundtrip_execute")));
        assert!(tool_permissions.iter().any(|permission| {
            permission.get("tool").and_then(|value| value.as_str()) == Some("entrance_issue_decide")
                && permission.get("access").and_then(|value| value.as_str())
                    == Some("human_decision")
                && permission
                    .pointer("/confirmation/receipt_schema")
                    .and_then(|value| value.as_str())
                    == Some(entrance_hive::OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION)
        }));
        assert!(tool_permissions.iter().any(|permission| {
            permission.get("tool").and_then(|value| value.as_str())
                == Some("entrance_connector_publish_execute")
                && permission.get("access").and_then(|value| value.as_str())
                    == Some("human_decision")
                && permission
                    .pointer("/confirmation/actions/0")
                    .and_then(|value| value.as_str())
                    == Some("connector_publish_execute")
        }));
        assert_eq!(
            policy
                .pointer("/blocked_boundary/resource")
                .and_then(|value| value.as_str()),
            Some("entrance://review-queue")
        );
        assert_eq!(
            policy
                .pointer("/confirmation_receipt/source")
                .and_then(|value| value.as_str()),
            Some("issue/status/comment")
        );
        assert_eq!(
            policy
                .pointer("/confirmation_receipt/schema_version")
                .and_then(|value| value.as_str()),
            Some(entrance_hive::OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION)
        );
        assert_eq!(
            policy
                .pointer("/confirmation_receipt/client_identity/source")
                .and_then(|value| value.as_str()),
            Some("initialize.clientInfo")
        );
        assert_eq!(
            policy
                .pointer("/confirmation_receipt/actor_identity/resource")
                .and_then(|value| value.as_str()),
            Some("entrance://policy/actor-identity")
        );
        assert_eq!(
            policy
                .pointer("/actor_identity/schema_version")
                .and_then(|value| value.as_str()),
            Some(MCP_ACTOR_IDENTITY_POLICY_SCHEMA_VERSION)
        );
        assert_eq!(
            policy
                .pointer("/actor_identity/verified_identity/available")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert!(policy
            .pointer("/confirmation_receipt/note_template")
            .and_then(|value| value.as_str())
            .is_some_and(|template| template.contains(MCP_PERMISSION_POLICY_SCHEMA_VERSION)));
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
        assert!(names.contains(&"entrance_loop_review"));
        assert!(names.contains(&"entrance_blocker_decision"));
        assert!(names.contains(&"entrance_connector_decision"));
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
