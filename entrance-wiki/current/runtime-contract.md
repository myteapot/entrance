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
cargo run -p entrance-app --bin entrance -- hive connector queue --provider remote-fixture --compact
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
runtime worker policy, issue transition policy, connector admission checks,
and the `remote-fixture` status mapping policy: supported runtimes, sandbox
mode, timeout and attempt bounds, env overrides, role binding, required worker
receipt metadata, and the connector admission required-check contract.
Connector admission keeps `required_checks` as the compatibility list and
exposes a structured `check_registry` with each check's severity, owner,
required evidence, and summary. Runtime and connector admission check rows
inherit that registry metadata so a failed check carries both observed details
and the policy owner/evidence contract.
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
probe, `runtime_capability_preview.v1`, current-round admission result, blocker,
failure list, and copyable next actions. The capability preview records worker
spawn readiness, runtime sandbox scope, artifact capture mode, connector
readiness for the loop review surface using the current `entrance.toml`
connector config, human confirmation boundaries, and worker context requirements
before any agent worker is spawned. The report only treats a preflight packet
from the current round as the current observation, so a retry into a new round
is not polluted by an older blocked preflight.
The `runtime_policy_ready` gate now checks that capability preview as part of
admission: unsupported runtimes, failed runtime probes, or unready configured
connector review surfaces reject the `PREFLIGHT_PACKET`, move the linked issue
to `Blocked`, and stop before Explorer/Developer/Reviewer workers are spawned.
The same report is available to MCP clients as
`entrance://loops/{loop_id}/runtime-preflight`; MCP issue control packets also
include a compact runtime preflight summary with gate, route, state, blocker,
and failure details, while MCP loop control embeds the full capability preview in
the runtime gate surface. The local Panel selected-issue detail renders this
report as a Runtime Preflight block before Worker Lifecycle, making the kernel
gate and pre-worker capability boundaries visible before operator attention
moves to workers.
`entrance hive loop dashboard <loop_id>` exposes the loop-level control surface
as `entrance.hive.loop_dashboard.v1`. It combines issue state, kernel preflight,
Explorer/Developer/Reviewer lane state, Reviewer score/budget, human decision
actions, health, blockers, comment summary, resources, primary next action,
copyable next actions, and per-round packet/admission/evidence/verdict
grouping into one read-only report. MCP clients can read the same report
through `entrance://loops/{loop_id}/dashboard`, and the Panel selected-issue
detail renders it above the more specific Evidence Drilldown, Runtime
Preflight, and Worker Lifecycle blocks.
`entrance hive loop evidence-drilldown <loop_id>` exposes
`entrance.hive.evidence_drilldown.v1`: a focused evidence report with worker
receipts, receipt gates, transcript/payload excerpts, remote connector receipt
summaries, artifact/path hints, payload key diffs relative to the previous
evidence row, evidence/loop-level blockers, blocker-bound decision surfaces,
resources, and next actions.
MCP clients can read it through
`entrance://loops/{loop_id}/evidence-drilldown`, and the Panel selected-issue
detail renders the same report below Loop Dashboard.
`entrance hive loop evidence-manifest <loop_id>` exposes
`entrance.hive.evidence_manifest.v1`: a ledger-oriented evidence manifest with
payload, worker receipt, transcript excerpt, artifact/path entries, digest
coverage, local path verification status, resources, and next actions. MCP
clients can read it through `entrance://loops/{loop_id}/evidence-manifest`,
and the Panel selected-issue detail renders it below Evidence Drilldown. Full
transcript expansion, durable remote receipt archives, real artifact manifest
generation/content verification, payload schema diffing, and richer blocker
decision workflows remain future work.
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
terminal rounds have exactly one verdict, then binds the current-round verdict
decision back to the terminal contract status unless a same-round
`operator_decision` has taken over the status. It also checks decision bindings,
required score-vector metrics, gate booleans, human options, and reason-code
evidence bindings, and binds standard verdict evidence back to round evidence
counts, runtime readiness, Reviewer invalid-budget use/exhaustion recomputed
from verdict history, and the reviewer worker receipt, while
admission-rejection verdicts bind back to the rejected admission, packet, and
admission-rejection evidence row. The `issue_surface` check verifies linked
issue status, typed comments, operator comment/decision evidence, and
evidence-to-comment author/action/body bindings. Operator comment/decision
evidence also binds its `issue_transition_admission.v1` receipt back to the
observed from/to issue status, policy resource, transition-policy resource, and
admitted action coverage, so human-controlled transitions are auditable instead
of only visible. Issue trace operator events also project the same transition
admission proof: admitted action, gate, from/to issue status, policy registry
resource, transition-policy resource, and confirmation requirement. Trace,
Doctor, and Panel summaries expose both failed audit check names and extracted
audit failure detail codes, so an operator can see the exact binding that broke
without opening raw JSON first.
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
invalid budget, and the `Blocked` fallback status. The invalid budget is
computed from consecutive invalid Reviewer verdicts in the ledger, so a round
number alone cannot exhaust it. Loop audit recomputes the same budget from
verdict history and rejects drifted score/gate/evidence self-reports. This is a
lifecycle observability contract. The
local Panel selected-issue detail calls the daemon
`hive_loop_worker_lifecycle` command and renders the same report as Worker
Lifecycle role lanes, round chips, fallback budget, timeout/failure summaries,
and copyable next actions. Durable worker heartbeat, resume, cancel,
replacement, and isolation are still future runtime-hardening work.

