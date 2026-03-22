# Recovery Docs

> Status: landed copy-first recovery import
> Source issue: `MYT-65`

## Purpose

- preserve the recovered NOTA-era architecture documents inside `Entrance`
- keep recovery provenance auditable without pretending every imported file is already canonical product truth
- make later runtime and storage migration work possible without depending on `A:/.agents/nota/data/docs/`

## Source Boundary

Imported on `2026-03-22` from:

- `A:/.agents/nota/data/docs/entrance-control-plane.md`
- `A:/.agents/nota/data/docs/entrance-dashboard-runtime.md`
- `A:/.agents/nota/data/docs/entrance-memory-migration.md`
- `A:/.agents/nota/data/docs/entrance-memory-sql-draft.md`
- `A:/.agents/nota/data/docs/entrance-open-questions.md`
- `A:/.agents/nota/data/docs/entrance-os-arch.md`
- `A:/.agents/nota/data/docs/entrance-ralph-loop.md`
- `A:/.agents/nota/data/docs/entrance-continuous-learning.md`
- `A:/.agents/nota/data/docs/entrance-remote-truth-source.md`
- `A:/.agents/nota/data/docs/recovery-report-2026-03-21.md`

These files were copied into `Entrance` without mutating the source files.

## Current Reading

This is a repo-side classification, not a claim that every imported file is now hot-root or runtime truth.

### Largely absorbed into current repo-side docs

- `entrance-os-arch.md`
  - foundation architecture signals now live across `specs/top/machine.md`, `specs/top/control.md`, `specs/top/truth.md`, and their mounted cold docs
- `entrance-continuous-learning.md`
  - continuous-learning signals are now mainly carried by `specs/top/3.1-learning-and-truth-system.md` and `specs/cold/3.1-learning-truth-system/minimal_truth_plane.md`
- `entrance-remote-truth-source.md`
  - its core correction now appears in the decommission and reconciliation docs that treat Entrance as canonical home and `.agents` as transitional substrate

### Partially absorbed, still useful as provenance

- `entrance-control-plane.md`
  - control-plane direction is partially reflected in current control and compiler docs, but this recovery copy still preserves earlier framing
- `entrance-memory-migration.md`
  - memory-migration direction is partially reflected in truth-plane docs and decommission planning, but this proposal remains a useful migration bridge

### Recovery-only or draft for now

- `entrance-dashboard-runtime.md`
- `entrance-memory-sql-draft.md`
- `entrance-open-questions.md`
- `entrance-ralph-loop.md`
- `recovery-report-2026-03-21.md`

These are preserved because they still carry recovery-era provenance, open questions, or draft directions that have not been fully absorbed into the current repo-side architecture surface.

## Rule

- keep these files as source-controlled recovery artifacts
- do not treat this directory as hot-root architecture
- promote specific facts upward only when current runtime, storage, and doc evidence support the move
