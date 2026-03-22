# Minimal Registry Cut

> Status: cold draft with approved core choreography points
> Scope: v0 OS/compiler/runtime boundary

## Purpose

- define the smallest registry set that can actually reject boundary violations
- keep registry count low before runtime implementation hardens
- give `simulation_evidence` a governed object shape instead of leaving it as narrative text
- harden by reducing writable row surface before adding new semantic layers

## Critical Correction

- Do not force every object kind to one fixed owner slot.
- Reusable kinds such as `SUBMISSION_PACKET`, `EXCEPTION_PACKET`, `SIMULATION_EVIDENCE`, and `RECEIPT` can appear in more than one slot lineage.
- Writer admissibility belongs to `control_policy`, not to `object_kind` alone.
- If a kind is truly slot-local, create a specialized kind code rather than smuggling ownership through prose.
- Prefer `derived, not declared` when runtime can compute route, effective policy, or return behavior from topology plus registry context.

## Registry Set

- `object_kind_registry`
- `state_code_registry`
- `control_policy_registry`

This is the minimum cut because:

- `object_kind_registry` defines what the object is
- `state_code_registry` defines machine-readable state projection
- `control_policy_registry` defines who may write, where it may route, which gates apply, and which sandbox/admission/projection checks must pass

## object_kind_registry

### Minimal columns

- `kind_code`
- `kind_family_code`
- `schema_family_code`
- `route_family_code`
- `storage_family_code`
- `default_control_policy_code`

### v0 seed kinds

| kind_code | kind_family_code | schema_family_code | route_family_code | storage_family_code | default_control_policy_code |
| --- | --- | --- | --- | --- | --- |
| `INTAKE_BUNDLE` | `BOUNDARY_PACKET` | `INTAKE_PAYLOAD` | `HUMAN_NOTA_BOUNDARY` | `OBJECT_LEDGER` | `CP_NOTA_BOUNDARY` |
| `LEARN_CAPTURE` | `LEARNING_OBJECT` | `LEARN_PAYLOAD` | `LOCAL_ONLY` | `OBJECT_LEDGER` | `CP_LEARN_CAPTURE` |
| `SUBMISSION_PACKET` | `UPWARD_PACKET` | `SUBMISSION_PAYLOAD` | `UPWARD_ONLY` | `OBJECT_LEDGER` | `CP_UPWARD_SUBMISSION` |
| `EXCEPTION_PACKET` | `UPWARD_PACKET` | `EXCEPTION_PAYLOAD` | `UPWARD_OR_EXCEPTION` | `OBJECT_LEDGER` | `CP_UPWARD_EXCEPTION` |
| `SIMULATION_EVIDENCE` | `EVIDENCE_OBJECT` | `EVIDENCE_PAYLOAD` | `UPWARD_ONLY` | `OBJECT_LEDGER` | `CP_LOCAL_EVIDENCE` |
| `VERDICT` | `VERDICT_OBJECT` | `VERDICT_PAYLOAD` | `LOCAL_ONLY` | `OBJECT_LEDGER` | `CP_RECEIVER_VERDICT` |
| `RECEIPT` | `RUNTIME_OBJECT` | `RECEIPT_PAYLOAD` | `RUNTIME_INTERNAL` | `OBJECT_LEDGER` | `CP_RUNTIME_RECEIPT` |
| `TAINT_EVENT` | `INTEGRITY_EVENT` | `INTEGRITY_EVENT_PAYLOAD` | `RUNTIME_INTERNAL` | `EVENT_LEDGER` | `CP_RUNTIME_INTEGRITY` |
| `ADMIN_EVENT` | `INTEGRITY_EVENT` | `ADMIN_EVENT_PAYLOAD` | `RUNTIME_INTERNAL` | `EVENT_LEDGER` | `CP_RUNTIME_INTEGRITY` |

## state_code_registry

### Minimal columns

- `state_code`
- `state_family_code`
- `composition_mode_code`

### v0 family rule

- `FLOW_PHASE` is `exclusive`
- `ATTENTION_STATE` is `exclusive`
- `INTEGRITY_OVERLAY` is `set`

### v0 seed codes

