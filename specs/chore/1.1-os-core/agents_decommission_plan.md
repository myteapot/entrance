# Entrance .agents Decommission Plan

> Arch design
> Status: Proposed
> Goal: make `A:/.agents` deletable in about two releases without losing Entrance capability

## 1. Background

The recovered NOTA and Duet substrate is currently split across three places:

- canonical product docs already committed in `Entrance`
- hot prompt and role files still living under `A:/.agents`
- runtime code in `Entrance` still reading old `.agents` paths directly

The recovery report dated `2026-03-21` recommends a copy-first migration of the recovered substrate into `Entrance`, instead of treating the standalone `.agents` DB and scripts as the final truth source.

That recommendation is now the correct direction for decommissioning `.agents`.

## 2. Current Reality

### 2.1 Already inside the `Entrance` repo

The following files already carry part of the architecture truth:

- `oracles/oracle.md`
- `oracles/dialog.md`
- `specs/backend.md`
- `specs/milestones.md`

These should continue to exist as source-controlled design truth.

### 2.2 Imported copy-first, source still preserved in `.agents`

The following hot files now have repo-side canonical copies under `Entrance/harness/bootstrap/`, while the original `.agents` files remain preserved:

- `A:/.agents/duet/SKILL.md`
- `A:/.agents/duet/roles/arch.md`
- `A:/.agents/duet/roles/dev.md`
- `A:/.agents/duet/roles/agent.md`
- `A:/.agents/nota/identity.md`
- `A:/.agents/nota/rules.md`

The following recovered documents now also have repo-side copies under `Entrance/specs/recovery/`, while the original `.agents` files remain preserved:

- `A:/.agents/nota/data/docs/entrance-control-plane.md`
- `A:/.agents/nota/data/docs/entrance-dashboard-runtime.md`
- `A:/.agents/nota/data/docs/entrance-memory-migration.md`
- `A:/.agents/nota/data/docs/entrance-memory-sql-draft.md`
- `A:/.agents/nota/data/docs/entrance-open-questions.md`
- `A:/.agents/nota/data/docs/entrance-os-arch.md`
- `A:/.agents/nota/data/docs/entrance-ralph-loop.md`
- `A:/.agents/nota/data/docs/entrance-continuous-learning.md`
- `A:/.agents/nota/data/docs/entrance-remote-truth-source.md`
- `A:/.agents/nota/data/docs/recovery-report-2026-03-21.md`

`A:/.agents/nota/todo.md` is currently not a reliable migration source and must not be treated as canonical without repair.

### 2.3 Runtime code no longer directly depends on `.agents`

As of this design, the active Forge runtime path no longer depends on old `.agents` paths:

- `src-tauri/src/plugins/forge/mod.rs`
  - prompt generation now reads Entrance-owned `harness/bootstrap/duet/SKILL.md`
  - managed worktree discovery now uses `%LOCALAPPDATA%/Entrance/worktrees/{project}/feat-{ISSUE}` as its only runtime owner path
- `src/pages/Forge.tsx`
  - UI now surfaces `Entrance-owned harness/bootstrap prompt`

This means `.agents` has now been demoted from "runtime dependency plus preserved recovery source" to "preserved recovery source pending verification," but it is still not deletable.

## 3. Decommission Goal

Two releases from now, deleting `A:/.agents` should remove only a transitional recovery substrate, not any required runtime capability.

To reach that state, `Entrance` must own all of the following:

- role and rule bootstrap assets
- prompt generation
- worktree discovery and worktree lifecycle
- memory import and memory storage
- recovered architecture documents
- runtime configuration for connectors and harness rules

## 4. Non-Goals

This plan does not do the following:

- delete `.agents` now
- move files destructively out of `.agents`
- assume all recovered files are clean enough to become canonical without review
- preserve the exact old path layout inside `Entrance`

## 5. Migration Principles

### 5.1 Copy first, never move first

Every recovered file must be copied into `Entrance` before any old source is retired.

No part of the migration should rely on destructive directory operations. Deletion of old files, if ever done, is a separate human-approved cleanup step after verification.

### 5.2 Capability parity beats file parity

The exit condition is not "all files were moved."

The real exit condition is:

