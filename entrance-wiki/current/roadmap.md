# Entrance Roadmap

Last updated: 2026-06-09

## Current Stop Point

Entrance has reached a local MVP unit:

- `Explorer -> Developer -> Reviewer` can run as a serial issue-bound loop;
  older `Doer/Evaluator` ledger rows remain compatibility data.
- The default runtime can use real `codex` workers and records worker receipts.
- SQLite/Hive stores loop contracts, packets, admissions, evidence, verdicts,
  comments, audit checks, and schema health.
- Loop runs now start with a kernel `PREFLIGHT_PACKET` admitted by
  `runtime_policy_ready`. Unsupported or probe-failed runtimes are blocked
  before Explorer/Developer/Reviewer workers spawn, with `runtime_policy`
  audit detail and a linked `Blocked` issue instead of a fake worker failure.
- Runtime preflight is now a first-class observable contract through
  `entrance hive loop preflight <loop_id>`,
  `entrance://loops/{loop_id}/runtime-preflight`, the MCP issue control
  packet, and the Panel selected-issue Runtime Preflight block. It exposes the
  active runtime policy, kernel `PREFLIGHT_PACKET` route, runtime probe,
  current admission result, blocker, failures, and next actions.
- Loop dashboard is now a first-class minimal control-plane contract through
  `entrance hive loop dashboard <loop_id>`,
  `entrance://loops/{loop_id}/dashboard`, and the Panel selected-issue Loop
  Dashboard block. It summarizes issue state, kernel preflight,
  Explorer/Developer/Reviewer lanes, reviewer budget, human decision actions,
  health, blockers, round packet/admission/evidence/verdict grouping, retry
  lineage, and next actions in one report.
- Evidence drilldown is now a first-class focused evidence contract through
  `entrance hive loop evidence-drilldown <loop_id>`,
  `entrance://loops/{loop_id}/evidence-drilldown`, and the Panel selected-issue
  Evidence Drilldown block. It exposes worker receipts, transcript/payload
  excerpts, remote receipt summaries, artifact/path hints, payload key diffs,
  blockers, blocker-bound decision surfaces, and next actions. Evidence-level
  blockers and Reviewer budget fallback loop-level blockers both carry primary
  action, issue command, confirmation policy, and review queue/policy resource.
- Evidence manifest is now a first-class ledger-oriented evidence contract
  through `entrance hive loop evidence-manifest <loop_id>`,
  `entrance://loops/{loop_id}/evidence-manifest`, and the Panel selected-issue
  Evidence Manifest block. It exposes payload, worker receipt, transcript
  excerpt, artifact/path entries, digest coverage, path verification state,
  resources, and next actions.
- Issue activity timeline is now a first-class issue-first control-plane
  contract through `entrance hive issue timeline <issue_id>`,
  `entrance://issues/{issue_id}/timeline`, and the Panel selected-issue
  Activity Timeline block. It combines issue creation, typed comments, stage
  evidence, verdicts, operator decisions, blockers, linked resources, and next
  actions in one chronological feed, plus round groups, item permalinks, a
  single-item resource surface, and a Blocked/Needs Review human decision surface
  with primary action, issue commands,
  operator confirmation receipt provenance, confirmation policy, and
  issue-control/review-queue resources.
- Issue transition policy is now a first-class issue-level control-plane
  contract through `entrance hive issue transition-policy <issue_id>`,
  `entrance://issues/{issue_id}/transition-policy`, and the Panel
  selected-issue Transition Policy block. It exposes the current state class,
  allowed actions, blocked actions, confirmation receipt requirements, Reviewer
  fallback budget, policy owner/scope, resources, and next actions in
  `entrance.hive.issue_transition_policy.v1`.
- The issue transition policy is now also bound to the Hive kernel policy
  registry. `entrance hive policy registry --compact` exposes the
  `issue_transitions` registry, each `issue_transition_policy.v1` report embeds
  a registry snapshot, and `entrance hive loop audit <loop_id>` includes an
  `issue_transition_policy` check for allowed/blocked action coverage,
  confirmation contract drift, and Reviewer fallback budget drift.
- The issue transition registry now includes a serialized state machine matrix
  for `Todo`, `Doing`, `Blocked`, `Needs Review`, `Done`, and `Canceled`.
  `entrance hive policy registry --compact` and the MCP policy registry expose
  each state's allowed/blocked actions, gates, confirmation requirements,
  terminal/human-decision class, loop-bound `run` condition, and retryable
  runtime-rejected `Canceled` condition. Hive unit tests now verify that the
  real issue action surface stays aligned with that matrix.
- Issue/status/comment execution now goes through transition admission. Local
  `entrance hive issue comment`, `issue decide`, `issue run`, and `issue
  retry-run` paths consult the kernel transition registry before mutating issue
  state, write `entrance.hive.issue_transition_admission.v1` receipts into
  operator comment/decision payloads, and require explicit CLI
  `--human-confirmed` for retry/review/cancel transitions. Issue surface audit
  verifies transition admission receipt shape and comment/evidence binding.
