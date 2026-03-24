# Next OS Action Primitives

> Owner: NOTA
> Status: Proposed
> Scope: control plane, compiler IR, hierarchical state machine, role boundaries

## 1. Why This Spec Exists

Entrance is not only an app shell or a harness.

It is intended to be a next-generation operating system composed of three cooperating layers:

- OS core
- hierarchical state machine
- compiler

If that is true, then the system cannot run on vague role descriptions alone.

It needs a small, hard set of action primitives that:

- define what each role is allowed to do
- define what each role is not allowed to do
- give the compiler a stable intermediate language
- keep elastic agents inside hard rooms

## 2. Core Design Rule

Agents may be elastic.
Actions may not be vague.

Every runtime action must live inside a hard room:

- the room defines allowed capabilities
- the room defines legal state transitions
- the room defines what kind of object can be touched
- the room defines which role may enter

This prevents role drift, capability bleed, and accidental cross-layer collapse.

## 3. Top-Level Model

### 3.1 NOTA owns the top surface

At the Human-facing surface, `NOTA` can be compressed into three macro-actions:

- `chat`
- `learn`
- `do`

This is a good surface model.

It is simple enough for Human interaction, and broad enough to cover most top-level NOTA duties.

`do` should be read as a Human-to-NOTA control mode rather than as a raw internal primitive menu.
Human should not be asked to drive `prepare / dispatch / review / integrate / archive / mark_conflict` directly at the top surface.
`NOTA` receives `do`, refreshes context as needed, and lowers it into the stricter internal action graph.

### 3.2 But the compiler cannot stop there

`chat / learn / do` is the Human-facing semantic shell.

It is not yet a sufficient compiler IR.

The compiler must lower those three surface actions into a smaller set of internal action primitives with hard constraints.

## 4. Three-Layer Interpretation

### 4.1 OS core layer

Provides hard capabilities and resource boundaries:

- memory store
- issue store
- worktree manager
- prompt generator
- task runtime
- review and merge gates
- connector and vault access
- approval and audit

The OS core does not decide intent.
It exposes bounded capability rooms.

### 4.2 Hierarchical state machine layer

Defines who is active, what state each role is in, and what transitions are legal.

The suggested top shape is:

```text
NOTA
  chat
  learn
  do
    Arch
      shape
      split
      assign
      update
      escalate
    Dev
      prepare
      dispatch
      review
      integrate
      repair
    Agent
      read
      make
      report
```

### 4.3 Compiler layer

Compiles:

- Human intent
- policy
- current graph state
- role boundaries
- room capabilities

into executable action plans.

The compiler should never emit free-form "just do it."

It must emit bounded actions in bounded rooms.

## 5. NOTA Surface Actions

### 5.1 `chat`

Meaning:

- converse with Human
- clarify intent
- preserve continuity
- expose current system meaning in Human language

Allowed outputs:

- clarification
- summary
- next-step proposal
- explanation
- escalation request

Not allowed by itself:

- direct hidden mutation of strategy or runtime state without going through a lower primitive

### 5.2 `learn`

Meaning:

- absorb new signals
- compare against current truth
- detect novelty, reinforcement, conflict, and drift
- write memory and concept-state updates

Allowed outputs:

- instinct updates
- concept updates
- conflict markings
- provenance-preserving notes
- cold-file governance changes

Special rule:

When Human introduces a concept that conflicts with current cold truth, `learn` must not silently normalize it away. It must surface contradiction and mark conflict state.

### 5.3 `do`

Meaning:

- cause the system to act
- compile intent into child-node actions
- invoke lower rooms and lower roles

Important constraint:

For NOTA, `do` does not mean "personally implement code."

It means:

- route
- coordinate
- trigger
- enforce
- close loops

So your proposed `chat / learn / do` triad is viable for NOTA, as long as `do` is interpreted as control-plane execution, not undifferentiated labor.

## 6. Internal Primitive Set

The compiler-level primitive set should stay small.

Recommended core set:

- `chat`
- `learn`
- `shape`
- `split`
- `assign`
- `prepare`
- `dispatch`
- `make`
- `review`
- `integrate`
- `update`
- `escalate`
- `repair`
- `report`

This set is small enough to reason about and rich enough to cover the current Duet / Entrance operating model.

## 7. Role Primitive Matrix

### 7.1 NOTA

Primary primitives:

- `chat`
- `learn`
- `do`

Expanded internal capabilities:

- `chat`
- `learn`
- `assign`
- `update`
- `escalate`

Interpretation:

- `NOTA` owns the top node
- `NOTA` does not own detailed strategy decomposition
- `NOTA` does not own code review
- `NOTA` does not own code execution
- `NOTA` owns continuity, conflict governance, top-level routing, and surface control

### 7.2 Arch

Recommended primitive set:

- `shape`
- `split`
- `assign`
- `update`
- `escalate`

Meaning:

- `shape`
  clarify concepts, architecture, specs, boundaries
- `split`
  decompose goals into stages, issues, contracts, parallel lanes
- `assign`
  decide dependency topology and maximize executable parallelism
- `update`
  advance stage and issue planning state
- `escalate`
  surface ambiguity, concept conflict, or high-risk decision back upward

Hard boundary:

- `Arch` does not prepare worktrees
- `Arch` does not generate prompts
- `Arch` does not dispatch runtime tasks
- `Arch` does not merge code

### 7.3 Dev

