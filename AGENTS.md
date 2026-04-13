# Entrance — Agent Context

> **Last updated**: 2026-04-13
> **Workspace**: Rust workspace + Tauri 2 + Electron + SolidJS

## What is Entrance

Agent OS — 面向智能体的操作系统。当前最终入口契约已经拆为：

- `entrance` → CLI
- `entrance-gui` → Tauri GUI
- `entrance-desktop-bridge` → Electron bridge sidecar
- `entrance-mcp` → MCP shell

核心编译管线仍是：
`ActionRecord → compile() → TypedActionPacket → lower_dispatch() → admit_dispatch() → resolve_return_route()`

## Key Paths

| Subsystem | Path |
|-----------|------|
| Compiler pipeline | `core/src/compiler/` |
| NOTA runtime | `core/src/nota/mod.rs` |
| Data layer | `core/src/data_store.rs` |
| Supervision | `core/src/supervision.rs` |
| Harness bootstrap | `harness/src/runtime.rs` |
| Config + path resolution | `harness/src/config.rs`, `harness/src/runtime.rs` |
| Projection export | `harness/src/projections.rs` |
| Forge engine | `harness/src/plugins/forge/` |
| Frontend pages | `shell/gui/renderer/pages/` |
| Design tokens | `shell/gui/renderer/styles/theme.css` |
| App shell CSS | `shell/gui/renderer/App.css` |
| Graph component | `shell/gui/renderer/components/ComputeGraph.tsx` |
| Graph engine | `shell/gui/renderer/features/dashboard/graphEngine.ts` |
| Graph store | `shell/gui/renderer/features/dashboard/graphStore.ts` |
| Electron shell | `shell/gui/electron/` |
| Tauri commands | `shell/gui/src/tauri_commands/` |
| MCP shell | `shell/mcp/src/` |

## Dev Environment

```bash
# Frontend (browser dev, uses mock desktop bridge)
pnpm dev                    # -> http://localhost:1420

# Workspace validation
cargo check --workspace
cargo test --workspace

# Main shells
cargo run -p entrance-cli --bin entrance -- --help
cargo run -p entrance-gui --bin entrance-gui
cargo run -p entrance-gui --bin entrance-desktop-bridge -- stdio
cargo run -p entrance-mcp --bin entrance-mcp -- stdio
```

## Build

```bash
pnpm build
pnpm tauri build
cargo build --workspace --release
```

## Coding Rules

1. `cargo check --workspace` → `cargo test --workspace` → commit → push
2. Test helpers that mutate shared env should use `crate::test_env_guard()`
3. Branch: `feat/<id>-<slug>`, squash merge to `main`
4. Do not reintroduce `hosts/desktop/tauri` or `surfaces/` product code
5. Shells must not depend on each other; shared behavior belongs in `core` or `harness`

## Current Shape

- `core` only owns runtime/domain logic, DTOs, schema, and services.
- `harness` owns config IO, path resolution, DB bootstrap, plugin wiring, and projection exports.
- `shell/cli`, `shell/gui`, and `shell/mcp` are independent shells over `core + harness`.
- Electron is maintained under `shell/gui` and talks to Rust through `entrance-desktop-bridge`.
