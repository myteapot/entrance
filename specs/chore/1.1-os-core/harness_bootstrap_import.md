# Harness Bootstrap Import

> Arch design
> Issue: `MYT-64`
> Status: Landed repo-side bootstrap import
> Stage: update phase, runtime cutover still deferred

## 1. Goal

Import the live bootstrap substrate from `A:/.agents` into a first-class, source-controlled home inside `Entrance`, using a strict copy-first migration.

This issue does not replace runtime behavior yet. It only establishes canonical owned copies for the bootstrap files that are still currently living in `.agents`.

Repo-side checkpoint now landed:

- `harness/bootstrap/duet/SKILL.md`
- `harness/bootstrap/duet/roles/arch.md`
- `harness/bootstrap/duet/roles/dev.md`
- `harness/bootstrap/duet/roles/agent.md`
- `harness/bootstrap/nota/identity.md`
- `harness/bootstrap/nota/rules.md`
- `harness/bootstrap/README.md`
- `harness/bootstrap/mapping.md`

## 2. Why This Comes First

Right now, the project still depends on `.agents` for two different reasons:

- runtime still calls old `.agents` paths
- the bootstrap prompt and role substrate still only exists there

If runtime replacement happens before bootstrap import, the project enters a half-cutover state:

- old substrate is no longer treated as canonical
- new substrate is not yet established

That is not acceptable during the update phase.

So the correct order is:

1. import bootstrap source
2. import recovery docs
3. replace runtime dependencies
4. verify self-bootstrap

## 3. Scope

### 3.1 In Scope

- create a Harness-owned bootstrap directory inside `Entrance`
- copy the active role, rule, and identity files into that directory
- define old-path to new-path mapping
- define ownership boundaries between shipped bootstrap and mutable runtime state
- make the new directory layout stable enough for later runtime cutover

### 3.2 Out of Scope

- replacing `control.py`
- replacing `.agents/.worktrees`
- deleting or mutating any `.agents` file
- migrating `todo.md`
- changing Forge runtime behavior

## 4. Source Files To Import

The following files are the live bootstrap substrate to import now:

- `A:/.agents/duet/SKILL.md`
- `A:/.agents/duet/roles/arch.md`
- `A:/.agents/duet/roles/dev.md`
- `A:/.agents/duet/roles/agent.md`
- `A:/.agents/nota/identity.md`
- `A:/.agents/nota/rules.md`

These are not archival references. They still actively define operating behavior.

## 5. Target Layout

Recommended new canonical home:

```text
harness/
  bootstrap/
    duet/
      SKILL.md
      roles/
        arch.md
        dev.md
        agent.md
    nota/
      identity.md
      rules.md
    README.md
    mapping.md
```

### 5.1 Rationale

- `harness/` makes ownership explicit
- `bootstrap/` distinguishes shipped startup substrate from runtime state
- `duet/` and `nota/` preserve recognizability from the old layout
- `mapping.md` preserves auditability during transition

## 6. Old To New Mapping

| Old path | New path |
|---|---|
| `A:/.agents/duet/SKILL.md` | `harness/bootstrap/duet/SKILL.md` |
| `A:/.agents/duet/roles/arch.md` | `harness/bootstrap/duet/roles/arch.md` |
| `A:/.agents/duet/roles/dev.md` | `harness/bootstrap/duet/roles/dev.md` |
| `A:/.agents/duet/roles/agent.md` | `harness/bootstrap/duet/roles/agent.md` |
| `A:/.agents/nota/identity.md` | `harness/bootstrap/nota/identity.md` |
| `A:/.agents/nota/rules.md` | `harness/bootstrap/nota/rules.md` |

## 7. Ownership Rules

### 7.1 Shipped bootstrap

These files are source-controlled and ship with the repo:

- role definitions
- duet bootstrap skill
- nota identity and rules
- migration mapping and bootstrap README

These belong inside `Entrance`.

### 7.2 Mutable local runtime state

These are not part of this import and should remain local-state concerns:

- Vault secrets
- worktree instances
- runtime DB rows
- user-local connector config
- temporary prompt outputs

These do not belong under `harness/bootstrap/`.

### 7.3 Explicit non-import for now

Do not import the following as canonical bootstrap in this issue:

- `A:/.agents/nota/todo.md`
- `A:/.agents/nota/data/store.db`
- `A:/.agents/nota/data/store.json`
- `A:/.agents/nota/scripts/control.py`
- `A:/.agents/nota/scripts/db.py`

They belong to later memory and runtime migration lines.

## 8. Import Method

Use a strict copy-first procedure:

1. create target directories in `Entrance`
2. copy source files into the new locations
3. add a short provenance header in copied files if needed
4. add `README.md` explaining purpose and transition state
5. add `mapping.md` with explicit old/new path mapping
6. do not edit old files
7. do not delete old files

The imported copies should initially preserve content as faithfully as possible.

This is not a rewrite pass. It is an ownership-establishment pass.

## 9. Minimal Deliverables

`MYT-64` should be considered complete only if all of the following exist:

- `harness/bootstrap/duet/SKILL.md`
- `harness/bootstrap/duet/roles/arch.md`
- `harness/bootstrap/duet/roles/dev.md`
- `harness/bootstrap/duet/roles/agent.md`
- `harness/bootstrap/nota/identity.md`
- `harness/bootstrap/nota/rules.md`
- `harness/bootstrap/README.md`
- `harness/bootstrap/mapping.md`

## 10. Acceptance Criteria

- [ ] target bootstrap directory exists in `Entrance`
- [ ] all six live source files have canonical copies
- [ ] mapping between old and new paths is explicit
- [ ] shipped bootstrap is clearly separated from mutable runtime state
- [ ] `.agents` original files remain untouched
- [ ] no runtime code is changed as part of this issue

## 11. Risks

- rewriting content during copy can accidentally change live behavior
- mixing bootstrap files with mutable state will recreate the same ownership confusion later
- importing `todo.md` now would likely import corruption
- bundling runtime replacement into this issue would hide whether bootstrap import itself succeeded

## 12. One-Line Conclusion

`MYT-64` exists to give `Entrance` a real, source-controlled bootstrap home before any runtime cutover begins.
