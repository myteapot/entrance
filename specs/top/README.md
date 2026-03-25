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
- `passed human round = acceptance`, and it must be formalized as a cadence object rather than left as chat implication.
- `fully settled round` is stricter than acceptance and only holds after acceptance, no next step, and checkpoint carry-forward all land.
- anti-Zeno is a first-class progress discipline derived from runtime truth, not a second truth plane.
- the numbered files in this directory remain mounted detail docs, but they do not outrank the hot root or the DB.
