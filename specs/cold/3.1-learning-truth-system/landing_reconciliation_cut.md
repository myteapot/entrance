# Landing Reconciliation Cut

> Status: first cold reconciliation cut from the 2026-03-22 Linear snapshot import

## Purpose

- classify the imported Linear planning shells without reheating the hot root
- separate external mirror evidence from Entrance-owned planning truth
- identify the narrowest bootstrap lane worth promoting into internal storage truth later

## Evidence Boundary

- Source snapshot: `A:\Agent\linear-entrance-snapshot-2026-03-22.json`
- Original landing proof DB: `A:\Agent\Entrance\.tmp\landing-appdata\entrance.db`
- Live runtime DB after real landing import and recovery-seed absorption: `%LOCALAPPDATA%/Entrance/entrance.db`
- Verified landing counts now present in the live runtime DB:
  - `external_issue_mirrors = 50`
  - `planning_items = 50`
  - `planning_item_links = 52`
  - `promotion_records = 100`
  - all `planning_items.status = seeded`
  - all `planning_items.reconciliation_status = unreconciled`
- Verified recovery-seed storage import now present in the live runtime DB:
  - `source_system = recovery_seed`
  - `ingest_run_id = 2`
  - `imported_table_count = 10`
  - `imported_row_count = 340`
  - `imported_artifact_count = 342`
- Landing code currently stores only:
  - external capture in `external_issue_mirrors`
  - seeded internal shells in `planning_items`
  - import-time promotion history in `promotion_records`
- Landing code does not yet persist a first-class reconciliation bucket such as `critical_path / cold_backlog / historical / promoted`.

## Storage Fact That Must Not Be Blurred

- `.tmp/landing-appdata/entrance.db` remains useful only as original proof residue for the landing path; it is not the production truth DB.
- The repo-root recovery seed has now been absorbed into live runtime storage as `recovery_seed` artifacts plus storage-only promotion markers.
- Therefore the first reconciliation cut below is a cold-truth classification pass over evidence that is now carried by the live runtime DB, not by a separate repo-root owner DB.

## Reconciliation Rule

- Prefer runtime and storage facts over Linear state labels.
- Do not treat Linear `Done` as automatic internal truth promotion.
- Keep external mirrors as evidence even when the corresponding planning shell is demoted, parked, or judged historical.
- Promote only the items that still define current Entrance ownership, provenance, or bootstrap direction.
- Keep UI, sync, and automation ideas cold until the current bootstrap absorption lane is reconciled.

## First Cut

### Bootstrap Critical Path

These are the only imported shells that currently form the active bootstrap absorption lane.

- `MYT-63` `decommission .agents master control`
  - This is the real master issue for turning external substrate into Entrance-owned assets.
  - It already names the live runtime dependencies that still block de-`.agents` operation.
- `MYT-64` `bootstrap import from .agents`
  - Repo-side canonical copies now exist under `harness/bootstrap/`.
  - This landed the bootstrap ownership cut without replacing runtime behavior yet.
- `MYT-65` `recovery docs import`
  - Repo-side recovery copies now exist under `specs/recovery/`.
  - This landed the provenance-preservation cut needed before later runtime cutover can claim durable ownership.

### Completed Gate Worth Keeping Hot-Adjacent

- `MYT-61` `runtime verification gate`
  - Keep this as a completed gate, not an active shell.
  - It matters because it tests whether already-merged claims were actually visible in runtime, which is exactly the same discipline needed for reconciliation.

### Cold Backlog

These items stay preserved in cold backlog, but they should not drive the current cycle.

- `MYT-45` `embedded NOTA chat UI`
- `MYT-46` `Harness management panel`
- `MYT-47` `dashboard orchestration visualization`
- `MYT-49` `MCP config UX`
- `MYT-50` `splash screen`
- `MYT-51` `Linear board mirror`
- `MYT-52` `NOTA DB connector bridge`
- `MYT-53` `Forge launch-all`
- `MYT-54` `auto-trigger dispatch`
- `MYT-60` `coffee-chat approval panel`
- `MYT-62` `GitLab connector auth consolidation`

Why these stay cold now:

- they are mostly UI, automation, or connector-hardening ideas
- the current cycle does not require Linear write-back, auto-trigger, or OAuth repair
- GitLab read access is already good enough for the present cycle, so `MYT-62` is structurally important but not phase-forcing today

### Historical, Repeated, Or Canceled Residue

These shells should remain mirrored as evidence but should not stay in the active planning queue.

- Explicit canceled or duplicate residue:
  - `MYT-15`
  - `MYT-16`
  - `MYT-17`
  - `MYT-18`
  - `MYT-44`
- Landed implementation history:
  - `31` issues are `Done`
  - most of them are already absorbed into code, repo docs, or prior operating history
  - they are useful as provenance and audit trail, not as an active reconciliation queue

### Do Not Promote Blindly From Linear State

- `MYT-57` `control.py retirement` is `Done` in Linear, but current repo facts still show a runtime cut that remains unverified under `.agents`-absent conditions:
  - `src-tauri/src/plugins/forge/mod.rs` now generates Agent prompts from `harness/bootstrap/duet/SKILL.md` and no longer shells to `A:/.agents/nota/scripts/control.py`
  - `src-tauri/src/plugins/forge/mod.rs` now uses `%LOCALAPPDATA%/Entrance/worktrees/{project}/feat-{ISSUE}` as its only runtime worktree owner path
  - `src/pages/Forge.tsx` now shows prompt source as `Entrance-owned harness/bootstrap prompt`
- Therefore `MYT-57` should still be treated as partial progress, not as fully promoted internal truth, because `.agents`-absent end-to-end verification is still open.

## Promotion Candidates For Later Internal Storage Truth

If a later schema or object-ledger pass promotes planning shells into stronger internal truth, the first candidates should be:

- `MYT-63`
  - current external-to-internal ownership boundary
- `MYT-64`
  - copy-first bootstrap absorption cut
- `MYT-65`
  - recovery provenance absorption cut
- `MYT-61`
  - completed runtime-verification gate
- `MYT-56`
  - earlier DB-ownership merge that explains why Entrance, not standalone NOTA storage, is the long-term owner

These are promotion candidates because they still shape current operating truth, not merely because they were once planned in Linear.

## Resulting Operating Posture

- Active reconciliation lane: `MYT-63`, with `MYT-64` and `MYT-65` already landed as repo-side import cuts
- Keep backlog preserved but cold: `MYT-45 / MYT-46 / MYT-47 / MYT-49 / MYT-50 / MYT-51 / MYT-52 / MYT-53 / MYT-54 / MYT-60 / MYT-62`
- Keep canceled and duplicate items mirrored only as external evidence
- Keep already-landed slices as provenance unless they still define present ownership boundaries
- Do not overwrite external mirrors when later promotion happens; append Entrance-owned reconciliation truth alongside them
