# Minimal Supervision Binding

> Status: pending draft
> Scope: v0 runtime binding between OTP-derived supervision and current IR/runtime objects

## Purpose

- bind supervision to the objects and signals that already exist
- avoid a fake complete supervision tree that has no enforcement meaning
- decide restart, block, replace, and escalate from typed runtime facts rather than ad hoc task strings

## Design Rule

- supervision should consume typed runtime signals, not conversational summaries
- v0 signal inputs should come from existing object families:
  - `RECEIPT`
  - `ADMISSION` receipt result
  - `VERDICT`
  - `INTEGRITY_OVERLAY`
  - runtime integrity or admin events

## Minimal Signal Families

### `EXECUTION_FAILURE_SIGNAL`

Derived from:

- `RECEIPT` with `receipt_kind_code in {ATTEMPT, ROOM_EXECUTION}`
- terminal failure or abnormal termination

Meaning:

- supervised child execution failed while trying to do work

### `ADMISSION_REJECTION_SIGNAL`

Derived from:

- `RECEIPT` with `receipt_kind_code = ADMISSION`
- `admission_result_code != ADMITTED`

Meaning:

- runtime rejected delivery before receiver ownership began

### `VERDICT_RETURN_SIGNAL`

Derived from:

- `VERDICT`

Meaning:

- receiver finished semantic evaluation and returned a typed result

### `INTEGRITY_SIGNAL`

Derived from:

- integrity overlay changes
- `TAINT_EVENT`
- `ADMIN_EVENT`

Meaning:

- trust, admin, or promotion-safety condition changed

## Minimal Supervisor Action Set

- `RESTART_CHILD`
- `REPLACE_CHILD`
- `ROUTE_RETURN`
- `BLOCK_LINEAGE`
- `QUARANTINE_LINEAGE`
- `ESCALATE_UP`
- `SURFACE_INCIDENT`

## Action Provenance

- supervisor actions are runtime outputs, not child-authored intents
- workers, packets, and verdict rows may trigger signals, but they do not write supervisor actions directly
- restart, replace, block, quarantine, and escalation remain runtime-owned decisions

## v0 Decision Table

| signal family | example source | default supervisor action | retry budget applies | overlay consequence |
| --- | --- | --- | --- | --- |
| `EXECUTION_FAILURE_SIGNAL` | abnormal `ATTEMPT` failure | `RESTART_CHILD` if budget remains, else `BLOCK_LINEAGE + ESCALATE_UP` | yes | `DEGRADED` while retrying, then `LINEAGE_BLOCKED` on exhaustion |
| `ADMISSION_REJECTION_SIGNAL` + `REJECT_WRITER` | wrong writer | `BLOCK_LINEAGE + SURFACE_INCIDENT` | no | derive `LINEAGE_BLOCKED` |
| `ADMISSION_REJECTION_SIGNAL` + `REJECT_ROUTE` | wrong route | `BLOCK_LINEAGE + SURFACE_INCIDENT` | no | derive `LINEAGE_BLOCKED` |
| `ADMISSION_REJECTION_SIGNAL` + `REJECT_SANDBOX` | forbidden sandbox | `BLOCK_LINEAGE + SURFACE_INCIDENT` | no | derive `LINEAGE_BLOCKED` |
| `ADMISSION_REJECTION_SIGNAL` + `REJECT_GATE` | missing simulation gate | `ROUTE_RETURN + SURFACE_INCIDENT` | no | none by default |
| `ADMISSION_REJECTION_SIGNAL` + `REJECT_POLICY` | policy denial | `ROUTE_RETURN + SURFACE_INCIDENT` | no | none by default |
| `VERDICT_RETURN_SIGNAL` + `ACCEPT` | accepted result | `ROUTE_RETURN` | no | none |
| `VERDICT_RETURN_SIGNAL` + `RETURN_FOR_REPAIR` | local fix requested | `ROUTE_RETURN` | no | none |
| `VERDICT_RETURN_SIGNAL` + `REJECT` + `QUALITY/EVIDENCE/DEPENDENCY` | ordinary semantic rejection | `ROUTE_RETURN + SURFACE_INCIDENT` | no | none |
| `VERDICT_RETURN_SIGNAL` + `REJECT` + `BOUNDARY` | semantic boundary breach | `BLOCK_LINEAGE + SURFACE_INCIDENT` | no | derive `LINEAGE_BLOCKED` |
| `VERDICT_RETURN_SIGNAL` + `REJECT` + `INTEGRITY` | invalid chain or taint | `QUARANTINE_LINEAGE + ESCALATE_UP` | no | derive `QUARANTINED` |
| `VERDICT_RETURN_SIGNAL` + `REJECT` + `ADMIN` | admin policy stop | `BLOCK_LINEAGE + SURFACE_INCIDENT` | no | derive `ADMIN_HOLD` or stronger admin path |
| `VERDICT_RETURN_SIGNAL` + `ESCALATE` | upward attention required | `ROUTE_RETURN + ESCALATE_UP` | no | none by default |
| `INTEGRITY_SIGNAL` + `TAINTED` | out-of-band touch | `QUARANTINE_LINEAGE + SURFACE_INCIDENT` | no | keep `TAINTED`, may add `QUARANTINED` |
| `INTEGRITY_SIGNAL` + `ADMIN_HOLD` | admin stop | `SURFACE_INCIDENT` | no | keep `ADMIN_HOLD` |

## Notes

