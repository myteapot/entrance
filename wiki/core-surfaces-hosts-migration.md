# Entrance `core / surfaces / hosts` Migration

> Status: ready to start
> Decision date: 2026-04-08

## Decision

The repository will converge on three top-level domains:

```text
/core
/surfaces
/hosts
```

Meaning:

- `core`: product body and long-lived business truth
- `surfaces`: every human-facing or machine-facing surface
- `hosts`: the environments that carry surfaces and connect them to OS, desktop shells, packaging, and delivery

`publish` stays a delivery branch. It is not the source of release logic.

## Scope Boundaries

### `core`

Owns:

- runtime engines
- domain types and invariants
- command and event semantics
- service orchestration
- persistence and configuration infrastructure

Must not depend on Tauri, Electron, SolidJS, or CLI UX concerns.

### `surfaces`

Owns:

- GUI
- CLI
- MCP
- shared contracts between surfaces and hosts

May depend on `core`.

### `hosts`

Owns:

- Tauri
- Electron
- OS-specific behavior
- packaging, updater, signing, and release wiring

Adapts `core` and `surfaces`. Must not become a second home for business logic.

## Current To Target Mapping

### Current `src/`

Target: `surfaces/gui/`

Examples:

- `src/pages/*` -> `surfaces/gui/renderer/pages/*`
- `src/components/*` -> `surfaces/gui/renderer/components/*`
- `src/features/*` -> `surfaces/gui/renderer/features/*`
- `src/styles/*` -> `surfaces/gui/renderer/styles/*`

### Current direct Tauri imports in renderer

These must move behind the desktop contract before any large directory move:

- `src/App.tsx`
- `src/LauncherWindow.tsx`
- `src/components/NodeInspector.tsx`
- `src/components/NotaDialog.tsx`
- `src/features/dashboard/graphEvents.ts`
- `src/features/dashboard/summary.ts`
- `src/features/forge/taskFeed.ts`
- `src/features/issues/client.ts`
- `src/features/landing/client.ts`
- `src/features/nota/overview.ts`
- `src/features/vault/client.ts`
- `src/pages/Chat.tsx`
- `src/pages/Console.tsx`
- `src/pages/Dashboard.tsx`
- `src/pages/Forge.tsx`

### Current browser mocks

Target: host-owned browser scaffold later under `hosts/desktop/browser/` or equivalent.

Examples:

- `src/mocks/tauri-core.ts`
- `src/mocks/tauri-event.ts`
- `src/mocks/tauri-window.ts`
- plugin mock modules under `src/mocks/`

These are host implementations, not product truth.

### Current Rust runtime candidates

Targets: `core/runtime/`, `core/services/`, and `core/infra/`

Likely `core/runtime/`:

- `src-tauri/src/core/compiler/*`
- `src-tauri/src/core/nota/*`
- `src-tauri/src/core/event_bus.rs`
- `src-tauri/src/core/instance_manager.rs`
- `src-tauri/src/core/recovery.rs`
- `src-tauri/src/core/supervision.rs`
- `src-tauri/src/core/parallel_budget.rs`
- `src-tauri/src/core/system_heartbeat.rs`

Likely `core/services/`:

- `src-tauri/src/core/overview.rs`
- `src-tauri/src/core/landing.rs`
- parts of `src-tauri/src/plugins/forge/`
- parts of `src-tauri/src/plugins/launcher/`
- parts of `src-tauri/src/plugins/vault/`

Likely `core/infra/`:

- `src-tauri/src/core/data_store.rs`
- `src-tauri/src/core/config_store.rs`
- `src-tauri/src/core/logging.rs`
- `src-tauri/src/core/memory_import.rs`

### Current CLI and MCP code

Targets:

- `surfaces/cli/`
- `surfaces/mcp/`

Examples:

