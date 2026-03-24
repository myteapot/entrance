# Entrance V0 Headless System Roadmap

> Status: execution staging

## Purpose

- define the milestone path for a relatively complete headless `Entrance V0`
- keep scope on `Entrance Core + Scheduler / Hierarchical State Machine / Compiler`
- make current `Codex CLI -> MCP -> NOTA runtime` an explicit system target rather than a side effect
- keep Windows GUI out of the critical path for `V0` completion while preserving it as a later control surface

## Target State

`Entrance V0` should be a runtime-first system with multiple ingress surfaces:

- `Codex CLI` can enter through MCP and operate as the normal `NOTA` runtime surface
- `NOTA / Arch / Dev / Agent` remain role projections over one shared runtime rather than four disconnected flows
- allocation, routing, supervision, receipts, evidence, and storage truth are runtime-owned
- the same runtime semantics can later be surfaced through `MCP / CLI / Windows GUI` without each ingress inventing its own scheduler

This roadmap does **not** define a GUI-complete product target.
It defines the headless system target that the GUI can later project.

## Current Baseline

The following substrate is already landed:

- role-scoped MCP surfaces exist for `nota / arch / dev`
- `forge_bootstrap_mcp_cycle` exists as a NOTA-owned bootstrap allocator surface
- multi-agent fan-out now uses dedicated managed slot worktrees rather than one shared worktree
- bootstrap fan-out now runs through a real Dev parent task rather than a `Pending` placeholder
- Forge task rows, supervision receipts, and runtime DB ownership are already storage-backed
- `action.rs` and `permission.rs` already define the role / primitive / room contract and the current MCP permission mapping

The following system gaps remain open:

- the scheduler is still bootstrap-specialized rather than a full generic allocator kernel
- the hierarchical state machine is still mostly specified in docs rather than materialized as runtime objects and transitions
- the compiler registry and lowering path are not yet runtime-complete
- admission, return routing, simulation gate, and typed supervision are not yet implemented as first-class runtime machinery
- structured result integration, recovery, and continuity are not yet complete enough to remove the human data-bus fallback

## Milestone Count

- `V0-min`: `8` required milestones
- `V0-full`: `10` milestones

`V0-min` means a relatively complete headless Entrance system.
`V0-full` adds the minimum continuity and learning/admin closure needed for a stronger long-running system.

## Roadmap

### M1. Scheduler Closure

Goal:
- close the bootstrap allocator loop so `Dev` owns `prepare -> dispatch -> wait -> collect -> return` end-to-end

Done means:
- bootstrap orchestration no longer relies on hidden top-level glue for the remaining `Dev` work
- child results are gathered as runtime objects, not just inferred from task completion
- `NOTA` coordinates, but does not impersonate `Dev` once the parent task is launched

Primary lane:
- scheduler

### M2. NOTA Runtime Host

Goal:
- make `Codex CLI -> MCP -> NOTA` a first-class runtime mode rather than an ad hoc operator pattern

Done means:
- a NOTA session has explicit runtime state
- the runtime can read current system state, choose the next bounded step, and pause at declared boundaries
- the NOTA host can continue a cycle without the human manually shuttling state between windows

Primary lane:
- control / scheduler

### M3. Allocation Object Model

Goal:
- promote assignment/allocation from task-side metadata into explicit runtime-owned objects

Done means:
- allocation lineage, child slot ownership, return target, escalation target, and allocator receipts are first-class runtime facts
- runtime can reconstruct a cycle from storage without replaying chat prose
- child fan-out and return-up semantics are visible independently from raw task logs

Primary lane:
- scheduler / truth

### M4. Minimal Compiler Registry

Goal:
- materialize the minimum compiler registry cut from the mounted design docs

Done means:
- `object_kind / state_code / control_policy` exist as explicit runtime-usable registry data
- writer, route, gate, sandbox, admission, and projection semantics are no longer prose-only
- runtime can derive effective control semantics from registry + topology

Primary lane:
- compiler

### M5. Compiler Lowering

Goal:
- lower role/room/action contracts into executable runtime decisions

Done means:
- `NOTA / Arch / Dev / Agent` actions lower into typed packets, rooms, and routes
- runtime enforces lowering outputs before execution rather than relying on review after the fact
- `action.rs` becomes one input to runtime behavior, not just a contract file