| state_family_code | state_code | composition_mode_code | note |
| --- | --- | --- | --- |
| `FLOW_PHASE` | `IN` | `exclusive` | ingress or pre-ownership intake |
| `FLOW_PHASE` | `CYCLE` | `exclusive` | owned local work loop |
| `FLOW_PHASE` | `OUT` | `exclusive` | emission, handoff, or local completion edge |
| `ATTENTION_STATE` | `READY` | `exclusive` | runnable with no active blocker |
| `ATTENTION_STATE` | `RUNNING` | `exclusive` | currently executing local step |
| `ATTENTION_STATE` | `WAITING` | `exclusive` | blocked on dependency, receipt, or window |
| `ATTENTION_STATE` | `STOPPED` | `exclusive` | no longer runnable in the current cycle |
| `INTEGRITY_OVERLAY` | `TAINTED` | `set` | out-of-band or trust-degrading influence detected |
| `INTEGRITY_OVERLAY` | `QUARANTINED` | `set` | isolated from normal promotion paths |
| `INTEGRITY_OVERLAY` | `ADMIN_HOLD` | `set` | paused by explicit runtime or human break-glass action |
| `INTEGRITY_OVERLAY` | `LINEAGE_BLOCKED` | `set` | lineage cannot promote until repair or replacement |

### Notes

- Do not encode detailed blocker reasons into `ATTENTION_STATE`; keep blocker causes in separate objects or refs.
- `STOPPED` only says the node is halted; normal completion vs abnormal termination should live in verdict, receipt, or a later terminal-outcome field rather than splitting attention state.
- Do not turn `REJECTED` into a durable integrity overlay; rejection belongs to verdict objects, not to long-lived node identity by default.

## control_policy_registry

### Minimal columns

- `control_policy_code`
- `writer_policy_code`
- `route_policy_code`
- `gate_policy_code`
- `sandbox_policy_code`
- `admission_policy_code`
- `projection_policy_code`

### v0 subcode seeds

#### writer_policy_code

- `WP_OWNER_APPEND`
- `WP_RUNTIME_APPEND`
- `WP_OWNER_OR_RUNTIME_APPEND`
- `WP_NOTA_BOUNDARY_ONLY`

#### route_policy_code

- `RP_HUMAN_NOTA_BOUNDARY`
- `RP_LOCAL_ONLY`
- `RP_UPWARD_ONLY`
- `RP_UPWARD_OR_EXCEPTION`
- `RP_RUNTIME_INTERNAL`

#### gate_policy_code

- `GP_NONE`
- `GP_SIM_REQUIRED`
- `GP_VERDICT_REQUIRED`
- `GP_SIM_AND_VERDICT_REQUIRED`

#### sandbox_policy_code

- `SP_NONE`
- `SP_READ_ONLY_ROOM`
- `SP_WORKTREE_RW_ALLOWLIST`
- `SP_RUNTIME_ADMIN_ONLY`

#### admission_policy_code

- `AP_STORAGE_ALWAYS`
- `AP_STORAGE_AND_COLD_ALWAYS`
- `AP_STORAGE_COLD_HOT_ON_ATTENTION`

#### projection_policy_code

- `PP_HOT_NEVER`
- `PP_HOT_ACTIVE_ONLY`
- `PP_HOT_ON_ATTENTION_OR_REJECT`

### v0 composed policies

