# Entrance V1 Release Handoff (Self-Consistency + Dual Host)

> Last updated: 2026-04-08
> Branch baseline: `codex/v1-windows-electron-validation`

## Purpose

This handoff captures the release closure contract for V1 and keeps the repo-side release truth aligned with operator-facing wiki pages.

## Baseline

- Runtime closure invariant lane is green when the active runtime round is fully settled.
- Landing reconciliation is intentionally partial-blocking for this cut.
- Electron must be landed on `main` as a first-class desktop host.
- Release merge gate is CI-backed: Linux verify + Windows native + Electron smoke.

## Batch-01 Reconciliation Semantics

Batch-01 is encoded in:

- `scripts/release/reconciliation-batch-01.json`
- `scripts/release/reconciliation-batch-01-report.snapshot.json` (captured reference snapshot)

The first pass reconciles 12 items and reserves `status` untouched (`seeded`) while writing only `reconciliation_status`.

Semantic buckets used in this cut:

- `ownership_foundation`
- `verification_gate_completed`
- `bootstrap_critical_path`
- `bootstrap_repo_landed`
- `cold_backlog`

The required key set for this cut:

- `MYT-56`
- `MYT-61`
- `MYT-63`
- `MYT-64`
- `MYT-65`

## Release Verification Entry

The release gate script is:

- `scripts/release/verify-v1-self-consistency.sh`
- `scripts/release/verify-v1-self-consistency.ps1`
- `scripts/release/electron-smoke.mjs`
- `scripts/release/run-windows-native-smoke.ps1`

Default chain:

1. `nota status` checks `fully_settled` + `carry_forward_checkpointed=true`.
2. `nota invariants` checks `failed_count=0`.
3. `nota repair` checks `open_count=0`.
4. `landing reconcile batch-apply` executes batch-01.
5. `landing reconcile report` checks `unreconciled_count<=38` and key-item classification.
6. `cargo test --lib` + `pnpm check`.
7. Browser e2e with Linux rollup native dependency preflight.
8. Electron smoke (`pnpm test:electron-smoke`) for route sweep + invoke/listen bridge.
9. Windows native smoke (`run-windows-native-smoke.ps1`) with `ENTRANCE_EXE_PATH`.

Artifacts are written to `test-results/release-self-consistency` by default.

## CI Gate Wiring

Release merge gate in `.gitlab-ci.yml`:

- `linux-verify`: `pnpm check` + `cargo test --lib` + `pnpm test:e2e` (includes rollup preflight).
- `electron-smoke`: `xvfb-run -a pnpm test:electron-smoke`.
- `windows-native`: release build + `run-windows-native-smoke.ps1`.

All three jobs must pass before MR2 merge.

## Wiki Sync Note

The GitLab wiki may lag active branch/runtime state. For release decisions in this cut:

- Repo docs and scripts are canonical.
- Wiki pages should be treated as operator-facing mirrors and updated after merge.
