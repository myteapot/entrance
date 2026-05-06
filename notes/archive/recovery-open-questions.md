# Entrance Open Questions

> This document records architecture questions that were explicitly surfaced in discussion but not yet resolved.
> These items should remain open until future Coffee Chat or implementation work produces enough clarity to promote them into canonical decisions.

## 1. Heartbeat Semantics

Current uncertainty:

- Why should an agent session have heartbeat?
- What exactly is the heartbeat proving: process liveness, task progress, control-plane connectivity, or human-visible freshness?
- Which layer should own it: runtime supervision, orchestration protocol, UI, or persistence?

Current stance:

- heartbeat is important enough to stay in active discussion
- heartbeat semantics should not be prematurely baked into the data schema before its ownership is clear

## 2. Worker vs Node

Current uncertainty:

- What is a worker?
- What is a node?
- Are they runtime processes, physical machines, logical scheduling units, or capability bundles?
- Which state belongs to worker, and which state belongs to node?

Current stance:

- worker and node must be modeled as distinct concepts if they represent different failure domains or scheduling boundaries
- their precise contract is still pending discussion

## 3. Distributed Layer

Current uncertainty:

- Should Entrance become a distributed multi-end architecture with shared message bus or broker?
- If so, what is the minimum viable boundary between local single-host mode and distributed multi-host mode?
- How much of this belongs in v1 versus later phases?

Current stance:

- do not freeze distributed assumptions too early
- keep platform variation separate from truth-source semantics
- prefer a unified core model first, then evolve toward distributed deployment

## 4. Dashboard Runtime Semantics

Current uncertainty:

- Which runtime states should be visible directly in the graph dashboard?
- How should hovering, focus, expansion, and live inspection work without turning the graph into noise?
- What is the right split between topology view, workflow view, and runtime view?

Current stance:

- the dashboard direction is accepted as graph-first
- the exact runtime interaction model is still an open design space
