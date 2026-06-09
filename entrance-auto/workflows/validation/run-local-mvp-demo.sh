#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  entrance-auto/workflows/validation/run-local-mvp-demo.sh [--full-gates] [--verify-golden|--update-golden] [--run-id <id>] [--app-root <path>] [--report-dir <path>] [--golden-dir <path>]

Runs the Entrance local issue-workbench MVP from a clean app root. Outputs stay
under ignored entrance-auto/tmp and entrance-auto/reports paths by default.
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
    --full-gates) FULL_GATES=1; shift ;;
    --verify-golden) GOLDEN_MODE="verify"; shift ;;
    --update-golden) GOLDEN_MODE="update"; shift ;;
    --run-id) RUN_ID="${2:?missing value for --run-id}"; shift 2 ;;
    --app-root) APP_ROOT="${2:?missing value for --app-root}"; shift 2 ;;
    --report-dir) REPORT_DIR="${2:?missing value for --report-dir}"; shift 2 ;;
    --golden-dir) GOLDEN_DIR="${2:?missing value for --golden-dir}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

APP_ROOT="${APP_ROOT:-$ROOT_DIR/entrance-auto/tmp/local-mvp-demo-$RUN_ID}"
REPORT_JSON="$REPORT_DIR/local-mvp-demo-$RUN_ID.json"
REPORT_MD="$REPORT_DIR/local-mvp-demo-$RUN_ID.md"
ISSUE_CREATE_JSON="$APP_ROOT/issue-create.json"
LOCAL_DEMO_JSON="$APP_ROOT/local-mvp-demo.json"
LOOP_CONTROL_JSON="$APP_ROOT/loop-control.json"
ISSUE_BOARD_JSON="$APP_ROOT/issue-board.json"
NORMALIZED_DIR="$APP_ROOT/normalized"
mkdir -p "$APP_ROOT" "$REPORT_DIR"

run_step() { local name="$1"; shift; printf '==> %s\n' "$name" >&2; "$@"; }
run_in_src() { local name="$1"; shift; printf '==> %s\n' "$name" >&2; (cd "$SRC_DIR" && "$@"); }

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

run_step "create local issue loop" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive issue create --title "Entrance local MVP" --goal "Run the local Developer -> Reviewer loop." --runtime local --compact > "$ISSUE_CREATE_JSON"

ISSUE_ID="$(node -e "const d=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8')); console.log(d.issue?.id ?? d.issue?.issue?.id ?? d.loop?.issue_id ?? d.issue?.issue_id);" "$ISSUE_CREATE_JSON")"
LOOP_ID="$(node -e "const d=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8')); console.log(d.loop?.id ?? d.loop?.loop_id ?? d.issue?.loop_id);" "$ISSUE_CREATE_JSON")"

run_step "run local issue loop" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive issue run "$ISSUE_ID" --runtime local --compact > "$LOCAL_DEMO_JSON"

run_step "local loop control packet" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive loop control "$LOOP_ID" > "$LOOP_CONTROL_JSON"

run_step "issue board snapshot" env ENTRANCE_APP_ROOT="$APP_ROOT" \
  "$ENTRANCE_BIN" hive issue list --compact > "$ISSUE_BOARD_JSON"

node - "$ISSUE_CREATE_JSON" "$LOCAL_DEMO_JSON" "$LOOP_CONTROL_JSON" "$ISSUE_BOARD_JSON" "$REPORT_JSON" "$REPORT_MD" "$APP_ROOT" "$RUN_ID" "$SOURCE_COMMIT" "$FULL_GATES" "$NORMALIZED_DIR" "$GOLDEN_DIR" "$GOLDEN_MODE" <<'NODE'
const fs = require("fs");
const path = require("path");
const [createPath, runPath, controlPath, boardPath, reportJsonPath, reportMdPath, appRoot, runId, sourceCommit, fullGates, normalizedDir, goldenDir, goldenMode] = process.argv.slice(2);
const readJson = (p) => JSON.parse(fs.readFileSync(p, "utf8"));
const assert = (ok, msg) => { if (!ok) throw new Error(msg); };
const created = readJson(createPath);
const run = readJson(runPath);
const control = readJson(controlPath);
const board = readJson(boardPath);
const issueId = created.issue?.id ?? created.issue?.issue?.id ?? created.loop?.issue_id ?? created.issue?.issue_id;
const loopId = created.loop?.id ?? created.loop?.loop_id ?? created.issue?.loop_id;
assert(Number.isInteger(issueId), "missing issue id");
assert(Number.isInteger(loopId), "missing loop id");
assert(run.loop?.status === "Done" || run.loop?.status === "kept", "local loop did not finish Done/kept");
assert(control.schema_version === "entrance.mcp.loop_control.v1", "unexpected loop control schema");
assert(board.schema_version === "entrance.hive.issue_board.compact.v1", "unexpected board schema");
const normalized = {
  "local-mvp-summary.json": {
    schema_version: "entrance.auto.golden.local_mvp_summary.v2",
    issue_id: issueId,
    loop_id: loopId,
    run_schema: run.schema_version,
    loop_status: run.loop?.status,
    issue_status: run.issue?.status ?? run.issue?.issue?.status ?? null,
  },
  "loop-control-summary.json": {
    schema_version: "entrance.auto.golden.loop_control_summary.v2",
    control_schema: control.schema_version,
    loop_id: control.loop?.id ?? loopId,
    loop_status: control.loop?.status,
    issue_status: control.issue?.status ?? control.issue?.issue?.status ?? null,
  },
  "issue-board-summary.json": {
    schema_version: "entrance.auto.golden.issue_board_summary.v2",
    board_schema: board.schema_version,
    counts: board.counts,
    issue_count: board.issues?.length ?? 0,
  },
};
fs.mkdirSync(normalizedDir, { recursive: true });
for (const [name, value] of Object.entries(normalized)) fs.writeFileSync(path.join(normalizedDir, name), `${JSON.stringify(value, null, 2)}\n`);
const files = Object.keys(normalized).sort();
if (goldenMode === "update") {
  fs.mkdirSync(goldenDir, { recursive: true });
  for (const name of files) fs.copyFileSync(path.join(normalizedDir, name), path.join(goldenDir, name));
} else if (goldenMode === "verify") {
  for (const name of files) {
    const expected = path.join(goldenDir, name);
    assert(fs.existsSync(expected), `missing golden ${name}`);
    assert(fs.readFileSync(expected, "utf8") === fs.readFileSync(path.join(normalizedDir, name), "utf8"), `golden mismatch ${name}`);
  }
}
const report = { schema_version: "entrance.auto.local_mvp_demo.v2", run_id: runId, generated_at: new Date().toISOString(), source_commit: sourceCommit, full_gates: fullGates === "1", app_root: appRoot, issue_id: issueId, loop_id: loopId, artifacts: { create: createPath, run: runPath, loop_control: controlPath, issue_board: boardPath, normalized: normalizedDir } };
fs.writeFileSync(reportJsonPath, `${JSON.stringify(report, null, 2)}\n`);
fs.writeFileSync(reportMdPath, [`# Local MVP Demo`, ``, `- Run id: ${runId}`, `- Issue: #${issueId}`, `- Loop: #${loopId}`, `- App root: \`${appRoot}\``, `- Report JSON: \`${reportJsonPath}\``].join("\n") + "\n");
NODE

printf 'Wrote %s\n' "$REPORT_JSON"
printf 'Wrote %s\n' "$REPORT_MD"
