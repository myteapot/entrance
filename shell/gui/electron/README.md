# Electron Adapter

Electron 现在只作为 `shell/gui` 的一部分维护，renderer 直接通过 preload 调用统一的 `entrance daemon`。

## Current Shape

- Renderer 只依赖 `window.__ENTRANCE_ELECTRON__.invoke()`.
- Electron main 启动统一 Rust binary：`entrance daemon`.
- GUI、CLI、MCP 都收敛到 `shell/app` 提供的一个入口。

## Dev Flow

1. `pnpm install`
2. `pnpm dev:electron`
3. 脚本会启动 `shell/gui/vite.config.ts` 指向的 Vite 服务，再以仓库根目录作为 Electron app 入口拉起桌面进程
4. Electron 优先复用 `target/debug/entrance`，否则回退到 `cargo run -p entrance-app --bin entrance -- daemon`

## Release Flow

1. `pnpm build:electron:rpm` 或 `pnpm build:electron:dir`
2. 脚本会构建前端资源、编译 `entrance`，然后把 `shell/gui/electron`、`shell/gui/dist`、`shell/gui/icons` 一起打包
3. 输出写入 `dist-electron/`