Primary lane:
- compiler / scheduler

### M6. Admission And Return Routing Kernel

Goal:
- implement the minimal runtime transport kernel for `SUBMISSION / EXCEPTION / RETURN`

Done means:
- runtime admits packets before receiver visibility
- rejected packets never silently appear in the receiver queue
- return routing is runtime-owned and lineage-aware
- sender-side waiting and receiver-side visibility are reconstructed from runtime facts

Primary lane:
- hierarchical state machine / compiler

### M7. Simulation Gate V0

Goal:
- implement the minimum evidence gate required for upward promotion at `V0`

Done means:
- upward submission requires `SIMULATION_EVIDENCE`
- the evidence bundle includes at least an attempt receipt and an artifact manifest
- runtime uses governed evidence objects rather than trusting self-reported simulation claims

Primary lane:
- machine / truth / compiler

### M8. Typed Supervision

Goal:
- upgrade current task supervision into the typed runtime signal model

Done means:
- runtime classifies at least `EXECUTION_FAILURE_SIGNAL / ADMISSION_REJECTION_SIGNAL / VERDICT_RETURN_SIGNAL / INTEGRITY_SIGNAL`
- supervisor actions are explicit runtime outputs rather than task-string conventions
- retry budget applies only to the allowed signal family
- incident visibility is preserved across retry and escalation

Primary lane:
- scheduler / machine

### M9. Return / Integrate / Repair Loop

Goal:
- complete the `Dev` side closure after child execution finishes

Done means:
- `review / integrate / repair / escalate` exist as runtime-governed steps
- child output is ingested structurally enough to support `Dev` decisions
- the system can finish a multi-agent cycle without reducing everything to logs and terminal status

Primary lane:
- scheduler / compiler

### M10. Recovery / Learn / Admin Closure

Goal:
- make the headless system resumable and continuity-safe

Done means:
- interrupted cycles can be recovered from storage-backed runtime state
- the minimum `LEARN_CAPTURE` and continuity/admin surfaces exist without violating storage-first truth
- operator recovery no longer requires replaying the entire chat history as the main source of truth

Primary lane:
- truth / recovery / control

## Dependency Shape

Strict serial chain:

1. `M1 Scheduler Closure`
2. `M2 NOTA Runtime Host`
3. `M3 Allocation Object Model`

Then the system can branch into two partially parallel tracks:

- compiler track:
  - `M4 Minimal Compiler Registry`
  - `M5 Compiler Lowering`
  - `M6 Admission And Return Routing Kernel`
- runtime safety track:
  - `M7 Simulation Gate V0`
  - `M8 Typed Supervision`

Final closure:

1. `M9 Return / Integrate / Repair Loop`
2. `M10 Recovery / Learn / Admin Closure`

Practical reading:

- `M1-M3` reduce the human data-bus role
- `M4-M8` turn mounted architecture into runtime machinery
- `M9-M10` make the system survivable as a long-running headless runtime

## Completion Lines

### `V0-min`

`Entrance V0-min` is complete when:

- the human can interact with one `Codex CLI` as `NOTA`
- that `NOTA` runtime can drive a full multi-role headless cycle through MCP/runtime
- allocation, routing, receipts, supervision, and upward evidence gates are runtime-owned
- the cycle can complete without a human manually relaying prompts, task IDs, or worktree state between roles

This line requires:

- `M1` through `M8`

### `V0-full`

`Entrance V0-full` is complete when:

- `V0-min` is complete
- return/integration/repair are structurally closed
- interrupted cycles can be resumed from runtime truth
- learning/admin continuity no longer depends on replaying transient operator context

This line requires:

- `M1` through `M10`

## Explicitly Out Of Scope For This Roadmap

- Windows GUI productization
- OAuth cleanup
- auto-sync
- external write-back loops such as Linear write-back
- connector expansion beyond what the headless runtime must already support

These may become later milestones, but they are not prerequisites for `Entrance V0` as a headless system.

## Recommended Next Move

The narrowest next move after landing this roadmap is:

- keep pushing the scheduler lane first
- treat `M1 Scheduler Closure` as the active frontier until `Dev` owns the full runtime loop without hidden bootstrap glue
- only after `M1-M3` are stable should the compiler lane become the primary trunk
