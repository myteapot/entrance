# Entrance Foundation Architecture

> This document records the architecture signals that were explicitly confirmed in Human conversation and then recovered on 2026-03-21.
> It is an honest reconstructed foundation for future Entrance migration, not a fake byte-for-byte restoration of lost history.

## 1. Product Identity

- Entrance is a next-generation operating system, not a traditional app, not a narrow harness, and not only an agent orchestrator.
- Entrance carries a unified entrance semantic: it should be able to absorb, wrap, and govern other apps rather than merely coexist beside them.
- Agent orchestration is only one plugin or subsystem inside Entrance. It is important, but it is not the whole product.

## 2. Core Architecture

Entrance should be designed as three cooperating layers:

1. OS layer
   Provides runtime substrate, resource governance, supervision, capability boundaries, and long-lived system semantics.
2. Hierarchical state machine layer
   The system state model should inherit from the 1987 hierarchical state machine tradition, with explicit hot/cold, active/pending/archived, and other structured runtime states.
3. Compiler layer
   Entrance should be able to compile higher-level intent, contracts, orchestration descriptions, and role semantics into executable system behavior.

This means Entrance should not collapse into a single harness abstraction. The intended product is closer to a Next OS made from OS semantics, state semantics, and compilation semantics together.

Reference influences that were explicitly named in discussion:

- OTP / Erlang style supervision and fault-model thinking
- operation primitives from prior large-scale orchestration practice
- the 1987 hierarchical state machine tradition

These references should shape runtime design and supervision semantics, but they should be adapted into Entrance's own model rather than copied mechanically.

## 3. Organizational Model

The system should preserve a first-class three-layer social architecture:

- Leader
  Responsible for resource scheduling and global priority.
- Manager
  Responsible for maximizing the output and coordination quality of agents.
- Agent
  Responsible for execution and concrete production work.

This model is not only an org chart metaphor. It is a design primitive for how Entrance should reason about delegation, supervision, visibility, and responsibility.

NOTA is the secretary-facing interface to this structure. The long-term direction is that Human can interact with NOTA and thereby operate the wider company and agent system through one control surface.

## 4. Platform Model

- Different platforms may ship different clients or shells, such as desktop, Linux server, Android, or IM surfaces.
- Those platform-specific surfaces should not own separate business truth.
- The canonical state model, contracts, and system semantics should remain unified across platforms.

In other words, platform variation is allowed at the client layer, but truth-source variation is not.

Longer term, this implies a multi-surface control plane:

- a desktop entrance
- server-side workers and supervision surfaces
- mobile-triggered interaction
- IM-based conversational control

These are surfaces over one system, not separate products with separate truth.

The fully distributed multi-node layer remains a future design space. It should be explored carefully rather than prematurely frozen into the first persistence model.

## 5. Data and Persistence Implications

The current recovered memory model already points toward the Entrance migration shape:

- Raw layer: `memory_fragments`
- Canonical layer: `instincts`, `documents`, `coffee_chats`, `todos`, `decisions`, `visions`
- Graph layer: `memory_links`

Additional guidance:

- Conflict should be modeled as relation state or UI state, not as the universal primary status of every record.
- Hot and cold distinctions matter even inside a single table. Temperature is a real modeling dimension.
- Directory-scale destructive actions must not be raw shell operations. They should become Entrance-provided controlled capabilities with preview, scope validation, approval, and audit.

## 6. Dashboard Direction

The dashboard should evolve toward a hybrid of Linear and Obsidian:

- graph-shaped rather than only list-shaped
- able to show operational relationships rather than static metadata alone
- able to reveal live runtime state when hovering or focusing nodes

The goal is not a static project board. The goal is an operational knowledge graph with runtime visibility.

## 7. Retrieval Direction

Keyword search alone is not enough for long-term memory and architecture retrieval.

Entrance should eventually support semantic or vector-assisted retrieval alongside keyword lookup. External reference repos such as `zeroclaw`, `nanobot`, and `pi-agent` should be studied to reduce unnecessary reinvention.

## 8. Migration Guidance

When the Entrance source tree is restored, this document should be treated as migration-ready guidance:

- keep product identity larger than "agent orchestrator"
- keep the three core layers visible in schema and runtime design
- keep platform shells separate from truth-source semantics
- keep organizational roles explicit
- keep memory graph structure first-class
- keep destructive capabilities policy-driven rather than raw-path driven
