# NOTA Concept Conflict State Model

> Owner: NOTA
> Status: Proposed
> Scope: cold-file governance, concept governance, cross-document consistency

## 1. Why This Is NOTA-Owned

This model belongs to `NOTA`, not `Arch`.

Reason:

- `NOTA` is the top control surface and routing layer
- `Arch` is a subordinate strategy node inside that larger control structure
- concept conflict governance is cross-role and cross-document by nature
- therefore it cannot be treated as an `Arch`-only local workflow rule

Current role evidence already points this way:

- `.agents/duet/SKILL.md` defines `NOTA` as route + aggregation layer
- `.agents/duet/SKILL.md` defines `Arch` as strategy layer
- `oracle.md` already treats NOTA as the Human-facing control surface

So when concepts clash, the arbiter is not a child strategy node alone. The conflict must be surfaced at the NOTA layer.

## 2. Problem

The system already has partial state modeling:

- workflow states
- review/conflict states
- temperature
- decisions / visions / discussions / memory fragments

But it does not yet have a first-class way to represent:

- a cold file contradicting another cold file
- a newly proposed concept contradicting accepted cold truth
- Human introducing a new statement that conflicts with existing accepted concepts
- unresolved concept pairs that must remain visible as conflict rather than being silently overwritten

This creates a dangerous failure mode:

- new concept lands
- old concept still exists
- no explicit conflict edge or conflict state exists
- the system appears coherent when it is actually split-brained

## 3. Design Goal

When concept-level conflict exists, the system must represent it explicitly instead of hiding it by overwrite, omission, or accidental precedence.

The design target is:

- surface conflict
- preserve both sides
- keep provenance
- wait for Human resolution when necessary
- then convert conflict into resolution state such as superseded or rejected

## 4. Concept Units Covered

This model applies to all concept-bearing cold artifacts, including:

- `oracles/oracle.md`
- `specs/*.md`
- recovery docs copied into `Entrance`
- decisions
- visions
- discussions
- memory fragments that are candidate concept sources

It does not primarily target runtime task states like `Todo`, `In Progress`, or `Done`.

## 5. Core Distinction

There are three different things that must not be collapsed into one field:

### 5.1 Lifecycle state

Examples:

- `proposed`
- `accepted`
- `superseded`
- `rejected`
- `archived`

### 5.2 Conflict/review state

Examples:

- `clear`
- `under_review`
- `conflicted`
- `resolved`

### 5.3 Temperature

Examples:

- `hot`
- `warm`
- `cold`

Conflict is not lifecycle.
Temperature is not conflict.
Lifecycle is not review.

## 6. New Concept States

Recommended concept lifecycle states:

- `proposed`
- `accepted`
- `superseded`
- `rejected`
- `archived`

Recommended conflict/review states:

- `clear`
- `discussion`
- `conflicted`
- `resolved`

Recommended document coherence states:

- `coherent`
- `conflicted`
- `stale`
- `superseded`

## 7. Conflict Relations

The graph layer should gain explicit concept-relations such as:

- `conflicts_with`
- `supersedes`
- `refines`
- `derived_from`
- `references`

Current system already uses `derived_from`, `references`, and limited `supersedes`.
It is missing `conflicts_with` as a first-class relation.

## 8. Conflict Triggers

### 8.1 Cold file vs cold file

If one cold file asserts a concept that contradicts another accepted cold file, both concepts must be marked as conflict participants until resolved.

### 8.2 New concept vs accepted concept

If a newly landed concept contradicts an accepted concept:

- the new concept must not silently replace the old one
- both must enter `conflicted` review state
- a `conflicts_with` relation must be created

### 8.3 Human language vs current cold truth

If Human introduces a new concept or phrasing that conflicts with current accepted truth:

- list the contradiction explicitly
- do not silently normalize it away
- keep both visible
- mark both sides `conflicted` until Human resolves or ratifies a replacement

### 8.4 Self-conflict inside one cold file

If the same cold file internally contains contradictory concept statements:

- the file gets document state `conflicted`
- the concepts involved get review state `conflicted`
- the file must be treated as non-coherent until repaired

## 9. Resolution Rules

### 9.1 Before Human resolution

When conflict is detected and Human has not yet resolved it:

- old concept: keep lifecycle as-is, set review state to `conflicted`
- new concept: keep lifecycle as `proposed` or `accepted` only if already ratified, but set review state to `conflicted`
- both sides gain `conflicts_with`
- affected cold files gain document state `conflicted`

This means unresolved conflict is not hidden by lifecycle labels alone.

### 9.2 After Human resolution

Once Human resolves:

- winning concept becomes `accepted` with review state `resolved`
- losing concept becomes `superseded` or `rejected`
- `conflicts_with` may remain historically
- `supersedes` should be added when replacement is directional
- affected documents return to `coherent` only after repair

## 10. Directionality

Conflict is symmetric.
Supersession is directional.

That means:

- `A conflicts_with B`
- `B conflicts_with A`

But:

- `A supersedes B`

does not imply:

- `B supersedes A`

This directional split is necessary because a new concept may challenge an old concept before Human decides whether it truly replaces it.

## 11. Minimal Operating Protocol

When a new concept is about to be landed:

1. compare against current accepted cold truth
2. list explicit contradictions
3. if contradiction exists and no Human resolution yet:
   set both sides to review state `conflicted`
4. mark affected document(s) `conflicted`
5. preserve provenance and both texts
6. only after Human resolution:
   add `supersedes` or `rejected`

## 12. Practical Example

Example:

- old accepted concept: `Arch is the top governance owner of concept conflict`
- new Human correction: `NOTA is the top node; Arch is a child strategy node`

Correct handling:

- do not silently overwrite old wording
- mark the old concept and the new concept as `conflicted`
- record that Human introduced a governance correction
- after confirmation, mark the old concept `superseded`
- mark the new concept `accepted`
- repair affected cold files back to `coherent`

## 13. Current Gap

Current recovered structures already support:

- accepted decisions
- active and emerging visions
- triaged and promoted fragments
- derived/reference/supersedes relations

But they do not yet fully support:

- `conflicted` as a first-class cross-concept review state
- `conflicts_with` as a first-class relation type
- document-level coherence state for cold files

So this design is an extension, not a replacement.

## 14. One-Line Conclusion

Concept conflict is a NOTA-level governance problem: until Human resolves contradiction, both new and old concept surfaces must remain explicitly marked as `conflicted`, and affected cold files must be treated as `conflicted` rather than silently coherent.
