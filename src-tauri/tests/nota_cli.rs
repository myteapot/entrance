use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Result<Self> {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time should be after UNIX_EPOCH")?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("entrance-nota-cli-{name}-{suffix}"));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create temp dir at {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn nota_checkpoint_cli_persists_cadence_checkpoint_without_memory_fragment_fallback() -> Result<()>
{
    let temp_dir = TempDir::new("checkpoint")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let first_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "checkpoint",
            "--stable-level",
            "single-ingress, checkpointed, DB-first NOTA host",
            "--landed",
            "cadence object storage cut",
            "--remaining",
            "Do automatic checkpoint/receipt",
            "--remaining",
            "design-governance persistence",
            "--human-continuity-bus",
            "reduced but not eliminated",
            "--selected-trunk",
            "cadence storage cut",
            "--next-start-hint",
            "wire Do receipts",
        ],
    )?;
    let first: Value = serde_json::from_str(&first_output)
        .context("nota checkpoint output should be valid JSON")?;
    assert_eq!(first["checkpoint"]["cadence_kind"], "CADENCE_CHECKPOINT");
    assert_eq!(
        first["checkpoint"]["payload"]["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host"
    );
    assert_eq!(first["superseded_checkpoint_id"], Value::Null);

    let second_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "checkpoint",
            "--title",
            "Second checkpoint",
            "--stable-level",
            "single-ingress, checkpointed, DB-first NOTA host",
            "--landed",
            "cadence supersession relation",
            "--remaining",
            "Do automatic checkpoint/receipt",
            "--human-continuity-bus",
            "reduced but not eliminated",
            "--selected-trunk",
            "Do automatic checkpoint/receipt",
            "--next-start-hint",
            "persist runtime transactions",
        ],
    )?;
    let second: Value = serde_json::from_str(&second_output)
        .context("second nota checkpoint output should be valid JSON")?;
    assert_eq!(
        second["supersession_link"]["relation_type"],
        "superseded_by"
    );

    let list_output = run_nota_cli(&app_data_dir, &["nota", "checkpoints"])?;
    let listed: Value = serde_json::from_str(&list_output)
        .context("nota checkpoints output should be valid JSON")?;
    assert_eq!(listed["checkpoint_count"], 2);
    assert_eq!(listed["checkpoints"][0]["title"], "Second checkpoint");
    assert_eq!(listed["checkpoints"][0]["is_current"], true);
    assert_eq!(listed["checkpoints"][1]["is_current"], false);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(count_rows(&connection, "cadence_objects")?, 2);
    assert_eq!(count_rows(&connection, "cadence_links")?, 1);
    assert_eq!(count_rows(&connection, "memory_fragments")?, 0);

    Ok(())
}

