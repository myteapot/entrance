# Entrance Roadmap

Last updated: 2026-06-09

## Current Stop Point

Entrance has reached a local MVP unit:

- `Explorer -> Developer -> Reviewer` can run as a serial issue-bound loop;
  older `Doer/Evaluator` ledger rows remain compatibility data.
- The default runtime can use real `codex` workers and records worker receipts.
- SQLite/Hive stores loop contracts, packets, admissions, evidence, verdicts,
  comments, audit checks, and schema health.
- Reviewer fallback has a first budget rule: if a candidate is still rejected at
  or after 3 rounds, the issue moves to `Blocked` for human decision.
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
  packet with status, action call templates, blockers, recent evidence, operator
  events, confirmation receipts, and actor identity context. The actor identity
  policy resource documents self-reported MCP actors and local Panel audit
  actors with `verified=false`.
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
  keep evidence are visible.

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
  cleanup.
- Separate worker execution policy from demo defaults so production loops can
  choose stricter runtime profiles.

### P1: Compiler and policy surface

- Promote the compiler/action IR idea out of archive into current docs and code.
- Version the typed loop contract, packet, receipt, evidence, and verdict
  schemas as first-class runtime objects.
- Make policy registry changes explicit and auditable, including admission gate
  versions, owner, required evidence, and migration behavior.
- Add admission previews before execution so a loop can be rejected before
  spawning workers when required capabilities are missing.

### P1: Loop dashboard

- Extend the current Panel Review Queue and issue board into a real loop
  dashboard: round timeline, role lanes, packet/admission/evidence/verdict
  grouping, and retry lineage.
- Add focused evidence drill-down views so operators can inspect transcripts,
  failed checks, connector receipts, and human decisions without reading raw
  JSON first.
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
