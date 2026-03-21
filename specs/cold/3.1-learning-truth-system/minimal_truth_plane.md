# Minimal Truth Plane

> Status: pending draft
> Scope: v0 truth ownership, admission, and projection across `Storage Truth / Cold Memory / Hot Working Set`

## Purpose

- define the smallest truth-plane split that can preserve evidence, fuzzy memory, approved truth, and hot attention without conflation
- bind `Storage / Cold / Hot` to writer authority and policy codes rather than to prose intent
- place concept conflict and document coherence in the truth system instead of the runtime handoff machine

## Design Rule

- storage is the canonical capture plane
- cold is the curated memory plane
- hot is the reconstructable working projection plane
- the same object may be present in more than one plane, but each plane has distinct ownership and admission semantics
- hot must never become a second canonical authoring plane

## Plane Semantics

### `Storage Truth`

Purpose:

- capture governed events and objects with provenance
- preserve runtime facts, boundary inputs, artifacts, and semantic submissions before later curation

Properties:

- canonical lowest anchor
- append-oriented
- may contain both approved and fuzzy material
- not optimized for Human working memory

Typical contents:

- `SESSION` and dialog substreams
- runtime and OS logs
- `RECEIPT`
- `TAINT_EVENT`
- `ADMIN_EVENT`
- artifact manifests and artifact blobs
- `INTAKE_BUNDLE`
- governed packets, verdicts, evidence, and other admitted objects

### `Cold Memory`

Purpose:

- curate durable memory that can reconstruct hot views and support later reasoning
- separate approved, fuzzy, conflicted, and superseded material without deleting provenance

Properties:

- selective rather than raw
- concept-aware
- review and coherence aware
- still canonical

Typical contents:

- top documents
- cold drafts
- `DECISION`
- `VISION`
- `TODO`
- `MEMORY_FRAGMENT`
- `MEMORY_LINK`
- conflict and coherence metadata

### `Hot Working Set`

Purpose:

- present the currently relevant subset of truth for active attention
- keep Human and active slots inside the `1 hotspot + <=3 chunks` budget

Properties:

- reconstructable projection
- disposable and refreshable
- not the place where canonical truth originates

Typical contents:

- active top documents
- current inbox/return status summaries
- current supervision projection
- current phase projection
- active pending asks or blockers

## Writer Ownership By Plane

| plane | canonical writer classes | non-writers |
| --- | --- | --- |
| `Storage Truth` | runtime for runtime facts; owning semantic slot for governed semantic objects; `NOTA` for boundary-scoped objects | Human direct canonical write, foreign-slot mutation, hot-only editing |
| `Cold Memory` | governed curation path from runtime or owning slot under admission policy; Arch/NOTA may curate docs through owned scope | direct runtime-handoff mutation by foreign slots, Human direct project-truth mutation |
| `Hot Working Set` | runtime/view projection only | direct semantic authoring by Human, `NOTA`, or inner slots |

### Notes

- `Hot Working Set` may be influenced by projection policy, but it is not a writer-owned semantic truth plane.
- `Storage Truth` preserves provenance even for items that later become rejected, conflicted, or superseded in cold memory.
- `Cold Memory` is where semantic curation, conflict marking, and coherence repair become durable.

## Admission Rule Matrix

### `AP_STORAGE_ALWAYS`

- admit to `Storage Truth`
- no default cold or hot landing

### `AP_STORAGE_AND_COLD_ALWAYS`

- admit to `Storage Truth`
- admit to `Cold Memory`
- hot landing still depends on projection policy

### `AP_STORAGE_COLD_HOT_ON_ATTENTION`

- admit to `Storage Truth`
- admit to `Cold Memory`
- project to `Hot Working Set` only when attention, rejection, conflict, or policy conditions demand it

## Event Ledger And Object Ledger Admission

### Event ledger rule

- event-ledger records are append-only fact records
- they preserve timing, provenance, and runtime or protocol occurrence
- they do not become semantic replacement objects by themselves

### v0 event-ledger defaults

- `RECEIPT` defaults to `AP_STORAGE_ALWAYS`
- `TAINT_EVENT` defaults to `AP_STORAGE_ALWAYS`
- `ADMIN_EVENT` defaults to `AP_STORAGE_ALWAYS`
- cold visibility for events should usually happen through selective curation, derived summaries, or policy-triggered promotion rather than automatic cold duplication of every event row

### Object ledger rule

- object-ledger records are governed semantic objects or curation objects
- they may land in storage only or storage plus cold depending on admission policy
- object-ledger rows remain the normal vehicle for semantic linking, review status, and later projection

### v0 object-ledger defaults

