# Entrance Runtime Contract

Run all commands from `entrance-src/`.

## CLI

```bash
cargo run -p entrance-app --bin entrance -- status
cargo run -p entrance-app --bin entrance -- drawer summary
cargo run -p entrance-app --bin entrance -- hive summary
cargo run -p entrance-app --bin entrance -- hive loop demo --runtime codex --worker-timeout-secs 90 --worker-attempts 1 --compact
cargo run -p entrance-app --bin entrance -- hive loop start --title "Local loop" --goal "Run the Hive loop MVP" --runtime codex --worker-timeout-secs 90 --worker-attempts 1 --compact
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
cargo run -p entrance-app --bin entrance -- hive connector fixture-demo --compact
cargo run -p entrance-app --bin entrance -- hive connector publish-plan --compact
cargo run -p entrance-app --bin entrance -- hive connector publish-execute --plan-id <sha256> --compact
cargo run -p entrance-app --bin entrance -- hive issue connector-admission 1 --compact
cargo run -p entrance-app --bin entrance -- hive issue decide 1 request-review --body "Need human call"
cargo run -p entrance-app --bin entrance -- hive schema --compact
cargo run -p entrance-app --bin entrance -- launcher list
cargo run -p entrance-app --bin entrance -- mcp stdio
```

`hive loop demo` is the default MVP bootstrap path: it fills in a demo contract,
runs the issue-first `Explorer -> Developer -> Reviewer` loop with `codex` by
default, and returns a compact outcome plus Panel startup hints when `--compact`
is present. `hive loop start` is the custom one-command MVP path: it creates the
linked issue, runs the issue-first loop once, and returns a compact
issue/Doctor/evidence outcome when `--compact` is present. Its compact recovery
section exposes retry commands, failed checks, missing receipts, failed worker
rows, attempt counts, timeouts, and retry exhaustion when the run does not
complete. `hive loop run`
returns the local compiler trace for the round: policy rows, versioned typed
packet envelopes, versioned admission receipts, evidence, and versioned verdict
receipts.
Use `--compact` on `hive loop create` to print the linked issue card and next
actions instead of the full empty loop report. Use `--compact` on
`hive loop run`, `hive issue run`, or `hive issue retry-run` when running
`codex`; the loop still records full worker transcripts in SQLite, but the CLI
prints the Doctor summary or compact issue card instead of the full report.
Compact issue cards include round recovery fields so a retry-run can show which
recent rounds failed and whether the current round recovered from them.
Pending Doctor next actions prefer the issue-first compact command
`hive issue run <id> --runtime <runtime> --compact` when a loop has a linked
issue, so operators stay on the issue/status/comment surface.
`hive schema --compact` reports the SQLite ledger schema contract for local
operators: core schema version, `PRAGMA user_version`, table/column/index
presence, and missing object lists. The Runtime panel shows the same schema
health line as `ok v1/1 tables 13/13 indexes 11/11` when the local ledger is
ready. Loop audit also runs this as the `store_schema` gate, so doctor and
issue/status/comment surfaces report `audit_failed` if the ledger schema drifts
before operators trust packet, evidence, or verdict rows.
Admission receipts include the packet receipt requirements, missing receipt
fields, and a boolean satisfied flag. Default MVP gates admit packets only when
their typed receipt requirements are present. Worker receipts are stricter than
plain presence checks: `role_worker` and `runtime_worker` must have `ok=true`
before the packet can pass admission, and loop audit verifies the worker `role`
still matches the packet writer role.
`hive policy registry` is the current source for typed admission gate specs,
runtime worker policy, and connector retry policy: supported runtimes, sandbox
mode, timeout and attempt bounds, env overrides, role binding, required worker
receipt metadata, the connector admission required-check contract, and the
GitHub/Linear remote connector retry budget. Connector admission keeps
`required_checks` as the compatibility list and exposes a structured
`check_registry` with each check's severity, owner, required evidence, and
summary. Runtime admission check rows inherit that registry metadata so a failed
check carries both observed details and the policy owner/evidence contract.
Admission gate failures are recorded as rejected receipts and returned as
blocked verdicts/issues instead of escaping as raw CLI errors.
Every new loop run starts with a kernel-owned `PREFLIGHT_PACKET` routed
`kernel -> explorer` and admitted by the `runtime_policy_ready` gate. That
packet records the selected runtime, the runtime probe result, the supported
runtime registry, the selected runtime sandbox/required context, and a blocker
such as `runtime.unsupported` or `runtime.probe_failed` when the gate fails.
If preflight is rejected, Hive creates a `kernel` stage, records
`admission_rejection` evidence and a blocked verdict, moves the linked issue to
`Blocked`, and does not spawn Explorer/Developer/Reviewer workers. Successful
preflight records only packet/admission receipts; the agent stage/evidence
ledger remains the three role stages.
`entrance hive loop preflight <loop_id>` exposes this boundary as
`entrance.hive.runtime_preflight.v1`: the runtime policy, supported runtime
registry, route `kernel -> explorer`, expected `PREFLIGHT_PACKET`, runtime
probe, current-round admission result, blocker, failure list, and copyable next
actions. The report only treats a preflight packet from the current round as the
current observation, so a retry into a new round is not polluted by an older
blocked preflight.
The same report is available to MCP clients as
`entrance://loops/{loop_id}/runtime-preflight`; MCP issue control packets also
include a compact runtime preflight summary with gate, route, state, blocker,
and failure details. The local Panel selected-issue detail renders this report
as a Runtime Preflight block before Worker Lifecycle, making the kernel gate
visible before operator attention moves to workers.
`entrance hive loop dashboard <loop_id>` exposes the loop-level control surface
as `entrance.hive.loop_dashboard.v1`. It combines issue state, kernel preflight,
Explorer/Developer/Reviewer lane state, Reviewer score/budget, human decision
actions, health, blockers, comment summary, resources, primary next action,
copyable next actions, and per-round packet/admission/evidence/verdict
grouping into one read-only report. MCP clients can read the same report
through `entrance://loops/{loop_id}/dashboard`, and the Panel selected-issue
detail renders it above the more specific Runtime Preflight and Worker
Lifecycle blocks. This is a dashboard grouping contract; focused transcript,
remote receipt, artifact manifest, raw-payload diff, and blocker decision
drilldowns are still future dashboard work.
The MVP runtime set is `local` and `codex`; unsupported runtime names are
reported as preflight-blocked verdicts. The `codex` runtime uses a read-only
`codex exec` worker for each `Explorer`, `Developer`, and `Reviewer` role and
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
Explorer/Developer/Reviewer stages, while legacy Doer/Evaluator rows remain
audit-compatible. The `stage_evidence` check verifies each
expected stage has exactly one stage-bound evidence row with the expected kind,
a `packet_sequence` check that rejects duplicate route packets in one round, a
`worker_receipts` check that verifies worker receipts carry bounded timeout and
attempt metadata plus the expected role, and a `runtime_policy` check that
verifies the contract runtime and current-round worker receipt kind/mode/role
values against the registry and packet writer.
The `active_policy_registry` check verifies the canonical Explorer/Developer/Reviewer
route and gate contract, including gate expected object kind and required
receipts. The `admission_receipts` check verifies every packet has exactly one
admission and that the stored admission receipt still binds to the packet row,
policy route, gate spec, required receipt list, missing receipt list, gate
result, and final admitted/rejected result. The `verdict_packets` check verifies
terminal rounds have exactly one verdict, then checks decision bindings,
required score-vector metrics, gate booleans, human options, and reason-code
evidence bindings. It also binds standard verdict evidence back to round
evidence counts, runtime readiness, and the reviewer worker receipt, while
admission-rejection verdicts bind back to the rejected admission, packet, and
admission-rejection evidence row. The `issue_surface` check verifies linked
issue status, typed comments, operator comment/decision evidence, and
evidence-to-comment author/action/body bindings so the control plane is
auditable instead of only visible. Trace, Doctor, and Panel summaries expose
both failed audit check names and extracted audit failure detail codes, so an
operator can see the exact binding that broke without opening raw JSON first.
For reviewer-path testing, `hive loop run` accepts
`--decision keep|reject|needs-review|blocked`.
At or after round 3, `--decision reject` falls back to a `blocked` verdict and
`Blocked` issue status because the automatic review budget is exhausted.
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
round were produced successfully and how long the runtime spent.

