# Entrance Dashboard Runtime Direction

> This document captures the recovered direction for the Entrance dashboard.
> It records the accepted graph-first direction while leaving exact interaction design open.

## 1. Core Shape

The dashboard should move toward a hybrid of Linear and Obsidian rather than a static list-heavy admin panel.

It should behave like an operational graph:

- relationship-aware
- topology-aware
- workflow-aware
- runtime-aware

## 2. Why Graph-First

The system being designed is not a flat issue tracker.

Entrance needs to show relationships among:

- Human
- NOTA
- organizational roles
- projects
- work items
- agents
- runtime executions
- memory and decisions

A graph surface is better suited than a pure table or kanban-only view for this kind of system.

## 3. Runtime Visibility

The desired interaction direction includes:

- hover or focus to inspect live runtime state
- movement between stable structure and live execution
- visibility into which parts of the system are active, pending, blocked, cold, or archived

## 4. Layer Separation

The dashboard should likely separate at least three viewing modes, even if they share one underlying graph:

- topology view
  what exists and how it connects
- workflow view
  what is planned, active, blocked, or done
- runtime view
  what is executing right now

These may be separate panels, filters, overlays, or zoom levels. The exact interaction model remains open.

## 5. Current Open Questions

- which node types must always be visible
- how much runtime detail should appear on hover
- how to avoid visual noise
- how graph state should relate to canonical memory state

## 6. Design Implication

When the Entrance source returns, dashboard work should preserve:

- graph-first information architecture
- live runtime observability
- explicit relationship modeling
- compatibility with the canonical memory graph rather than a disconnected UI-only graph
