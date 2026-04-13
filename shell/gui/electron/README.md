# Electron Adapter

Electron 现在作为 `shell/gui` 的一部分维护，renderer 通过 `shell/gui/contracts/desktop/*` 与 Rust sidecar 通信，不再经过旧的桌面桥接 CLI 契约。

## Current Shape

- Renderer 使用 `shell/gui/contracts/desktop/*` 桥接模块，而不是直接导入 Tauri API。
- Electron preload 暴露 dialogs、window lifecycle、`invoke`、`listen` 等桌面能力。
- Electron main 启动独立 sidecar：`entrance-desktop-bridge stdio`。
- GUI、Electron、CLI、MCP 都读取同一套 `core + harness` 运行时状态。

## Dev Flow

1. `pnpm install`
2. `pnpm dev:electron`
3. 脚本会启动 `shell/gui/vite.config.ts` 指向的 Vite 服务，再以仓库根目录作为 Electron app 入口拉起桌面进程
4. Electron 优先复用 `target/debug/entrance-desktop-bridge`，否则回退到 `cargo run -p entrance-gui --bin entrance-desktop-bridge -- stdio`

## Release Flow

1. `pnpm build:electron:rpm` 或 `pnpm build:electron:dir`
2. 脚本会构建前端资源、编译 `entrance-desktop-bridge`，然后把 `shell/gui/electron`、`shell/gui/dist`、`shell/gui/icons` 一起打包
3. 输出写入 `dist-electron/`