| control_policy_code | writer_policy_code | route_policy_code | gate_policy_code | sandbox_policy_code | admission_policy_code | projection_policy_code |
| --- | --- | --- | --- | --- | --- | --- |
| `CP_NOTA_BOUNDARY` | `WP_NOTA_BOUNDARY_ONLY` | `RP_HUMAN_NOTA_BOUNDARY` | `GP_NONE` | `SP_NONE` | `AP_STORAGE_AND_COLD_ALWAYS` | `PP_HOT_ON_ATTENTION_OR_REJECT` |
| `CP_LEARN_CAPTURE` | `WP_OWNER_APPEND` | `RP_LOCAL_ONLY` | `GP_NONE` | `SP_NONE` | `AP_STORAGE_AND_COLD_ALWAYS` | `PP_HOT_ACTIVE_ONLY` |
| `CP_LOCAL_EVIDENCE` | `WP_OWNER_APPEND` | `RP_UPWARD_ONLY` | `GP_NONE` | `SP_WORKTREE_RW_ALLOWLIST` | `AP_STORAGE_AND_COLD_ALWAYS` | `PP_HOT_ACTIVE_ONLY` |
| `CP_UPWARD_SUBMISSION` | `WP_OWNER_APPEND` | `RP_UPWARD_ONLY` | `GP_SIM_REQUIRED` | `SP_WORKTREE_RW_ALLOWLIST` | `AP_STORAGE_AND_COLD_ALWAYS` | `PP_HOT_ON_ATTENTION_OR_REJECT` |
| `CP_UPWARD_EXCEPTION` | `WP_OWNER_APPEND` | `RP_UPWARD_OR_EXCEPTION` | `GP_NONE` | `SP_WORKTREE_RW_ALLOWLIST` | `AP_STORAGE_AND_COLD_ALWAYS` | `PP_HOT_ON_ATTENTION_OR_REJECT` |
| `CP_RECEIVER_VERDICT` | `WP_OWNER_APPEND` | `RP_LOCAL_ONLY` | `GP_NONE` | `SP_NONE` | `AP_STORAGE_AND_COLD_ALWAYS` | `PP_HOT_ON_ATTENTION_OR_REJECT` |
| `CP_RUNTIME_RECEIPT` | `WP_RUNTIME_APPEND` | `RP_RUNTIME_INTERNAL` | `GP_NONE` | `SP_RUNTIME_ADMIN_ONLY` | `AP_STORAGE_ALWAYS` | `PP_HOT_NEVER` |
| `CP_RUNTIME_INTEGRITY` | `WP_RUNTIME_APPEND` | `RP_RUNTIME_INTERNAL` | `GP_NONE` | `SP_RUNTIME_ADMIN_ONLY` | `AP_STORAGE_ALWAYS` | `PP_HOT_ON_ATTENTION_OR_REJECT` |

### Notes

- Retry policy is intentionally excluded here; it belongs to supervision policy rather than to object admission/routing control.
- Projection is one subdomain of control policy, not a sibling top registry.

## simulation_evidence

### Minimal object skeleton

- `evidence_ref`
- `lineage_ref`
- `producer_slot_code`
- `subject_ref`
- `attempt_receipt_ref` (required)
- `artifact_manifest_ref` (required)
- `summary_text`
- `created_at`

### Minimal artifact families

- `OUTPUT`
- `LOG`
- `SCREENSHOT`
- `RESULT`
- `TRACE`

### Admissibility checks

1. `subject_ref` must resolve to a governed object in the same lineage.
2. `attempt_receipt_ref` is mandatory and must resolve to a runtime or room-generated receipt in the same lineage.
3. `artifact_manifest_ref` is mandatory and must resolve to at least one stored artifact object.
4. Artifact refs must originate from a governed room or from an explicit import receipt.
5. Artifact families must satisfy the target object's active `gate_policy_code`.
6. `summary_text` may explain evidence, but never satisfies a gate by itself and never substitutes for the required receipt plus artifact chain.
7. The evidence object must pass its own writer and route checks before it can be considered by an upward submission.
8. Evidence carrying `TAINTED` or `QUARANTINED` overlay cannot satisfy upward promotion gates unless a stronger admin path is explicitly defined later.
9. `simulation_evidence` unlocks submission eligibility only; it does not self-promote and does not replace verdict objects.

### Derived fields

- `effective_control_policy_code` should be resolved by runtime from `kind_code`, lineage context, and registry state rather than trusted from model-written evidence rows.

## Resolved Boundary

### v0 rule

- `LEARN_CAPTURE` stays a first-class local learning object kind at v0 rather than being normalized into packet plus projection
- `VERDICT` remains `LOCAL_ONLY` at v0; upward attention should flow through `ESCALATE` and new routed objects rather than by reusing verdict rows as upward packets
- `CP_LOCAL_EVIDENCE` stays owner-appended at v0; runtime-generated evidence should surface through separate receipts or runtime-owned objects rather than mixed writer append on one evidence row

### Consequence

- learn landing stays explicit and reconstructable inside owned scope
- verdict semantics stay local and evaluative instead of turning into a shadow routing lane
- machine-generated artifacts remain visible without weakening single-writer boundaries on evidence objects

## Engagement-Profile Boundary

### v0 rule

- no separate compiler-core engagement-profile registry is required at v0
- engagement presets may tune surfacing density, budget, or view behavior downstream of canonical routing and truth policy
- engagement presets must not rewrite writer, route, gate, admission, or canonical truth semantics

## verdict

### Minimal object skeleton