`entrance hive loop worker-lifecycle <loop_id>` exposes the same worker facts
as a first-class `entrance.hive.worker_lifecycle.v1` report: expected
Explorer/Developer/Reviewer roles, observed workers by round, missing roles,
timeouts, attempts, retry exhaustion, receipt errors, the 3-round Reviewer
invalid budget, and the `Blocked` fallback status. This is a lifecycle
observability contract. The local Panel selected-issue detail calls the daemon
`hive_loop_worker_lifecycle` command and renders the same report as Worker
Lifecycle role lanes, round chips, fallback budget, timeout/failure summaries,
and copyable next actions. Durable worker heartbeat, resume, cancel,
replacement, and isolation are still future runtime-hardening work.

Operator comments and decisions are summarized into a current-round operator trail plus
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
adapter, status, comment surface, digest, and readback command. The built-in
`local-hive-panel` surface is an in-process issue/status/comment board; its
publish/readback checks are satisfied from SQLite and do not create an external
publish queue item. `file:`, `remote-fixture:`, `linear:`, and `github:` are the
surfaces that represent external mirror or remote issue sync. `hive issue
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
service. `hive connector fixture-demo --compact` is the default non-local
external-surface dry-run: it creates a `remote-fixture:ENTRANCE-DEMO` loop issue,
runs publish -> readback -> admission -> final readback, records
`connector_readback` and `connector_admission` evidence, and returns an
`entrance.hive.connector_fixture_demo.v1` report with issue, connector, queue,
and roundtrip summaries. The Panel `Run Fixture` button calls the same daemon
path, selects the created issue, and refreshes the connector queue/detail view.
Remote GitHub/Linear providers remain visible with adapter blockers
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
to provider readiness, mirror readback, remote-write requirements, or retry
policy budget drift.
Provider overrides are read from `entrance.toml` under `[connectors.<provider>]`.
GitHub and Linear both have guarded remote publish/readback slices when enabled
with a configured token env. `[connectors.github] enabled = true` with
`GITHUB_TOKEN` or `GH_TOKEN` activates REST issue/comment publish operations;
`[connectors.linear] enabled = true` with an env such as `LINEAR_API_KEY`
activates GraphQL issue/comment publish operations. Both record
`entrance.hive.connector_remote_write_execute.v1` plus redacted
`entrance.hive.connector_remote_write_receipt.v1` evidence. GitHub comment
publish uses an issue-stable Entrance idempotency marker: it lists issue
comments, patches the matching comment when present, and creates one only when
the marker is absent. GitHub readback uses REST `GET` issue plus `GET` issue
comments, follows `Link` pagination for the comment list, emits
`entrance.hive.connector_remote_readback.v1`, and connector admission is ready
only when the typed target, auth, issue state/body, latest comment, and
write-receipt binding checks pass. Linear publish reads the issue UUID by
identifier, updates title/description through GraphQL, updates the matching
issue-stable comment marker when present, creates one only when absent, emits the
same remote write/readback schemas, and gates admission on typed target, auth,
issue body, comment surface, and write-receipt checks. GitHub REST and Linear
GraphQL operations expose attempt metadata, retry transient HTTP `5xx` responses
with bounded backoff, and classify `403/429` rate limits as typed
`remote_rate_limited` blockers without immediate retry; Linear also classifies
GraphQL rate-limit errors as the same typed blocker. Connector status and queue
reports include compact remote diagnostics, letting the Panel surface write or
readback retry/rate-limit signals as first-class chips and expand selected issue
diagnostics into per-attempt HTTP status, failed-check, retry reason, and backoff
rows. The same GitHub/Linear retry budget is exposed through
`hive policy registry --compact` and embedded in active remote contracts.
Connector admission previews include a `retry_policy_bound` check that compares
observed write/readback attempt counts with that active budget before a remote
issue surface can be admitted. `hive policy registry --compact` and
`hive connector registry --compact` expose the same connector admission
`required_checks` list and structured `check_registry` so Panel chips and CLI
previews can be checked against the policy surface. Actual admission check rows
include the matched owner, severity, required evidence, and policy summary.
Production drift handling, richer Linear state mapping, real-token coverage, and
configurable/adaptive retry policy are still pending.
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

## MCP Stdio

```bash
cargo run -p entrance-app --bin entrance -- mcp stdio
```

The MCP stdio surface accepts newline-delimited JSON-RPC 2.0 messages and
returns one JSON-RPC response per line. It is the first local MCP-native control
surface for the Hive kernel, not a separate remote MCP service.

Supported methods:

- `initialize`, `notifications/initialized`, and `ping`
- `tools/list`
- `tools/call`
- `prompts/list`
- `prompts/get`
- `resources/list`
- `resources/read`
- `resources/templates/list`

Every entry returned by `tools/list` includes
`annotations.entrance_permission` with schema
`entrance.mcp.tool_permission.v1`. The same per-tool permission records are
exposed as `tool_permission_registry` in `entrance://policy/mcp-permissions`,
with derived `read_only_tools`, `write_tools`, `human_decision_tools`, and
`requires_human_confirmation` lists. This keeps tool discovery and policy
inspection on the same registry instead of maintaining separate lists.

