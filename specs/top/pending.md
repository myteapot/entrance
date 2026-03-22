# Pending

> Status: hot utility

## Purpose

- hold unresolved but non-oracle items without polluting the semantic hot root
- remain the explicit overflow surface for future ambiguity, deferred cuts, or human-held questions

## Current State

- No active architecture pending items are currently registered in the DB-backed working set.

## Rule

- only non-oracle unresolved items belong here
- pending items should stay short, mounted, and reconstructable from cold docs or DB records
- once a pending item becomes oracle, move it into `Machine / Control / Truth`
- once a pending item is abandoned or absorbed into harness/runtime, remove it from the hot root

## TODO(fill)

- repopulate only when a new unresolved architecture question actually appears
