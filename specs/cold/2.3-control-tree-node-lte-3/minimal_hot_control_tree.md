# Minimal Hot Control Tree

> Status: pending draft
> Scope: v0 hot/control compression and top-surface organization

## Purpose

- define how the hot surface stays within the `Node <= 3` budget without flattening canonical truth
- provide a compression target for the current top-doc set before full hot-doc merging
- separate semantic trunks from utility overlays such as pending and phased work queues

## Design Rule

- `Node <= 3` applies to semantic hot-control navigation, not to raw storage or full cold graphs
- the hot tree should minimize active semantic branches first, then push detail down into cold docs and DB-backed summaries
- compression should preserve oracle summaries in hot while relocating bulk detail to cold mounts or database truth

## Root Cut

### Semantic trunks

The v0 hot root should target three semantic trunks:

1. `Machine`
2. `Control`
3. `Truth`

### Utility surfaces

The root may also expose two non-semantic utility surfaces:

- `Phase Todo`
- `Pending`

### Rule

- utility surfaces are overlays or service leaves, not semantic trunks
- they do not justify expanding the semantic branching factor beyond three

## Candidate Mapping From Current Top Docs

### `Machine`

- `1.1 OS Core`
- `1.2 Hierarchical State Machine`
- `1.3 Compiler / Action IR`
- `2.1 OTP Supervisor Model`

### `Control`

- `2.2 Lead Model (3)`
- `2.3 Control Tree (Node <= 3)`

### `Truth`

- `3.1 Learning And Truth System`

## Compression Rule

### When a semantic trunk is compressed

- keep only the stable summary and current oracle in the hot trunk doc
- mount detailed rationale, open design cuts, and examples from cold docs
- keep unresolved items in either `Phase Todo` or `Pending`
- preserve reconstructability through DB decisions, memory fragments, and mounted cold docs

### When a trunk should split internally

Split only if:

- the hot summary exceeds the `1 hotspot + <=3 chunks` budget
- the trunk mixes more than one semantic responsibility that cannot be summarized cleanly
- active work on that trunk requires simultaneous attention to more than three unresolved subareas

### Preferred response before splitting

1. compress repeated rationale into one hot summary
2. move supporting detail to cold
3. move unresolved but non-active points to `Pending`
4. split the trunk only if compression still fails

## Archive Rule

- once a branch is stable, keep a short hot summary and archive the detailed branch into cold or DB-backed truth
- archived detail should remain mounted and reconstructable, not lost
- archival reduces active hot complexity; it does not delete provenance

## Pending Surface Rule

- `Pending` stores unresolved ideas that should remain visible without contaminating active hot structure
- pending items are not oracle
- pending should prefer short summaries backed by cold docs or memory fragments rather than free-floating prose

## Phase Todo Rule

- `Phase Todo` is the execution-facing active work queue for the current stage
- it is not a semantic trunk of the architecture itself
- phase todo should point into the relevant semantic trunk rather than duplicate its design content

## Why This Cut Is Harder Than Seven Flat Hot Docs

- it preserves the `<=3` control budget at the root
- it distinguishes semantic organization from utility overlays
- it lets current seven docs converge toward a smaller stable hot surface without deleting any design truth
- it makes future compression an explicit operation instead of an ad hoc cleanup pass

## Resolved Boundary

### v0 rule

- authority summaries stay under `Control`; they should not be absorbed into `Machine`
- the repo may keep [README](../../top/README.md) as a navigation landing page, but the semantic hot root remains the direct `Machine / Control / Truth` trunk set plus utility surfaces
- the compressed root is already switched semantically; v0 does not require a physical merge or path churn of the numbered mounted docs
- `Control Tree` is the canonical naming; legacy `Bt tree` wording should be treated as historical residue only

### Consequence

- control topology stays distinct from machine runtime semantics
- navigation may stay friendly without creating an extra semantic trunk
- mounted detail docs can remain in place as reconstructable transitional detail while the root shape stays stable

## Open Questions

- none mounted at v0; reopen only if compression fails the `<=3` hot-control budget again
