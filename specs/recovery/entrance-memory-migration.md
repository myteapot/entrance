# Entrance Memory Migration Proposal

> This document is a migration-oriented proposal, not a finalized schema decree.
> It translates the currently recovered NOTA memory model into an Entrance-ready direction.

## 1. Current Recovered Layers

The recovered memory stack already has three useful layers:

- raw layer: `memory_fragments`
- canonical layer: `instincts`, `documents`, `coffee_chats`, `todos`, `decisions`, `visions`
- graph layer: `memory_links`

This is strong enough to serve as the conceptual base of an Entrance memory subsystem.

## 2. Migration Principle

Migration into Entrance should preserve meaning, not file shape.

That means:

- keep raw recovered context available
- promote stable knowledge into canonical tables
- preserve provenance links
- allow human-readable views without treating markdown as the only truth source

## 3. State Model Guidance

State should stay layered instead of overloaded into one field.

Suggested modeling split:

- lifecycle or workflow state
  examples: active, pending, archived, done
- review or conflict state
  examples: triaged, promoted, discussion, contested
- temperature
  examples: hot, warm, cold

Temperature should remain an orthogonal modeling dimension rather than being collapsed into lifecycle status.

## 4. Documents Migration

Current `documents` already stores canonical text content.

The likely Entrance direction is:

- canonical documents in database tables
- file exports or markdown views as secondary human-facing projections

The earlier `project_documents` idea still looks promising, but it should remain a migration proposal until the Entrance source repo returns and surrounding schema can be evaluated in context.

## 5. File Views vs Truth Source

A useful working rule for Entrance is:

- database tables hold the canonical state
- files exist as views, exports, or editing surfaces where that improves human usability

This preserves migration compatibility with the current recovery effort while avoiding a permanent dependency on ad hoc markdown topology.

## 6. Migration Phases

Suggested future sequence:

1. restore Entrance source tree
2. define Entrance memory SQL migrations based on recovered canonical tables
3. copy `.agents` recovery records into Entrance-owned tables and documents without deleting the recovery substrate
4. verify that the Entrance-backed result is complete and auditable
5. keep markdown export or view generation as compatibility layer
6. only after verification, demote `.agents` from recovery carrier to historical staging substrate

## 7. Open Cautions

- do not prematurely freeze distributed-runtime fields into the memory schema
- do not overload conflict into the primary lifecycle field
- do not lose provenance during import
- do not couple destructive capabilities to raw filesystem path access
