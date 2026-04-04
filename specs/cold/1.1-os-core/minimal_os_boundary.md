# Minimal OS Boundary

> Status: pending draft
> Scope: v0 OS-core boundary against runtime, harness, and platform shells

## Purpose

- define the smallest hard OS/core cut that keeps Entrance OS-driven rather than prompt-driven
- separate semantic control and storage truth from raw platform capability
- keep capability enforcement in runtime structure instead of courtesy prose

## Boundary Layers

### `OS core`

- owns the canonical governance surface for object kinds, state codes, control policy, permission checks, and bounded work surfaces
- owns runtime truth for mechanical facts when those facts become canonical project records
- exposes governed capability rooms rather than ad hoc tool access

### `Runtime`

- acts as the enforcing execution authority inside the OS core boundary
- writes runtime-only facts such as receipts, delivery edges, taint, and admin actions
- derives effective route, policy, and return behavior from topology plus registry context rather than trusting row-local prose

### `Harness / orchestration shell`

- may provide prompting, process startup, and outer execution convenience
- is not the canonical authority for project truth, routing, or capability policy
- must remain downstream of OS-core policy rather than silently redefining it

### `Platform shell`

- provides raw filesystem, process, network, and OS primitives
- does not become semantic authority by virtue of lower-level power
- should be wrapped by bounded rooms, permission guards, and runtime policy before touching project truth

## Capability Rule

### v0 rule

- capability should be granted through governed rooms, queues, worktrees, and runtime policy rather than through free-form role prestige
- lower execution paths default toward bounded sandboxes or allowlisted worktrees
- break-glass remains a runtime control path, not a semantic authoring path

### Consequence

- a stronger actor may halt or replace execution without gaining direct project-truth mutation rights
- capability hardening should prefer smaller writable surfaces and more runtime derivation before adding new semantic layers

## Routing Rule

### v0 rule

- normal semantic ingress enters through `NOTA` boundary objects
- project-internal routing then proceeds through runtime admission, owned rooms, packets, verdicts, and return queues
- harness-level convenience paths must not bypass the same governed routing and write-order rules

## Truth Rule

### v0 rule

- semantic truth is single-writer and scope-bound
- runtime truth is mechanical and append-oriented
- when one action affects both canonical cold truth and hot operational surface, the write order stays `cold -> hot`

## Non-Goal

- do not treat every platform or harness concern as a new semantic trunk
- do not reopen OS-core scope just to restate already-governed runtime or compiler detail
