#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  entrance-auto/workflows/validation/run-local-mvp-demo.sh [--full-gates] [--verify-golden|--update-golden] [--run-id <id>] [--app-root <path>] [--report-dir <path>] [--golden-dir <path>]

Runs the Entrance local MVP demo from a clean app root, then runs the
remote-fixture external issue/status/comment roundtrip. Outputs stay under
ignored entrance-auto/tmp and entrance-auto/reports paths by default.

Options:
  --full-gates        Also run cargo check/test, pnpm check/build, fmt check, and git diff check.
  --verify-golden     Compare normalized output contracts with tracked golden fixtures.
  --update-golden     Update tracked golden fixtures from this run.
  --run-id <id>      Stable run id for report and app-root names.
  --app-root <path>  Override ENTRANCE_APP_ROOT for this run.
  --report-dir <dir> Override report output directory.
  --golden-dir <dir> Override golden fixture directory.
  -h, --help         Show this help.
USAGE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
SRC_DIR="$ROOT_DIR/entrance-src"

FULL_GATES=0
GOLDEN_MODE="none"
RUN_ID="${ENTRANCE_DEMO_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
APP_ROOT="${ENTRANCE_DEMO_APP_ROOT:-}"
REPORT_DIR="${ENTRANCE_DEMO_REPORT_DIR:-$ROOT_DIR/entrance-auto/reports}"
GOLDEN_DIR="${ENTRANCE_DEMO_GOLDEN_DIR:-$ROOT_DIR/entrance-auto/fixtures/golden/local-mvp-demo}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --full-gates)
      FULL_GATES=1
      shift
      ;;
    --verify-golden)
      GOLDEN_MODE="verify"
      shift
      ;;
    --update-golden)
      GOLDEN_MODE="update"
      shift
      ;;
    --run-id)
      RUN_ID="${2:?missing value for --run-id}"
      shift 2
      ;;
    --app-root)
      APP_ROOT="${2:?missing value for --app-root}"
      shift 2
      ;;
    --report-dir)
      REPORT_DIR="${2:?missing value for --report-dir}"
      shift 2
      ;;
    --golden-dir)
      GOLDEN_DIR="${2:?missing value for --golden-dir}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

APP_ROOT="${APP_ROOT:-$ROOT_DIR/entrance-auto/tmp/local-mvp-demo-$RUN_ID}"
REPORT_JSON="$REPORT_DIR/local-mvp-demo-$RUN_ID.json"
REPORT_MD="$REPORT_DIR/local-mvp-demo-$RUN_ID.md"
LOCAL_DEMO_JSON="$APP_ROOT/local-mvp-demo.json"
LOOP_CONTROL_JSON="$APP_ROOT/loop-control.json"
FIXTURE_DEMO_JSON="$APP_ROOT/remote-fixture-demo.json"
ISSUE_BOARD_JSON="$APP_ROOT/issue-board.json"
CONNECTOR_QUEUE_JSON="$APP_ROOT/connector-queue.json"
NORMALIZED_DIR="$APP_ROOT/normalized"
EVIDENCE_TSV="$APP_ROOT/evidence.tsv"

mkdir -p "$APP_ROOT" "$REPORT_DIR"

run_step() {
  local name="$1"
  shift
  printf '==> %s\n' "$name" >&2
  "$@"
}

run_in_src() {
  local name="$1"
  shift
  printf '==> %s\n' "$name" >&2
  (cd "$SRC_DIR" && "$@")
}

SOURCE_COMMIT="$(git -C "$ROOT_DIR" rev-parse --short HEAD)"
ENTRANCE_BIN="$SRC_DIR/target/debug/entrance"

if [[ "$FULL_GATES" -eq 1 ]]; then
  run_in_src "cargo check --workspace" cargo check --workspace
  run_in_src "cargo test --workspace" cargo test --workspace
  run_in_src "pnpm check" pnpm check
  run_in_src "pnpm build" pnpm build
  run_in_src "cargo fmt --all --check" cargo fmt --all --check
  run_step "git diff --check" git -C "$ROOT_DIR" diff --check
