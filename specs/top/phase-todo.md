# Phase Todo

> Status: hot utility

## Purpose

- hold the active cross-trunk work queue for the current design/landing phase
- avoid duplicating semantic design content already held by `Machine / Control / Truth`

## Current Focus

- keep the compressed hot root canonical while treating reconciliation detail as cold-truth work
- use the first landing reconciliation cut to filter the 50 imported planning shells before any new UI, sync, or automation work
- keep the active queue on `MYT-63`; `MYT-64` and `MYT-65` now have repo-side canonical copies under `harness/bootstrap/` and `specs/recovery/`
- keep the rest of the imported backlog parked unless runtime fact forces promotion

## Landing Sequence

1. keep `machine.md / control.md / truth.md / phase-todo.md / pending.md` as the active hot root
2. keep the first reconciliation cut in cold detail at `specs/cold/3.1-learning-truth-system/landing_reconciliation_cut.md`
3. use the landed bootstrap and recovery imports as the repo-side source for the next decoupling step under `MYT-63`
4. only after the first reconciliation cut is absorbed decide whether storage needs an explicit owned reconciliation bucket beyond `unreconciled`

## Current Boundary

- current default boundary is `root switched, paths unchanged`
- the new hot root is canonical for navigation now
- numbered docs remain in place only as mounted transitional detail with local detail or mounted links
- landing `v0` currently proves external capture plus seeded planning shells, not completed internal roadmap ownership
- repo-root `entrance.db` remains a copy-only recovery seed; the verified landing import currently lives in `.tmp/landing-appdata/entrance.db`
- physical relocation of transitional detail is optional future cleanup, not a prerequisite for the compressed root

## Active Chunks

- `Landing / Reconciliation`
  move only the de-`.agents` absorption lane now: `MYT-63`, with `MYT-64` and `MYT-65` landed as repo-side imports, `MYT-61` kept as a completed verification gate, and the rest of the imported shells preserved in cold backlog or historical residue.
- `Machine`
  keep the machine trunk stable; only reopen it if reconciliation exposes a genuine runtime-routing or ownership-boundary gap.
- `Truth`
  keep the truth trunk stable; only reopen it if reconciliation exposes a real storage/cold/hot mismatch that cannot be resolved locally.
- `Control`
  keep the control trunk parked; only reopen it if reconciliation exposes a real ownership-host conflict or hot-surface compression failure.

## Rule

- items here should point back into one semantic trunk
- this doc is a queue, not a fourth semantic architecture trunk
- parked backlog stays parked until the active reconciliation lane stops being the narrowest hard next move