- Entrance can bootstrap the same roles
- Entrance can generate the same prompts
- Entrance can run the same worktree flow
- Entrance can access the same memory domains
- Entrance can operate with `.agents` absent

### 5.3 `Entrance` becomes the canonical home

After migration:

- repo-shipped prompt and role assets live in `Entrance`
- mutable local state lives in `Entrance` app data and DB
- `.agents` becomes backup material only

### 5.4 Recovery artifacts remain auditable

Recovered docs must be copied with provenance preserved so that future audits can trace where the rebuilt architecture came from.

## 6. Target Layout Inside `Entrance`

The new canonical layout should separate three concerns.

### 6.1 Source-controlled architecture truth

Continue using:

- `oracles/` for product truth and decision extraction
- `specs/` for implementation and migration design

Recovered recovery-era docs should be copied into a source-controlled recovery area, recommended as:

- `specs/recovery/`

### 6.2 Source-controlled harness bootstrap assets

The prompt and role substrate should move under a first-class harness-owned location, recommended as:

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
```

This location becomes the shipped bootstrap source for role, rule, and prompt behavior.

### 6.3 Local runtime state

Mutable user or machine state should live in app data, not in the repo:

- `%LOCALAPPDATA%/Entrance/entrance.db`
- `%LOCALAPPDATA%/Entrance/entrance.toml`
- Vault secret storage
- managed worktrees

Managed worktrees should no longer live under `A:/.agents/.worktrees`.

Recommended new owner:

- Harness or Forge managed path rooted in app data, for example
  `%LOCALAPPDATA%/Entrance/worktrees/{project}/feat-{ISSUE}`

The exact path may differ, but it must no longer depend on `.agents`.

## 7. Release Plan

### 7.1 Release N: Copy-first consolidation

Release N should do the following:

- copy recovered docs from `A:/.agents/nota/data/docs/` into `Entrance`
- copy hot role and rule files from `.agents` into `Entrance/harness/bootstrap/`
- define canonical ownership for each copied file
- add a mapping document from old path to new path
- keep `.agents` fully intact
- keep runtime compatibility with old flow while new sources are being established

Release N must not claim `.agents` is removable yet.

### 7.2 Release N+1: Runtime dependency replacement

Release N+1 should replace all active runtime dependencies on `.agents`:

- Forge prompt generation no longer shells out to `control.py`; it now reads the repo-owned bootstrap source
- Forge worktree discovery no longer reads `A:/.agents/.worktrees`
- prompt and role loading must read from Harness-owned sources
- memory reads must stop depending on `db.py` as the primary runtime path
- UI now advertises `Entrance-owned harness/bootstrap prompt` as the active source

At the end of Release N+1, `.agents` may still exist, but it should no longer be required by runtime code; the remaining gate is verification with `.agents` absent.

### 7.3 Release N+2: Removal readiness verification

Release N+2 is the verification release.

It should prove that:

- `Entrance` boots and runs with `.agents` renamed or absent
- Forge dispatch still works
- Board and Connector still see the expected memory domains
- worktree creation and prompt generation still work from the new owners
- recovered docs remain available from inside `Entrance`

A smaller headless verification cut is already landed:

- `cargo test prepare_dispatch_pipeline_builds_without_agents_runtime --lib --config "build.rustc-wrapper=''" `
- this proves Forge dispatch preparation can build from the Entrance-owned worktree and bootstrap prompt paths without active `.agents` runtime dependencies
- it does not yet replace the full app-level `.agents`-absent verification run above

A stronger bootstrapped headless verification cut is now also landed:

- `cargo test prepare_agent_dispatch_works_after_bootstrap_without_agents_runtime --lib --config "build.rustc-wrapper=''" `
- this boots fresh app-data config with Forge enabled, runs `prepare_agent_dispatch()`, and persists the resulting task request without active `.agents` runtime dependencies
- it also proves Forge's Linear token fallback tolerates fresh-bootstrap stores where Vault tables are absent
- it still does not replace the full GUI/app-level `.agents`-absent verification run above

A runtime-facing headless verification entrypoint is now also available:

- `entrance forge prepare-dispatch --project-dir <path>`
- this uses real CLI bootstrap plus `prepare_agent_dispatch()` and prints the prepared Forge dispatch payload as JSON
- it gives Entrance an operator-visible verification path outside internal-only Rust tests
- it still does not replace the full GUI/app-level `.agents`-absent verification run above

A stronger runtime-facing headless verification entrypoint is now also available:

- `entrance forge verify-dispatch --project-dir <path>`
- this uses real CLI bootstrap plus `prepare_agent_dispatch()`, translates the result into a default Codex task request, and persists a Pending Forge task
- it gives Entrance an operator-visible verification path for dispatch preparation plus Forge task persistence outside internal-only Rust tests
- it still does not replace the full GUI/app-level `.agents`-absent verification run above

Only after that verification should manual `.agents` cleanup even be considered.

## 8. Work Streams Required

### 8.1 Harness bootstrap migration

Needed outcome:

- role and rule files are owned by `Entrance`
- prompt templates and role prompts are generated from `Entrance` sources

This is the main ownership line for `MYT-46` style Harness work.

### 8.2 Prompt and worktree runtime replacement

Needed outcome:

- no runtime call to `control.py`
- no runtime assumption of `.agents/.worktrees`

This is the operational replacement line behind the earlier `control.py` retirement effort.

### 8.3 Memory migration and local truth-source migration

Needed outcome:

- recovered NOTA memory becomes `Entrance`-owned local truth
- docs, instincts, todos, and coffee chats have a stable home in `Entrance`

This is the long-tail data migration line.

### 8.4 Recovery artifact preservation

Needed outcome:

- recovery docs are copied and traceable
- architectural provenance stays reviewable after `.agents` is gone

## 9. Acceptance Criteria

`.agents` is only considered removable when all items below are true:

- [ ] all required hot files have canonical copies inside `Entrance`
- [ ] recovered docs are copied into `Entrance` with provenance retained
- [x] runtime code no longer depends on `A:/.agents/nota/scripts/control.py`
- [x] runtime code no longer depends on `A:/.agents/.worktrees`
- [x] Forge UI no longer names ``control.py prompt`` as the live prompt source
- [ ] role and rule bootstrap comes from `Entrance`-owned harness assets
- [ ] memory domains needed for NOTA and Duet are available from `Entrance` local state
- [ ] a verification run succeeds with `.agents` absent or renamed

The checked Forge runtime items mean prompt generation and worktree discovery now belong to Entrance. Broader role/rule bootstrap ownership, memory-domain migration, and `.agents`-absent verification are still open.

Recommended verification command before claiming readiness:

```text
rg -n "\.agents|control\.py|db\.py" src src-tauri
```

That scan should return no active runtime dependency hits, or only intentionally retained historical documentation references.

## 10. Risks

- Copying without ownership mapping can create two silent truths.
- Treating `todo.md` as authoritative can import corruption.
- Marking migration issues done without rechecking the repo can create false confidence.
- Moving worktrees too early can break the current self-bootstrap loop.

## 11. Transition Operating Mode

Until runtime decoupling is actually complete, the project stays in an update-phase operating mode:

- human paste-prompt flow remains the active dispatch path
- prompt generation now follows the Entrance-owned `harness/bootstrap/` path
- worktree discovery now follows the Entrance-managed app-data root
- `Entrance` may assist and observe, but is not yet allowed to claim full bootstrap ownership
- runtime replacement starts only after bootstrap assets and recovery docs are safely imported

This avoids a half-cutover state where the old substrate is no longer trusted, while the new substrate is not yet fully live.

## 12. Awake Budget

From the current state, the expected remaining human awake cycles before "Entrance self-bootstrap without paste-prompt" are:

1. Bootstrap import cycle
   Covers hot-file import and recovery-doc import.
2. Runtime decoupling cycle
   Covers `.agents`-absent end-to-end verification and any fixups it exposes.
3. Self-bootstrap verification cycle
   Reserved only if the first verification pass exposes a rollback or repair loop.

Conservative estimate:

- baseline: `1` awake cycle
- with one verification rollback/fix loop: `2` awake cycles

This budget assumes no major architectural reversal and no new hidden `.agents` runtime dependency is discovered.

## 13. One-Line Conclusion

`A:/.agents` should be retired as a transitional recovery substrate only after `Entrance` first absorbs its hot files, recovered docs, prompt pipeline, worktree pipeline, and memory ownership into first-class Harness, Forge, Connector, and local app-data surfaces.
