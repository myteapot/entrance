use std::{env, path::Path, thread, time::Duration};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    core::{action::ActorRole, mcp_stdio_client::SpawnedMcpStdioClient},
    plugins::forge::{
        allocate_agent_slot_worktree, CreateTaskRequest, ForgePlugin, ForgeTaskMetadata,
    },
};

const BOOTSTRAP_DEV_RUNTIME_MODE: &str = "bootstrap_dev_runtime_task";

#[derive(Debug, Clone, Default)]
pub struct ForgeBootstrapMcpCycleOptions {
    pub project_dir: Option<String>,
    pub model: String,
    pub agent_command: Option<String>,
    pub agent_count: usize,
}

#[derive(Clone, Serialize)]
pub struct ForgeBootstrapMcpSurfaceSummary {
    pub coordinator_role: &'static str,
    pub arch_surface_role: &'static str,
    pub dev_surface_role: &'static str,
    pub dev_assignment_surface: &'static str,
    pub dev_execution_mode: &'static str,
    pub agent_dispatch_surface: &'static str,
    pub agent_wait_mode: &'static str,
}

#[derive(Clone, Serialize)]
pub struct ForgeBootstrapMcpCycleReport {
    pub bootstrap_surface: ForgeBootstrapMcpSurfaceSummary,
    pub requested_agent_count: usize,
    pub agent_worktree_mode: &'static str,
    pub shared_worktree_boundary: Option<String>,
    pub dev_assignment: Value,
    pub agent_prepare: Value,
    pub agent_prepares: Vec<Value>,
    pub agent_dispatches: Vec<Value>,
    pub parent_status: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapPreparedAgentPlan {
    child_slot: String,
    issue_id: String,
    worktree_path: String,
    prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapDevTaskPlan {
    parent_task_id: i64,
    model: String,
    agent_command: Option<String>,
    prepared_agents: Vec<BootstrapPreparedAgentPlan>,
}

pub fn run_forge_bootstrap_mcp_cycle(
    forge: &ForgePlugin,
    app_data_dir: &Path,
    options: ForgeBootstrapMcpCycleOptions,
) -> Result<ForgeBootstrapMcpCycleReport> {
    let mut arch_surface = SpawnedMcpStdioClient::spawn(app_data_dir, ActorRole::Arch)?;
    let initialize_arch = arch_surface.initialize()?;
    assert_surface_role(&initialize_arch, "arch")?;

    let mut dev_surface = SpawnedMcpStdioClient::spawn(app_data_dir, ActorRole::Dev)?;
    let initialize_dev = dev_surface.initialize()?;
    assert_surface_role(&initialize_dev, "dev")?;

    let project_arguments = project_dir_tool_arguments(options.project_dir.as_deref(), None);
    let verified_dev_assignment = arch_surface
        .call_tool("forge_verify_dev_dispatch", project_arguments.clone())?
        .get("structuredContent")
        .cloned()
        .context("forge_verify_dev_dispatch should return structuredContent")?;
    let parent_task_id = json_i64(&verified_dev_assignment, &["task_id"])
        .context("forge_verify_dev_dispatch should return a parent task id")?;

    let prepared_agent = dev_surface
        .call_tool("forge_prepare_agent_dispatch", project_arguments)?
        .get("structuredContent")
        .cloned()
        .context("forge_prepare_agent_dispatch should return structuredContent")?;
    let worktree_path = json_string(&prepared_agent, &["worktree_path"])
        .context("forge_prepare_agent_dispatch should return worktree_path")?;
    let agent_worktree_mode = if options.agent_count > 1 {
        "per_agent_slot_worktree"
    } else {
        "single_managed_worktree"
    };
    let shared_worktree_boundary = None;

    let mut prepared_agent_reports = Vec::with_capacity(options.agent_count);
    let mut prepared_agent_plans = Vec::with_capacity(options.agent_count);
    for index in 0..options.agent_count {
        let slot = format!("agent-{}", index + 1);
        let child_prepare = if options.agent_count > 1 {
            let child_worktree =
                allocate_agent_slot_worktree(&worktree_path, &slot).map_err(anyhow::Error::msg)?;
            let prepare_arguments = project_dir_tool_arguments(
                options.project_dir.as_deref(),
                Some(child_worktree.as_str()),
            );
            dev_surface
                .call_tool("forge_prepare_agent_dispatch", prepare_arguments)?
                .get("structuredContent")
                .cloned()
                .context(
                    "forge_prepare_agent_dispatch should return structuredContent for child worktree",
                )?
        } else {
            prepared_agent.clone()
        };

        let issue_id = json_string(&child_prepare, &["issue_id"])
            .context("forge_prepare_agent_dispatch should return issue_id")?;
        let child_worktree_path = json_string(&child_prepare, &["worktree_path"])
            .context("forge_prepare_agent_dispatch should return worktree_path")?;
        let prompt = json_string(&child_prepare, &["prompt"])
            .context("forge_prepare_agent_dispatch should return prompt")?;

        prepared_agent_reports.push(strip_prompt_fields(with_child_slot(child_prepare, &slot)));
        prepared_agent_plans.push(BootstrapPreparedAgentPlan {
            child_slot: slot,
            issue_id,
            worktree_path: child_worktree_path,
            prompt,
        });
    }

    let parent_request = build_bootstrap_dev_runtime_request(
        parent_task_id,
        &verified_dev_assignment,
        &options.model,
        options.agent_command.clone(),
        prepared_agent_plans.clone(),
    )?;
    forge.replace_pending_task_request(parent_task_id, parent_request)?;
    forge
        .engine()
        .spawn_task(parent_task_id)
        .with_context(|| format!("failed to start bootstrap dev task `{parent_task_id}`"))?;

    let parent_status = wait_for_terminal_forge_tasks(&mut dev_surface, &[parent_task_id])?
        .into_iter()
        .next()
        .context("bootstrap cycle should collect the parent dev task status")?;
    let dev_assignment =
        build_bootstrap_dev_assignment_report(&verified_dev_assignment, &parent_status)?;

    let child_task_ids = json_array(&parent_status, &["supervision", "child_receipts"])
        .map(|receipts| {
            receipts
                .iter()
                .filter_map(|receipt| json_i64(receipt, &["child_task_id"]))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if child_task_ids.len() != options.agent_count {
        anyhow::bail!(
            "bootstrap dev parent task `{parent_task_id}` produced {} child receipt(s) for requested agent_count {}",
            child_task_ids.len(),
            options.agent_count
        );
    }
    let child_statuses = if child_task_ids.is_empty() {
        Vec::new()
    } else {
        wait_for_terminal_forge_tasks(&mut dev_surface, &child_task_ids)?
    };
    let agent_dispatches = child_statuses
        .into_iter()
        .map(build_agent_dispatch_report)
        .collect::<Result<Vec<_>>>()?;

    Ok(ForgeBootstrapMcpCycleReport {
        bootstrap_surface: ForgeBootstrapMcpSurfaceSummary {
            coordinator_role: "nota",
            arch_surface_role: "arch",
            dev_surface_role: "dev",
            dev_assignment_surface: "forge_verify_dev_dispatch",
            dev_execution_mode: BOOTSTRAP_DEV_RUNTIME_MODE,
            agent_dispatch_surface: "forge_dispatch_agent",
            agent_wait_mode: "dev_parent_waits_children",
        },
        requested_agent_count: options.agent_count,
        agent_worktree_mode,
        shared_worktree_boundary,
        dev_assignment,
        agent_prepare: prepared_agent_reports
            .first()
            .cloned()
            .context("bootstrap cycle should collect at least one prepared agent dispatch")?,
        agent_prepares: prepared_agent_reports,
        agent_dispatches,
        parent_status,
    })
}

pub fn run_forge_bootstrap_dev_task(app_data_dir: &Path, raw_plan: &str) -> Result<Value> {
    let plan: BootstrapDevTaskPlan =
        serde_json::from_str(raw_plan).context("bootstrap dev task plan should be valid JSON")?;
    if plan.parent_task_id <= 0 {
        anyhow::bail!("bootstrap dev task plan must include a positive parent_task_id");
    }
    if plan.prepared_agents.is_empty() {
        anyhow::bail!("bootstrap dev task plan must include at least one prepared agent dispatch");
    }

    let mut dev_surface = SpawnedMcpStdioClient::spawn(app_data_dir, ActorRole::Dev)?;
    let initialize_dev = dev_surface.initialize()?;
    assert_surface_role(&initialize_dev, "dev")?;

    let mut child_task_ids = Vec::with_capacity(plan.prepared_agents.len());
    for prepared_agent in &plan.prepared_agents {
        let mut dispatch_arguments = json!({
            "issue_id": prepared_agent.issue_id,
            "worktree_path": prepared_agent.worktree_path,
            "model": plan.model,
            "prompt": prepared_agent.prompt,
            "parent_task_id": plan.parent_task_id,
            "supervision_strategy": "one_for_one",
            "child_slot": prepared_agent.child_slot,
        });
        if let Some(agent_command) = plan.agent_command.as_ref() {
            dispatch_arguments["agent_command"] = Value::String(agent_command.clone());
        }

        let dispatched_agent = dev_surface
            .call_tool("forge_dispatch_agent", dispatch_arguments)?
            .get("structuredContent")
            .cloned()
            .context("forge_dispatch_agent should return structuredContent")?;
        let child_task_id = json_i64(&dispatched_agent, &["task_id"])
            .context("forge_dispatch_agent should return a child task id")?;
        child_task_ids.push(child_task_id);
    }

    let child_statuses = wait_for_terminal_forge_tasks(&mut dev_surface, &child_task_ids)?;
    let failed_children = child_statuses
        .iter()
        .filter_map(|status| json_string(status, &["task", "status"]))
        .filter(|status| status != "Done")
        .collect::<Vec<_>>();

    let report = json!({
        "parent_task_id": plan.parent_task_id,
        "agent_count": plan.prepared_agents.len(),
        "child_task_ids": child_task_ids,
        "child_statuses": child_statuses,
    });

    if failed_children.is_empty() {
        Ok(report)
    } else {
        anyhow::bail!(
            "bootstrap dev task observed non-Done child statuses: {}",
            failed_children.join(", ")
        );
    }
}

fn build_bootstrap_dev_runtime_request(
    parent_task_id: i64,
    verified_dev_assignment: &Value,
    model: &str,
    agent_command: Option<String>,
    prepared_agents: Vec<BootstrapPreparedAgentPlan>,
) -> Result<CreateTaskRequest> {
    let verified_dispatch = path_value(verified_dev_assignment, &["dispatch"])
        .cloned()
        .context("forge_verify_dev_dispatch should return dispatch details")?;
    let issue_id = json_string(&verified_dispatch, &["issue_id"])
        .context("forge_verify_dev_dispatch should report dispatch.issue_id")?;
    let worktree_path = json_string(&verified_dispatch, &["worktree_path"])
        .context("forge_verify_dev_dispatch should report dispatch.worktree_path")?;
    let command = env::current_exe()
        .context("failed to resolve current entrance executable")?
        .to_string_lossy()
        .to_string();
    let args = serde_json::to_string(&vec![
        "forge".to_string(),
        "run-bootstrap-dev-plan".to_string(),
    ])
    .context("failed to serialize bootstrap dev runtime args")?;
    let stdin_text = serde_json::to_string(&BootstrapDevTaskPlan {
        parent_task_id,
        model: model.to_string(),
        agent_command,
        prepared_agents,
    })
    .context("failed to serialize bootstrap dev task plan")?;
    let required_tokens = serde_json::to_string(&Vec::<String>::new())
        .context("failed to serialize bootstrap dev runtime required tokens")?;
    let metadata = serde_json::to_string(&ForgeTaskMetadata {
        kind: Some("dev_dispatch".to_string()),
        issue_id: Some(issue_id.clone()),
        worktree_path: Some(worktree_path.clone()),
        model: Some(model.to_string()),
        dispatch_role: Some(ActorRole::Dev),
        dispatch_tool_name: Some("forge_dispatch_dev".to_string()),
        allocator_role: Some(ActorRole::Nota),
        allocator_surface: Some("forge_bootstrap_mcp_cycle".to_string()),
        runtime_mode: Some(BOOTSTRAP_DEV_RUNTIME_MODE.to_string()),
    })
    .context("failed to serialize bootstrap dev runtime metadata")?;

    Ok(CreateTaskRequest {
        name: format!("Dev {issue_id}"),
        command,
        args,
        working_dir: Some(worktree_path),
        stdin_text: Some(stdin_text),
        required_tokens,
        metadata,
        dispatch_receipt: None,
    })
}

fn build_bootstrap_dev_assignment_report(
    verified_dev_assignment: &Value,
    parent_status: &Value,
) -> Result<Value> {
    let dispatch = path_value(verified_dev_assignment, &["dispatch"])
        .cloned()
        .context("forge_verify_dev_dispatch should return dispatch details")?;
    let task_id = json_i64(parent_status, &["task_id"])
        .context("forge_status should return task_id for parent bootstrap dev task")?;
    let task_status = json_string(parent_status, &["task", "status"])
        .context("forge_status should return task.status for parent bootstrap dev task")?;
    let task_command = path_value(parent_status, &["task", "command"])
        .cloned()
        .unwrap_or(Value::Null);
    let task_working_dir = path_value(parent_status, &["task", "working_dir"])
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "dispatch": strip_prompt_fields(dispatch),
        "task_id": task_id,
        "task_status": task_status,
        "task_command": task_command,
        "task_working_dir": task_working_dir,
        "execution_mode": BOOTSTRAP_DEV_RUNTIME_MODE,
    }))
}

fn build_agent_dispatch_report(final_status: Value) -> Result<Value> {
    let task_id = json_i64(&final_status, &["task_id"])
        .context("forge_status should return task_id for bootstrap child agent task")?;
    let task = path_value(&final_status, &["task"])
        .cloned()
        .unwrap_or(Value::Null);
    let supervision = path_value(&final_status, &["supervision"])
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "dispatch": {
            "dispatch_role": "agent",
            "dispatch_tool_name": "forge_dispatch_agent",
            "task_id": task_id,
            "task": task,
            "supervision": supervision,
        },
        "final_status": final_status,
    }))
}