- Reviewer fallback has a first budget rule: if a candidate is still rejected at
  or after 3 rounds, the issue moves to `Blocked` for human decision.
- Worker lifecycle is now a first-class observable contract through
  `entrance hive loop worker-lifecycle <loop_id>` and
  `entrance://loops/{loop_id}/worker-lifecycle`, exposing expected
  Explorer/Developer/Reviewer roles, observed workers by round, receipt status,
  timeout/attempt/retry-exhaustion metadata, failures, and the 3-round Reviewer
  invalid-budget fallback.
- The built-in `local-hive-panel` is now the default in-process
  issue/status/comment surface. It reports current local issues without
  requiring an external mirror file.
- `entrance mcp stdio` now exposes the local Hive issue/status/comment kernel
  as a minimal MCP tool/resource/prompt surface for creating, running, retrying,
  commenting on, reading, prompting issue-bound loops, and listing the
  `Blocked`/`Needs Review` review queue. MCP retry/review/cancel calls now
  require `human_confirmed=true` and expose the permission policy through
  `entrance://policy/mcp-permissions`. `tools/list` and the permission resource
  now share a per-tool `entrance.mcp.tool_permission.v1` registry; confirmed MCP
  human decisions also write an action/author/policy marker into the operator
  decision note and a typed `entrance.hive.operator_confirmation_receipt.v1`
  receipt into the operator decision comment/evidence payload. If the MCP
  client sends `initialize.clientInfo`, that self-reported client identity is
  copied into the receipt for audit context. `entrance_issue_control` and
  `entrance://issues/{issue_id}/control` now expose a single issue control
  packet with status, action call templates, blockers, transition policy
  resource, runtime preflight summary, worker lifecycle summary, timeline resource, recent evidence, operator events,
  confirmation receipts, and actor identity context. The actor identity policy
  resource documents self-reported MCP actors and local Panel audit actors with
  `verified=false`.
- CLI and Panel can show issue status, comments, connector state, Doctor/audit
  summaries, and human retry/review/cancel options. The Panel also has a
  Review Queue band that lifts `Blocked` and `Needs Review` issues above the
  board with verdict reason, blockers, recent evidence, and decision actions.
  Retry/review/cancel issue actions now carry a typed operator confirmation
  contract, and Panel daemon decisions write `source=panel` confirmation
  receipts into the operator decision comment/evidence ledger.
- `hive connector fixture-demo --compact` and the Panel `Run Fixture` action
  now create a `remote-fixture:ENTRANCE-DEMO` issue and run the full external
  issue/status/comment dry-run roundtrip. The path writes the file-backed remote
  fixture mirror, validates the remote write receipt and readback contract,
  records connector readback/admission evidence, republishes those observations,
  and ends with the connector surface current. The Panel path has been validated
  through the local HTTP daemon bridge with a real Browser click from a clean
  temporary app root.
- `entrance-auto/workflows/validation/run-local-mvp-demo.sh --full-gates` now
  runs the local MVP loop, the `remote-fixture:` external dry-run, full Rust and
  frontend gates, formatting checks, diff checks, and a machine-readable report
  from a clean app root.
- The same workflow now supports `--verify-golden` and `--update-golden` for
  committed normalized output contracts under
  `entrance-auto/fixtures/golden/local-mvp-demo/`.
- `entrance-auto/workflows/validation/capture-panel-screenshot.mjs --full-gates`
  now captures the Panel Issue board from the same clean app root and writes
  screenshot metadata proving that the local MVP issue, `remote-fixture:` issue,
  connector queue, `Run Fixture` actions, `Todo`/`Done` columns, and reviewer
  keep evidence are visible. The screenshot workflow also asserts that the
  selected issue Loop Dashboard, round grouping, Evidence Drilldown, Runtime
  Preflight, Worker Lifecycle, Developer/Reviewer lanes, fallback budget,
  receipt detail, payload diff, and lifecycle state are visible.
- The Panel selected-issue detail now consumes the daemon `hive_loop_dashboard`
  report and renders Loop Dashboard state, kernel gate, role lanes, reviewer
  budget, human decision status, round grouping, retry lineage, blockers, and
  copyable next actions.
- The Panel selected-issue detail now consumes the daemon
  `hive_loop_evidence_drilldown` report and renders worker receipts,
  transcript/payload excerpts, remote receipt summaries, artifact/path hints,
  payload key diffs, blockers, human decision status, and copyable next
  actions.
- The Panel selected-issue detail now consumes the daemon
  `hive_loop_evidence_manifest` report and renders evidence coverage,
  payload/receipt/artifact entries, digest prefixes, path verification state,
  and copyable next actions.
