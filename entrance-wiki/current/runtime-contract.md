# Entrance Runtime Contract

Run all commands from `entrance-src/`.

## CLI

```bash
cargo run -p entrance-app --bin entrance -- status
cargo run -p entrance-app --bin entrance -- drawer summary
cargo run -p entrance-app --bin entrance -- hive summary
cargo run -p entrance-app --bin entrance -- hive loop create --title "Local loop" --goal "Run the Hive loop MVP" --runtime codex --compact
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime codex
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime codex --worker-timeout-secs 20 --worker-attempts 2
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime codex --compact
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime local --decision reject
cargo run -p entrance-app --bin entrance -- hive issue list
cargo run -p entrance-app --bin entrance -- hive issue show 1
cargo run -p entrance-app --bin entrance -- hive issue decide 1 request-review --body "Need human call"
cargo run -p entrance-app --bin entrance -- launcher list
```

`hive loop run` returns the local compiler trace for the round: policy rows,
versioned typed packet envelopes, versioned admission receipts, evidence, and
versioned verdict receipts.
Use `--compact` on `hive loop create` to print the linked issue card and next
actions instead of the full empty loop report. Use `--compact` on
`hive loop run`, `hive issue run`, or `hive issue retry-run` when running
`codex`; the loop still records full worker transcripts in SQLite, but the CLI
prints the Doctor summary or compact issue card instead of the full report.
Pending Doctor next actions prefer the issue-first compact command
`hive issue run <id> --runtime <runtime> --compact` when a loop has a linked
issue, so operators stay on the issue/status/comment surface.
Admission receipts include the packet receipt requirements, missing receipt
fields, and a boolean satisfied flag. Default MVP gates admit packets only when
their typed receipt requirements are present. Worker receipts are stricter than
plain presence checks: `role_worker` and `runtime_worker` must have `ok=true`
before the packet can pass admission, and loop audit verifies the worker `role`
still matches the packet writer role.
`hive policy registry` is the current source for both typed admission gate
specs and runtime worker policy: supported runtimes, sandbox mode, timeout and
attempt bounds, env overrides, role binding, and required worker receipt
metadata.
Admission gate failures are recorded as rejected receipts and returned as
blocked verdicts/issues instead of escaping as raw CLI errors.
The MVP runtime set is `local` and `codex`; unsupported runtime names are
reported as blocked verdicts. The `codex` runtime uses a read-only
`codex exec` worker for each `Explorer`, `Doer`, and `Evaluator` role and
records stdout, stderr, and last-message transcript data in the stage evidence
for that role. Trace, Doctor, and Panel card summaries aggregate current-round
worker duration, timeouts, and retry exhaustion so slow codex runs are visible
without opening full transcripts.
Worker timeout defaults to 60 seconds, can be overridden with
`--worker-timeout-secs <n>` or `ENTRANCE_HIVE_WORKER_TIMEOUT_SECS`, and is
recorded on worker evidence so slow or timed-out codex runs are reviewable.
Worker attempts default to 1, can be overridden with `--worker-attempts <n>` or
`ENTRANCE_HIVE_WORKER_ATTEMPTS`, and are recorded as attempt count/max attempts
plus the raw attempt receipts on codex workers.
Loop audit includes a `stage_sequence` check that rejects duplicate role stages
in one round and verifies terminal loops still have the expected current-round
Explorer/Doer/Evaluator stages, a `stage_evidence` check that verifies each
expected stage has exactly one stage-bound evidence row with the expected kind,
a `packet_sequence` check that rejects duplicate route packets in one round, a
`worker_receipts` check that verifies worker receipts carry bounded timeout and
attempt metadata plus the expected role, and a `runtime_policy` check that
verifies the contract runtime and current-round worker receipt kind/mode/role
values against the registry and packet writer.
The `active_policy_registry` check verifies the canonical Explorer/Doer/Evaluator
route and gate contract, including gate expected object kind and required
receipts. The `admission_receipts` check verifies every packet has exactly one
admission and that the stored admission receipt still binds to the packet row,
policy route, gate spec, required receipt list, missing receipt list, gate
result, and final admitted/rejected result. The `verdict_packets` check verifies
terminal rounds have exactly one verdict, then checks decision bindings,
required score-vector metrics, gate booleans, human options, and reason-code
evidence bindings. It also binds standard verdict evidence back to round
evidence counts, runtime readiness, and the evaluator worker receipt, while
admission-rejection verdicts bind back to the rejected admission, packet, and
admission-rejection evidence row. The `issue_surface` check verifies linked
issue status, typed comments, operator comment/decision evidence, and
evidence-to-comment author/action/body bindings so the control plane is
auditable instead of only visible. Trace, Doctor, and Panel summaries expose
both failed audit check names and extracted audit failure detail codes, so an
operator can see the exact binding that broke without opening raw JSON first.
For evaluator-path testing, `hive loop run` accepts
`--decision keep|reject|needs-review|blocked`.
Human decisions are available through `hive issue decide <id>
<retry|request-review|cancel>` and are recorded as operator comments while
also moving the linked loop contract state.
For the issue-first control plane, `hive issue run <id>` runs a `Todo` issue
directly, and `hive issue retry-run <id> --body <note>` records the retry
decision before running the linked loop. Runtime overrides on a run are written
back to the loop contract so later audits inspect the actual runtime used.
`hive loop run` only starts work when the linked contract is in `todo`; for
running or terminal states it returns the current report without appending
duplicate packets, admissions, evidence, verdicts, or comments.
Loop audit also rejects packet replay if a duplicate packet appears through
external corruption or manual database edits.
Issue panel trace summaries are round-aware: they expose the current round,
current-round packet/admission/evidence/verdict counts, and total historical
counts so retries do not look like stale verdicts from the previous round.
Trace summaries also expose role-worker coverage and current-round worker
runtime totals so the Panel can show whether all role receipts in the current
round were produced successfully and how long the runtime spent. Operator
comments and decisions are summarized into a current-round operator trail plus
total operator event counts, making human retry, review, cancel, and comment
actions visible without opening the raw evidence stream.
Issue human options are status-aware: `Blocked` issues can retry, request
review, or cancel; `Needs Review` issues can retry or cancel; `Todo` issues can
be canceled before running; terminal human-canceled issues only allow comments.

## Daemon

```bash
cargo run -p entrance-app --bin entrance -- daemon
cargo run -p entrance-app --bin entrance -- daemon stdio
cargo run -p entrance-app --bin entrance -- daemon http
```

The stdio daemon accepts one JSON invoke request per line and returns one JSON
response per line. The HTTP daemon exposes:

- `GET /health`
- `POST /invoke`

## Config And Data

- Default app root: `~/.entrance`
- Override: `ENTRANCE_APP_ROOT`
- Config: `~/.entrance/entrance.toml`
- Database: `~/.entrance/data/entrance.db`
- Vault key: `~/.entrance/vault.key`
- Drawer filesystem root: configured by `[drawer].root`

Invalid config must fail startup instead of silently defaulting.