#[test]
fn nota_do_cli_creates_runtime_transaction_receipts_and_checkpoint() -> Result<()> {
    let temp_dir = TempDir::new("do-dispatch")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_forge_app_state(&app_data_dir)?;

    let project_root = temp_dir.path().join("Entrance");
    let bootstrap_skill = project_root.join("harness").join("bootstrap").join("duet");
    fs::create_dir_all(&bootstrap_skill)?;
    fs::write(bootstrap_skill.join("SKILL.md"), "# test skill\n")?;

    let managed_worktree = app_data_dir
        .join("worktrees")
        .join("Entrance")
        .join("feat-MYT-48");
    fs::create_dir_all(&managed_worktree)?;
    init_git_repo(&managed_worktree)?;

    let fake_agent = temp_dir.path().join("fake-agent.cmd");
    fs::write(&fake_agent, "@echo off\r\nexit /b 0\r\n")?;

    let output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "do",
            "--project-dir",
            project_root
                .to_str()
                .context("project root should be valid UTF-8")?,
            "--model",
            "codex",
            "--agent-command",
            fake_agent
                .to_str()
                .context("fake agent path should be valid UTF-8")?,
            "--title",
            "Do dispatch MYT-48",
        ],
    )?;
    let report: Value =
        serde_json::from_str(&output).context("nota do output should be valid JSON")?;
    assert_eq!(report["transaction"]["surface_action"], "do");
    assert_eq!(
        report["transaction"]["transaction_kind"],
        "forge_agent_dispatch"
    );
    assert_eq!(report["dispatch"]["issue_id"], "MYT-48");
    assert_eq!(report["allocation"]["allocator_role"], "nota");
    assert_eq!(report["allocation"]["allocator_surface"], "nota_do");
    assert_eq!(
        report["allocation"]["allocation_kind"],
        "forge_agent_dispatch"
    );
    assert_eq!(
        report["allocation"]["source_transaction_id"],
        report["transaction"]["id"]
    );
    assert_eq!(report["allocation"]["child_execution_kind"], "forge_task");
    assert_eq!(
        report["allocation"]["return_target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        report["allocation"]["escalation_target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(report["checkpoint"]["cadence_kind"], "CADENCE_CHECKPOINT");
    assert_eq!(report["spawn_error"], Value::Null);
    assert_eq!(
        report["receipts"]
            .as_array()
            .context("receipts should be an array")?
            .len(),
        5
    );
    assert_eq!(report["receipts"][2]["receipt_kind"], "ALLOCATION_RECORDED");

    let transaction_id = report["transaction"]["id"]
        .as_i64()
        .context("transaction id should be present")?;
    let allocation_id = report["allocation"]["id"]
        .as_i64()
        .context("allocation id should be present")?;
    let lineage_ref = report["allocation"]["lineage_ref"]
        .as_str()
        .context("allocation lineage_ref should be present")?;
    let receipts_output = run_nota_cli(&app_data_dir, &["nota", "receipts"])?;
    let receipts: Value = serde_json::from_str(&receipts_output)
        .context("nota receipts output should be valid JSON")?;
    assert_eq!(receipts["receipt_count"], 5);
    assert!(receipts["requested_transaction_id"].is_null());
    assert_eq!(receipts["receipts"][0]["receipt_kind"], "DO_ACCEPTED");
    assert_eq!(
        receipts["receipts"][4]["receipt_kind"],
        "CADENCE_CHECKPOINT_WRITTEN"
    );

    let filtered_receipts_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "receipts",
            "--transaction-id",
            &transaction_id.to_string(),
        ],
    )?;
    let filtered_receipts: Value = serde_json::from_str(&filtered_receipts_output)
        .context("filtered nota receipts output should be valid JSON")?;
    assert_eq!(filtered_receipts["receipt_count"], 5);
    assert_eq!(
        filtered_receipts["requested_transaction_id"],
        transaction_id
    );

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    let task_id = report["task_id"]
        .as_i64()
        .context("task id should be present")?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = NULL, finished_at = NULL WHERE id = ?1",
        rusqlite::params![task_id, "Running"],
    )?;

    let transactions_output = run_nota_cli(&app_data_dir, &["nota", "transactions"])?;
    let transactions: Value = serde_json::from_str(&transactions_output)
        .context("nota transactions output should be valid JSON")?;
    assert_eq!(transactions["transaction_count"], 1);
    assert_eq!(transactions["transactions"][0]["surface_action"], "do");

    let allocations_output = run_nota_cli(&app_data_dir, &["nota", "allocations"])?;
    let allocations: Value = serde_json::from_str(&allocations_output)
        .context("nota allocations output should be valid JSON")?;
    assert_eq!(allocations["allocation_count"], 1);
    assert_eq!(
        allocations["allocations"][0]["source_transaction_id"],
        report["transaction"]["id"]
    );

    let overview_output = run_nota_cli(&app_data_dir, &["nota", "overview"])?;
    let overview: Value = serde_json::from_str(&overview_output)
        .context("nota overview output should be valid JSON")?;
    assert_eq!(overview["allocations"]["allocation_count"], 1);
    assert_eq!(
        overview["allocations"]["allocations"][0]["source_transaction_id"],
        report["transaction"]["id"]
    );
    assert!(overview["recommended_checkpoint"].is_null());

    assert_eq!(count_rows(&connection, "nota_runtime_transactions")?, 1);
    assert_eq!(count_rows(&connection, "nota_runtime_receipts")?, 5);
    assert_eq!(count_rows(&connection, "nota_runtime_allocations")?, 1);
    assert_eq!(count_rows(&connection, "cadence_objects")?, 1);
    assert_eq!(count_rows(&connection, "plugin_forge_tasks")?, 1);
    let allocation_boundary = connection.query_row(
        r#"
        SELECT
            source_transaction_id,
            child_execution_kind,
            return_target_kind,
            escalation_target_kind
        FROM nota_runtime_allocations
        "#,
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )?;
    assert_eq!(
        allocation_boundary.0,
        report["transaction"]["id"]
            .as_i64()
            .context("transaction id should be present")?
    );
    assert_eq!(allocation_boundary.1, "forge_task");
    assert_eq!(allocation_boundary.2, "nota_runtime_transaction");
    assert_eq!(allocation_boundary.3, "nota_runtime_transaction");

    let task_id = report["task_id"]
        .as_i64()
        .context("task id should be present")?;
    connection.execute(
        "UPDATE plugin_forge_tasks SET status = ?2, status_message = ?3, finished_at = ?4 WHERE id = ?1",
        rusqlite::params![
            task_id,
            "Blocked",
            "请先在 Vault 添加 openai",
            "2026-03-23T00:00:00Z"
        ],
    )?;

    let blocked_allocations_output = run_nota_cli(&app_data_dir, &["nota", "allocations"])?;
    let blocked_allocations: Value = serde_json::from_str(&blocked_allocations_output)
        .context("blocked nota allocations output should be valid JSON")?;
    assert_eq!(
        blocked_allocations["allocations"][0]["status"],
        "escalated_blocked"
    );
    let blocked_payload_json = blocked_allocations["allocations"][0]["payload_json"]
        .as_str()
        .context("allocation payload_json should be present")?;
    let blocked_payload: Value = serde_json::from_str(blocked_payload_json)
        .context("allocation payload_json should stay valid JSON")?;
    assert_eq!(
        blocked_payload["terminal_outcome"]["boundary_kind"],
        "escalation"
    );
    assert_eq!(
        blocked_payload["terminal_outcome"]["child_execution_status"],
        "Blocked"
    );
    assert_eq!(
        blocked_payload["terminal_outcome"]["child_execution_status_message"],
        "请先在 Vault 添加 openai"
    );
    assert_eq!(
        blocked_payload["terminal_outcome"]["target_kind"],
        "nota_runtime_transaction"
    );
    assert_eq!(
        blocked_payload["terminal_outcome"]["target_ref"],
        report["transaction"]["id"].to_string()
    );
    let blocked_message = blocked_payload["terminal_outcome"]["child_execution_status_message"]
        .as_str()
        .context("blocked terminal outcome message should be present")?;

    let blocked_receipts_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "receipts",
            "--transaction-id",
            &transaction_id.to_string(),
        ],
    )?;
    let blocked_receipts: Value = serde_json::from_str(&blocked_receipts_output)
        .context("blocked nota receipts output should be valid JSON")?;
    assert_eq!(blocked_receipts["receipt_count"], 6);
    assert_eq!(
        blocked_receipts["receipts"][5]["receipt_kind"],
        "ALLOCATION_TERMINAL_OUTCOME_RECORDED"
    );
    let blocked_receipt_payload_json = blocked_receipts["receipts"][5]["payload_json"]
        .as_str()
        .context("blocked receipt payload_json should be present")?;
    let blocked_receipt_payload: Value = serde_json::from_str(blocked_receipt_payload_json)
        .context("blocked receipt payload_json should stay valid JSON")?;
    assert_eq!(
        blocked_receipt_payload["lineage_ref"],
        report["allocation"]["lineage_ref"]
    );
    assert_eq!(blocked_receipt_payload["boundary_kind"], "escalation");
    assert_eq!(blocked_receipt_payload["child_execution_status"], "Blocked");
    assert_eq!(
        blocked_receipt_payload["child_execution_status_message"],
        "请先在 Vault 添加 openai"
    );
    assert_eq!(
        blocked_receipt_payload["target_ref"],
        report["transaction"]["id"].to_string()
    );

    let blocked_overview_output = run_nota_cli(&app_data_dir, &["nota", "overview"])?;
    let blocked_overview: Value = serde_json::from_str(&blocked_overview_output)
        .context("blocked nota overview output should be valid JSON")?;
    assert_eq!(
        blocked_overview["recommended_checkpoint"]["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host with single-lane honest allocator truth checkpointed into runtime continuity"
    );
    assert_eq!(
        blocked_overview["recommended_checkpoint"]["selected_trunk"],
        "single-lane honest allocator continuity"
    );
    assert_eq!(
        blocked_overview["recommended_checkpoint"]["landed"][0],
        format!(
            "Single-lane NOTA allocation {} preserves lineage {} from runtime transaction {} into Forge task {}.",
            allocation_id,
            lineage_ref,
            transaction_id,
            task_id
        )
    );
    assert_eq!(
        blocked_overview["recommended_checkpoint"]["landed"][2],
        format!(
            "Transaction {transaction_id} receipt history now has 6 receipts, with latest terminal receipt ALLOCATION_TERMINAL_OUTCOME_RECORDED capturing allocation {} back to nota_runtime_transaction {}.",
            allocation_id,
            transaction_id
        )
    );
    assert_eq!(
        blocked_overview["recommended_checkpoint"]["remaining"][0],
        format!(
            "L3 remains open until the current Blocked gate is cleared: {}.",
            blocked_message
        )
    );
    assert_eq!(
        blocked_overview["recommended_checkpoint"]["next_start_hints"][2],
        format!(
            "Treat lineage `{}` as the canonical single-lane allocator thread until the blocked gate is cleared.",
            lineage_ref
        )
    );

    let stored_allocation_outcome = connection.query_row(
        "SELECT status, payload_json FROM nota_runtime_allocations",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    assert_eq!(stored_allocation_outcome.0, "escalated_blocked");
    let stored_payload: Value = serde_json::from_str(&stored_allocation_outcome.1)
        .context("stored allocation payload_json should be valid JSON")?;
    assert_eq!(
        stored_payload["terminal_outcome"]["child_execution_status"],
        "Blocked"
    );

    Ok(())
}

#[test]
fn nota_decision_cli_persists_design_decisions_and_governance_links() -> Result<()> {
    let temp_dir = TempDir::new("design-decision")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let first_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "decision",
            "--title",
            "Chat and Do only",
            "--statement",
            "Human-facing surface should shrink to Chat / Do.",
            "--rationale",
            "Reduce ingress sprawl.",
            "--decision-type",
            "ui_surface",
            "--scope-type",
            "project",
            "--scope-ref",
            "Entrance",
            "--source-ref",
            "nota:test:first",
        ],
    )?;
    let first: Value = serde_json::from_str(&first_output)
        .context("first decision output should be valid JSON")?;
    let first_id = first["decision"]["id"]
        .as_i64()
        .context("first decision id should be present")?;

    let second_output = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "decision",
            "--title",
            "Cadence stays out of memory fragments",
            "--statement",
            "Cadence continuity must not be stored in memory_fragments.",
            "--rationale",
            "Continuity and memory curation are adjacent but distinct.",
            "--decision-type",
            "storage",
            "--scope-type",
            "project",
            "--scope-ref",
            "Entrance",
            "--source-ref",
            "nota:test:second",
            "--supersedes",
            &first_id.to_string(),
            "--conflicts-with",
            &first_id.to_string(),
        ],
    )?;
    let second: Value = serde_json::from_str(&second_output)
        .context("second decision output should be valid JSON")?;
    assert_eq!(second["links"].as_array().map(Vec::len), Some(2));

    let listed_output = run_nota_cli(&app_data_dir, &["nota", "decisions"])?;
    let listed: Value = serde_json::from_str(&listed_output)
        .context("nota decisions output should be valid JSON")?;
    assert_eq!(listed["decision_count"], 2);
    assert_eq!(listed["link_count"], 2);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(count_rows(&connection, "decisions")?, 2);
    assert_eq!(count_rows(&connection, "decision_links")?, 2);

    Ok(())
}

