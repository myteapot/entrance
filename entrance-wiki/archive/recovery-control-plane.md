# Entrance Control Plane

> This document captures the currently recovered control-plane direction for Entrance.
> It records confirmed structure and explicitly avoids freezing unresolved distributed-runtime mechanics.

## 1. Purpose

Entrance should expose one company-scale control plane rather than a pile of unrelated operator surfaces.

Human should be able to interact with NOTA as the secretary-facing surface and thereby influence the wider system without needing to manually descend into every internal role or host.

## 2. Surface vs Role

The control surface and the internal role model are not the same thing.

- NOTA
  Secretary-facing interface layer. Receives Human intent, preserves continuity, and routes work into the system.
- Leader
  Resource scheduling layer.
- Manager
  Output-maximization and coordination layer.
- Agent
  Execution layer.

This distinction matters because a single Human-facing interface may coordinate multiple internal roles without collapsing them into one runtime primitive.

## 3. Multi-Surface Reach

The same control plane should eventually be reachable from multiple surfaces:

- desktop app
- mobile trigger surface
- IM conversation surface
- server-side operational surfaces

These should all speak to one underlying truth source and one core role model.

## 4. Control-Plane Responsibilities

The control plane should eventually be responsible for:

- receiving intent from Human
- routing work to the appropriate role or subsystem
- exposing live operational visibility
- preserving continuity and memory
- mediating approvals for high-risk capabilities

## 5. Boundaries

The control plane should not be treated as identical to:

- the worker runtime
- the node topology
- the message bus
- the low-level supervision mechanism

Those may become dependencies or subordinate layers, but they are not the same abstraction.

## 6. Current Open Questions

The following remain unresolved and should stay open:

- heartbeat semantics
- worker vs node contract
- local-first vs distributed-first runtime progression
- exact protocol boundaries between control plane and execution plane

## 7. Design Implication

When Entrance source is restored, control-plane work should be shaped around:

- one Human-facing secretary surface
- one underlying truth source
- explicit role separation
- policy-aware operational control rather than raw shell reach
