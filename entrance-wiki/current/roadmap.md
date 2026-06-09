# Entrance Roadmap

Last updated: 2026-06-09

## Current Stop Point

Entrance has been converged toward a local MCP-native, Linear-like issue workbench.

- `Explorer -> Developer -> Reviewer` can run as a serial issue-bound loop.
- SQLite/Hive stores loop contracts, packets, admissions, evidence, verdicts, comments, audit checks, and schema health.
- The default runtime can use local or `codex` workers and records worker receipts.
- Issue/status/comment execution goes through transition admission and records typed receipts.
- Reviewer fallback uses a ledger-backed 3-invalid-round budget and moves issues to `Blocked` for human decision.
- MCP exposes local issue tools/resources plus issue/loop control packets and review queue.
- The Electron Panel shows the local issue board, issue detail, comments, evidence, review result, runtime preflight, worker lifecycle, and human actions.
- Remote synchronization, external issue mirrors, publish/readback/roundtrip, and fixture demos have been removed from CLI/MCP/daemon/GUI as active surfaces.

## Current Validation

Run from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
```

Current run status for this convergence pass:

- `cargo check --workspace`: passed.
- `cargo test --workspace`: passed.
- `pnpm check`: passed.

## Next Work

- Finish migrating internals out of the compatibility implementation file into the new Hive modules.
- Replace compatibility tests that still exercise historical remote synchronization internals with local issue-workbench tests.
- Add first-class persisted claim/assignee fields instead of representing claim as a local comment.
- Harden Reviewer semantic scoring beyond current ledger-derived gates.
- Keep Panel screenshot validation current for the local issue board/detail workflow.
