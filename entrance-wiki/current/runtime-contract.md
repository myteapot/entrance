# Entrance Runtime Contract

Run all commands from `entrance-src/`.

## CLI

```bash
cargo run -p entrance-app --bin entrance -- status
cargo run -p entrance-app --bin entrance -- drawer summary
cargo run -p entrance-app --bin entrance -- hive summary
cargo run -p entrance-app --bin entrance -- hive loop create --title "Local loop" --goal "Run the Hive loop MVP" --runtime codex
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime codex
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime codex --worker-timeout-secs 20 --worker-attempts 2
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime local --decision reject
cargo run -p entrance-app --bin entrance -- hive issue list
cargo run -p entrance-app --bin entrance -- hive issue show 1
cargo run -p entrance-app --bin entrance -- hive issue decide 1 request-review --body "Need human call"
cargo run -p entrance-app --bin entrance -- launcher list
```

`hive loop run` returns the local compiler trace for the round: policy rows,
versioned typed packet envelopes, versioned admission receipts, evidence, and
versioned verdict receipts.
Admission receipts include the packet receipt requirements, missing receipt
fields, and a boolean satisfied flag. Default MVP gates admit packets only when
their typed receipt requirements are present. Worker receipts are stricter than
plain presence checks: `role_worker` and `runtime_worker` must have `ok=true`
before the packet can pass admission.
`hive policy registry` is the current source for both typed admission gate
specs and runtime worker policy: supported runtimes, sandbox mode, timeout and
attempt bounds, env overrides, and required worker receipt metadata.
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
Loop audit includes a `worker_receipts` check that verifies worker receipts
carry bounded timeout and attempt metadata, plus a `runtime_policy` check that
verifies the contract runtime and current-round worker receipt kind/mode values
against the registry. The `verdict_packets` check verifies decision bindings,
required score-vector metrics, gate booleans, human options, and reason-code
evidence bindings. The `issue_surface` check verifies linked issue status,
typed comments, and operator comment/decision evidence so the control plane is
auditable instead of only visible.
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
Issue panel trace summaries are round-aware: they expose the current round,
current-round packet/admission/evidence/verdict counts, and total historical
counts so retries do not look like stale verdicts from the previous round.
Trace summaries also expose role-worker coverage and current-round worker
runtime totals so the Panel can show whether all role receipts in the current
round were produced successfully and how long the runtime spent.
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
