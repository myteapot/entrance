# Phase Todo

> Status: hot utility

## Purpose

- hold the active cross-trunk work queue for the current design/landing phase
- avoid duplicating semantic design content already held by `Machine / Control / Truth`

## Current Focus

- keep the single-lane `NOTA -> agent` allocator slice honest, queryable, and explicitly incomplete
- keep one persistent `NOTA` monitor/planner window as the only global continuation authority
- keep long-term direction and current `v0` policy separated across `decisions` and `checkpoints`
- keep the next implementation frontier ordered as `agent surface hardening -> dev lane -> permission wiring -> multi-role allocator`
- keep human relay burden shrinking by resuming from DB-backed truth before replaying chat

## Execution Strategy

1. start from `entrance nota checkpoints` for the active `v0` cut and `entrance nota decisions` for direction
2. let the persistent `NOTA` monitor/planner choose one active milestone or state-expansion cut
3. if worker windows are needed, keep them inside that one milestone and give each a single owned lane plus bounded exit criteria
4. let workers land code, tests, receipts, or blockers, but do not let them self-promote the global level
5. let `NOTA` audit the returned evidence, write the new checkpoint when the active cut changes, and write a decision only when direction changes

## Current Boundary

- the active hot root remains `machine.md / control.md / truth.md / phase-todo.md / pending.md`
- live runtime continuity is `DB-first` under `%LOCALAPPDATA%/Entrance/entrance.db`
- `L3` is still not passed: the active allocator slice is single-lane `NOTA -> agent`, not a full multi-role allocator
- `Dev` runtime is not landed yet
- role and permission wiring still are not fully enforced in runtime
- semantic continuation policy is still hardcoded in `v0`; it is not yet human-configurable

## Active Chunks

- `Allocator v0 hardening`
  keep the `NOTA -> agent` slice reconstructable from runtime allocations, transactions, receipts, and checkpoints.
- `Dev lane`
  land a real `Dev` runtime slice with storage-backed dispatch truth before claiming multi-role routing.
- `Permission wiring`
  connect the existing role or primitive or room contract to actual runtime admission and execution walls.
- `Allocator expansion`
  only after the prior three land, move to an honest `DEV / AGENT` allocator.

## Rule

- items here should point back into one semantic trunk
- this doc is a queue, not a fourth semantic architecture trunk
- one active milestone exists at a time even if multiple worker windows execute inside it
- parallelism is subordinate execution, not parallel global continuation
- any strategy change should update `checkpoints`, and any longer-lived architectural direction change should update `decisions`