Issue tools:

- `entrance_issue_list`
- `entrance_issue_show`
- `entrance_review_queue`
- `entrance_issue_comment`
- `entrance_loop_create`
- `entrance_issue_run`
- `entrance_issue_retry`
- `entrance_issue_decide`

Resources:

- `entrance://status`
- `entrance://issues`
- `entrance://review-queue`
- `entrance://issues/{issue_id}`
- `entrance://issues/{issue_id}/control`
- `entrance://loops/{loop_id}/dashboard`
- `entrance://loops/{loop_id}/runtime-preflight`
- `entrance://loops/{loop_id}/worker-lifecycle`
- `entrance://policy/registry`
- `entrance://policy/mcp-permissions`
- `entrance://policy/actor-identity`
- `entrance://schema/status`

Prompts:

- `entrance_loop_contract`: compile a human goal into an issue-bound loop
  contract and require `entrance_loop_create` before implementation.
- `entrance_issue_advance`: read an issue resource and advance only when the
  current status allows a Developer/Reviewer run.
- `entrance_blocker_decision`: summarize `Blocked` or `Needs Review` state into
  retry/review/cancel options for a human decision.

`entrance_loop_create` creates an issue-bound
`Explorer -> Developer -> Reviewer` contract. `entrance_issue_run` and
`entrance_issue_retry` advance the linked loop through the same Hive runtime
used by the CLI and Panel, including Developer/Reviewer verdicts and the
3-round reviewer-invalid fallback to `Blocked`.
`entrance_review_queue` and `entrance://review-queue` expose only `Blocked` and
`Needs Review` issues, with reviewer decision, reason code, human options,
actions, blockers, latest comment, and recent evidence summaries.
`entrance_issue_control` and `entrance://issues/{issue_id}/control` expose a
single issue as `entrance.mcp.issue_control.v1`, aggregating state, action call
templates, MCP permissions, blockers, recent evidence, operator events, and
operator confirmation receipts so agents do not have to infer the control
surface from raw issue JSON. The control packet now includes `loop_dashboard`,
`runtime_preflight`, and `worker_lifecycle` resource pointers plus compact
runtime preflight and worker lifecycle summaries. The full
`entrance.hive.loop_dashboard.v1` report gives agents one loop-level control
view before they inspect lower-level resources; the full
`entrance.hive.worker_lifecycle.v1` report lists expected roles, observed
workers, receipt status, timeout/attempt metadata, retry exhaustion, and the
Reviewer invalid-budget fallback. `entrance://policy/actor-identity` documents
the current actor bindings: MCP actors come from the self-reported `author`
argument, Panel actors come from the daemon author argument, and neither is a
verified login identity yet.
`entrance_issue_retry` and `entrance_issue_decide` require
`human_confirmed=true`; without it the MCP tool result is an error. The
permission boundary is documented at `entrance://policy/mcp-permissions` and is
also included in review queue item policy metadata. Review queue items include
`mcp_policy.action_tool_permissions`, so each visible issue action points to the
MCP tool and permission record that would execute it. When a confirmed MCP
retry/review/cancel call is accepted, the MCP layer appends an
`MCP confirmation:` marker with action, author, and policy schema to the
operator decision note and passes a typed
`entrance.hive.operator_confirmation_receipt.v1` receipt into Hive. Hive then
persists the readable note as both the issue comment body and the linked
`operator_decision` evidence payload, and persists the typed receipt at both
`issue_comment.payload.confirmation_receipt` and
`loop_evidence.payload.operator.confirmation_receipt`. If `initialize` provided
`clientInfo.name` and optional `clientInfo.version`, the MCP stdio session also
copies that self-reported client identity into the receipt at
`confirmation_receipt.client`. The receipt also records
`confirmation_receipt.actor` with an id, label, source, trust level, and
`verified=false`. This is audit context, not a strong authentication guarantee.
The issue-surface audit checks the receipt schema and binds the comment/evidence
receipt copies together.
The Electron Panel mirrors that same decision surface as a Review Queue band
above the status board, using the existing issue actions for retry, review,
cancel, comment, detail focus, and evidence focus. The issue action contract
itself marks retry/review/cancel with
`entrance.hive.operator_action_policy.v1`, `operator_confirmed`, and
`entrance.hive.operator_confirmation_receipt.v1`; Panel daemon invocations use
that contract to record `source=panel` confirmation receipts with
`client.name=local-hive-panel`, `client.source=daemon.invoke`, and
`actor.trust=local_panel_audit`.

## Config And Data

- Default app root: `~/.entrance`
- Override: `ENTRANCE_APP_ROOT`
- Config: `~/.entrance/entrance.toml`
- Database: `~/.entrance/data/entrance.db`
- Vault key: `~/.entrance/vault.key`
- Drawer filesystem root: configured by `[drawer].root`

Invalid config must fail startup instead of silently defaulting.