- `verdict_ref`
- `lineage_ref`
- `evaluator_slot_code`
- `subject_ref`
- `verdict_code`
- `reason_family_code`
- `reason_code`
- `terminal_outcome_code`
- `evidence_refs`
- `created_at`

### Minimal verdict codes

- `ACCEPT`
- `RETURN_FOR_REPAIR`
- `REJECT`
- `ESCALATE`

### Minimal reason families

- `QUALITY`
- `EVIDENCE`
- `DEPENDENCY`
- `BOUNDARY`
- `INTEGRITY`
- `ADMIN`

### Notes

- `REJECT` belongs to verdict semantics, not to integrity overlay.
- Ordinary rejection does not mutate node integrity by itself.
- `terminal_outcome_code` stays a verdict or receipt field at v0; do not create a separate top-level state family unless multiple projections truly require it later.

## receipt

### Minimal object skeleton

- `receipt_ref`
- `lineage_ref`
- `runtime_emitter_code`
- `subject_ref`
- `receipt_kind_code`
- `receipt_status_code`
- `admission_result_code`
- `terminal_outcome_code`
- `artifact_refs`
- `created_at`

### Minimal receipt kind codes

- `ATTEMPT`
- `ADMISSION`
- `ARTIFACT_CAPTURE`
- `IMPORT`
- `ROOM_EXECUTION`
- `RUNTIME_ACTION`

### Minimal receipt status codes

- `STARTED`
- `SUCCEEDED`
- `FAILED`
- `CANCELLED`

### Notes

- Receipts record mechanical fact, not evaluative judgment.
- A failed receipt does not automatically mean the lineage is tainted.
- `attempt_receipt_ref` inside `simulation_evidence` should point to a receipt with `receipt_kind_code = ATTEMPT` and a terminal status.
- `admission_result_code` is only populated for `receipt_kind_code = ADMISSION`.

## minimal code families

### admission_result_code

- `ADMITTED`
- `REJECT_WRITER`
- `REJECT_ROUTE`
- `REJECT_GATE`
- `REJECT_SANDBOX`
- `REJECT_POLICY`

### Notes

- Keep admission failure explanation inside a single code family before introducing finer receipt kinds.
- `REJECT_POLICY` is the residual bucket for admission-policy denial that is not better expressed as writer, route, gate, or sandbox failure.

### reason_code

#### `QUALITY`

- `QUALITY_INSUFFICIENT`
- `QUALITY_NONCONFORMANT`

#### `EVIDENCE`

- `EVIDENCE_MISSING`
- `EVIDENCE_INVALID`

#### `DEPENDENCY`

- `DEPENDENCY_UNREADY`
- `DEPENDENCY_BLOCKED`

#### `BOUNDARY`

- `BOUNDARY_WRITER`
- `BOUNDARY_ROUTE`
- `BOUNDARY_SANDBOX`

#### `INTEGRITY`

- `INTEGRITY_TAINT`
- `INTEGRITY_CHAIN_INVALID`

#### `ADMIN`

- `ADMIN_HOLD`
- `ADMIN_OVERRIDE_REQUIRED`

### Notes

- `reason_family_code` remains coarse and projection-friendly.
- `reason_code` carries the minimum extra precision needed for runtime routing and later analytics.
- Boundary and integrity codes are the only default families that may derive integrity overlays at v0.

### return_action_code

- `OBSERVE_FINAL`
- `REENTER_LOCAL`
- `WAIT_UPSTREAM`
- `REOPEN_BLOCKED`

### Notes

- `return_action_code` is a runtime routing hint attached to returned feedback, not a replacement for verdict semantics.
- The same verdict code may map to different return actions later under stronger policy, but v0 keeps the mapping simple.
- If persisted at all, `return_action_code` should be written by runtime as derived routing metadata rather than declared by model-authored verdict rows.

## minimal field ownership cut

### Field classes

- `model-authored`
- `runtime-derived`
- `runtime-only`

### Rule of use

- `model-authored` fields are the smallest fact set the owning slot must supply.
- `runtime-derived` fields are computed from registry, topology, state, and returned objects; they are not trusted as free row declarations.
- `runtime-only` fields are written only by runtime and define identity, timing, queue placement, admission outcome, or delivery edges.

### v0 examples

