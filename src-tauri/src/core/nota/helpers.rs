use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use super::*;

pub(super) fn default_nota_dispatch_execution_host() -> String {
    NotaDispatchExecutionHost::InProcess.as_str().to_string()
}

pub(super) fn build_do_allocation_lineage_ref(transaction_id: i64, task_id: i64) -> String {
    build_nota_allocation_lineage_ref("do", transaction_id, task_id)
}

pub(super) fn build_dev_allocation_lineage_ref(transaction_id: i64, task_id: i64) -> String {
    build_nota_allocation_lineage_ref("dev", transaction_id, task_id)
}

fn build_nota_allocation_lineage_ref(
    surface_action: &str,
    transaction_id: i64,
    task_id: i64,
) -> String {
    format!("nota/{surface_action}/transaction/{transaction_id}/forge-task/{task_id}")
}

pub(super) fn normalize_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(super) fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn capture_repo_context(project_dir: &str) -> Result<RepoContext> {
    let project_path = Path::new(project_dir);
    let normalized_project_dir = project_path.to_string_lossy().replace('\\', "/");
    if !project_path.exists() {
        return Ok(RepoContext {
            project_dir: normalized_project_dir,
            git_branch: None,
            git_head: None,
        });
    }

    Ok(RepoContext {
        project_dir: normalized_project_dir,
        git_branch: run_git_command(project_path, &["rev-parse", "--abbrev-ref", "HEAD"]).ok(),
        git_head: run_git_command(project_path, &["rev-parse", "HEAD"]).ok(),
    })
}

fn run_git_command(project_path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .with_context(|| {
            format!(
                "failed to run git {} in {}",
                args.join(" "),
                project_path.display()
            )
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed in {}: {}",
            args.join(" "),
            project_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("git {} output was not valid UTF-8", args.join(" ")))?;
    Ok(value.trim().to_string())
}

pub(super) fn actor_role_slug(role: crate::core::action::ActorRole) -> &'static str {
    match role {
        crate::core::action::ActorRole::Nota => "nota",
        crate::core::action::ActorRole::Arch => "arch",
        crate::core::action::ActorRole::Dev => "dev",
        crate::core::action::ActorRole::Agent => "agent",
    }
}

pub(super) fn build_checkpoint_summary(stable_level: &str, landed: &[String]) -> String {
    match landed.first() {
        Some(first_landed) => format!("{stable_level}. Landed: {first_landed}"),
        None => stable_level.to_string(),
    }
}
