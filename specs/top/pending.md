# Pending

> Status: hot utility

## Purpose

- hold unresolved but non-oracle items without polluting the semantic hot root
- remain the explicit overflow surface for deferred cuts or human-held questions

## Current State

- No root-level architecture rewrite is pending; the work now is consolidation and sharpening.
- `v0` continuation policy is still partly hardcoded; a fuller human-configurable cadence policy remains future work.
- Multi-role allocator expansion remains deferred until the post-consolidation cut is truly settled.
- Cold-doc full DB canonicalization plus periodic projection sync still needs more implementation depth.
- A native `NOTA` product shell must continue to reuse the same runtime truth plane rather than creating a shadow control plane.

## Rule

- only non-oracle unresolved items belong here
- once a pending item becomes truth, land it in DB-first runtime truth and project it back out
- do not let file-local TODOs outrank checkpoints, acceptance bundles, or receipts
- once a pending item is abandoned or absorbed into runtime, remove it from the hot root