| object kind | model-authored | runtime-derived | runtime-only |
| --- | --- | --- | --- |
| `SIMULATION_EVIDENCE` | `subject_ref`, `attempt_receipt_ref`, `artifact_manifest_ref`, `summary_text` | `effective_control_policy_code` | `evidence_ref`, `created_at` |
| `SUBMISSION_PACKET` | `subject_ref`, `evidence_refs` | `effective_target_slot_ref`, `effective_control_policy_code` | `packet_ref`, `created_at`, admission queue placement, delivery edge refs |
| `EXCEPTION_PACKET` | `blocked_subject_ref`, `blocker_refs`, `exception_family_code`, `ask_code` | `effective_target_slot_ref`, `effective_control_policy_code` | `exception_ref`, `created_at`, admission queue placement, delivery edge refs |
| `VERDICT` | `subject_ref`, `verdict_code`, `reason_family_code`, `reason_code`, `evidence_refs`, `terminal_outcome_code` | `effective_return_action_code` | `verdict_ref`, `created_at`, return queue placement |
| `RECEIPT` | none | none | `receipt_ref`, `runtime_emitter_code`, `receipt_kind_code`, `receipt_status_code`, `admission_result_code`, `artifact_refs`, `terminal_outcome_code`, `created_at` |

### Notes

- Object ids and timestamps are runtime-only across every object kind.
- Queue placement and delivery edges are runtime-only even when they refer to model-authored objects.
- `effective_target_slot_ref` is runtime-derived for normal upward lanes; explicit target override remains out of scope for v0.
- `effective_control_policy_code` is runtime-derived from registry plus context even when a default policy is mounted from object kind.
- `effective_return_action_code` is runtime-derived from verdict semantics and routing policy rather than selected by the model.

## minimal writer-authority matrix

### Core rule

- Single-writer ownership applies to effective write authority, not to prestige or hierarchy naming.
- `Policy` is the highest internal strategy slot, but it is not a global mutable superuser.
- A stronger slot may decide, replace, block, or escalate across slots only through routed objects and runtime-owned control paths, not by directly rewriting foreign owned truth.
- `Human` sovereignty enters the system either as normal semantic boundary input through `NOTA` or as break-glass runtime control; it does not directly write project-internal canonical truth.

### Surface matrix

| authority actor | boundary bundle ledger | owned room and owned outbox | foreign room or foreign outbox | inbox / return queue | runtime objects and delivery surfaces |
| --- | --- | --- | --- | --- | --- |
| `Human` | no direct canonical write; semantic input must enter through `NOTA` | none | forbidden | none | break-glass may request runtime control, but runtime writes the resulting `ADMIN_EVENT` or stronger action |
| `NOTA` | may write `INTAKE_BUNDLE` and NOTA-local boundary records | none by default | forbidden | read-only hot/view surfaces only | none |
| `Policy` | none by default | may write `model-authored` fields for `LEARN_CAPTURE`, `SIMULATION_EVIDENCE`, `SUBMISSION_PACKET`, `EXCEPTION_PACKET`, and local owned subjects in `Policy` scope | forbidden | read-only; resolution happens by new owned writes in `OWNER_ROOM` | none |
| `Operation` | none by default | may write `model-authored` fields for local owned work objects, `SIMULATION_EVIDENCE`, `SUBMISSION_PACKET`, `EXCEPTION_PACKET`, and receiver-owned `VERDICT` rows in `Operation` scope | forbidden | read-only; resolution happens by new owned writes in `OWNER_ROOM` | none |
| `Execution` | none by default | may write `model-authored` fields for local owned work objects, `SIMULATION_EVIDENCE`, `SUBMISSION_PACKET`, `EXCEPTION_PACKET`, and receiver-owned `VERDICT` rows in `Execution` scope | forbidden | read-only; resolution happens by new owned writes in `OWNER_ROOM` | none |
| `Runtime` | none | may append only runtime-owned rows or metadata in owner scopes where policy allows | may deliver, reject, quarantine, or block, but never authors semantic work on behalf of the foreign owner | exclusive append to `RUNTIME_ADMISSION_QUEUE`, `OWNER_INBOX`, and `OWNER_RETURN_QUEUE` | exclusive writer of `RECEIPT`, `TAINT_EVENT`, `ADMIN_EVENT`, queue placement, delivery edges, ids, timestamps, and other `runtime-only` fields |

### Field-class matrix

