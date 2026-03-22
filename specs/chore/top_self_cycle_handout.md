# Top Self-Cycle Handout

> Purpose: continuation packet for resuming Entrance from the compressed top root into the newly landed minimal landing layer, without reheating root ambiguity or losing the reconciliation queue

## Current Snapshot

- Branch: `codex/docs-top-self-cycle-handout-20260322`
- Commit checkpoint: `4cc849a52b84585e43b1110ae7f9e4a460370ff4`
- Active MR: `http://server:9311/pub/entrance/-/merge_requests/3`
- Previously merged checkpoint MR: `http://server:9311/pub/entrance/-/merge_requests/2`
- Minimal landing layer `v0` is now landed in core code and should be treated as live program truth, not as a side plugin experiment.
- Real Linear snapshot import has been executed successfully from:
  - `A:\Agent\linear-entrance-snapshot-2026-03-22.json`
- Real import checkpoint:
  - `ingest_run_id = 1`
  - `artifact_sha256 = efacdf3f3fed206bc4c29325fc84a26544eb895c32fb62a69f8dd02a1de49f4d`
  - `imported_issue_count = 50`
  - `imported_document_count = 1`
  - `imported_milestone_count = 0`
  - `imported_planning_item_count = 50`
- Post-import landing state in the verification DB:
  - `external_issue_mirrors = 50`
  - `planning_items = 50`
  - `planning_item_links = 52`
  - `promotion_records = 100`
  - `unreconciled_planning_items = 50`
- The next real phase is not another architecture rewrite; it remains `Landing / Reconciliation`.
- The first cold reconciliation cut is now landed:
  - `specs/cold/3.1-learning-truth-system/landing_reconciliation_cut.md`
- That first cut classifies the imported shells as:
  - `bootstrap critical path = 3` (`MYT-63 / MYT-64 / MYT-65`)
  - `cold backlog = 11`
  - `canceled or duplicate residue = 5`
  - `done historical items = 31`
- `MYT-61` is now treated as a completed verification gate kept hot-adjacent, not as an active shell.
- The compressed hot root is now canonical:
  - `specs/top/machine.md`
  - `specs/top/control.md`
  - `specs/top/truth.md`
  - `specs/top/phase-todo.md`
  - `specs/top/pending.md`
- The numbered top docs remain mounted transitional detail only; they must not grow back into a second hot root.
- Codex now has a configured `GitLab MCP` server entry:
  - name: `gitlab`
  - url: `http://server:9311/api/v4/mcp`
  - auth mode: bearer token via `GITLAB_MCP_BEARER_TOKEN`
- Cold local-detail mounts now exist for the active machine/truth/control cuts, including:
  - `specs/cold/1.1-os-core/minimal_os_boundary.md`
  - `specs/cold/1.2-hierarchical-state-machine/minimal_top_graph.md`
  - `specs/cold/1.3-compiler-action-ir/minimal_registry_cut.md`
  - `specs/cold/2.1-otp-supervisor-model/minimal_supervision_binding.md`
  - `specs/cold/2.2-lead-model-3/minimal_control_slot_model.md`
  - `specs/cold/2.3-control-tree-node-lte-3/minimal_hot_control_tree.md`
  - `specs/cold/3.1-learning-truth-system/landing_reconciliation_cut.md`
  - `specs/cold/3.1-learning-truth-system/minimal_truth_plane.md`
- `pending.md` should now be read as "no active architecture or operational blocker in hot view"; prior GitLab MCP token notes are fallback history, not active pending.
- Repo-root `entrance.db` and `entrance.db.manifest.json` remain copy-only recovery seed artifacts and do not currently carry the landed landing tables.
- The verified landing import currently lives in `.tmp/landing-appdata/entrance.db`; treat it as evidence of the import path, not as canonical production truth.
- Do not churn either DB for editorial cleanup only.
- Local repo caution:
  - `src-tauri/src/plugins/forge/mod.rs` is modified in the worktree and was not touched by the landing commits in this window.
  - `.tmp/` exists as local verification residue and currently includes landing-layer temp appdata.

## Runtime Connector Note

- `GitLab MCP` is now proven live in-session for read access.
- This window successfully queried `pub/entrance`, MR `!3`, the MR diffs, and the MR commit list through GitLab MCP.
- The earlier bearer-token validation attempt that returned `403 insufficient_scope` is now historical fallback rather than an active blocker.
- If re-provisioning is needed outside the current session, the explicit required scope signal returned earlier was: `mcp api read_api`.
- The OAuth path remained unreliable on this machine because GitLab's OAuth discovery advertised `issuer/registration_endpoint = http://9123126222e6`, and that host was not locally resolvable.

## Landing Layer Checkpoint

### What Landed

- A minimal landing layer now exists as `core`, not as a plugin:
  - `src-tauri/migrations/0005_create_core_landing_tables.sql`
  - `src-tauri/src/core/data_store.rs`
  - `src-tauri/src/core/landing.rs`
  - `src-tauri/src/core/mod.rs`
  - `src-tauri/src/lib.rs`
