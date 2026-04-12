#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
manifest_path="$repo_root/hosts/release/reconciliation-batch-01.json"
output_dir="${VERIFY_V1_OUTPUT_DIR:-$repo_root/test-results/release-self-consistency}"
run_e2e=1

usage() {
  cat <<'USAGE'
Usage:
  hosts/release/verify-v1-self-consistency.sh [--manifest <path>] [--output-dir <path>] [--skip-e2e]

Environment:
  ENTRANCE_EXE_PATH   Optional path to an Entrance executable.
  VERIFY_V1_OUTPUT_DIR  Optional directory for captured JSON snapshots.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest)
      manifest_path="$2"
      shift 2
      ;;
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --skip-e2e)
      run_e2e=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

mkdir -p "$output_dir"

use_cargo=1
entrance_bin="${ENTRANCE_EXE_PATH:-}"
if [[ -n "$entrance_bin" ]]; then
  use_cargo=0
elif [[ -x "$repo_root/hosts/desktop/tauri/target/debug/entrance" ]]; then
  entrance_bin="$repo_root/hosts/desktop/tauri/target/debug/entrance"
  use_cargo=0
elif [[ -x "$repo_root/hosts/desktop/tauri/target/debug/entrance.exe" ]]; then
  entrance_bin="$repo_root/hosts/desktop/tauri/target/debug/entrance.exe"
  use_cargo=0
fi

run_entrance() {
  if [[ "$use_cargo" -eq 1 ]]; then
    cargo run --manifest-path "$repo_root/hosts/desktop/tauri/Cargo.toml" -- "$@"
  else
    "$entrance_bin" "$@"
  fi
}

assert_status() {
  python - "$output_dir/nota-status.json" <<'PY'
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
round_state = payload.get("round_state", {})
state = round_state.get("state")
carry = round_state.get("carry_forward_checkpointed")
if state != "fully_settled":
    raise SystemExit(f"expected round_state.state=fully_settled, got {state!r}")
if carry is not True:
    raise SystemExit(f"expected carry_forward_checkpointed=true, got {carry!r}")
PY
}

assert_invariants() {
  python - "$output_dir/nota-invariants.json" <<'PY'
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
failed = payload.get("failed_count")
if failed != 0:
    raise SystemExit(f"expected nota invariants failed_count=0, got {failed!r}")
PY
}

assert_repair() {
  python - "$output_dir/nota-repair.json" <<'PY'
import json
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
open_count = payload.get("open_count")
if open_count != 0:
    raise SystemExit(f"expected nota repair open_count=0, got {open_count!r}")
PY
}

assert_reconcile() {
  python - "$output_dir/landing-reconcile-report.json" "$output_dir/landing-planning.json" <<'PY'
import json
import sys

report = json.load(open(sys.argv[1], encoding="utf-8"))
planning = json.load(open(sys.argv[2], encoding="utf-8"))

unreconciled_count = report.get("unreconciled_count")
if unreconciled_count is None:
    raise SystemExit("landing reconcile report missing unreconciled_count")
if unreconciled_count > 38:
    raise SystemExit(f"expected unreconciled_count <= 38, got {unreconciled_count}")

required_keys = [
    "linear:microt:Entrance:issue:MYT-56",
    "linear:microt:Entrance:issue:MYT-61",
    "linear:microt:Entrance:issue:MYT-63",
    "linear:microt:Entrance:issue:MYT-64",
    "linear:microt:Entrance:issue:MYT-65",
]

index = {item.get("canonical_key"): item for item in planning if isinstance(item, dict)}
missing = [key for key in required_keys if key not in index]
if missing:
    raise SystemExit(f"missing planning items for keys: {', '.join(missing)}")

for key in required_keys:
    status = index[key].get("reconciliation_status")
    if status == "unreconciled":
        raise SystemExit(f"expected {key} to be reconciled, got unreconciled")
PY
}

ensure_rollup_native() {
  if [[ "$(uname -s)" != "Linux" ]]; then
    return 0
  fi

  if pnpm -C "$repo_root" exec vite --version >/dev/null 2>&1; then
    return 0
  fi

  echo "vite/rollup runtime probe failed; reinstalling dependencies" >&2
  rm -rf "$repo_root/node_modules"
  pnpm -C "$repo_root" install --frozen-lockfile
  pnpm -C "$repo_root" exec vite --version >/dev/null
}

echo "[verify] capturing runtime closure status"
run_entrance nota status > "$output_dir/nota-status.json"
assert_status

run_entrance nota invariants > "$output_dir/nota-invariants.json"
assert_invariants

run_entrance nota repair > "$output_dir/nota-repair.json"
assert_repair

echo "[verify] applying reconciliation batch"
run_entrance landing reconcile batch-apply --file "$manifest_path" > "$output_dir/landing-reconcile-batch-apply.json"
run_entrance landing reconcile report > "$output_dir/landing-reconcile-report.json"
run_entrance landing planning > "$output_dir/landing-planning.json"
assert_reconcile

echo "[verify] running type + rust baselines"
cargo test --manifest-path "$repo_root/hosts/desktop/tauri/Cargo.toml" --lib
pnpm -C "$repo_root" check

if [[ "$run_e2e" -eq 1 ]]; then
  echo "[verify] running browser e2e"
  ensure_rollup_native
  pnpm -C "$repo_root" test:e2e
fi

echo "[verify] complete; snapshots written to $output_dir"
