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
cargo run -p entrance-app --bin entrance -- hive connector registry --compact
cargo run -p entrance-app --bin entrance -- hive connector queue --compact
cargo run -p entrance-app --bin entrance -- hive connector queue --provider linear --compact
cargo run -p entrance-app --bin entrance -- hive connector publish-plan --compact
cargo run -p entrance-app --bin entrance -- hive connector publish-execute --plan-id <sha256> --compact
cargo run -p entrance-app --bin entrance -- hive issue connector-admission 1 --compact
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
Compact issue surfaces also expose connector mirror drift: `hive issue show
<id> --compact` includes a `connector` block, while `hive issue list --compact`
adds a `connector_queue` with publish-required issue ids and commands.
The connector registry is available through `hive connector registry --compact`;
it distinguishes active local/file providers from configured remote GitHub/Linear
issue providers, names the admission gate, and exposes provider-specific
admission status/blockers. `hive connector queue --compact` returns a provider-scoped
publish queue, and `--provider <name>` narrows the dry-run plan to one
issue-surface provider. `hive connector publish-plan --compact` creates a
digest-bound local mirror publish plan from the current queue and the provider
writer adapter contract; `hive connector publish-execute --plan-id <sha256>
--compact` recomputes that plan and refuses to execute if the queue, issue
mirror digest, or provider writer capability changed. Successful execution
records a typed connector publish comment/evidence on each issue before writing
the mirror, and each publish returns a typed connector write receipt with
adapter, status, comment surface, digest, and readback command. `hive issue
mirror-roundtrip <id> --compact` wraps the issue-scoped publish -> readback ->
admission workflow into one typed report; recorded readback/admission
observations are republished before the final readback check. `hive connector
roundtrip-plan --compact` creates a digest-bound queue plan for that same
roundtrip operation, including provider writer/readback/admission blockers, and
`hive connector roundtrip-execute --plan-id <sha256> --compact` recomputes the
plan before executing it. Successful queue execution records a typed
`connector_roundtrip_execute` comment/evidence on every issue, then runs the
issue-scoped roundtrip and returns a compact per-issue completion summary. The
active
`remote-fixture:` provider is a file-backed remote issue API fixture that writes
`entrance.hive.connector_remote_write_receipt.v1` and verifies
`entrance.hive.connector_remote_readback.v1` without contacting a third-party
service. Remote GitHub/Linear providers remain visible with adapter blockers
until configured, and active providers expose an
`entrance.hive.connector_remote_contract.v1` that specifies remote object kind,
write receipt schema, readback schema, idempotency key parts, auth env, and
required pre/post-write checks. Their admission previews also include
`entrance.hive.connector_remote_target.v1`, parsed from provider-specific review
surfaces such as `github:owner/repo#123`,
`github:https://github.com/owner/repo/issues/123`, `linear:TEAM-123`, or a
Linear issue URL. Invalid targets surface typed blockers such as
`remote_target_invalid`, `github_owner_missing`, or `github_repo_missing` before
any remote writer can be admitted. The Panel displays those parsed targets as
connector target chips in the issue detail, board card, and connector queue
surfaces, using warning styling for invalid targets. Queue and publish-plan
issues also expose `entrance.hive.connector_remote_write_plan.v1`, a typed
provider request envelope with auth expectations, source issue fields,
HTTP/GraphQL/file operations, receipt/readback schemas, and blockers. GitHub
plans enumerate REST issue/comment operations, Linear plans enumerate GraphQL
issue/comment operations, and inactive or unsupported providers remain blocked
at the plan boundary. The Panel renders those envelopes as remote write-plan
chips in the issue detail, board card, and connector queue without implying that
the third-party write has executed. `hive issue
connector-admission <id>
--compact` is the issue-scoped dry-run for routing a current mirror to
`external_issue_surface`; it now emits a typed provider check vector, writer
adapter blockers, and any remote contract so rejected admissions can be traced
to provider readiness, mirror readback, or remote-write requirements.
Provider overrides are read from `entrance.toml` under `[connectors.<provider>]`.
GitHub and Linear both have guarded remote publish/readback slices when enabled
with a configured token env. `[connectors.github] enabled = true` with
`GITHUB_TOKEN` or `GH_TOKEN` activates REST issue/comment publish operations;
`[connectors.linear] enabled = true` with an env such as `LINEAR_API_KEY`
activates GraphQL issue/comment publish operations. Both record
`entrance.hive.connector_remote_write_execute.v1` plus redacted
`entrance.hive.connector_remote_write_receipt.v1` evidence. GitHub comment
publish uses an Entrance idempotency marker: it lists issue comments, patches
the matching comment when present, and creates one only when the marker is
absent. GitHub readback uses REST `GET` issue plus `GET` issue comments, follows
`Link` pagination for the comment list, emits
`entrance.hive.connector_remote_readback.v1`, and connector admission is ready
only when the typed target, auth, issue state/body, latest comment, and
write-receipt binding checks pass. Linear publish reads the issue UUID by
identifier, updates title/description through GraphQL, appends
idempotency-marked comments, emits the same remote write/readback schemas, and
gates admission on typed target, auth, issue body, comment surface, and
write-receipt checks. GitHub REST operations now expose attempt metadata, retry
transient `5xx` responses with bounded backoff, and classify `403/429` rate
limits as typed `remote_rate_limited` blockers without immediate retry.
Production drift handling, richer Linear state mapping, real-token coverage, and
broader retry policy are still pending.
`hive issue mirror-admit <id> --compact` uses the same provider admission
status as `hive issue connector-admission <id> --compact`.

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