fn project_dir_tool_arguments(project_dir: Option<&str>, worktree_path: Option<&str>) -> Value {
    let mut arguments = serde_json::Map::new();
    if let Some(project_dir) = project_dir {
        arguments.insert(
            "project_dir".to_string(),
            Value::String(project_dir.to_string()),
        );
    }
    if let Some(worktree_path) = worktree_path {
        arguments.insert(
            "worktree_path".to_string(),
            Value::String(worktree_path.to_string()),
        );
    }
    Value::Object(arguments)
}

fn wait_for_terminal_forge_tasks(
    surface: &mut SpawnedMcpStdioClient,
    task_ids: &[i64],
) -> Result<Vec<Value>> {
    let mut terminal_statuses = vec![None; task_ids.len()];

    for _ in 0..400 {
        let mut all_terminal = true;
        for (index, task_id) in task_ids.iter().enumerate() {
            if terminal_statuses[index].is_some() {
                continue;
            }

            let status = surface
                .call_tool("forge_status", json!({ "task_id": task_id }))?
                .get("structuredContent")
                .cloned()
                .context("forge_status should return structuredContent while waiting")?;
            let task_status = json_string(&status, &["task", "status"])
                .context("forge_status should return a task.status string")?;
            if matches!(
                task_status.as_str(),
                "Done" | "Failed" | "Cancelled" | "Blocked"
            ) {
                terminal_statuses[index] = Some(status);
            } else {
                all_terminal = false;
            }
        }

        if all_terminal && terminal_statuses.iter().all(Option::is_some) {
            return terminal_statuses
                .into_iter()
                .map(|status| status.context("terminal forge task status should be collected"))
                .collect();
        }

        thread::sleep(Duration::from_millis(25));
    }

    anyhow::bail!(
        "timed out waiting for {} forge task(s) to reach a terminal state",
        task_ids.len()
    )
}

fn assert_surface_role(response: &Value, expected_role: &str) -> Result<()> {
    let actual = response
        .get("result")
        .and_then(|value| value.get("entranceSurface"))
        .and_then(|value| value.get("actorRole"))
        .and_then(Value::as_str)
        .context("initialize response should report entranceSurface.actorRole")?;
    if actual != expected_role {
        anyhow::bail!("expected actor role `{expected_role}`, got `{actual}`");
    }
    Ok(())
}

fn json_array<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    path_value(value, path).and_then(Value::as_array)
}

fn json_i64(value: &Value, path: &[&str]) -> Option<i64> {
    path_value(value, path).and_then(Value::as_i64)
}

fn json_string(value: &Value, path: &[&str]) -> Option<String> {
    path_value(value, path)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn path_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn strip_prompt_fields(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("prompt".to_string(), Value::Null);
        if let Some(dispatch) = object.get_mut("dispatch").and_then(Value::as_object_mut) {
            dispatch.insert("prompt".to_string(), Value::Null);
        }
    }
    value
}

fn with_child_slot(mut value: Value, child_slot: &str) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "child_slot".to_string(),
            Value::String(child_slot.to_string()),
        );
    }
    value
}
