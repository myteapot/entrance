#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  entrance-auto/workflows/validation/run-local-mvp-demo.sh [--full-gates] [--run-id <id>] [--app-root <path>] [--report-dir <path>]

Runs the Entrance local MVP demo from a clean app root, then runs the
remote-fixture external issue/status/comment roundtrip. Outputs stay under
ignored entrance-auto/tmp and entrance-auto/reports paths by default.

Options:
  --full-gates        Also run cargo check/test, pnpm check/build, fmt check, and git diff check.
  --run-id <id>      Stable run id for report and app-root names.
  --app-root <path>  Override ENTRANCE_APP_ROOT for this run.
  --report-dir <dir> Override report output directory.
  -h, --help         Show this help.
USAGE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
SRC_DIR="$ROOT_DIR/entrance-src"

FULL_GATES=0
RUN_ID="${ENTRANCE_DEMO_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
APP_ROOT="${ENTRANCE_DEMO_APP_ROOT:-}"
REPORT_DIR="${ENTRANCE_DEMO_REPORT_DIR:-$ROOT_DIR/entrance-auto/reports}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --full-gates)
      FULL_GATES=1
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
FIXTURE_DEMO_JSON="$APP_ROOT/remote-fixture-demo.json"
ISSUE_BOARD_JSON="$APP_ROOT/issue-board.json"
CONNECTOR_QUEUE_JSON="$APP_ROOT/connector-queue.json"
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

node - "$LOCAL_DEMO_JSON" "$FIXTURE_DEMO_JSON" "$ISSUE_BOARD_JSON" "$CONNECTOR_QUEUE_JSON" "$REPORT_JSON" "$REPORT_MD" "$APP_ROOT" "$RUN_ID" "$SOURCE_COMMIT" "$FULL_GATES" <<'NODE'
const fs = require("fs");

const [
  localPath,
  fixturePath,
  boardPath,
  queuePath,
  reportJsonPath,
  reportMdPath,
  appRoot,
  runId,
  sourceCommit,
  fullGates,
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
const fixture = readJson(fixturePath);
const board = readJson(boardPath);
const queue = readJson(queuePath);
const boardIssues = (board.columns ?? []).flatMap((column) => column.issues ?? []);

assert(local.schema_version === "entrance.hive.loop_demo.compact.v1", "unexpected local demo schema");
assert(local.ready === true, "local demo did not report ready=true");
assert(local.loop?.schema_version === "entrance.hive.loop_start.compact.v1", "unexpected local loop schema");
assert(local.loop?.runtime === "local", "local loop did not use local runtime");
assert(local.loop?.status === "Done", "local loop did not end Done");
assert(local.loop?.decision === "keep", "local loop reviewer did not keep candidate");
assert(local.loop?.counts?.worker_ok === 3, "local loop did not record 3 ok workers");
assert(local.loop?.counts?.receipt_missing === 0, "local loop has missing receipts");

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

const summary = {
  schema_version: "entrance.auto.local_mvp_demo_report.v1",
  run_id: runId,
  generated_at: new Date().toISOString(),
  source_commit: sourceCommit,
  full_gates: fullGates === "1",
  app_root: appRoot,
  artifacts: {
    local_demo: localPath,
    remote_fixture_demo: fixturePath,
    issue_board: boardPath,
    connector_queue: queuePath,
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
  `- Remote fixture JSON: \`${summary.artifacts.remote_fixture_demo}\``,
  `- Issue board JSON: \`${summary.artifacts.issue_board}\``,
  `- Connector queue JSON: \`${summary.artifacts.connector_queue}\``,
  "",
].join("\n");

fs.writeFileSync(reportMdPath, md);
console.log(`validated local MVP + remote fixture demo`);
console.log(`report: ${reportJsonPath}`);
console.log(`summary: ${reportMdPath}`);
NODE

printf '\nEntrance local MVP demo validated.\n'
printf 'Report JSON: %s\n' "$REPORT_JSON"
printf 'Report Markdown: %s\n' "$REPORT_MD"
printf 'App root: %s\n' "$APP_ROOT"