| authority actor | `model-authored` fields | `runtime-derived` fields | `runtime-only` fields |
| --- | --- | --- | --- |
| `Human` | none directly | none | none |
| `NOTA` | boundary-scope only | none | none |
| `Policy` | only in `Policy`-owned scope | none | none |
| `Operation` | only in `Operation`-owned scope | none | none |
| `Execution` | only in `Execution`-owned scope | none | none |
| `Runtime` | only for runtime-owned object kinds such as `RECEIPT`, `TAINT_EVENT`, `ADMIN_EVENT` | exclusive computation authority | exclusive write authority |

### Notes

- `OWNER_INBOX` and `OWNER_RETURN_QUEUE` are delivery surfaces, not semantic authoring surfaces.
- The receiving slot does not edit the delivered packet in place; it resolves by writing new owned rows in its own room.
- A returned verdict does not grant write access back into the receiver room.
- `Policy` may own policy-scope truth, but it cannot directly mutate `Operation` or `Execution` owned rows just because it sits higher in the control tree.
- `NOTA` is allowed to translate Human intent into boundary objects, but it is not allowed to mutate project-level policy truth or project issue state directly.
- Any direct Human intervention inside an inner slot or runtime session should derive taint and/or admin overlays rather than silently modifying semantic truth.

### Break-glass rule

- break-glass actions remain limited to `observe / pause / stop / quarantine / revoke / replace`
- break-glass is a runtime control path, not a semantic authoring path
- if a Human touches an inner runtime directly, the runtime should emit `ADMIN_EVENT` and, when promotion safety is affected, `TAINT_EVENT`
- break-glass may halt or replace a lineage, but it must not be interpreted as ordinary in-band work completion

## submission_packet

### Minimal object skeleton

- `packet_ref`
- `lineage_ref`
- `sender_slot_code`
- `subject_ref`
- `evidence_refs`
- `created_at`

### Admissibility checks

1. `subject_ref` must resolve to a governed object in the same lineage.
2. `evidence_refs` must resolve to admissible `SIMULATION_EVIDENCE` objects in the same lineage.
3. Runtime must derive the normal target from lineage topology and route policy rather than trusting a model-written target field.
4. Runtime must emit an `ADMISSION` receipt before the packet becomes visible to the receiver queue.
5. Admission failure stops delivery locally; receiver does not write a verdict for packets that never passed runtime admission.
6. The packet may propose promotable work, but it never self-promotes and never mutates target-owned truth directly.

### Notes

- `SUBMISSION_PACKET` is the normal upward lane for promotable work.
- At v0, `UPWARD_ONLY` should be read as the normal direct-parent route; any skip-level bypass remains pending rather than silently allowed.
- `effective_control_policy_code` should be resolved by runtime from registry plus context rather than carried as a freely writable packet field.

## exception_packet

### Minimal object skeleton

- `exception_ref`
- `lineage_ref`
- `sender_slot_code`
- `blocked_subject_ref`
- `blocker_refs`
- `exception_family_code`
- `ask_code`
- `created_at`

### Minimal exception families

- `DEPENDENCY_BLOCK`
- `AUTHORITY_BLOCK`
- `INTEGRITY_BLOCK`
- `RESOURCE_BLOCK`
- `ADMIN_BLOCK`

### Minimal ask codes

- `UNBLOCK`
- `DECIDE`
- `REPLACE`
- `OVERRIDE`

### Admissibility checks

1. `blocker_refs` must resolve to at least one governed blocker object such as verdict, receipt, dependency, or integrity event.
2. Runtime must derive the normal target from lineage topology and route policy rather than trusting a model-written target field.
3. Runtime must emit an `ADMISSION` receipt before the exception enters the receiver queue.
4. `EXCEPTION_PACKET` is admissible for blocked-state reporting or decision requests, not for smuggling promotable deliverables around submission gates.
5. If the packet claims work readiness or promotable output, it is misclassified and should be rejected as a boundary failure rather than treated as a valid exception.

### Notes

- `EXCEPTION_PACKET` exists to reduce invalid Human wake-up by routing only unresolved non-local blockers upward.
- Local quality defects or locally repairable evidence gaps should not use the exception lane by default.
- `EXCEPTION_PACKET` is not a shadow submission lane and must not be used to bypass evidence-gated promotion.
- `effective_control_policy_code` should be resolved by runtime from registry plus context rather than carried as a freely writable packet field.