#[test]
fn nota_chat_archive_policy_and_capture_cli_keep_raw_chat_separate_from_decisions() -> Result<()> {
    let temp_dir = TempDir::new("chat-archive")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    let summary_policy = run_nota_cli(
        &app_data_dir,
        &["nota", "chat-policy", "--policy", "summary"],
    )?;
    let summary_policy: Value = serde_json::from_str(&summary_policy)
        .context("chat-policy summary output should be valid JSON")?;
    assert_eq!(summary_policy["setting"]["archive_policy"], "summary");

    let summary_capture = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "capture-chat",
            "--role",
            "human",
            "--content",
            "Raw chat should not be promoted into a design decision by default.",
        ],
    )?;
    let summary_capture: Value = serde_json::from_str(&summary_capture)
        .context("summary chat capture output should be valid JSON")?;
    assert_eq!(summary_capture["stored"], true);
    assert_eq!(summary_capture["record"]["capture_mode"], "summary_capture");
    assert_eq!(summary_capture["record"]["content"], "");

    run_nota_cli(&app_data_dir, &["nota", "chat-policy", "--policy", "full"])?;
    let full_capture = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "capture-chat",
            "--role",
            "nota",
            "--content",
            "Checkpoint created; next step is to inspect the transaction receipt.",
            "--summary",
            "Checkpoint created and receipt inspection is next.",
        ],
    )?;
    let full_capture: Value = serde_json::from_str(&full_capture)
        .context("full chat capture output should be valid JSON")?;
    assert_eq!(full_capture["record"]["capture_mode"], "raw_chat_capture");
    assert_eq!(
        full_capture["record"]["content"],
        "Checkpoint created; next step is to inspect the transaction receipt."
    );

    run_nota_cli(&app_data_dir, &["nota", "chat-policy", "--policy", "off"])?;
    let off_capture = run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "capture-chat",
            "--role",
            "human",
            "--content",
            "This one should not be archived because policy is off.",
        ],
    )?;
    let off_capture: Value = serde_json::from_str(&off_capture)
        .context("off chat capture output should be valid JSON")?;
    assert_eq!(off_capture["stored"], false);

    let listed_output = run_nota_cli(&app_data_dir, &["nota", "chat-captures"])?;
    let listed: Value = serde_json::from_str(&listed_output)
        .context("chat-captures output should be valid JSON")?;
    assert_eq!(listed["capture_count"], 2);

    let db_path = app_data_dir.join("entrance.db");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite database at {}", db_path.display()))?;
    assert_eq!(count_rows(&connection, "chat_archive_settings")?, 1);
    assert_eq!(count_rows(&connection, "chat_capture_records")?, 2);
    assert_eq!(count_rows(&connection, "decisions")?, 0);

    Ok(())
}

