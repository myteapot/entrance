# Entrance Current Architecture

Entrance V2 is a compact Rust workspace with one binary and three plugin crates.

## Active Source Shape

- `entrance-src/core/`: shared runtime primitives, boot, config, store, bus,
  versioning, filesystem, crypto, scheduler, and supervision.
- `entrance-src/plugins/drawer/`: durable notes, memory imports, vault records,
  organization plans, and drawer snapshots.
- `entrance-src/plugins/hive/`: local dispatch ledger, engine reports,
  callbacks, and review state.
- `entrance-src/plugins/launcher/`: local app indexing, search, pinning, and
  launch dispatch.
- `entrance-src/shell/app/`: the only Rust binary, exposing CLI and daemon
  transports.
- `entrance-src/shell/gui/`: Electron + SolidJS frontend that invokes
  `entrance daemon`.

## Deleted Shape

Do not reintroduce `harness/`, `shell/cli/`, `shell/mcp/`,
`hosts/desktop/tauri/`, or Tauri product code. Historical documents may mention
those paths, but they are archive-only context.

## Runtime Boundary

External callers should use `entrance daemon` over stdio or
`entrance daemon http` over loopback HTTP. There is no standalone MCP server in
the active V2 shape.
