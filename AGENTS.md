# Entrance — Agent Context

> **Last updated**: 2026-04-05
> **Branch**: `main`, HEAD: `b5c9ee6`
> **Stack**: Tauri 2 + Rust backend + SolidJS frontend

## What is Entrance

Agent OS — 面向智能体的操作系统。五层种姓制：`Human → NOTA → Arch → Dev → Agent`

核心 = 编译器管线：`ActionRecord → compile() → TypedActionPacket → lower_dispatch() → admit_dispatch() → resolve_return_route()`

## Key Paths

| Subsystem | Path |
|-----------|------|
| Compiler pipeline | `src-tauri/src/core/compiler/` |
| NOTA runtime | `src-tauri/src/core/nota/mod.rs` (~8000 LOC) |
| Data layer | `src-tauri/src/core/data_store.rs` (SQLite, ~7500 LOC) |
| Supervision | `src-tauri/src/core/supervision.rs` |
| Forge engine | `src-tauri/src/plugins/forge/` |
| Frontend pages | `src/pages/` (Chat, Forge, Dashboard, Console, Settings) |
| Design tokens | `src/styles/theme.css` |
| App shell CSS | `src/App.css` |
| Graph component | `src/components/ComputeGraph.tsx` |
| Graph engine | `src/features/dashboard/graphEngine.ts` |
| Graph store | `src/features/dashboard/graphStore.ts` |

## Design Language: Carbon

- **Zero gradient, zero glow, zero shadowBlur** — flat surfaces only
- **Color palette**: grey-in-black + muted accents (desaturated)
  - nota: `#7c83c9`, active: `#5a9e82`, steady: `#7a8a5e`, warming: `#9e8a4a`, caution: `#9e5a5a`
- **Sidebar**: 260px, 0.95rem font, text-only nav with hotkey pills
- **ComputeGraph**: dual mode — ⊤ Tree (default, static hierarchy) / ◎ Force (d3 physics)
- **Cards**: flush-left, no nesting indent

## Dev Environment

```bash
# Frontend (browser dev, uses mock IPC bridge)
pnpm dev                    # → http://localhost:1420

# Backend
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib  # 244 tests

# Full app (Tauri desktop)
pnpm tauri dev
```

## Build

```bash
pnpm build                  # TypeScript check + Vite production build
pnpm tauri build            # Full desktop app bundle
```

## Coding Rules

1. `cargo check` → `cargo test --lib` → commit → push
2. All test functions: `let _guard = crate::test_env_guard();`
3. Branch: `feat/<id>-<slug>`, squash merge to main
4. **No PowerShell file writes** — use Python or built-in editor tools
5. **Parallel agents must use `git worktree`** — never `git checkout -b` in shared worktree

## Current Status

All V1 backend tasks complete (2A/2B/2C/2D). All GUI tasks complete (G1/G2/G3).
Carbon visual migration complete. Next: v1.0.0 release prep.

## Deep Context

For full project history, roadmap, and architectural decisions, see:
- Obsidian: `大吕/11-entrance/ENTRANCE HANDOFF.md`
- Obsidian: `大吕/11-entrance/ENTRANCE ROADMAP.md`
