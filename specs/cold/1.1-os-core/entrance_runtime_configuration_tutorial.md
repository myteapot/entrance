# Entrance Runtime Configuration Tutorial

> Status: cold doc

This document explains the runtime owner root, configuration file, database path, projection exports, and migration behavior for Entrance after the `V0` post-consolidation path unification cut.

## 1. Canonical Owner Root

Entrance now uses one owner root per user:

- Linux: `~/.entrance`
- WSL: `/home/<user>/.entrance`
- Windows: `C:\Users\<user>\.entrance`

This is the only canonical runtime ownership root. Runtime continuity, exported views, logs, caches, and managed worktrees all live under this root.

Compatibility notes:

- `ENTRANCE_HOME` may override the owner root when a test harness or a controlled runtime sandbox needs a different root.
- `ENTRANCE_APP_DATA_DIR` is still accepted as a legacy compatibility override, but it now means the same thing: the Entrance owner root.

## 2. Default Layout

The default layout is:

```text
~/.entrance/
├── entrance.toml
├── data/
│   └── entrance.db
├── logs/
├── cache/
├── exports/
│   └── hot-root/
├── snapshots/
└── worktrees/
```

Meaning:

- `entrance.toml`: the only external configuration entrypoint
- `data/entrance.db`: canonical runtime continuity DB
- `logs/`: runtime logs
- `cache/`: disposable cache material
- `exports/`: projected file views generated from DB truth
- `snapshots/`: imported or retained snapshot artifacts
- `worktrees/`: Entrance-owned managed worktrees

## 3. Configuration File

The default `entrance.toml` now looks like this:

```toml
[core]
theme = "dark"
log_level = "info"
mcp_enabled = true

[paths]
runtime_db = "data/entrance.db"
logs = "logs"
cache = "cache"
exports = "exports"
snapshots = "snapshots"
worktrees = "worktrees"

[plugins.launcher]
enabled = true
hotkey = "Alt+Space"
scan_paths = []

[plugins.forge]
enabled = false
http_port = 9721

[plugins.vault]
enabled = false
```

Important rules:

- all path entries in `[paths]` are resolved relative to the owner root
- absolute paths are rejected
- `..` escapes are rejected
- `entrance.toml` itself stays at the owner-root top level

This keeps every external path governed by one root and one TOML file.

## 4. Database Path

The canonical database path is:

- `~/.entrance/data/entrance.db`

This replaces older runtime layouts that placed `entrance.db` directly under a platform-specific app-data directory.

The database is the only canonical writer for runtime continuity. Files, hot root docs, GUI summaries, CLI summaries, and MCP summaries are downstream projections.

## 5. Managed Worktrees

The canonical managed worktree owner path is:

- `~/.entrance/worktrees/{project}/feat-{ISSUE}`

Examples:

- Linux / WSL: `/home/rain/.entrance/worktrees/Entrance/feat-MYT-48`
- Windows: `C:\Users\rain\.entrance\worktrees\Entrance\feat-MYT-48`

If slot worktrees are created, they remain subordinate to the same owner root and project tree.

## 6. Exported Hot Root

Hot-root files are now explicitly treated as exported views.

Default export location:

- `~/.entrance/exports/hot-root/`

Current exported files:

- `README.md`
- `machine.md`
- `control.md`
- `truth.md`
- `phase-todo.md`
- `pending.md`

These files are not the source of truth. They are regenerated from runtime truth.

## 7. Acceptance and Anti-Zeno

The cadence layer now treats:

- `passed human round = acceptance`

This is formalized as `CADENCE_ACCEPTANCE_BUNDLE` in `cadence_objects`, instead of remaining only an implied receipt-chain meaning.

Also:

- anti-Zeno is a first-class derived projection
- anti-Zeno is not a second truth plane
- anti-Zeno must be visible in status, overview, and exported hot-root views

`fully settled round` is stricter than acceptance. It means:

- acceptance exists
- no next step remains
- checkpoint carry-forward has landed

## 8. Migration Behavior

When Entrance boots against an older owner root layout that still contains:

- `~/.entrance/entrance.db`

and the new canonical DB path does not yet exist, Entrance migrates the DB to:

- `~/.entrance/data/entrance.db`

This lets old local state survive the path-layout unification cut.

## 9. Operational Guidance

Recommended habits:

- read runtime state from `entrance nota status` and `entrance nota overview`
- keep runtime truth in DB first
- use `entrance nota export-hot-root` when you want a refreshed file projection
- treat exported files as views for navigation, review, and preservation
- never use Markdown edits as a substitute for runtime truth writes

## 10. Phase Rule

This document reflects the `V0` post-consolidation phase:

- do not reopen broad feature expansion
- keep sharpening owner root, cadence objects, anti-Zeno visibility, and projection boundary
- keep all surfaces converging toward one DB-first runtime law