#[test]
fn nota_overview_cli_returns_db_first_continuity_bundle() -> Result<()> {
    let temp_dir = TempDir::new("overview")?;
    let app_data_dir = temp_dir.path().join("appdata");
    seed_app_state(&app_data_dir)?;

    run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "checkpoint",
            "--stable-level",
            "single-ingress, checkpointed, DB-first NOTA host",
            "--landed",
            "cadence cut landed",
            "--remaining",
            "headless continuity bundle",
            "--human-continuity-bus",
            "reduced but still present",
        ],
    )?;
    run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "decision",
            "--title",
            "Chat is the continuity surface",
            "--statement",
            "Chat should read the runtime DB continuity bundle instead of replaying raw chat.",
            "--rationale",
            "Resume should start from canonical runtime state.",
            "--decision-type",
            "ui_surface",
            "--scope-type",
            "project",
            "--scope-ref",
            "Entrance",
            "--source-ref",
            "nota:test:overview",
        ],
    )?;
    run_nota_cli(&app_data_dir, &["nota", "chat-policy", "--policy", "full"])?;
    run_nota_cli(
        &app_data_dir,
        &[
            "nota",
            "capture-chat",
            "--role",
            "nota",
            "--content",
            "Overview should expose checkpoint, decision, and archive state together.",
        ],
    )?;

    let output = run_nota_cli(&app_data_dir, &["nota", "overview"])?;
    let overview: Value =
        serde_json::from_str(&output).context("nota overview output should be valid JSON")?;
    assert_eq!(overview["checkpoints"]["checkpoint_count"], 1);
    assert_eq!(overview["decisions"]["decision_count"], 1);
    assert_eq!(overview["chat_captures"]["capture_count"], 1);
    assert_eq!(overview["transactions"]["transaction_count"], 0);
    assert_eq!(overview["allocations"]["allocation_count"], 0);
    assert_eq!(overview["chat_policy"]["setting"]["archive_policy"], "full");
    assert_eq!(
        overview["checkpoints"]["checkpoints"][0]["payload"]["stable_level"],
        "single-ingress, checkpointed, DB-first NOTA host"
    );

    Ok(())
}