- The landing split is now explicit and OS-driven:
  - `source_artifacts` / `external_issue_mirrors` hold external captured truth
  - `planning_items` / `promotion_records` hold Entrance-owned planning objects and promotion history
- Minimal manual entrypoints now exist:
  - `entrance landing import --file <path>`
  - `entrance landing runs`
  - `entrance landing mirrors`
  - `entrance landing planning`
  - `entrance landing unreconciled`
- The first real bridge from Linear into Entrance now works without requiring live OAuth, write-back, or UI.

### What Was Explicitly Deferred

- No auto-sync
- No OAuth repair work
- No UI for landing/reconciliation
- No Linear write-back
- No attempt yet to redesign the whole planning system before reconciliation truth exists

### Verification

- Tests passed:
  - `cargo test landing_tables_round_trip --lib --config "build.rustc-wrapper=''" `
  - `cargo test imports_linear_snapshot_into_landing_tables --lib --config "build.rustc-wrapper=''" `
- Real import was exercised against the exported snapshot, not just fixtures.
- The temp verification DB lives under `.tmp/landing-appdata/` and should be treated as a sandbox proof point rather than canonical production data.

### What It Means

- Entrance now has the minimal landing substrate needed to absorb external planning truth into its own DB.
- Entrance is not yet fully detached from Linear at the operating level, because the first reconciliation cut is now classified in docs but not yet absorbed as owned storage truth.
- The correct next move is to keep working the `MYT-63 / MYT-64 / MYT-65` absorption lane, then selectively absorb milestones/issues into internal storage by hot/cold fact, and only then redesign `NEXT WAVE`.

## State-Machine Reading Of The Program

### Global Projection

- `FLOW_PHASE = CYCLE + LANDING_V0_LANDED`
  the compressed root is landed, and the minimal landing substrate now exists; the active queue has shifted to reconciliation rather than root design.
- `ATTENTION_STATE = READY`
  there is no known blocking dependency that requires Human wake-up before the next self-cycle.
- `INTEGRITY_OVERLAY = RECONCILIATION_BACKLOG_CLASSIFIED`
  there is no active top-level architecture conflict; the imported shells now have a first cold-truth classification, but owned storage reconciliation has not happened yet.

### Human-Interruption Rule

- Do not wake Human for hot-root restatement, editorial cleanup, or local-detail completion.
- Do not wake Human for landing-layer ingestion, reconciliation classification, or cold/hot sync as long as they stay within the already chosen substrate boundaries.
- Wake Human only if a new canonical boundary decision appears or a hard `state / route / writer / truth` conflict cannot be resolved locally.

## Canonical Read Set For A New Window

Read in this order:

1. `specs/chore/top_self_cycle_handout.md`
2. `specs/top/README.md`
3. `specs/top/machine.md`
4. `specs/top/control.md`
5. `specs/top/truth.md`
6. `specs/top/phase-todo.md`
7. `specs/top/pending.md`
8. `specs/cold/3.1-learning-truth-system/landing_reconciliation_cut.md`
9. `src-tauri/migrations/0005_create_core_landing_tables.sql`
10. `src-tauri/src/core/landing.rs`
11. `src-tauri/src/core/data_store.rs`
12. `src-tauri/src/lib.rs`

Then descend only into the single selected trunk or substrate lane for the current cycle.

## Trunk State

### Machine

Current state:

- `CYCLE + low-motion`

What is already landed:

- hot root summary is stable in `machine.md`
- mounted hot detail docs are slimmed and no longer act as root competitors
- cold machine-side drafts now cover the whole-system graph, registry cut, and supervision binding
- packet resolution now stays explicitly runtime-routed, with sender re-entry derived from returned objects rather than packet mutation
- phase remains projection and cadence remains Human-window protocol rather than peer machine state
- boundary intake now stays boundary-specific at v0, with `Policy` as the default internal ingress target when project lineage begins

What is still open:

- no active machine-side document ambiguity is currently mounted
- later machine work should stay implementation-facing and land below the root unless a new oracle appears

### Truth

Current state:

- `CYCLE + low-motion`

What is already landed:

- hot root summary is stable in `truth.md`
- storage/cold/hot split is fixed
- `minimal_truth_plane.md` now holds the denser landing rules below the root
- truth-side admission defaults, cadence subtype defaults, and retrieval attachment rules are now mounted below the root

What is still open:

- no active truth-side document ambiguity is currently mounted
- later truth work should stay implementation-facing and land below the root unless a new oracle appears

### Control

Current state:

- `CYCLE + low-motion`

What is already landed:

- hot-control compression target is fixed at `3 semantic hot docs + 1 phase todo + 1 pending`
- `NOTA` is fixed as Human-facing boundary host rather than internal strategy superuser
- `2.2 / 2.3` are now mounted detail, not root competitors
- control-slot boundary and hot-control compression cut are now both mounted below the root
- legacy `Bt tree` wording is now historical only; the canonical naming is `Control Tree`

