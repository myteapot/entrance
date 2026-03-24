use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

#[test]
fn forge_bootstrap_skill_points_to_entrance_owned_dispatch_runtime() -> Result<()> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("src-tauri should live under the Entrance repo root")?
        .to_path_buf();
    let skill_path = repo_root
        .join("harness")
        .join("bootstrap")
        .join("duet")
        .join("SKILL.md");
    let contents = fs::read_to_string(&skill_path)
        .with_context(|| format!("failed to read bootstrap skill at {}", skill_path.display()))?;

    assert!(contents.contains("harness/bootstrap/duet/SKILL.md"));
    assert!(contents.contains("entrance forge prepare-dispatch"));
    assert!(contents.contains("entrance forge verify-dispatch"));
    assert!(contents.contains("%LOCALAPPDATA%/Entrance/worktrees/{project}/feat-{ISSUE}"));

    assert!(!contents.contains("**DB 查询**: `python .agents/nota/scripts/db.py doc list`"));
    assert!(!contents
        .contains("**Agent Prompt 生成**: `python .agents/nota/scripts/control.py prompt ...`"));
    assert!(!contents.contains("**Git 操作**: `python .agents/nota/scripts/control.py [check|init|commit|worktree|merge] ...`"));
    assert!(!contents.contains("- ❌ 不建 worktree、不生成 prompt、不跑 control.py"));
    assert!(!contents.contains("- 创建 Worktree (control.py worktree add)"));
    assert!(!contents.contains("- 生成 Agent Prompt (control.py prompt)"));
    assert!(!contents.contains("- 合并代码 (control.py merge)"));

    Ok(())
}

#[test]
fn arch_bootstrap_role_points_to_entrance_owned_dispatch_runtime() -> Result<()> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("src-tauri should live under the Entrance repo root")?
        .to_path_buf();
    let role_path = repo_root
        .join("harness")
        .join("bootstrap")
        .join("duet")
        .join("roles")
        .join("arch.md");
    let contents = fs::read_to_string(&role_path).with_context(|| {
        format!(
            "failed to read arch bootstrap role at {}",
            role_path.display()
        )
    })?;

    assert!(contents.contains("harness/bootstrap/nota/identity.md"));
    assert!(contents.contains("harness/bootstrap/nota/rules.md"));
    assert!(contents.contains("legacy `db.py` bridge"));
    assert!(contents.contains("entrance forge prepare-dispatch"));
    assert!(contents.contains("entrance forge verify-dispatch"));

    assert!(!contents.contains(".agents/nota/"));
    assert!(!contents.contains("control.py worktree add"));
    assert!(!contents.contains("control.py prompt"));
    assert!(!contents.contains("control.py dev-prompt"));
    assert!(!contents.contains("{control.py"));

    Ok(())
}
