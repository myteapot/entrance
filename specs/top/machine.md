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
- `human round` is now the primary continuity object for interaction, checkpoint, acceptance, projection, and repair.
- `passed human round` is modeled as acceptance, while `fully settled round` is a stricter machine condition layered above acceptance.
- anti-Zeno is enforced as boundary progress across accepted rounds rather than endless smallest-step recursion.
- canonical round state and detail round state are distinct surfaces; the ladder stays small while runtime may still expose finer closure detail.
- phase remains projection over the canonical graph rather than a peer truth family.

## Canonical Round Ladder

- `opened` means a human round exists but has not yet been checkpointed.
- `checkpointed` means runtime continuity has a current checkpoint for the round.
- `accepted` means acceptance is formalized for the round boundary.
- `settling` means follow-on next-step, projection refresh, or repair closure is still open.
- `fully_settled` means the accepted boundary has no remaining next-step debt and its retained projections are in sync.

## Detail State Projection

- `uncheckpointed` is the detail state under canonical `opened`
- `checkpointed_pending_acceptance` is the detail state under canonical `checkpointed`
- `accepted_waiting_carry_forward` is the detail state under canonical `accepted`
- `accepted_followup_open` is the detail state under canonical `settling`
- `fully_settled` remains both a canonical and detail state

## Mounted Detail Docs

- [1.1-os-core.md](./1.1-os-core.md)
- [1.2-hierarchical-state-machine.md](./1.2-hierarchical-state-machine.md)
- [1.3-compiler-action-ir.md](./1.3-compiler-action-ir.md)
- [2.1-otp-supervisor-model.md](./2.1-otp-supervisor-model.md)
- [human_round_state_machine.md](../cold/1.2-hierarchical-state-machine/human_round_state_machine.md)