fi

run_in_src "build entrance debug binary" cargo build -q -p entrance-app --bin entrance

run_step "local MVP loop demo" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive loop demo --runtime local --compact > "$LOCAL_DEMO_JSON"

LOCAL_LOOP_ID="$(node -e "const data = JSON.parse(require('fs').readFileSync(process.argv[1], 'utf8')); const id = data.loop && data.loop.loop_id; if (!id) process.exit(1); console.log(id);" "$LOCAL_DEMO_JSON")"

run_step "local MVP loop control packet" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive loop control "$LOCAL_LOOP_ID" > "$LOOP_CONTROL_JSON"

run_step "remote fixture connector demo" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive connector fixture-demo --compact > "$FIXTURE_DEMO_JSON"

run_step "issue board snapshot" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive issue list --compact > "$ISSUE_BOARD_JSON"

run_step "remote-fixture connector queue" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive connector queue --provider remote-fixture --compact > "$CONNECTOR_QUEUE_JSON"

if command -v sqlite3 >/dev/null 2>&1; then
  sqlite3 "$APP_ROOT/data/entrance.db" \
    "select id, kind, coalesce(json_extract(payload_json, '$.schema_version'), ''), summary from hive_loop_evidence order by id;" \
    > "$EVIDENCE_TSV"
fi

node - "$LOCAL_DEMO_JSON" "$LOOP_CONTROL_JSON" "$FIXTURE_DEMO_JSON" "$ISSUE_BOARD_JSON" "$CONNECTOR_QUEUE_JSON" "$REPORT_JSON" "$REPORT_MD" "$APP_ROOT" "$RUN_ID" "$SOURCE_COMMIT" "$FULL_GATES" "$NORMALIZED_DIR" "$GOLDEN_DIR" "$GOLDEN_MODE" <<'NODE'
const fs = require("fs");
const path = require("path");

const [
  localPath,
  loopControlPath,
  fixturePath,
  boardPath,
  queuePath,
  reportJsonPath,
  reportMdPath,
  appRoot,
  runId,
  sourceCommit,
  fullGates,
  normalizedDir,
  goldenDir,
  goldenMode,
] = process.argv.slice(2);

