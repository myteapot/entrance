# Truth

> Status: hot root

## Purpose

- hold the compressed hot-root summary for truth ownership, learning, storage, and projection
- keep projection boundary explicit so files never outrank DB truth

## Confirmed Oracle Points

- Truth substrate distinguishes `Storage Truth -> Cold Memory -> Hot Working Set`.
- `Storage Truth` is the canonical capture plane.
- `Cold Memory` is the curated and conflict-aware memory plane.
- `Hot Working Set` is a reconstructable projection plane rather than a canonical semantic authoring plane.
- Learn landing follows `Storage -> Cold -> Hot projection`; hot-first learning is invalid.
- DB is the only canonical writer for runtime continuity.
- `~/.entrance/data/entrance.db` is the canonical runtime continuity owner path.
- `~/.entrance/exports/` and projected Markdown files are export surfaces, not source-of-truth planes.
- `projection boundary` means every write lands in DB truth first, and only then may be projected to hot root, cold docs, GUI, CLI, or MCP.
- `CADENCE_CHECKPOINT` and `CADENCE_ACCEPTANCE_BUNDLE` are durable truth objects; `PHASE` and anti-Zeno views remain projections.
- anti-Zeno is derived progress visibility rather than a shadow truth plane.

## Mounted Detail Docs

- [3.1-learning-and-truth-system.md](./3.1-learning-and-truth-system.md)

## Mounted Cold Docs

- [concept_conflict_state_model.md](../cold/1.2-hierarchical-state-machine/concept_conflict_state_model.md)
- [minimal_truth_plane.md](../cold/3.1-learning-truth-system/minimal_truth_plane.md)