## terminal outcome

### v0 rule

- Keep terminal outcome as a schema field carried by verdict or receipt objects.
- Do not promote terminal outcome to a separate state family at v0.

### Minimal terminal outcome codes

- `NONE`
- `COMPLETED`
- `ABORTED`
- `FAILED`

### Notes

- `ATTENTION_STATE.STOPPED` answers whether the node is still runnable.
- `terminal_outcome_code` answers how the current attempt or evaluation ended.
- This split keeps runnability, evaluation, and trust semantics separate.

## rejection and integrity derivation

### v0 rule

- `REJECT` does not derive `LINEAGE_BLOCKED` by default.
- Only boundary or integrity class failures may derive integrity overlays.

### Minimal derivation policy

| verdict_code | reason_family_code | default effect | integrity derivation |
| --- | --- | --- | --- |
| `RETURN_FOR_REPAIR` | `QUALITY` | local repair loop | none |
| `RETURN_FOR_REPAIR` | `EVIDENCE` | gather missing evidence | none |
| `RETURN_FOR_REPAIR` | `DEPENDENCY` | wait or reroute dependency | none |
| `REJECT` | `QUALITY` | reject submission | none |
| `REJECT` | `EVIDENCE` | reject submission | none |
| `REJECT` | `DEPENDENCY` | reject submission | none |
| `REJECT` | `BOUNDARY` | reject submission | derive `LINEAGE_BLOCKED` |
| `REJECT` | `INTEGRITY` | reject submission | derive `LINEAGE_BLOCKED` and/or `QUARANTINED` |
| `REJECT` | `ADMIN` | reject submission | derive `ADMIN_HOLD` or stronger admin path |
| `ESCALATE` | any | upward attention required | no default integrity derivation |

### Notes

- Boundary failure means wrong writer, wrong route, forbidden sandbox, or equivalent hard-constraint breach.
- Integrity failure means tainted lineage, invalid evidence chain, or trust-degrading intervention.
- Quality and evidence insufficiency are normal workflow outcomes and should not poison lineage identity.

## minimal choreography

### Normal submission lane

1. Local execution begins and runtime emits an `ATTEMPT` receipt.
2. The governed room or runtime captures artifacts and emits any needed `ARTIFACT_CAPTURE` receipts.
3. The producing slot writes one or more `SIMULATION_EVIDENCE` objects referencing the terminal attempt receipt plus artifact manifest.
4. The producing slot writes a `SUBMISSION_PACKET` referencing the subject and evidence set.
5. Runtime checks writer, route, gate, sandbox, and admission policy, then emits an `ADMISSION` receipt.
6. Only admitted packets enter the receiver queue.
7. The receiver writes a `VERDICT`; it never edits the incoming packet in place.
8. Follow-up happens through new owner-written objects or local repair, not by mutating the resolved packet.

### Exception lane

1. Local repair, critique, and retry budget are exhausted, or a non-local blocker remains.
2. The producing slot writes an `EXCEPTION_PACKET` with blocker refs and an explicit ask code.
3. Runtime checks writer, route, sandbox, and admission policy, then emits an `ADMISSION` receipt.
4. Only admitted exceptions enter the receiver queue.
5. The receiver writes a `VERDICT` or equivalent response object in its own scope.
6. The exception lane never substitutes for the submission lane when promotable work claims are involved.

### Core invariants

- Runtime admission precedes receiver verdict.
- Receiver verdict follows admitted packets only.
- Submission and exception are distinct lanes with distinct admissibility semantics.
- Evidence unlocks eligibility; verdict resolves meaning; receipts record mechanical fact.
- No object in this lane mutates upper-slot owned truth directly.
- A single `ADMISSION` receipt kind is sufficient at v0; finer-grained admission-reject receipt families remain deferred.

## runtime ownership graph

### Queue and room families

#### `OWNER_ROOM(slot_ref, lineage_ref)`

- purpose: local working room for owned execution, evidence assembly, and verdict authoring
- primary writer: owning slot
- runtime writes allowed: receipts and integrity events only
- foreign slot write: forbidden

#### `OWNER_OUTBOX(slot_ref, lineage_ref, lane_code)`

- purpose: sender-owned emission buffer for upward packets
- lane codes at v0: `SUBMISSION`, `EXCEPTION`
- primary writer: owning slot
- runtime reads for admission
- foreign slot write: forbidden

