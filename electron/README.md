# Electron Adapter

This branch keeps the renderer DB-first and frontend-compatible while backing Electron with the real Rust runtime over stdio.

## Current Shape

- Renderer code now talks to `src/platform/*` bridge modules instead of importing Tauri APIs directly.
- Electron preload exposes dialogs, relaunch, window lifecycle, `invoke`, and top-level `listen`.
- Electron main spawns `entrance electron-bridge stdio`, forwards invoke calls to Rust, and relays backend events back into renderer channels.
- Launcher actions, Forge task operations, dashboard/system events, issue CRUD, NOTA overview/status, and Vault flows now run against the same Rust-owned runtime used by Tauri.

## Dev Flow

1. Run `pnpm install`.
2. Start the scaffold shell with `pnpm dev:electron`.
3. The script starts Vite on `http://127.0.0.1:1420` and then launches Electron with `electron/main.mjs`.
4. Electron starts a Rust sidecar from `src-tauri/target/debug/entrance` when available and falls back to `cargo run --manifest-path src-tauri/Cargo.toml -- electron-bridge stdio`.
