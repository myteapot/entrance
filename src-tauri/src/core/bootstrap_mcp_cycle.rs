use std::{path::Path, thread, time::Duration};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::core::{action::ActorRole, mcp_stdio_client::SpawnedMcpStdioClient};

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
    pub agent_dispatch_surface: &'static str,
    pub agent_wait_mode: &'static str,
}

#[derive(Clone, Serialize)]
pub struct ForgeBootstrapMcpCycleReport {
    pub bootstrap_surface: ForgeBootstrapMcpSurfaceSummary,
    pub requested_agent_count: usize,
    pub shared_worktree_boundary: Option<String>,
    pub dev_assignment: Value,
    pub agent_prepare: Value,
    pub agent_dispatches: Vec<Value>,
    pub parent_status: Value,
}

pub fn run_forge_bootstrap_mcp_cycle(
    app_data_dir: &Path,
    options: ForgeBootstrapMcpCycleOptions,
) -> Result<ForgeBootstrapMcpCycleReport> {
    let mut arch_surface = SpawnedMcpStdioClient::spawn(app_data_dir, ActorRole::Arch)?;
    let initialize_arch = arch_surface.initialize()?;
    assert_surface_role(&initialize_arch, "arch")?;

    let mut dev_surface = SpawnedMcpStdioClient::spawn(app_data_dir, ActorRole::Dev)?;
    let initialize_dev = dev_surface.initialize()?;
    assert_surface_role(&initialize_dev, "dev")?;

    let project_arguments = project_dir_tool_arguments(options.project_dir.as_deref());
    let dev_assignment = arch_surface
        .call_tool("forge_verify_dev_dispatch", project_arguments.clone())?
        .get("structuredContent")
        .cloned()
        .context("forge_verify_dev_dispatch should return structuredContent")?;
    let parent_task_id = json_i64(&dev_assignment, &["task_id"])
        .context("forge_verify_dev_dispatch should return a parent task id")?;

    let prepared_agent = dev_surface
        .call_tool("forge_prepare_agent_dispatch", project_arguments)?
        .get("structuredContent")
        .cloned()
        .context("forge_prepare_agent_dispatch should return structuredContent")?;
    let issue_id = json_string(&prepared_agent, &["issue_id"])
        .context("forge_prepare_agent_dispatch should return issue_id")?;
    let worktree_path = json_string(&prepared_agent, &["worktree_path"])
        .context("forge_prepare_agent_dispatch should return worktree_path")?;
    let prompt = json_string(&prepared_agent, &["prompt"])
        .context("forge_prepare_agent_dispatch should return prompt")?;
    let shared_worktree_boundary = (options.agent_count > 1).then(|| {
        format!(
            "Current bootstrap cut fans out {count} agent children through one Dev surface, but all child agents still share resolved worktree `{worktree}`. This is transport-level fan-out, not a per-agent worktree allocator yet.",
            count = options.agent_count,
            worktree = worktree_path,
        )
    });

    let mut dispatched_agents = Vec::with_capacity(options.agent_count);
    let mut child_task_ids = Vec::with_capacity(options.agent_count);
    for index in 0..options.agent_count {
        let slot = format!("agent-{}", index + 1);
        let mut dispatch_arguments = json!({
            "issue_id": issue_id.clone(),
            "worktree_path": worktree_path.clone(),
            "model": options.model.clone(),
            "prompt": prompt.clone(),
            "parent_task_id": parent_task_id,
            "supervision_strategy": "one_for_one",
            "child_slot": slot,
        });
        if let Some(agent_command) = options.agent_command.as_ref() {
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
        dispatched_agents.push(dispatched_agent);
    }

    let child_statuses = wait_for_terminal_forge_tasks(&mut dev_surface, &child_task_ids)?;
    let parent_status = dev_surface
        .call_tool("forge_status", json!({ "task_id": parent_task_id }))?
        .get("structuredContent")
        .cloned()
        .context("forge_status should return structuredContent for parent task")?;
    let agent_dispatches = dispatched_agents
        .into_iter()
        .zip(child_statuses)
        .map(|(dispatch, final_status)| {
            json!({
                "dispatch": strip_prompt_fields(dispatch),
                "final_status": final_status,
            })
        })
        .collect();

    Ok(ForgeBootstrapMcpCycleReport {
        bootstrap_surface: ForgeBootstrapMcpSurfaceSummary {
            coordinator_role: "nota",
            arch_surface_role: "arch",
            dev_surface_role: "dev",
            dev_assignment_surface: "forge_verify_dev_dispatch",
            agent_dispatch_surface: "forge_dispatch_agent",
            agent_wait_mode: "fanout_then_wait",
        },
        requested_agent_count: options.agent_count,
        shared_worktree_boundary,
        dev_assignment: strip_prompt_fields(dev_assignment),
        agent_prepare: strip_prompt_fields(prepared_agent),
        agent_dispatches,
        parent_status,
    })
}

fn project_dir_tool_arguments(project_dir: Option<&str>) -> Value {
    let mut arguments = serde_json::Map::new();
    if let Some(project_dir) = project_dir {
        arguments.insert(
            "project_dir".to_string(),
            Value::String(project_dir.to_string()),
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
