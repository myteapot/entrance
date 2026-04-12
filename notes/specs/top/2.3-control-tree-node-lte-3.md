# 2.3 Control Tree (Node <= 3) Design

> Status: mounted hot detail (transitional)

## Purpose

- define the top control-tree document as part of the first design batch
- define the routing/topology side without collapsing into UI detail
- hold the place for the `Node <= 3` control-tree constraint line

## Mounted Root

- [control.md](./control.md)

## Confirmed Oracle Points

- This document preserves the hot-control compression cut beneath [control.md](./control.md): branching budget, trunk separation, and hot-surface topology.
- `Node <= 3` remains the active constraint marker for this design line.
- `Node <= 3` constrains hot/control projection rather than raw storage truth or full dependency graphs.
- Active hot/control surfaces should target `1 hotspot + <=3 chunks`.
- The hot/control root should converge toward at most three semantic trunks rather than seven flat hot peers.
- The compressed hot-surface target remains `3 semantic hot docs + 1 phase todo doc + 1 pending doc`, with trunk names `Machine / Control / Truth`.
- `Phase Todo` and `Pending` remain utility surfaces rather than semantic trunks.
- Human-facing phase and surface-density presets remain view-layer choices that are orthogonal to runtime engagement profiles.
- Real dependency structure may remain richer in storage and cold layers as long as the hot projection stays bounded.
- When hot complexity grows, compression should prefer summary-in-hot plus detail-in-cold/DB before introducing new hot branches.
- Hot branches should split only after compression fails the `<=3` budget.
- The current seven numbered hot docs remain a transitional decomposition toward the compressed target rather than the intended steady-state root shape.
- Authority summaries remain under `Control`; they should not be reabsorbed into `Machine`.
- `README.md` may remain the navigation landing page, but the semantic hot root is still the direct `Machine / Control / Truth` trunk set plus utility surfaces.
- `Control Tree` is the canonical naming; legacy `Bt tree` wording is historical only.

## Current Boundary Reading

- this document is about control topology rather than front-end surface detail
- the hot control tree may stay small even when the cold graph remains richer underneath
- semantic trunks and utility surfaces should stay distinct so work queues do not bloat architecture topology
- actual merge timing may remain operational, but the semantic target shape should no longer remain ambiguous

## Mounted Cold Docs

- [front.md](../cold/2.3-control-tree-node-lte-3/front.md)
- [minimal_hot_control_tree.md](../cold/2.3-control-tree-node-lte-3/minimal_hot_control_tree.md)

## Mounted Chore Docs

- none currently

## TODO(fill)

- keep the mounted summary aligned with the cold hot-control tree cut
- defer any optional physical relocation of numbered mounted docs unless compression pressure returns
