# Minimal Top Graph

> Status: pending draft
> Scope: whole-system canonical machine from `Human / NOTA` boundary through `Policy / Operation / Execution`

## Purpose

- define one minimal whole-system graph instead of separate ad hoc mini-machines
- keep canonical machine semantics small enough to stay hard
- show where `simulation`, `submission`, `exception`, `return`, and supervision projection actually attach

## Design Rule

- the whole-system graph should be composed from a small number of reusable graph pieces
- role projection should not create new canonical state families
- canonical truth should stay in `FLOW_PHASE / ATTENTION_STATE / INTEGRITY_OVERLAY`
- transport, verdict, receipt, and supervision semantics should attach through objects and runtime actions rather than by inventing extra peer lifecycle families

## Scope Correction

- concept-conflict governance is important, but it should not be mistaken for the canonical runtime machine
- concept conflict belongs closer to truth/learning governance than to the execution handoff graph
- this file focuses on work lineage flow, not on document-review lifecycle

## Minimal Whole-System Graph

### Layer A: Human boundary graph

1. `Human` produces semantic input
2. `NOTA.IN` ingests boundary input
3. `NOTA.CYCLE` either:
   - answers locally
   - records learn capture
   - emits inward intake for project work
4. `NOTA.OUT` either:
   - returns a Human-facing response
   - emits an `INTAKE_BUNDLE` into internal runtime admission

### Layer B: reusable owned-node graph

This graph is reused by `Policy`, `Operation`, and `Execution`.

1. `IN`
   admitted work becomes visible in the node's owned inbox or owned room
2. `CYCLE`
   the node performs local owned work, critique, evidence assembly, repair, dependency waiting, and verdict authoring when it is the receiver
3. `OUT`
   the node either:
   - finishes locally
   - emits `SUBMISSION_PACKET`
   - emits `EXCEPTION_PACKET`
   - waits for returned feedback before re-entering `CYCLE`

### Layer C: runtime transport graph

1. sender writes governed objects in `OWNER_ROOM`
2. sender appends packet ref to `OWNER_OUTBOX`
3. runtime performs admission in `RUNTIME_ADMISSION_QUEUE`
4. if admitted, runtime appends packet ref to receiver `OWNER_INBOX`
5. receiver resolves inside receiver `OWNER_ROOM`
6. receiver writes `VERDICT`
7. runtime appends returned result to sender `OWNER_RETURN_QUEUE`
8. sender either re-enters `CYCLE`, observes terminal result, or remains blocked pending a stronger path

## Canonical Node Template

### `FLOW_PHASE`

- `IN`
  ingress or newly admitted work is becoming locally visible
- `CYCLE`
  local owned work loop is active
- `OUT`
  the node is emitting, locally closing, or pausing on the edge of ownership transfer

### `ATTENTION_STATE`

- `READY`
  runnable with no active blocker
- `RUNNING`
  currently executing a local step
- `WAITING`
  blocked on dependency, receipt, return, or external window
- `STOPPED`
  no longer runnable in the current cycle

### `INTEGRITY_OVERLAY`

- `TAINTED`
- `QUARANTINED`
- `ADMIN_HOLD`
- `LINEAGE_BLOCKED`

### Rule

- every node in the graph projects these same canonical families
- node meaning changes by writer authority, route topology, and object kinds, not by inventing node-specific state families

## Node-Local CYCLE Loop

### Minimal loop

1. `READY -> RUNNING`
2. local work emits artifacts or partial objects
3. if a local defect is found and locally repairable:
   - stay inside `CYCLE`
   - produce more evidence or revised local objects
4. if waiting on dependency or return path:
   - project `WAITING`
5. once the local result is ready:
   - move toward `OUT`

### Important rule

- local critique and local repair do not count as escalation by themselves
- the upper level should see only admitted upward packets or explicit exceptions, not every failed local attempt

## Upward Lanes

### `SUBMISSION` lane

Use when:

- promotable work exists
- required `simulation_evidence` exists
- sender is ready to request semantic evaluation from the direct parent

Graph edge:

- `node.OUT -> OWNER_OUTBOX(SUBMISSION) -> RUNTIME_ADMISSION_QUEUE -> parent.OWNER_INBOX -> parent.CYCLE`

### `EXCEPTION` lane

Use when:

- a non-local blocker remains after local repair is exhausted
- the sender needs decision, unblock, replace, or override from the direct parent

Graph edge:

- `node.OUT -> OWNER_OUTBOX(EXCEPTION) -> RUNTIME_ADMISSION_QUEUE -> parent.OWNER_INBOX -> parent.CYCLE`

### Rule

- `EXCEPTION` does not bypass `SUBMISSION`
- if the sender is actually claiming promotable work, it must use `SUBMISSION` and satisfy the simulation gate

## Return Lane

### Graph edge

- `receiver.VERDICT -> runtime routing -> sender.OWNER_RETURN_QUEUE -> sender.CYCLE or sender.STOPPED`

