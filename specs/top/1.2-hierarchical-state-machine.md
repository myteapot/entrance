# 1.2 Hierarchical State Machine Design

> Status: mounted hot detail (transitional)

## Purpose

- define the whole-system state machine rather than a role-local fragment
- define how `NOTA / Arch / Dev / Agent` coexist inside one larger machine
- define how simulation, repair, upward handoff, and escalation interact

## Mounted Root

- [machine.md](./machine.md)

## Confirmed Oracle Points

- This document preserves the whole-system graph cut that sits beneath [machine.md](./machine.md): `Human / NOTA` boundary graph, reusable owned-node graph, and runtime transport graph.
- `Policy / Operation / Execution` reuse one canonical owned-node template; role meaning comes from writer authority, route topology, and object kinds rather than role-local state families.
- `IN / CYCLE / OUT` should remain orthogonal to slot identity, while `FLOW_PHASE / ATTENTION_STATE / INTEGRITY_OVERLAY` remain the canonical machine-readable state-code families.
- `NOTA` may run a boundary-scoped `IN / CYCLE / OUT`, but project-internal lineage begins only after runtime admits boundary output into internal scope.
- Upward promotion stays evidence-gated: `simulation` is mandatory on upward submission at v0, not a free-standing trusted action claim.
- `SUBMISSION / EXCEPTION / RETURN` remain runtime transport lanes attached to ownership transfer edges rather than canonical node phases.
- Runtime admission precedes receiver visibility; packets that fail admission never enter the receiver queue.
- `EXCEPTION_PACKET` is for unresolved blocked-state asks, not for bypassing submission evidence gates with promotable work.
- `STOPPED` answers runnability only; completion, rejection, and failure semantics stay in verdict or receipt objects rather than inflating the canonical machine.
- Sender-side packet resolution stays runtime-routed: waiting on return is canonical `WAITING`, and return arrival may re-enter `CYCLE` or end in `STOPPED` without inventing a new peer phase.
- `PHASE` remains a Human-facing graph summary, while cadence organizes Human windows and continuity without rewriting effective machine state.
- `INTAKE_BUNDLE` remains a boundary-specific ingress object at v0, with `Policy` as the default internal ingress target when boundary output becomes project-internal lineage.

## Current Boundary Reading

- this document owns whole-system graph shape, not a second root summary of machine semantics
- upward handoff is where the simulation gate becomes structurally visible
- packet resolution happens through new owner-written objects rather than by editing the original packet in place
- supervision and phase remain projections over the canonical graph rather than peer state families
- cadence may organize Human engagement windows, but must not rewrite effective machine state

## Mounted Cold Docs

- [minimal_top_graph.md](../cold/1.2-hierarchical-state-machine/minimal_top_graph.md)

## Mounted Chore Docs

- [simulation_gate_handout.md](../chore/1.2-hierarchical-state-machine/simulation_gate_handout.md)
- [simulation_gate_todo.md](../chore/1.2-hierarchical-state-machine/simulation_gate_todo.md)

## TODO(fill)

- keep the mounted summary aligned with the cold graph while any remaining NOTA-boundary intake nuance stays below the root
