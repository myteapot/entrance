# Entrance — Agent Context

> **Last updated**: 2026-04-20
> **Workspace**: Rust workspace + Electron + SolidJS

## What is Entrance

Agent OS — 面向智能体的操作系统。当前仓库已经切到 V2 微内核架构：

- `entrance` → 唯一 Rust binary
- `shell/gui` → Electron + SolidJS 前端
- `core` → 微内核能力层
- `plugins/{drawer,hive,launcher}` → 插件层

运行契约已经收敛为：
- `entrance status`
- `entrance drawer ...`
- `entrance hive ...`
- `entrance launcher ...`
- `entrance daemon`
- `entrance mcp stdio`
- `entrance mcp http`

## Key Paths

| Subsystem | Path |
|-----------|------|
| Boot + paths | `core/src/boot.rs` |
| Config | `core/src/config.rs` |
| Data layer | `core/src/store.rs` |
| Bus | `core/src/bus.rs` |
| Supervision | `core/src/supervision.rs` |
| Plugin API | `core/src/plugin_api.rs` |
| Drawer plugin | `plugins/drawer/src/lib.rs` |
| Hive plugin | `plugins/hive/src/lib.rs` |
| Launcher plugin | `plugins/launcher/src/lib.rs` |
| Unified app binary | `shell/app/src/main.rs` |
| Daemon + MCP transport | `shell/app/src/daemon.rs` |
| Frontend renderer | `shell/gui/renderer/` |
| Frontend app | `shell/gui/renderer/App.tsx` |
| Navigation | `shell/gui/renderer/components/Nav.tsx` |
| Theme tokens | `shell/gui/renderer/styles/theme.css` |
| App shell CSS | `shell/gui/renderer/styles/app.css` |
| Electron shell | `shell/gui/electron/` |

## Dev Environment

```bash
# Frontend
pnpm dev
pnpm dev:electron

# Workspace validation
cargo check --workspace
cargo test --workspace

# Main binary
cargo run -p entrance-app --bin entrance -- --help
cargo run -p entrance-app --bin entrance -- status
cargo run -p entrance-app --bin entrance -- daemon
cargo run -p entrance-app --bin entrance -- mcp stdio
```

## Build

```bash
pnpm build
cargo build --workspace --release
```

## Coding Rules

1. `cargo check --workspace` → `cargo test --workspace` → commit → push
2. Test helpers that mutate shared env should use `crate::test_env_guard()`
3. Branch: `feat/<id>-<slug>`, squash merge to `main`
4. Do not reintroduce `harness/`, `shell/cli/`, `shell/mcp/`, or any Tauri product code
5. Plugin 之间不得互相依赖；共享行为必须进入 `core`

## Current Shape

- `core` owns微内核能力：store、bus、config、fs、crypto、scheduler、supervision、versioning。
- `plugins/drawer` 负责抽屉式存储与导入。
- `plugins/hive` 负责任务分发账本。
- `plugins/launcher` 负责本地启动项索引与搜索。
- `shell/app` 是唯一 Rust binary，同时暴露 CLI、daemon 与 MCP。
- `shell/gui` 是纯 Electron + SolidJS 前端，只通过 preload 调用 `entrance daemon`。
