# Entrance Ralph Loop

> This document is the Entrance-local reinterpretation of Ralph Loop.
> It keeps the parts that remain valuable to us and discards the parts that do not fit our architecture.

## 1. Role in the System

Ralph Loop belongs to the execution layer, not the whole system.

In Entrance terms, it primarily applies to:

- Dev-managed implementation loops
- Agent execution inside bounded worktrees
- focused coding or migration tasks that benefit from short, verifiable cycles

It does **not** define:

- the full Entrance architecture
- the company/control-plane model
- the memory ontology
- the Human-facing product identity

## 2. Preserved Essence

The parts worth preserving are:

- fresh execution context
  each implementation pass should start from explicit externalized state rather than rely on bloated conversational carryover
- one atomic step at a time
  each loop should consume a task small enough to finish and verify cleanly
- hard feedback loop
  build, test, runtime checks, or equivalent validation should gate progress
- durable externalization
  what was learned must be written back into persistent system memory, not left in the transient session

## 3. Entrance Adaptation

Entrance changes the loop in three major ways:

- DB-first memory
  durable learning does not stop at plain text notes; it enters canonical tables and graph links
- role separation
  Arch, Dev, Agent, and NOTA do not collapse into one loop actor
- graph-aware continuity
  context is not only task text; it can be reconstructed from decisions, visions, instincts, documents, todos, and links

## 4. What We Reject

We do not inherit the weaker parts of the original pattern:

- flat task-file thinking as the only memory structure
- treating coding loop semantics as the whole product architecture
- relying on one textual progress artifact where a structured memory graph is more appropriate

## 5. Practical Rule

Inside Entrance, Ralph Loop should mean:

1. load the smallest reliable context
2. execute one bounded task
3. validate the result
4. externalize the durable learning
5. re-enter with fresh state instead of dragging unnecessary session mass forward

## 6. Boundary

If a question becomes architectural, organizational, or product-semantic rather than implementation-local, it should leave Ralph Loop and move upward into:

- decisions
- visions
- coffee chat
- architecture documents

Ralph Loop remains valuable, but only as a disciplined execution loop inside a larger Entrance system.