- `src-tauri/src/cli/compiler_cli.rs`
- `src-tauri/src/cli/forge_cli.rs`
- `src-tauri/src/cli/issues_cli.rs`
- `src-tauri/src/cli/memory_cli.rs`
- `src-tauri/src/cli/nota_cli.rs`
- `src-tauri/src/cli/mcp_cli.rs`
- `src-tauri/src/core/mcp_server.rs`

### Current Tauri host code

Target: `hosts/desktop/tauri/`

Examples:

- `src-tauri/src/lib.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/commands/*`
- `src-tauri/src/core/window.rs`
- `src-tauri/src/core/theme.rs`
- `src-tauri/src/core/updater.rs`
- `src-tauri/src/core/hotkey.rs`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/*`
- `src-tauri/icons/*`

### Existing Electron migration inputs on `dev-electron`

Treat these as migration inputs, not a second truth source:

- `src/platform/*`
- `electron/main.mjs`
- `electron/preload.mjs`
- `electron/dev.mjs`
- `src-tauri/src/cli/electron_bridge_cli.rs`

### Release and publish

- `hosts/release/` owns packaging, updater, signing, and CI release logic
- `publish` remains a Git branch for delivery snapshots and public-facing outputs
- `releases/` remains an output tree until later cleanup

## Migration Rules

1. Do not mix business-logic extraction and shell rewiring in the same MR when avoidable.
2. Do not combine large directory renames with protocol changes in one MR.
3. Renderer code must stop importing `@tauri-apps/*` directly before shell swaps.
4. Tauri and Electron must both talk through the same desktop contract.
5. `publish` is not the source of release logic.
6. Browser mocks are host implementations, not fake product truth.
7. `main` remains the only long-lived audited trunk.
8. `dev-electron` remains a migration and integration line until Electron support is absorbed into `main`.

## Migration Order

### Phase 0: Prep and freeze

Deliverables:

- naming frozen as `core / surfaces / hosts`
- this wiki doc as the current migration truth source
- explicit boundary for `publish`

### Phase 1: Introduce the desktop contract in place

Goal:

- make the renderer host-agnostic before any large move

Actions:

- bring the `dev-electron` `src/platform/*` bridge into `main`
- replace direct `@tauri-apps/*` imports in renderer with that bridge
- keep files under current `src/` for now
- adjust `vite.config.ts` so host selection is not a Tauri-only special case

Done when:

- GUI uses the desktop contract
- browser mode uses a browser host implementation
- Tauri still works

### Phase 2: Land Electron as a first-class host

Goal:

- make Electron another host over the same contract and runtime

Actions:

- land `electron/main.mjs`, `electron/preload.mjs`, and `electron/dev.mjs`
- wire the Rust stdio bridge into the shared command and event contract

Done when:

- one renderer contract
- two working desktop hosts
- no runtime-backed behavior in Electron falls back to mocks

### Phase 3: Split Rust runtime from Tauri bootstrapping

Goal:

- extract product truth from Tauri-specific assembly

Actions:

- move runtime logic out of Tauri-labeled modules into `core/*`
- reduce `src-tauri/src/lib.rs` to app assembly
- treat `src-tauri/src/commands/*` as Tauri bridge code

### Phase 4: Move surfaces into final homes

Goal:

- align the physical tree after behavior is already separated

Actions:

- move GUI from `src/` to `surfaces/gui/`
- move CLI from `src-tauri/src/cli/` to `surfaces/cli/`
- move MCP-facing code to `surfaces/mcp/`
- move shared contract modules to `surfaces/contracts/`

### Phase 5: Normalize release and publish concerns

Goal:

- keep release logic in source while `publish` remains a delivery branch

Actions:

- move packaging, updater, signing, and CI release assets under `hosts/release/`
- keep generated outputs and public delivery concerns separate

## First Real Implementation Batch

The first real migration batch should be desktop-contract extraction on `main`:

- import the `src/platform/*` bridge approach from `dev-electron`
- switch renderer call sites to that contract
- keep paths stable during that batch

This is the first move that should touch behavior.
