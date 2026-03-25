# Machine

> Status: hot root

## Purpose

- hold the compressed hot-root summary for machine semantics
- keep the machine cut reconstructable from runtime truth rather than prose-only recall

## Confirmed Oracle Points

- Entrance keeps `OS + hierarchical state machine + compiler` as the foundational triple.
- `Policy / Operation / Execution` reuse one canonical owned-node template rather than separate role-local state machines.
- The canonical machine stays in `FLOW_PHASE / ATTENTION_STATE / INTEGRITY_OVERLAY`.
- `NOTA` may run its own boundary-scoped flow, but internal project lineage begins only after runtime admission.
- `SUBMISSION / EXCEPTION / RETURN` are runtime transport lanes rather than canonical node phases.
- Canonical truth follows single-writer ownership; foreign-slot mutation is invalid.
- Cadence truth now includes both `CADENCE_CHECKPOINT` and `CADENCE_ACCEPTANCE_BUNDLE`.
- `passed human round` is modeled as acceptance, while `fully settled round` is a stricter machine condition layered above acceptance.
- anti-Zeno is enforced as boundary progress across accepted rounds rather than endless smallest-step recursion.
- phase remains projection over the canonical graph rather than a peer truth family.

## Mounted Detail Docs

- [1.1-os-core.md](./1.1-os-core.md)
- [1.2-hierarchical-state-machine.md](./1.2-hierarchical-state-machine.md)
- [1.3-compiler-action-ir.md](./1.3-compiler-action-ir.md)
- [2.1-otp-supervisor-model.md](./2.1-otp-supervisor-model.md)