- The Panel selected-issue detail now consumes the daemon
  `hive_issue_timeline` report and renders Activity Timeline with comments,
  evidence, verdicts, operator decisions, blockers, linked ids, and copyable
  next actions.
- The Panel selected-issue detail now consumes the daemon
  `hive_issue_transition_policy` report and renders Transition Policy state,
  allowed/blocked actions, confirmation requirements, Reviewer fallback budget,
  and linked resources before loop internals.
- The Panel selected-issue detail now consumes the daemon
  `hive_loop_runtime_preflight` report and renders Runtime Preflight state,
  route, object kind, gate result, runtime policy/probe, blockers, failures,
  and copyable next actions before worker lifecycle details.
- The Panel selected-issue detail now consumes the daemon
  `hive_loop_worker_lifecycle` report and renders Worker Lifecycle state,
  expected role lanes, observed worker receipts, round chips, fallback budget,
  timeout/failure summaries, and copyable next actions.
- State-changing Panel actions now refresh the selected issue control surfaces
  after the board refresh. Create, run, retry, review, cancel, comment, issue
  mirror sync/publish/verify/readback/admit/roundtrip, connector
  publish/roundtrip execute, and fixture demo paths force a fresh read of the
  selected issue Transition Policy, Loop Dashboard, Evidence Drilldown, Evidence
  Manifest, Activity Timeline, Runtime Preflight, and Worker Lifecycle.

This is usable as a local control-plane prototype, but it is not yet the final
multi-agent runtime/compiler product.

## Unfinished Work

### P0: External issue surfaces

- Productize the MCP stdio surface: client config docs, stronger protocol
  tests, real auth/identity policy on top of the local tool-permission
  registry, verified actor identity mapping beyond self-reported author and
  `initialize.clientInfo`, and compatibility checks against real MCP clients.
- Add verified operator identity for local Panel/daemon decisions beyond the
  current `local-hive-panel` audit context.
- Add live token-backed GitHub and Linear validation runs, including safe
  credential checks, idempotent comment updates, readback verification, error
  handling, rate-limit behavior, and redacted receipts.
- Expose external connector blockers in the Panel as operator decisions, not
  just CLI details.

### P0: Worker runtime hardening

- Productize worker isolation: sandbox mode, allowed filesystem scope, network
  policy, environment redaction, and output limits.
- Add durable worker lifecycle handling across process restarts: heartbeat,
  resume, cancel, retry, replacement, timeout recovery, and stale worker
  cleanup. The current `worker_lifecycle.v1` report is observable state only,
  not durable process supervision.
- Separate worker execution policy from demo defaults so production loops can
  choose stricter runtime profiles.

### P1: Compiler and policy surface

- Promote the compiler/action IR idea out of archive into current docs and code.
- Version the typed loop contract, packet, receipt, evidence, and verdict
  schemas as first-class runtime objects.
- Make policy registry changes explicit and auditable, including admission gate
  versions, owner, required evidence, and migration behavior.
- Productize the current `issue_transition_policy.v1`
  registry/report/audit/admission/state-machine binding with version migration,
  stronger policy lifecycle semantics, and remote issue status mapping.
- Extend the new runtime preflight admission into a fuller capability preview:
  sandbox scope, connector readiness, artifact capture expectations, and human
  preference boundaries before any agent worker is spawned. Current
  `runtime_preflight.v1` is observable, but it still mostly previews runtime
  support/probe rather than full execution capability.

### P1: Loop dashboard

- Productize the current Evidence Drilldown/Manifest beyond the minimum
  reports: full transcript expansion, durable remote receipt archives, real
  artifact manifest generation/content verification, payload schema diffing,
  and blocker decision workflow.
- Productize the current Activity Timeline beyond the minimum report: filters,
  remote issue comment mapping, inline decision refresh state, receipt
  drilldown, and stronger blocked action provenance.
- Reduce repeated status chips and make the primary next action obvious for
  `Todo`, `Running`, `Blocked`, `Needs Review`, and `Done` issues.

### P1: Store migrations and drift recovery

- Add explicit SQLite migration files and migration tests instead of relying
  only on schema creation plus health checks.
- Add repair guidance for schema drift, missing indexes, stale connector rows,
  and incompatible policy versions.
- Add backup/export/import paths for loop ledger data before destructive
  migrations.

### P2: Release and validation workflow

- Keep Panel screenshot metadata current as the issue board and connector
  dashboard evolve.
- Keep the local MVP golden fixtures current as the intended issue/status/comment
  contracts evolve.
- Add release notes and operator docs that clearly separate local MVP, external
  fixture demo, and real GitHub/Linear integrations.

## Next Recommended Loop

Run one larger convergence loop on productionizing external issue surfaces:

1. Choose between live token-backed GitHub/Linear validation or worker
   lifecycle hardening.