### Minimal return meanings

- `ACCEPT`
  sender observes terminal success
- `RETURN_FOR_REPAIR`
  sender re-enters local `CYCLE`
- `REJECT`
  sender observes terminal negative result for the submitted packet
- `ESCALATE`
  receiver becomes sender relative to its parent; original sender waits on returned status

## Packet Resolution Rule

### v0 rule

- once a sender emits an upward packet, the sender-side machine remains on the ownership-transfer edge rather than reopening the packet locally
- while a receiver verdict or stronger upstream status is pending, the sender should project `ATTENTION_STATE = WAITING`
- returned feedback is resolved by runtime-owned routing into `OWNER_RETURN_QUEUE`, not by mutating the original submitted packet
- once the returned object is observed:
  - `RETURN_FOR_REPAIR` may re-enter sender `CYCLE`
  - terminal `ACCEPT` or terminal `REJECT` may end in sender `STOPPED`
  - `ESCALATE` keeps sender waiting on the stronger path rather than pretending the local cycle is complete

### Consequence

- the canonical machine does not need a separate peer phase for "returned but not yet re-entered"
- a UI may briefly render edge-handling states such as `OUT + WAITING`, but that remains projection rather than a new canonical family

## NOTA Position

### v0 rule

- `NOTA` is the Human-facing boundary host, not an internal project-truth superwriter
- `NOTA` may run its own boundary-scoped `IN / CYCLE / OUT` flow
- project-internal semantic lineage begins only when runtime admits boundary output into internal scope

### Consequence

- `NOTA` can answer, clarify, learn, and route
- `NOTA` does not directly mutate `Policy` truth
- once admitted inward, the lineage follows the same reusable owned-node graph as every internal slot

## Boundary Intake Rule

### v0 rule

- the default internal ingress target for admitted `INTAKE_BUNDLE` lineage is `Policy`
- some NOTA-local work may still close entirely at the boundary without spawning project-internal lineage
- `INTAKE_BUNDLE` remains boundary-specific at v0; do not introduce a separate canonical packet lane for it unless boundary routing later needs stronger shared machinery

### Consequence

- boundary-local continuity and internal project execution remain distinguishable
- the machine keeps one reusable internal packet model without inflating boundary ingress into a new peer transport family

## Supervision Attachment

### Rule

- supervision is not another canonical node phase
- supervision attaches to the graph through runtime facts:
  - execution receipts
  - admission receipts
  - verdict returns
  - integrity/admin events

### Projection point

- hot supervision labels such as `Retrying`, `Degraded`, or `Blocked` project over the graph
- they do not replace `FLOW_PHASE`, `ATTENTION_STATE`, or `INTEGRITY_OVERLAY`

## Phase Projection

### v0 rule

- `PHASE` is a Human-facing projection over the graph, not a peer state family
- one phase may summarize several runtime graph positions
- different UI density presets may project the same underlying graph differently without changing truth

## Phase And Cadence Relation

### v0 rule

- `FLOW_PHASE / ATTENTION_STATE / INTEGRITY_OVERLAY` remain the only canonical machine-readable state families
- `PHASE` summarizes graph position, local runnability, and return or supervision context for Human-facing understanding
- cadence protocol organizes Human engagement windows, checkpoints, and handouts across time
- cadence may reference phase summaries, but it must not overwrite or author canonical machine state by itself

### Consequence

- one cadence window may contain several node-local machine transitions
- the same node-local machine state may persist across more than one cadence window
- phase may compress multiple graph positions for readability without becoming a fourth peer machine family

## Stop Conditions

### Minimal v0 stop conditions

- local completion with no admitted upward work pending
- terminal returned verdict observed and no re-entry path remains
- execution failure that ends in non-restartable stop
- admin stop or quarantine that removes runnability in the current cycle

### Rule

- `STOPPED` answers runnability only
- completion, abortion, and failure should stay in verdict or receipt outcome fields

## Minimal ASCII Graph

```text
Human
  -> NOTA.IN
  -> NOTA.CYCLE
  -> NOTA.OUT
     -> Human reply
     -> runtime admission
        -> Policy.IN
        -> Policy.CYCLE
        -> Policy.OUT
           -> local stop
           -> SUBMISSION -> parent inbox -> parent CYCLE -> VERDICT -> return queue -> child CYCLE/STOPPED
           -> EXCEPTION  -> parent inbox -> parent CYCLE -> VERDICT -> return queue -> child CYCLE/STOPPED

Reusable internal template:
IN -> CYCLE -> OUT
ATTENTION overlays: READY / RUNNING / WAITING / STOPPED
INTEGRITY overlays: TAINTED / QUARANTINED / ADMIN_HOLD / LINEAGE_BLOCKED
Supervision: projected from runtime facts, not a peer canonical family
```

## Open Questions

- none mounted at v0; reopen only if boundary ingress later needs a stronger shared transport model