#### `RUNTIME_ADMISSION_QUEUE(lineage_ref)`

- purpose: runtime-only staging area for writer, route, gate, sandbox, and admission checks
- writer: runtime only
- reader: runtime only
- owner slots never write this queue directly

#### `OWNER_INBOX(slot_ref, lineage_ref)`

- purpose: receiver-owned queue of admitted upward packets
- runtime appends admitted packet refs
- owning slot reads and resolves
- foreign slot write: forbidden

#### `OWNER_RETURN_QUEUE(slot_ref, lineage_ref)`

- purpose: local return path for verdict refs and follow-up directives flowing back to the sender scope
- runtime appends routed return refs
- owning slot reads and continues work or observes terminal feedback
- foreign slot write: forbidden

### Minimal lane flow

#### Submission lane

1. Sender writes work artifacts and evidence in `OWNER_ROOM`.
2. Sender emits `SUBMISSION_PACKET` into `OWNER_OUTBOX(..., SUBMISSION)`.
3. Runtime copies packet ref into `RUNTIME_ADMISSION_QUEUE`.
4. Runtime validates writer, route, gate, sandbox, and admission policy, then emits `ADMISSION` receipt.
5. If admitted, runtime appends the packet ref to receiver `OWNER_INBOX`.
6. Receiver resolves inside its own `OWNER_ROOM` and writes `VERDICT`.
7. Runtime appends the receiver verdict ref to sender `OWNER_RETURN_QUEUE`.
8. If the verdict is `RETURN_FOR_REPAIR`, the sender re-enters local work from its own return queue.
9. If the verdict is `ESCALATE`, the receiver becomes a sender relative to its parent and uses its own `OWNER_OUTBOX`.

#### Exception lane

1. Sender records blocker objects in `OWNER_ROOM`.
2. Sender emits `EXCEPTION_PACKET` into `OWNER_OUTBOX(..., EXCEPTION)`.
3. Runtime copies packet ref into `RUNTIME_ADMISSION_QUEUE`.
4. Runtime validates route and admission constraints, then emits `ADMISSION` receipt.
5. If admitted, runtime appends the packet ref to receiver `OWNER_INBOX`.
6. Receiver writes its response in its own `OWNER_ROOM`.
7. Any return path back downward is routed by runtime into the appropriate `OWNER_RETURN_QUEUE` or equivalent local follow-up queue, never by direct foreign-room write.

### Ownership invariants

- Sender-owned rooms and outboxes are append-only to the sender plus runtime receipts; receivers never write them.
- Receiver-owned inboxes are append-only to runtime delivery; senders never write them directly.
- Runtime admission is the only bridge from sender outbox to receiver inbox.
- Verdict feedback, return-for-repair, and unblock responses flow through runtime-owned routing, not direct cross-slot mutation.
- Escalation creates new owner-written packets from the current owner scope; it never forwards foreign-owned mutable state upward.
- `NOTA` and `Human` do not bypass these queues for normal semantic routing.

### Return semantics by verdict

#### `ACCEPT`

- runtime appends verdict ref to sender `OWNER_RETURN_QUEUE`
- runtime sets `return_action_code = OBSERVE_FINAL`
- sender observes acceptance as returned feedback, not as foreign-room mutation
- current submitted packet becomes terminal in the sender scope unless later linked work opens a new cycle

#### `RETURN_FOR_REPAIR`

- runtime appends verdict ref to sender `OWNER_RETURN_QUEUE`
- runtime sets `return_action_code = REENTER_LOCAL`
- sender re-enters its own local work cycle from returned feedback
- repair remains local unless a non-local blocker later forces an exception lane

#### `REJECT`

- runtime appends verdict ref to sender `OWNER_RETURN_QUEUE`
- runtime sets `return_action_code = OBSERVE_FINAL`
- sender observes a terminal negative result for the current packet
- no integrity derivation occurs unless the rejection reason family explicitly triggers one

#### `ESCALATE`

- runtime appends verdict ref to sender `OWNER_RETURN_QUEUE`
- runtime sets `return_action_code = WAIT_UPSTREAM`
- receiver becomes the new sender relative to its parent and emits any further packet from its own `OWNER_OUTBOX`
- original sender receives status visibility without gaining write access to the escalated scope
