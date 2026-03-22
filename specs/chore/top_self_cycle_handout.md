# Top Self-Cycle Handout

> Purpose: new-window continuation packet for advancing Entrance top architecture without reheating root ambiguity

## Current Snapshot

- Branch: `codex/docs-top-self-cycle-handout-20260322`
- Commit checkpoint: `c36589ad1359e239afb428ac41c246b868109472`
- Active MR: `http://server:9311/pub/entrance/-/merge_requests/3`
- Previously merged checkpoint MR: `http://server:9311/pub/entrance/-/merge_requests/2`
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
- Cold local-detail drafts now exist for the heaviest remaining machine/truth/control cuts:
  - `specs/cold/1.2-hierarchical-state-machine/minimal_top_graph.md`
  - `specs/cold/1.3-compiler-action-ir/minimal_registry_cut.md`
  - `specs/cold/2.1-otp-supervisor-model/minimal_supervision_binding.md`
  - `specs/cold/2.3-control-tree-node-lte-3/minimal_hot_control_tree.md`
  - `specs/cold/3.1-learning-truth-system/minimal_truth_plane.md`
- `pending.md` should now be read as "no active architecture or operational blocker in hot view"; prior GitLab MCP token notes are fallback history, not active pending.
- `entrance.db` and `entrance.db.manifest.json` were synced in the MR checkpoint; do not churn DB for editorial cleanup only.

## Runtime Connector Note

- `GitLab MCP` is now proven live in-session for read access.
- This window successfully queried `pub/entrance`, MR `!3`, the MR diffs, and the MR commit list through GitLab MCP.
- The earlier bearer-token validation attempt that returned `403 insufficient_scope` is now historical fallback rather than an active blocker.
- If re-provisioning is needed outside the current session, the explicit required scope signal returned earlier was: `mcp api read_api`.
- The OAuth path remained unreliable on this machine because GitLab's OAuth discovery advertised `issuer/registration_endpoint = http://9123126222e6`, and that host was not locally resolvable.

## State-Machine Reading Of The Program

### Global Projection

- `FLOW_PHASE = CYCLE`
  the compressed root is landed, but local detail completion is still ongoing.
- `ATTENTION_STATE = READY`
  there is no known blocking dependency that requires Human wake-up before the next self-cycle.
- `INTEGRITY_OVERLAY = none projected`
  no active top-level architecture conflict is currently mounted in hot view.

### Human-Interruption Rule

- Do not wake Human for hot-root restatement, editorial cleanup, or local-detail completion.
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

Then descend only into the single selected trunk for the current cycle.

## Trunk State

### Machine

Current state:

- `CYCLE + READY`

What is already landed:

- hot root summary is stable in `machine.md`
- mounted hot detail docs are slimmed and no longer act as root competitors
- cold machine-side drafts now cover the whole-system graph, registry cut, and supervision binding
- packet resolution now stays explicitly runtime-routed, with sender re-entry derived from returned objects rather than packet mutation
- phase remains projection and cadence remains Human-window protocol rather than peer machine state

What is still open:

- remaining narrow strategy-mapping and stronger replacement-path detail in supervision binding

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

What is still open:

- whether any legacy mounted detail is still worth preserving after compression stabilizes
- eventual naming cleanup such as old `Bt tree` wording
- any later physical relocation, but only after compression clearly holds

### Control Warning

- `specs/cold/2.2-lead-model-3/prd.md` is legacy and currently mojibake-prone in terminal rendering.
- Treat it as a weak historical reference, not as canonical architecture source.

## Self-Cycle Protocol For The Next Agent

### `IN`

1. Verify branch/MR context and confirm the read set above.
2. Confirm that the compressed hot root is still canonical.
3. Choose exactly one semantic trunk for this cycle:
   - `Machine`
   - `Truth`
   - `Control`

### `CYCLE`

1. Descend only into that trunk's mounted detail and cold docs.
2. Classify the intended work as one of:
   - editorial compression
   - local-detail completion
   - new oracle
3. If the work is editorial compression:
   - keep the change in mounted hot detail or cold docs
   - do not update DB
   - do not reopen root structure
4. If the work is local-detail completion:
   - prefer cold docs first
   - promote to hot only if the hot root truly needs a sharper oracle summary
5. If the work produces a new oracle:
   - update the relevant hot trunk
   - write DB decision or memory records
   - sync `entrance.db.manifest.json`
6. If ambiguity remains unresolved after local critique:
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
- Do not trust row-local prose when a fact should be runtime-derived from registry plus topology.
- Do not wake Human for local-quality, local-structure, or local-compression work that can be resolved inside the current trunk.

## Recommended Priority Rule

- Prefer `Machine` for the next self-cycle.
- Keep `Truth` parked unless runtime implementation exposes a genuine new truth-plane gap.
- Keep `Control` parked unless compression pressure, naming cleanup, or legacy-pruning work creates a stronger reason to move it.
- Never advance more than one semantic trunk in the same self-cycle unless a newly discovered oracle forces a cross-trunk promotion.

## Success Condition For The Next Window

A good next cycle should end with all of the following true:

- one trunk moved forward without reheating root ambiguity
- hot root stayed compressed
- any new detail landed below the root first
- Human interruption budget remained unused unless a genuine new canonical decision appeared