function readJson(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const local = readJson(localPath);
const loopControl = readJson(loopControlPath);
const fixture = readJson(fixturePath);
const board = readJson(boardPath);
const queue = readJson(queuePath);
const boardIssues = (board.columns ?? []).flatMap((column) => column.issues ?? []);
const boardColumns = board.columns ?? [];

assert(local.schema_version === "entrance.hive.loop_demo.compact.v1", "unexpected local demo schema");
assert(local.ready === true, "local demo did not report ready=true");
assert(local.loop?.schema_version === "entrance.hive.loop_start.compact.v1", "unexpected local loop schema");
assert(local.loop?.runtime === "local", "local loop did not use local runtime");
assert(local.loop?.status === "Done", "local loop did not end Done");
assert(local.loop?.decision === "keep", "local loop reviewer did not keep candidate");
assert(local.loop?.counts?.worker_ok === 3, "local loop did not record 3 ok workers");
assert(local.loop?.counts?.receipt_missing === 0, "local loop has missing receipts");

assert(loopControl.schema_version === "entrance.mcp.loop_control.v1", "unexpected loop control schema");
assert(loopControl.loop_id === local.loop?.loop_id, "loop control does not match local loop id");
assert(loopControl.state?.issue_id === local.loop?.issue_id, "loop control does not match local issue id");
assert(loopControl.state?.issue_status === "Done", "loop control issue status changed");
assert(loopControl.state?.loop_status === "kept", "loop control loop status changed");
assert(loopControl.state?.reviewer_decision === "keep", "loop control reviewer decision changed");
assert(loopControl.state?.reviewer_invalid_rounds_used === 0, "loop control reviewer invalid rounds changed");
assert(loopControl.state?.reviewer_invalid_round_budget === 3, "loop control reviewer budget changed");
assert(loopControl.state?.needs_human_decision === false, "loop control unexpectedly needs human decision");
assert(loopControl.reviewer_gate_surface?.role === "Reviewer", "loop control missing Reviewer gate surface");
assert(loopControl.reviewer_gate_surface?.gates?.runtime_preflight?.state === "admitted", "loop control runtime gate changed");
assert(loopControl.reviewer_gate_surface?.gates?.worker_lifecycle?.state === "succeeded", "loop control worker lifecycle gate changed");
assert(loopControl.reviewer_gate_surface?.gates?.evidence_manifest?.state === "ok", "loop control evidence gate changed");
assert(loopControl.reviewer_gate_surface?.target_drift_check?.state === "shallow", "loop control drift check state changed");

const scoreNames = new Set((loopControl.reviewer_gate_surface?.score_vector ?? []).map((item) => item.name));
for (const name of ["stage_completeness", "runtime_readiness", "evidence_presence", "admission_integrity"]) {
  assert(scoreNames.has(name), `loop control missing score ${name}`);
}
const optionKeys = new Set((loopControl.operator_decision_surface?.options ?? []).map((option) => option.key));
for (const key of ["A", "B", "C"]) {
  assert(optionKeys.has(key), `loop control missing operator option ${key}`);
}
assert(loopControl.operator_decision_surface?.primary_action === "inspect", "loop control primary action changed");
assert(loopControl.human_decision_boundary?.required === false, "loop control human decision boundary changed");
assert((loopControl.human_decision_boundary?.options ?? []).includes("comment"), "loop control missing comment option");
assert(Boolean(loopControl.resources?.loop_control), "loop control missing loop_control resource");
assert(Boolean(loopControl.resources?.issue_control), "loop control missing issue_control resource");
assert(Boolean(loopControl.resources?.transition_policy), "loop control missing transition_policy resource");

const stages = local.loop?.stages ?? [];
const expectedRoles = ["explorer", "developer", "reviewer"];
for (const role of expectedRoles) {
  const stage = stages.find((item) => item.role === role);
  assert(stage, `missing ${role} stage`);
  assert(stage.status === "done", `${role} stage did not finish`);
  assert(stage.admission === "admitted", `${role} stage was not admitted`);
  assert(stage.worker?.ok === true, `${role} worker was not ok`);
}

assert(fixture.schema_version === "entrance.hive.connector_fixture_demo.v1", "unexpected fixture demo schema");
assert(fixture.provider === "remote-fixture", "fixture demo did not use remote-fixture provider");
assert(fixture.review_surface === "remote-fixture:ENTRANCE-DEMO", "fixture demo used unexpected review surface");
assert(fixture.completed === true, "fixture demo did not complete");
assert(fixture.result === "completed", "fixture demo result is not completed");
assert(fixture.summary?.stage_count === 6, "fixture demo stage count changed");
assert(fixture.summary?.passed_stage_count === fixture.summary?.stage_count, "fixture demo did not pass all stages");
assert((fixture.summary?.failed_stages ?? []).length === 0, "fixture demo has failed stages");
assert(fixture.summary?.final_readback_passed === true, "fixture final readback did not pass");
assert((fixture.summary?.recorded_evidence_ids ?? []).length >= 2, "fixture demo did not record connector evidence");
assert(fixture.connector?.current === true, "fixture connector is not current");
assert(fixture.connector?.publish_required === false, "fixture connector still requires publish");

assert(board.schema_version === "entrance.hive.issue_board.compact.v1", "unexpected issue board schema");
assert(boardIssues.some((card) => card.id === local.loop?.issue_id && card.status === "Done"), "issue board missing done local MVP issue");
assert(boardIssues.some((card) => card.id === fixture.issue_id && card.status === "Todo"), "issue board missing fixture issue");

assert(queue.provider_filter === "remote-fixture", "connector queue provider filter changed");
assert(queue.publish_required_count === 0, "remote-fixture queue has publish-required items");
assert(queue.current_count >= 1, "remote-fixture queue has no current items");

const normalized = {
  "local-mvp-summary.json": {
    schema_version: "entrance.auto.golden.local_mvp_summary.v1",
    local_demo_schema: local.schema_version,
    ready: local.ready,
    loop: {
      schema_version: local.loop?.schema_version,
      runtime: local.loop?.runtime,
      status: local.loop?.status,
      decision: local.loop?.decision,
      reason_code: local.loop?.reason_code,
      health: local.loop?.health,
      worker_ok: local.loop?.counts?.worker_ok,
      workers: local.loop?.counts?.workers,
      receipt_required: local.loop?.counts?.receipt_required,
      receipt_missing: local.loop?.counts?.receipt_missing,
    },
    roles: expectedRoles.map((role) => {
      const stage = stages.find((item) => item.role === role);
      return {
        role,
        status: stage?.status,
        admission: stage?.admission,
        worker_kind: stage?.worker?.kind,
        worker_ok: stage?.worker?.ok,
      };
    }),
    panel_handoff: {
      api_url: local.panel?.api_url ?? null,
      daemon_command: local.panel?.daemon?.command ?? null,
      dev_server_url: local.panel?.dev_server?.url ?? null,
    },
  },
  "remote-fixture-summary.json": {
    schema_version: "entrance.auto.golden.remote_fixture_summary.v1",
    report_schema: fixture.schema_version,
    provider: fixture.provider,
    review_surface: fixture.review_surface,
    completed: fixture.completed,
    result: fixture.result,
    stage_count: fixture.summary?.stage_count,
    passed_stage_count: fixture.summary?.passed_stage_count,
    failed_stage_count: (fixture.summary?.failed_stages ?? []).length,
    recorded_evidence_count: (fixture.summary?.recorded_evidence_ids ?? []).length,
    remote_object_kind: fixture.summary?.remote_object_kind,
    final_readback_passed: fixture.summary?.final_readback_passed,
    connector: {
      provider: fixture.connector?.provider,
      review_surface: fixture.connector?.review_surface,
      current: fixture.connector?.current,
      publish_required: fixture.connector?.publish_required,
      reason: fixture.connector?.reason,
    },
    queue: {
      provider_filter: fixture.queue?.provider_filter,
      current_count: fixture.queue?.current_count,
      publish_required_count: fixture.queue?.publish_required_count,
    },
  },
  "loop-control-summary.json": {
    schema_version: "entrance.auto.golden.loop_control_summary.v1",
    control_schema: loopControl.schema_version,
    state: {
      issue_status: loopControl.state?.issue_status,
      loop_status: loopControl.state?.loop_status,
      active_phase: loopControl.state?.active_phase,
      current_round: loopControl.state?.current_round,
      reviewer_decision: loopControl.state?.reviewer_decision,
      reviewer_reason_code: loopControl.state?.reviewer_reason_code,
      reviewer_invalid_rounds_used: loopControl.state?.reviewer_invalid_rounds_used,
      reviewer_invalid_round_budget: loopControl.state?.reviewer_invalid_round_budget,
      reviewer_invalid_budget_exhausted: loopControl.state?.reviewer_invalid_budget_exhausted,
      needs_human_decision: loopControl.state?.needs_human_decision,
      primary_action: loopControl.state?.primary_action,
    },
    reviewer_gate_surface: {
      role: loopControl.reviewer_gate_surface?.role,
      runtime_preflight_state: loopControl.reviewer_gate_surface?.gates?.runtime_preflight?.state,
      runtime_gate: loopControl.reviewer_gate_surface?.gates?.runtime_preflight?.gate,
      runtime_passed: loopControl.reviewer_gate_surface?.gates?.runtime_preflight?.passed,
      worker_lifecycle_state: loopControl.reviewer_gate_surface?.gates?.worker_lifecycle?.state,
      observed_roles: loopControl.reviewer_gate_surface?.gates?.worker_lifecycle?.observed_roles ?? [],
      evidence_manifest_state: loopControl.reviewer_gate_surface?.gates?.evidence_manifest?.state,
      evidence_count: loopControl.reviewer_gate_surface?.gates?.evidence_manifest?.coverage?.evidence_count,
      digest_count: loopControl.reviewer_gate_surface?.gates?.evidence_manifest?.coverage?.digest_count,
      drift_state: loopControl.reviewer_gate_surface?.target_drift_check?.state,
      score_names: (loopControl.reviewer_gate_surface?.score_vector ?? []).map((item) => item.name),
    },
    operator_decision_surface: {
      primary_action: loopControl.operator_decision_surface?.primary_action,
      blocked_fallback_active: loopControl.operator_decision_surface?.blocked_fallback?.active,
      blocked_fallback_status: loopControl.operator_decision_surface?.blocked_fallback?.status,
      options: (loopControl.operator_decision_surface?.options ?? []).map((option) => ({
        key: option.key,
        label: option.label,
        enabled: option.enabled,
        tool: option.tool ?? null,
      })),
    },
    human_decision_boundary: {
      required: loopControl.human_decision_boundary?.required,
      issue_status: loopControl.human_decision_boundary?.issue_status,
      options: loopControl.human_decision_boundary?.options ?? [],
      confirmation_arg: loopControl.human_decision_boundary?.confirmation_arg,
    },
    resources: {
      loop_control: Boolean(loopControl.resources?.loop_control),
      issue_control: Boolean(loopControl.resources?.issue_control),
      transition_policy: Boolean(loopControl.resources?.transition_policy),
      review_queue: Boolean(loopControl.resources?.review_queue),
    },
  },
  "issue-board-summary.json": {
    schema_version: "entrance.auto.golden.issue_board_summary.v1",
    board_schema: board.schema_version,
    total: board.total,
    columns: boardColumns.map((column) => ({
      status: column.status,
      count: column.count,
      issues: (column.issues ?? []).map((card) => ({
        title: card.title,
        status: card.status,
        summary: card.summary,
        connector_provider: card.connector?.provider ?? null,
        connector_current: card.connector?.current ?? null,
        action_labels: (card.actions ?? []).map((action) => action.label),
        human_options: card.trace?.human_options ?? [],
      })),
    })),
  },
  "remote-fixture-queue-summary.json": {
    schema_version: "entrance.auto.golden.remote_fixture_queue_summary.v1",
    queue_schema: queue.schema_version ?? null,
    provider_filter: queue.provider_filter,
    provider_known: queue.provider_known,
    current_count: queue.current_count,
    publish_required_count: queue.publish_required_count,
    providers: (queue.providers ?? []).map((provider) => ({
      provider: provider.provider ?? provider.adapter?.provider ?? null,
      display_name: provider.display_name,
      configured: provider.configured,
      admission_status: provider.admission_status,
      current_count: provider.current_count,
      publish_required_count: provider.publish_required_count,
      adapter_status: provider.adapter?.status,
      adapter_mode: provider.adapter?.mode,
      supports_publish: provider.adapter?.supports_publish,
      supports_readback: provider.adapter?.supports_readback,
      supports_admission: provider.adapter?.supports_admission,
    })),
  },
};

fs.mkdirSync(normalizedDir, { recursive: true });
for (const [fileName, value] of Object.entries(normalized)) {
  fs.writeFileSync(path.join(normalizedDir, fileName), `${JSON.stringify(value, null, 2)}\n`);
}

const goldenFiles = Object.keys(normalized).sort();
if (goldenMode === "update") {
  fs.mkdirSync(goldenDir, { recursive: true });
  for (const fileName of goldenFiles) {
    fs.copyFileSync(path.join(normalizedDir, fileName), path.join(goldenDir, fileName));
  }
} else if (goldenMode === "verify") {
  for (const fileName of goldenFiles) {
    const actual = fs.readFileSync(path.join(normalizedDir, fileName), "utf8");
    const expectedPath = path.join(goldenDir, fileName);
    assert(fs.existsSync(expectedPath), `missing golden fixture ${expectedPath}`);
    const expected = fs.readFileSync(expectedPath, "utf8");
    assert(actual === expected, `golden fixture drift: ${fileName}`);
  }
} else if (goldenMode !== "none") {
  throw new Error(`unsupported golden mode: ${goldenMode}`);
}

const summary = {
  schema_version: "entrance.auto.local_mvp_demo_report.v1",
  run_id: runId,
  generated_at: new Date().toISOString(),
  source_commit: sourceCommit,
  full_gates: fullGates === "1",
  app_root: appRoot,
  artifacts: {
    local_demo: localPath,
    loop_control: loopControlPath,
    remote_fixture_demo: fixturePath,
    issue_board: boardPath,
    connector_queue: queuePath,
    normalized_dir: normalizedDir,
    golden_dir: goldenMode === "none" ? null : goldenDir,
  },
  golden: {
    mode: goldenMode,
    files: goldenFiles,
  },
  local_mvp: {
    schema_version: local.schema_version,
    ready: local.ready,
    loop_id: local.loop?.loop_id,
    issue_id: local.loop?.issue_id,
    status: local.loop?.status,
    decision: local.loop?.decision,
    runtime: local.loop?.runtime,
    worker_ok: local.loop?.counts?.worker_ok,
    receipt_missing: local.loop?.counts?.receipt_missing,
    roles: expectedRoles.map((role) => {
      const stage = stages.find((item) => item.role === role);
      return {
        role,
        status: stage?.status,
        admission: stage?.admission,
        worker_ok: stage?.worker?.ok,
      };
    }),
  },
  loop_control: {
    schema_version: loopControl.schema_version,
    issue_status: loopControl.state?.issue_status,
    loop_status: loopControl.state?.loop_status,
    reviewer_decision: loopControl.state?.reviewer_decision,
    reviewer_reason_code: loopControl.state?.reviewer_reason_code,
    reviewer_invalid_rounds_used: loopControl.state?.reviewer_invalid_rounds_used,
    reviewer_invalid_round_budget: loopControl.state?.reviewer_invalid_round_budget,
    needs_human_decision: loopControl.state?.needs_human_decision,
    primary_action: loopControl.operator_decision_surface?.primary_action,
    operator_options: (loopControl.operator_decision_surface?.options ?? []).map((option) => option.key),
    score_names: (loopControl.reviewer_gate_surface?.score_vector ?? []).map((item) => item.name),
  },
  remote_fixture: {
    schema_version: fixture.schema_version,
    provider: fixture.provider,
    review_surface: fixture.review_surface,
    completed: fixture.completed,
    result: fixture.result,
    issue_id: fixture.issue_id,
    stage_count: fixture.summary?.stage_count,
    passed_stage_count: fixture.summary?.passed_stage_count,
    recorded_evidence_ids: fixture.summary?.recorded_evidence_ids ?? [],
    final_readback_passed: fixture.summary?.final_readback_passed,
    connector_current: fixture.connector?.current,
  },
  board: {
    schema_version: board.schema_version,
    issue_count: boardIssues.length,
    review_queue_count: (board.review_queue ?? []).length,
  },
  connector_queue: {
    schema_version: queue.schema_version ?? null,
    provider_filter: queue.provider_filter,
    current_count: queue.current_count,
    publish_required_count: queue.publish_required_count,
  },
  panel_handoff: local.panel ?? null,
};

fs.writeFileSync(reportJsonPath, `${JSON.stringify(summary, null, 2)}\n`);

const md = [
  "# Entrance Local MVP Demo Report",
  "",
  `- Run id: ${summary.run_id}`,
  `- Source commit: ${summary.source_commit}`,
  `- App root: \`${summary.app_root}\``,
  `- Full gates: ${summary.full_gates ? "yes" : "no"}`,
  `- Golden mode: ${summary.golden.mode}`,
  "",
  "## Local MVP",
  "",
  `- Issue: #${summary.local_mvp.issue_id}`,
  `- Loop: #${summary.local_mvp.loop_id}`,
  `- Status: ${summary.local_mvp.status}`,
  `- Reviewer decision: ${summary.local_mvp.decision}`,
  `- Runtime: ${summary.local_mvp.runtime}`,
  `- Workers ok: ${summary.local_mvp.worker_ok}`,
  `- Missing receipts: ${summary.local_mvp.receipt_missing}`,
  "",
  "## Loop Control",
  "",
  `- Schema: ${summary.loop_control.schema_version}`,
  `- Issue status: ${summary.loop_control.issue_status}`,
  `- Loop status: ${summary.loop_control.loop_status}`,
  `- Reviewer decision: ${summary.loop_control.reviewer_decision}`,
  `- Reviewer budget: ${summary.loop_control.reviewer_invalid_rounds_used}/${summary.loop_control.reviewer_invalid_round_budget}`,
  `- Human decision required: ${summary.loop_control.needs_human_decision}`,
  `- Operator options: ${summary.loop_control.operator_options.join(", ")}`,
  "",
  "## Remote Fixture",
  "",
  `- Issue: #${summary.remote_fixture.issue_id}`,
  `- Review surface: ${summary.remote_fixture.review_surface}`,
  `- Result: ${summary.remote_fixture.result}`,
  `- Stages: ${summary.remote_fixture.passed_stage_count}/${summary.remote_fixture.stage_count}`,
  `- Evidence: ${summary.remote_fixture.recorded_evidence_ids.map((id) => `E#${id}`).join(", ")}`,
  `- Connector current: ${summary.remote_fixture.connector_current}`,
  "",
  "## Panel Handoff",
  "",
  summary.panel_handoff?.daemon?.command
    ? `- Daemon: \`ENTRANCE_APP_ROOT=${summary.app_root} ${summary.panel_handoff.daemon.command}\``
    : "- Daemon: unavailable",
  summary.panel_handoff?.dev_server?.command
    ? `- Dev server: \`${summary.panel_handoff.dev_server.command}\``
    : "- Dev server: unavailable",
  summary.panel_handoff?.dev_server?.url
    ? `- URL: ${summary.panel_handoff.dev_server.url}`
    : "- URL: unavailable",
  "",
  "## Artifacts",
  "",
  `- Local demo JSON: \`${summary.artifacts.local_demo}\``,
  `- Loop control JSON: \`${summary.artifacts.loop_control}\``,
  `- Remote fixture JSON: \`${summary.artifacts.remote_fixture_demo}\``,
  `- Issue board JSON: \`${summary.artifacts.issue_board}\``,
  `- Connector queue JSON: \`${summary.artifacts.connector_queue}\``,
  `- Normalized snapshots: \`${summary.artifacts.normalized_dir}\``,
  summary.artifacts.golden_dir ? `- Golden fixtures: \`${summary.artifacts.golden_dir}\`` : null,
  "",
].filter((line) => line !== null).join("\n");

fs.writeFileSync(reportMdPath, md);
console.log(`validated local MVP + remote fixture demo`);
if (goldenMode === "update") {
  console.log(`updated golden fixtures: ${goldenDir}`);
} else if (goldenMode === "verify") {
  console.log(`verified golden fixtures: ${goldenDir}`);
}
console.log(`report: ${reportJsonPath}`);
console.log(`summary: ${reportMdPath}`);
NODE

printf '\nEntrance local MVP demo validated.\n'
printf 'Report JSON: %s\n' "$REPORT_JSON"
printf 'Report Markdown: %s\n' "$REPORT_MD"
printf 'App root: %s\n' "$APP_ROOT"
