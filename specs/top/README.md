# Top Layer

> Status: compressed hot root

The active hot root is a retained projection of runtime truth, not a second authoring plane.

The active six-file surface is:

- [README.md](./README.md)
- [machine.md](./machine.md)
- [control.md](./control.md)
- [truth.md](./truth.md)
- [phase-todo.md](./phase-todo.md)
- [pending.md](./pending.md)

Runtime ownership is now unified under the Entrance owner root:

- `~/.entrance/entrance.toml`
- `~/.entrance/data/entrance.db`
- `~/.entrance/logs/`
- `~/.entrance/cache/`
- `~/.entrance/exports/`
- `~/.entrance/snapshots/`
- `~/.entrance/worktrees/`

Top-layer rules:

- DB is the only canonical writer.
- README, hot root, cold docs, GUI, CLI, and MCP are all projections from DB truth.
- `README(Oracle)` is a routing projection, not a peer truth plane.
- `Hot Root` is fixed at six files and should not grow through convenience.
- `Cold Docs` remain canonicalized in DB even when they are exported back to files.
- `passed human round = acceptance`, and it must be formalized as a cadence object rather than left as chat implication.
- `fully settled round` is stricter than acceptance and only holds after acceptance, no next step, and checkpoint carry-forward all land.
- anti-Zeno is a first-class progress discipline derived from runtime truth, not a second truth plane.
- a `human round` is the primary continuity unit for interaction, checkpointing, acceptance, projection, and repair.
- projection freshness, dirty state, and repair are part of runtime truth rather than operator folklore.
- Human is the final sovereign, but `NOTA` remains the only normal semantic ingress and egress.
- `Policy` is the highest internal writer; `Arch / Dev / Agent` are bounded execution lanes only.
- the numbered files in this directory remain mounted detail docs, but they do not outrank the hot root or the DB.

Constitutional detail docs:

- [v0_constitution.md](../cold/1.1-os-core/v0_constitution.md)
- [projection_boundary.md](../cold/1.1-os-core/projection_boundary.md)
- [human_round_state_machine.md](../cold/1.2-hierarchical-state-machine/human_round_state_machine.md)
- [authority_matrix.md](../cold/2.2-lead-model-3/authority_matrix.md)
- [v0_exit_criteria.md](../cold/1.1-os-core/v0_exit_criteria.md)
