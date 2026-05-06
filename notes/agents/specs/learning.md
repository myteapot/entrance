# 3.1 Continuous Learning And Truth System Design

> Status: mounted hot detail (transitional)

## Purpose

- define how learning lands in truth
- define how approved and unapproved content are separated in storage
- ensure the hot surface can be reconstructed from canonical records

## Mounted Root

- [truth.md](./truth.md)

## Confirmed Oracle Points

- This document preserves the truth-plane landing rules beneath [truth.md](./truth.md): admission, projection, retention, and reconstructability detail.
- Learn landing remains `Storage -> Cold -> Hot projection`; hot-first learning is invalid.
- `Session` is only one storage substream; runtime logs, OS logs, receipts, artifacts, and other governed execution evidence also belong to storage truth.
- Admission into `Storage / Cold / Hot` should be governed by typed policy codes rather than prose-only field meanings.
- The minimal v0 admission policy meanings stay `AP_STORAGE_ALWAYS`, `AP_STORAGE_AND_COLD_ALWAYS`, and `AP_STORAGE_COLD_HOT_ON_ATTENTION`.
- The minimal v0 projection policy meanings stay `PP_HOT_NEVER`, `PP_HOT_ACTIVE_ONLY`, and `PP_HOT_ON_ATTENTION_OR_REJECT`.
- Registry layout stays split: object-type policy defaults live separately from concept-review and document-coherence code families.
- `RECEIPT` remains a runtime fact object in the object ledger, while `TAINT_EVENT` and `ADMIN_EVENT` remain storage-first event-ledger families.
- `SIMULATION_EVIDENCE`, its lineage-linked attempt receipt, and its artifact manifest must land in storage truth together, while the evidence object plus manifest refs must persist in cold memory for reconstructability.
- Cadence-side objects should be governed durable objects, not ephemeral chat-only context; `PHASE` remains a hot/view projection.
- The minimal cadence subtype cut is `CADENCE_CHECKPOINT / CADENCE_HANDOUT / CADENCE_WAKE_REQUEST / CADENCE_POLICY_NOTE`; `PHASE` is not a cadence object subtype.
- Cadence continuity should land in a dedicated cadence object cut rather than being collapsed into generic `memory_fragments`; memory curation and continuity protocol are adjacent but not identical object families.
- Concept-level conflict review state and document-level coherence state remain separate code families inside truth governance.
- Vector and retrieval indexes remain derived search structures rather than a shadow truth plane, and hot projection must dereference canonical source refs before surfacing retrieved memory.
- Engagement profiles may tune surfacing density, but not storage/cold landing, provenance retention, or conflict retention.

## Current Boundary Reading

- this document should stay focused on truth-plane landing rules, not become a second root summary
- approved and fuzzy content should coexist without conflation
- reconstructability matters as much as immediate readability
- evidence-bearing truth objects should remain verifiable independently of later summary text
- quieter execution modes may lower hot interruption before they lower cold or storage capture

## Mounted Cold Docs

- [concept_conflict_state_model.md](../cold/1.2-hierarchical-state-machine/concept_conflict_state_model.md)
- [data_schema.md](../cold/3.1-learning-truth-system/data_schema.md)
- [landing_reconciliation_cut.md](../cold/3.1-learning-truth-system/landing_reconciliation_cut.md)
- [minimal_truth_plane.md](../cold/3.1-learning-truth-system/minimal_truth_plane.md)

## Mounted Chore Docs

- `TODO(fill)`

## TODO(fill)

- keep only stable truth-plane summary here
- push any later implementation-specific wrinkles downward into cold docs or DB-backed truth rather than regrowing this mounted summary