fn seed_app_state(app_data_dir: &Path) -> Result<()> {
    fs::create_dir_all(app_data_dir)?;
    fs::write(
        app_data_dir.join("entrance.toml"),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = false

[plugins.launcher]
enabled = false
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = false
http_port = 9721

[plugins.vault]
enabled = false
"#,
    )?;

    Ok(())
}

fn seed_forge_app_state(app_data_dir: &Path) -> Result<()> {
    fs::create_dir_all(app_data_dir)?;
    fs::write(
        app_data_dir.join("entrance.toml"),
        r#"[core]
theme = "dark"
log_level = "info"
mcp_enabled = false

[plugins.launcher]
enabled = false
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = true
http_port = 9721

[plugins.vault]
enabled = false
"#,
    )?;

    Ok(())
}

fn init_git_repo(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(path)
        .output()
        .with_context(|| format!("failed to initialize git repo at {}", path.display()))?;

    if !output.status.success() {
        anyhow::bail!(
            "git init failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn run_nota_cli(app_data_dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_entrance"))
        .args(args)
        .env("ENTRANCE_APP_DATA_DIR", app_data_dir)
        .output()
        .with_context(|| format!("failed to spawn `entrance {}`", args.join(" ")))?;

    if !output.status.success() {
        anyhow::bail!(
            "`entrance {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    String::from_utf8(output.stdout).context("CLI stdout should be valid UTF-8")
}

fn count_rows(connection: &Connection, table: &str) -> Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(connection.query_row(&query, [], |row| row.get(0))?)
}
