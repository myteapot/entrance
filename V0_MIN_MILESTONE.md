# Entrance V0-min Compiler Pipeline — Milestone Complete

**Date**: 2026-04-03
**MRs**: !4 through !20 (16 total)

## What V0-min established

A type-safe, side-effect-free compiler pipeline from human intent to agent execution:

```
ActionRecord → compile() → TypedActionPacket
  → lower_dispatch() → LoweredDispatch
    → admit_dispatch() → AdmittedDispatch  (proof type)
      → resolve_return_route() → ReturnRoute
```

Every stage is a pure function. Every transition produces a typed proof
that downstream stages require as input. There is no way to bypass
admission or routing at the type level.

## Subsystems delivered

| Subsystem | MRs | What it does |
|-----------|-----|-------------|
| Structure | A.1, A.2 | Module split, lib.rs scaffold |
| Compiler Registry | M4.1–M4.3 | Primitive data model, query API, control semantics |
| Compiler Lowering | M5.1–M5.3 | Typed packets, dispatch lowering, enforcement |
| Admission/Routing | M6.1–M6.3 | Admission gate, return routing, visibility reconstruction |
| Simulation Gate | M7.1–M7.2 | Evidence model, gate enforcement |
| Typed Supervision | M8.1–M8.3 | Signal classification, supervisor actions, incident visibility |

## What comes next (V0-full)

- Runtime context injection (budget, deduplication)
- Cross-agent dispatch routing
- Full simulation gate with live evidence collection
- Projection pipeline materialization