Operator comments and decisions are summarized into a current-round operator
trail plus total operator event counts. Decision events include transition
admission action, gate, from/to status, policy registry resource,
transition-policy resource, and confirmation requirement, making human retry,
review, cancel, and comment actions visible and policy-bound without opening the
raw evidence stream.
`entrance hive issue timeline <issue_id>` exposes the issue-first activity feed
as `entrance.hive.issue_timeline.v1`. The report combines issue creation,
typed comments, stage evidence, verdicts, operator decisions, blockers, linked
resources, and next actions into one chronological control-plane view. MCP
clients can read the same report through
`entrance://issues/{issue_id}/timeline`, and the Panel selected-issue detail
renders it as Activity Timeline below Evidence Manifest.
Issue human options are status-aware: `Blocked` issues can retry, request
review, or cancel; `Needs Review` issues can retry or cancel; `Todo` issues can
be canceled before running; terminal human-canceled issues only allow comments.
Compact issue surfaces also expose connector mirror drift: `hive issue show
<id> --compact` includes a `connector` block, while `hive issue list --compact`
adds a `connector_queue` with publish-required issue ids and commands.
The connector registry is available through `hive connector registry --compact`;
it distinguishes the active `local-hive-panel`, `file`, and `remote-fixture`
issue-surface providers, names the admission gate, and exposes provider-specific
admission status/blockers. `hive connector queue --compact` returns a
provider-scoped publish queue, and `--provider <name>` narrows the dry-run plan
to one issue-surface provider. `hive connector publish-plan --compact` creates
a digest-bound local mirror publish plan from the current queue and the provider
writer adapter contract; `hive connector publish-execute --plan-id <sha256>`
recomputes that plan and refuses to execute if the queue, issue mirror digest,
or provider writer capability changed. Successful execution records a typed
connector publish comment/evidence on each issue before writing the mirror, and
each publish returns a typed connector write receipt with adapter, status,
comment surface, digest, and readback command. The built-in `local-hive-panel`
surface is an in-process issue/status/comment board; its publish/readback checks
are satisfied from SQLite and do not create an external publish queue item.
`file:` represents a local JSON mirror, while `remote-fixture:` / `fixture:`
represent the local file-backed external issue/status/comment dry-run surface.
`hive issue mirror-roundtrip <id> --compact` wraps the issue-scoped publish ->
readback -> admission workflow into one typed report; recorded readback/admission
observations are republished before the final readback check. `hive connector
roundtrip-plan --compact` creates a digest-bound queue plan for that same
roundtrip operation, including provider writer/readback/admission blockers, and
`hive connector roundtrip-execute --plan-id <sha256> --compact` recomputes the
plan before executing it. Successful queue execution records a typed
`connector_roundtrip_execute` comment/evidence on every issue, then runs the
issue-scoped roundtrip and returns a compact per-issue completion summary.
The active `remote-fixture:` provider is a file-backed remote issue API fixture
that writes `entrance.hive.connector_remote_write_receipt.v1` and verifies
`entrance.hive.connector_remote_readback.v1` without contacting a third-party
service. `hive connector fixture-demo --compact` is the default non-local
external-surface dry-run: it creates a `remote-fixture:ENTRANCE-DEMO` loop issue,
runs publish -> readback -> admission -> final readback, records
`connector_readback` and `connector_admission` evidence, and returns an
`entrance.hive.connector_fixture_demo.v1` report with issue, connector, queue,
and roundtrip summaries. The Panel `Run Fixture` button calls the same daemon
path, selects the created issue, and refreshes the connector queue/detail view.
`remote-fixture:` exposes `entrance.hive.connector_remote_contract.v1`,
`entrance.hive.connector_remote_target.v1`, and
`entrance.hive.connector_remote_write_plan.v1` so operators can inspect target,
status mapping, fixture write/readback schema, idempotency key parts, blockers,
and planned local fixture operations before admission. Fixture targets accept
`remote-fixture:<key>` or `fixture:<key>`; missing targets surface typed blockers
such as `remote_target_invalid` or `fixture_target_missing`. Provider overrides
are read from `entrance.toml` under `[connectors.<provider>]`; the current local
target uses `[connectors.file]` for local JSON mirror configuration and keeps
remote fixture execution credential-free. Connector admission previews include a
`retry_policy_bound` compatibility check and expose the connector admission
`required_checks` list plus structured `check_registry` so Panel chips and CLI
previews can be checked against the policy surface. Actual admission check rows
include the matched owner, severity, required evidence, and policy summary.
Production drift handling, remote MCP server shape, richer external workflow
discovery, and configurable/adaptive retry policy are still pending.
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

