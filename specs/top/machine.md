# Machine

> Status: hot root

## Purpose

- hold the compressed hot-root summary for machine semantics
- keep the root view small while mounting detailed runtime and compiler docs below it

## Confirmed Oracle Points

- Entrance keeps `OS + hierarchical state machine + compiler` as the foundational triple.
- `Policy / Operation / Execution` reuse one canonical owned-node template rather than separate role-local state machines.
- The canonical machine stays in `FLOW_PHASE / ATTENTION_STATE / INTEGRITY_OVERLAY`.
- `NOTA` may run its own boundary-scoped flow, but internal project lineage begins only after runtime admission.
- `SUBMISSION / EXCEPTION / RETURN` are runtime transport lanes rather than canonical node phases.
- Simulation gates upward promotion through governed `simulation_evidence` backed by receipts and artifacts.
- Canonical truth follows single-writer ownership; foreign-slot mutation is invalid.
- Compiler IR distinguishes `model-authored / runtime-derived / runtime-only`.
- Supervision remains runtime-owned control plus hot projection; only execution failure consumes retry budget at v0.

## Mounted Detail Docs

- [1.1-os-core.md](./1.1-os-core.md)
- [1.2-hierarchical-state-machine.md](./1.2-hierarchical-state-machine.md)
- [1.3-compiler-action-ir.md](./1.3-compiler-action-ir.md)
- [2.1-otp-supervisor-model.md](./2.1-otp-supervisor-model.md)

## TODO(fill)

- keep only cross-machine oracle summary here
- move repeated rationale downward as transitional detail collapses into the compressed target