Recommended primitive set:

- `prepare`
- `dispatch`
- `review`
- `integrate`
- `repair`

Meaning:

- `prepare`
  create worktrees, prompts, task envelopes, test setup
- `dispatch`
  start agents and bind them to bounded rooms
- `review`
  verify output, run checks, judge quality
- `integrate`
  merge accepted work and update execution state
- `repair`
  perform bounded fixes, conflict resolution, and rework handoff

Hard boundary:

- `Dev` does not redefine product truth
- `Dev` does not own stage topology
- `Dev` does not own top-level concept governance

### 7.4 Agent

Recommended primitive set:

- `read`
- `make`
- `report`

Meaning:

- `read`
  load issue, prompt, refs, and local worktree context
- `make`
  produce the bounded artifact inside the assigned room
- `report`
  summarize outcome, commit, and hand back state

Hard boundary:

- `Agent` does not choose its own room
- `Agent` does not move stage state
- `Agent` does not merge into protected truth
- `Agent` does not redefine policy

## 8. Hard Rooms

Each primitive must run in a room.

Recommended room set:

- `surface_room`
  Human conversation and explanation
- `memory_room`
  instincts, documents, concept state, conflict markers
- `strategy_room`
  specs, stages, dependency topology, issue decomposition
- `prep_room`
  worktree, prompt, task envelope, execution preparation
- `work_room`
  bounded artifact production inside assigned scope
- `review_room`
  validation, quality judgment, acceptance or rework
- `integration_room`
  merge, state advancement, closure
- `approval_room`
  Human escalation and high-risk gatekeeping

## 9. Primitive To Room Mapping

Recommended default mapping:

- `chat` -> `surface_room`
- `learn` -> `memory_room`
- `shape` -> `strategy_room`
- `split` -> `strategy_room`
- `assign` -> `strategy_room`
- `prepare` -> `prep_room`
- `dispatch` -> `prep_room`
- `make` -> `work_room`
- `review` -> `review_room`
- `integrate` -> `integration_room`
- `update` -> `strategy_room` or `integration_room`, depending on object
- `escalate` -> `approval_room`
- `repair` -> `review_room` or `work_room`, depending on scope
- `report` -> `surface_room` plus state writeback

## 10. Compiler IR Shape

The compiler should emit action records like:

```text
Action {
  verb,
  actor_role,
  room,
  subject_kind,
  subject_ref,
  preconditions,
  expected_outputs,
  state_effects,
  escalation_policy
}
```

Example:

```text
Action {
  verb: assign,
  actor_role: Arch,
  room: strategy_room,
  subject_kind: issue_group,
  subject_ref: Entrance/S2/bootstrap-import,
  preconditions: [spec_exists, issue_graph_coherent],
  expected_outputs: [parallel_issue_lanes >= 3],
  state_effects: [issues_backlog_to_todo],
  escalation_policy: human_if_dependency_ambiguous
}
```

This is how the compiler keeps elastic intelligence inside hard boundaries.

## 11. Parallelism Rule

Parallelism is not optional strategy polish.
It is a first-class compiler concern.

The compiler should distinguish:

- logical dependency
- execution dependency

A logical dependency does not automatically imply serial execution.

If interfaces, contracts, or room boundaries are already strong enough, the compiler should emit parallel lanes.

For `Arch`, this means `assign` is not just "who works on what."
It also means:

- identify which dependencies are real
- identify which are only conceptual
- maximize executable concurrency without violating truth safety

## 12. State-Machine Interpretation

### 12.1 NOTA top node

`NOTA` is the top control node.

Its stable surface states are:

- `chat`
- `learn`
- `do`

### 12.2 Child nodes

Inside `do`, child state machines run:

- `Arch`
- `Dev`
- `Agent`

These are subordinate operational nodes, not equal top-level surfaces.

### 12.3 Transition logic

Typical flow:

1. Human -> NOTA.chat
2. NOTA.learn
3. NOTA.do
4. Arch.shape / split / assign
5. Dev.prepare / dispatch
6. Agent.read / make / report
7. Dev.review / integrate
8. Arch.update
9. NOTA.chat back to Human

## 13. Why NOTA `chat / learn / do` Works

Your proposed triad is strong because it separates:

- communication
- memory and interpretation
- system action

That is the right top-level compression for a secretary-facing control surface.

The only correction needed is:

- `do` must be compiled downward, not left as an untyped catch-all

So the design answer is:

- yes, `chat / learn / do` covers the main responsibilities of NOTA
- yes, it is viable
- but only if lower nodes use stricter internal primitives

If a later UI exposes top-surface modes, `chat / learn / do` is the right semantic set.
`auto` should be treated as an execution or autonomy policy layered over those modes, not as a peer semantic action beside them.

## 14. Recommended Next-Step Primitive Sets

Recommended stable sets for now:

- NOTA: `chat`, `learn`, `do`
- Arch: `shape`, `split`, `assign`, `update`, `escalate`
- Dev: `prepare`, `dispatch`, `review`, `integrate`, `repair`
- Agent: `read`, `make`, `report`

This is a good v1 action algebra for the Next OS direction.

## 15. One-Line Conclusion

`NOTA` can expose `chat / learn / do` as the top-node surface, while the compiler lowers that surface into harder role-bound primitives that live inside explicit rooms, allowing Entrance to function as a true Next OS built from OS core, hierarchical state machine, and compiler semantics together.