- Retry budget belongs only to execution failure recovery, not to semantic rejection loops.
- Automatic retry eligibility is restricted to `EXECUTION_FAILURE_SIGNAL` at v0.
- `ADMISSION_REJECTION_SIGNAL`, `VERDICT_RETURN_SIGNAL`, and `INTEGRITY_SIGNAL` are retry-ineligible by default even when they are visible and severe.
- `REJECT_GATE` should not trigger process restart by default; the missing input is structural, not a crash.
- Boundary and integrity failures are stricter than ordinary quality failure because they question routing, trust, or capability correctness.
- `SURFACE_INCIDENT` never means silent; it requires log, event, and visible state transition even when no restart occurs.
- `ACCEPT` should return through ordinary runtime routing without manufacturing a supervision incident.

## Retry Eligibility Rule

### v0 rule

- Only `EXECUTION_FAILURE_SIGNAL` may consume automatic retry budget.
- Every other signal family must resolve through return, block, quarantine, or escalation rather than restart loops.

### Reason

- process crash recovery and semantic correction are different things
- retrying admission rejection or ordinary semantic verdicts as if they were crashes would hide structural problems behind restart noise

## Hot Runtime Visibility

### Minimum v0 surface

- `current_supervision_state`
- `retry_count`
- `last_failure_signal_family`
- `last_failure_code`
- `last_supervisor_action`
- `escalation_pending`

### Notes

- `Retrying`, `Degraded`, and `Blocked` should project from supervisor actions and retry counters rather than from free task labels.
- Hot runtime state should remain a projection over canonical receipts, verdicts, overlays, and incidents.
- Do not introduce a separate canonical `SUPERVISION_STATE` family at v0; supervision remains runtime-owned control plus hot projection.

## Supervision State Projection

### v0 rules

- `retry_count` increases only on `EXECUTION_FAILURE_SIGNAL` restart attempts
- admission rejection and semantic rejection do not consume retry budget
- `SURFACE_INCIDENT` guarantees visibility but does not by itself imply restartability

### Minimal projection table

| runtime condition | projected supervision state | notes |
| --- | --- | --- |
| `RESTART_CHILD` scheduled or in flight | `Retrying` | active automatic recovery path |
| `retry_count > 0` and lineage remains runnable | `Degraded` | recovery happened or is happening, but incident history remains visible |
| `BLOCK_LINEAGE` applied | `Blocked` | no automatic restart path remains |
| `QUARANTINE_LINEAGE` applied | `Blocked` | pair with integrity overlay rather than inventing a separate hot lifecycle |
| execution failure with retry budget exhausted | `Blocked` | automatic restart stops; any `Failed` wording should remain a UI alias rather than a separate projected state |
| `ESCALATE_UP` pending | `escalation_pending = true` | orthogonal to blocked or degraded |
| ordinary `ROUTE_RETURN` only | no supervision-state escalation by itself | defer to workflow/result semantics |

### Notes

- Ordinary semantic rejection should not project to `Retrying`.
- `Failed` should be reserved for terminal execution failure or explicit non-restartable runtime failure, not for ordinary semantic verdict rejection.
- `Blocked` is stronger than `Degraded`; once blocked, retrying must cease until replacement or explicit administrative intervention.
- The canonical machine still relies on `FLOW_PHASE`, `ATTENTION_STATE`, and `INTEGRITY_OVERLAY`; supervision visibility is derived from runtime facts rather than authored as another peer family.
- v0 should project a single hot state for retry exhaustion rather than splitting between `Blocked` and `Failed`.

## Replacement Rule

- `REPLACE_CHILD` should create a newly compiled instance, not resume a tainted mid-state.
- v0 may defer active replace implementation, but the supervision binding should reserve replacement for tainted or admin-forced paths rather than for ordinary quality rejection.

## Strategy Mapping Boundary

### v0 rule

- the automatic restart or replace unit should map to the execution child or compiled worker instance, not to the whole dispatch pipeline or session bundle
- dispatch pipeline surfaces are routing and admission infrastructure; they should route, reject, or surface incidents, but they are not normal retry targets
- session bundles or lineage containers are visibility and admin envelopes; stronger bundle-level replacement should remain a distinct runtime or admin path rather than the default meaning of `REPLACE_CHILD`

### Consequence

- retry budget attaches to execution-child failure, not to whole conversation or session containers
- ordinary `REPLACE_CHILD` swaps a fresh runnable child instance first
- stronger replacement of a broader bundle should remain explicit, visible, and rarer than ordinary child restart or replace

## Replacement Activation Path

### v0 rule

1. mark the current child non-runnable through block, quarantine, or equivalent supervisor action as needed
2. preserve existing receipts, incidents, and return lineage rather than reusing mutable mid-state
3. compile or create a fresh child instance under runtime-owned activation
4. route future work through the normal runtime-owned queues and receipts of the new child instance

### Consequence

- replacement is visible and reconstructable rather than silent resume
- replacement does not grant a shortcut around writer, route, or admission rules

## Slice Order

1. Classify typed signal from receipt, verdict, or integrity event
2. Append incident visibility
3. Compute supervisor action from policy plus signal family
4. Apply retry budget only if the action is in the execution-failure restart path
5. Update overlay or blocked state if required
6. Route return or escalation through runtime-owned queues

## Non-Goals For v0

- full distributed supervision trees
- worker-authored restart decisions
- retrying semantic verdict loops as if they were process crashes
- hidden recovery that erases incident history