What is still open:

- no active control-side document ambiguity is currently mounted
- later physical relocation remains optional cleanup only if compression pressure returns

### Control Warning

- `specs/cold/2.2-lead-model-3/prd.md` is legacy and currently mojibake-prone in terminal rendering.
- Treat it as a weak historical reference, not as canonical architecture source.

## Landing / Reconciliation State

Current state:

- `LANDING = landed-v0`
- `RECONCILIATION = first-cut-classified`

What is already landed:

- external truth can be imported into Entrance DB through a stable, manually triggered path
- every imported Linear issue now has a mirrored landing record plus an Entrance planning shell
- promotion history is being recorded from the first import onward
- the minimal landing handoff layer now exists in code, not just in prose
- a first cold reconciliation cut now exists and classifies the 50 imported shells into:
  - `MYT-63 / MYT-64 / MYT-65` as the active bootstrap absorption lane
  - `11` parked backlog items
  - `5` canceled or duplicate residue items
  - `31` done historical items that remain provenance rather than live queue

What is still open:

- define which Linear milestones/issues deserve promotion into internal storage as active program truth
- absorb the current classification into stronger owned storage truth when the schema/object path is ready
- redesign `NEXT WAVE` only after the first reconciliation cut exists

Local caution:

- do not treat `.tmp/landing-appdata/entrance.db` as production truth
- do not mistake repo-root `entrance.db` for landing truth; it is still a copy-only recovery seed
- do not casually revert `src-tauri/src/plugins/forge/mod.rs`; it is unrelated local state in the current worktree

## Self-Cycle Protocol For The Next Agent

### `IN`

1. Verify branch/MR context and confirm the read set above.
2. Confirm that the compressed hot root is still canonical.
3. Choose exactly one semantic trunk or substrate lane for this cycle:
   - `Machine`
   - `Truth`
   - `Control`
   - `Landing / Reconciliation`

### `CYCLE`

1. Descend only into that trunk's mounted detail and cold docs.
2. Classify the intended work as one of:
   - editorial compression
   - local-detail completion
   - reconciliation / absorption
   - new oracle
3. If the work is editorial compression:
   - keep the change in mounted hot detail or cold docs
   - do not update DB
   - do not reopen root structure
4. If the work is local-detail completion:
   - prefer cold docs first
   - promote to hot only if the hot root truly needs a sharper oracle summary
5. If the work is reconciliation / absorption:
   - start from the already-landed first cut in `landing_reconciliation_cut.md`
   - keep `MYT-63 / MYT-64 / MYT-65` as the default active lane unless runtime fact disproves it
   - promote only the items that deserve internal ownership by current hot/cold fact
   - keep external mirrors intact as captured evidence
6. If the work produces a new oracle:
   - update the relevant hot trunk
   - write DB decision or memory records
   - sync `entrance.db.manifest.json`
7. If ambiguity remains unresolved after local critique:
   - park it in cold or `pending.md`
   - do not bloat the hot root with speculative text

### `OUT`

1. Leave a small, explicit landed delta.
2. State whether the cycle changed:
   - hot root
   - mounted detail
   - cold detail
   - DB truth
3. Keep the next cycle startable without rereading the whole conversation.

## Hard Guardrails

- Do not re-expand the hot root beyond `Machine / Control / Truth + phase-todo + pending`.
- Do not let numbered top docs regrow into root summaries.
- Do not treat `do / learn / chat` as reopened root architecture unless Human explicitly asks to reopen it.
- Do not create DB churn for editorial cleanup.
- Do not mistake landing-layer existence for completed de-Linear sovereignty; reconciliation still has to happen.
- Do not build sync, OAuth, UI, or write-back before the reconciliation model is proven locally.
- Do not trust row-local prose when a fact should be runtime-derived from registry plus topology.
- Do not wake Human for local-quality, local-structure, or local-compression work that can be resolved inside the current trunk.

## Recommended Priority Rule

- No semantic trunk currently requires forced wake-up by default.
- The default next move is `Landing / Reconciliation`, not another root-level architecture pass.
- Prefer the narrowest reconciliation cut that turns imported shells into owned internal classification truth.
- Inside that lane, prefer `MYT-64` and `MYT-65` as the smallest next absorption steps under the `MYT-63` master control boundary.
- Keep `Truth` parked unless landing/reconciliation exposes a genuine new truth-plane gap.
- Keep `Control` parked unless reconciliation exposes a real control-surface or ownership-host conflict.
- Never advance more than one semantic trunk in the same self-cycle unless a newly discovered oracle forces a cross-trunk promotion.

## Success Condition For The Next Window

A good next cycle should end with all of the following true:

- one trunk moved forward without reheating root ambiguity
- or one bounded reconciliation cut landed without inventing unnecessary new structure
- hot root stayed compressed
- any new detail landed below the root first
- Human interruption budget remained unused unless a genuine new canonical decision appeared
