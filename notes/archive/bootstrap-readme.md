# Harness Bootstrap

> Status: landed copy-first bootstrap import
> Source issue: `MYT-64`

## Purpose

- give `Entrance` a source-controlled canonical home for the live bootstrap substrate that was still only living under `A:/.agents`
- separate shipped bootstrap assets from mutable runtime state
- establish ownership before any runtime cutover work begins

## What Is Included

- `duet/SKILL.md`
- `duet/roles/arch.md`
- `duet/roles/dev.md`
- `duet/roles/agent.md`
- `nota/identity.md`
- `nota/rules.md`
- `mapping.md`

## Import Rule

- these files were copied from `A:/.agents` without mutating the source files
- this directory is an ownership-establishment surface, not a runtime-cutover claim
- runtime code may still read old `.agents` paths until a later replacement cycle lands

## Source Boundary

Imported on `2026-03-22` from:

- `A:/.agents/duet/SKILL.md`
- `A:/.agents/duet/roles/arch.md`
- `A:/.agents/duet/roles/dev.md`
- `A:/.agents/duet/roles/agent.md`
- `A:/.agents/nota/identity.md`
- `A:/.agents/nota/rules.md`

## Not Included

These remain out of scope for this bootstrap import:

- `A:/.agents/nota/todo.md`
- `A:/.agents/nota/data/store.db`
- `A:/.agents/nota/data/store.json`
- `A:/.agents/nota/scripts/control.py`
- `A:/.agents/nota/scripts/db.py`
- `A:/.agents/.worktrees/...`

## Ownership Reading

- source-controlled bootstrap assets belong here
- mutable local runtime state belongs in app data, DB, Vault, or managed worktree locations
- later runtime replacement should read from this directory instead of claiming `.agents` is still the canonical bootstrap home