- packets, verdicts, evidence objects, intake bundles, and cadence objects follow their own admission policy code
- if an object must later support review, conflict handling, or hot reconstruction, it should not remain storage-only by default
- curation objects such as decisions, memory fragments, visions, and top-doc summaries belong in cold memory even when they also retain storage provenance

## Projection Rule Matrix

### `PP_HOT_NEVER`

- object never projects directly to hot
- still eligible for indirect summary through a different hot object

### `PP_HOT_ACTIVE_ONLY`

- object projects only while active, queried, or locally relevant

### `PP_HOT_ON_ATTENTION_OR_REJECT`

- object projects when active
- also projects when rejection, conflict, or incident visibility requires it

## Learn Landing Protocol

### v0 rule

1. capture to `Storage Truth`
2. classify and admit to `Cold Memory` if policy allows
3. resolve approval, fuzzy status, conflict, or supersession in cold
4. project to `Hot Working Set` only through projection policy

### Consequences

- learning never lands in hot first
- hot-only insight with no cold/storage anchor is invalid
- quieter engagement profiles may reduce hot surfacing, but they do not cancel required storage capture

## Simulation Evidence Retention

### v0 retention rule

- `SIMULATION_EVIDENCE`, its lineage-linked attempt receipt, and its artifact manifest must all land in `Storage Truth`
- the evidence object and artifact manifest refs must land in `Cold Memory` so hot reconstruction and later audit do not depend on transient room state
- bulky raw artifacts may remain storage-first as long as cold memory preserves stable manifest refs, provenance, and integrity-relevant metadata
- hot projection of evidence should stay selective and occur only when active attention, rejection, incident visibility, or explicit query requires it

### Consequences

- evidence review should be able to happen from cold plus storage references, not only from live runtime rooms
- storage may carry more artifact volume than cold, but cold must still preserve enough structure to reconstruct why promotion was allowed or denied

## Concept Conflict Placement

### Rule

- concept conflict is a cold-memory governance concern, not a runtime handoff state
- conflict review state is distinct from lifecycle and from temperature
- document coherence is a cold-memory property, not a node execution state

### Minimum cold semantics

- `conflicts_with` should be a first-class relation in cold memory
- conflict review state should remain explicit until resolved
- affected cold docs may carry document coherence such as `conflicted`
- resolution should preserve both sides and provenance rather than overwriting the losing side out of existence

## Coherence Registry Rule

### v0 rule

- concept-level conflict review state and document-level coherence state should remain separate code families
- concept conflict answers whether concepts disagree
- document coherence answers whether a concrete cold artifact is currently internally or relationally coherent

### Consequence

- one conflicted concept pair may affect multiple documents
- one document may be stale or conflicted for reasons that are not identical to one concept-review state
- do not collapse concept review, document coherence, lifecycle, and temperature into one shared code family

## Truth Ownership Rule

### v0 rule

- runtime owns mechanical truth
- owning semantic slots own their semantic objects inside owned scope
- cold memory owns curation outcomes such as approved, fuzzy, conflicted, superseded, and linked
- hot owns no canonical semantic truth; it only projects

### Consequence

- phase, supervision state labels, and similar high-level summaries must be derivable from lower planes rather than hand-authored as free truth

## Engagement-Profile Learn Rule

### v0 rule

- engagement profiles may tune learn surfacing, summary density, and promotion prompts
- engagement profiles must not suppress required storage landing, required cold landing, conflict retention, or provenance retention

### Consequence

- profile variance changes attention cost, not truth preservation guarantees

## Reconstructability Rule

### v0 rule

- every hot object or hot summary should be reconstructable from cold docs, links, and storage-backed evidence
- if a hot surface cannot be reconstructed, it should be treated as convenience cache rather than truth

## Vector And Retrieval Attachment

### v0 rule

- vector indexes and retrieval indexes attach as derived search structures over storage and cold memory
- indexes are rebuildable and non-canonical
- retrieval results may accelerate hot projection or recall, but they do not become a shadow truth plane

### Consequence

- deletion, supersession, conflict, and curation status must still be resolved from canonical planes rather than from index contents alone

## Phase And Cadence Projection Rule

### v0 rule

- `PHASE` remains a hot/view projection over lower planes
- cadence protocol objects should default to `AP_STORAGE_AND_COLD_ALWAYS`
- cadence objects project to hot only when attention, batching, or interruption policy requires them

### Consequence

- cadence stays durable and auditable without becoming a separate semantic truth trunk
- phase and cadence remain related but distinct: one is projection, the other is governed protocol object capture

## Minimal Open Questions

- whether cadence protocol objects need more than one subtype-specific admission profile after v0
- whether retrieval indexes should expose conflict-aware ranking hints directly in the hot layer or only in cold/query services