The protocol surface has a reusable smoke workflow:

```bash
entrance-auto/workflows/validation/run-mcp-stdio-smoke.mjs
```

The smoke starts `entrance mcp stdio` from a clean app root, negotiates
`initialize` with `clientInfo`, lists tools/prompts/resource templates, creates
and runs a local issue-bound loop through MCP tools, reads the same loop control
packet through both `tools/call` and `resources/read`, fetches the loop review
prompt with the embedded resource, creates a `remote-fixture:` issue through
MCP, verifies connector queue/control/decision prompt A/B/C options, executes a
digest-bound connector roundtrip only after `human_confirmed=true`, verifies the
connector queue becomes current, and verifies that retry or connector roundtrip
calls without `human_confirmed=true` are refused.

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
- `entrance://loops/{loop_id}/evidence-drilldown`
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
3-round reviewer-invalid fallback to `Blocked` when the verdict ledger proves
three consecutive invalid review rounds.
Reviewer verdict score/gate payloads are computed from the same current-round
ledger: stage completeness, runtime readiness, prior evidence presence,
admission integrity, Developer accepted-candidate binding, missing receipts, and
failure reasons. If a Reviewer tries to keep a candidate while those ledger
gates are incomplete, the runtime records a `reject` verdict instead of marking
the issue `Done`.
Developer `EXECUTION_PACKET` admission uses `accepted_candidate_bound`: the
packet must carry `accepted_candidate`, and that value must match the same-round
admitted Explorer candidate before Reviewer can receive the packet. Loop audit
recomputes this target binding from the packet/admission ledger and rejects
drifted `target_binding` admission receipts. The same admission audit recomputes
missing receipts, gate pass/fail, gate reason, and admitted/rejected result
bindings from the current packet/policy ledger.
Verdict audit similarly recomputes Reviewer invalid-round budget use/exhaustion
from verdict history, so the `Blocked` fallback must be backed by consecutive
invalid Reviewer verdicts rather than only score/evidence receipt self-report.
`entrance_review_queue` and `entrance://review-queue` expose only `Blocked` and
`Needs Review` issues, with reviewer decision, reason code, human options,
actions, blockers, latest comment, and recent evidence summaries.
`entrance_issue_control` and `entrance://issues/{issue_id}/control` expose a
single issue as `entrance.mcp.issue_control.v1`, aggregating state, action call
templates, MCP permissions, blockers, recent evidence, operator events, and
operator confirmation receipts so agents do not have to infer the control
surface from raw issue JSON. The control packet now includes `loop_dashboard`,
`evidence_drilldown`, `evidence_manifest`, `timeline`, `runtime_preflight`, and
`worker_lifecycle` resource pointers plus compact runtime preflight and worker
lifecycle summaries. The full `entrance.hive.issue_timeline.v1` report gives
agents an issue-first activity feed before they inspect lower-level resources;
the full `entrance.hive.loop_dashboard.v1` report gives
agents one loop-level control view before they inspect lower-level resources;
the full `entrance.hive.evidence_drilldown.v1` report exposes worker receipts,
transcript/payload excerpts, remote receipt summaries, artifact/path hints,
payload key diffs, blockers, and blocker-bound decision surfaces; the full
`entrance.hive.evidence_manifest.v1` report exposes payload/receipt/artifact
entries, digest coverage, and path verification state; the full
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
