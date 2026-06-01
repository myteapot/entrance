# Entrance Roadmap

Last updated: 2026-06-01

## Current Stop Point

Entrance has reached a local MVP unit:

- `Explorer -> Doer -> Evaluator` can run as a serial issue-bound loop.
- The default runtime can use real `codex` workers and records worker receipts.
- SQLite/Hive stores loop contracts, packets, admissions, evidence, verdicts,
  comments, audit checks, and schema health.
- The built-in `local-hive-panel` is now the default in-process
  issue/status/comment surface. It reports current local issues without
  requiring an external mirror file.
- CLI and Panel can show issue status, comments, connector state, Doctor/audit
  summaries, and human retry/review/cancel options.

This is usable as a local control-plane prototype, but it is not yet the final
multi-agent runtime/compiler product.

## Unfinished Work

### P0: External issue surfaces

- Make `file:` and `remote-fixture:` connector roundtrips the default external
  dry-run demo path, with one clear CLI command and one clear Panel action.
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

- Turn the current Panel issue board into a real loop dashboard: round timeline,
  role lanes, packet/admission/evidence/verdict grouping, and retry lineage.
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

- Create a reproducible demo script that runs local MVP, external fixture
  roundtrip, Panel browser validation, and full gates from a clean app root.
- Keep representative golden fixtures for compact CLI outputs and Panel-visible
  connector states.
- Add release notes and operator docs that clearly separate local MVP, external
  fixture demo, and real GitHub/Linear integrations.

## Next Recommended Loop

Run one larger convergence loop on external issue surfaces:

1. Make `remote-fixture:` the default non-local connector demo.
2. Add a Panel action that runs external fixture roundtrip and shows readback.
3. Validate with CLI, Browser, and full gates.
4. Then decide whether to spend the next loop on real GitHub/Linear credentials
   or worker lifecycle hardening.
