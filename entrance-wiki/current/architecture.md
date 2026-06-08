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
- `entrance-src/shell/app/`: the only Rust binary, exposing CLI, daemon
  transports, and the MCP stdio issue surface.
- `entrance-src/shell/gui/`: Electron + SolidJS frontend that invokes
  `entrance daemon`.

## Deleted Shape

Do not reintroduce `harness/`, `shell/cli/`, `shell/mcp/`,
`hosts/desktop/tauri/`, or Tauri product code. Historical documents may mention
those paths, but they are archive-only context.

## Runtime Boundary

External GUI and automation callers should use `entrance daemon` over stdio or
`entrance daemon http` over loopback HTTP. MCP clients can use
`entrance mcp stdio`, which is implemented inside the same `shell/app` binary
and exposes the local Hive issue/status/comment kernel as tools and resources.
There is still no separate `shell/mcp/` package or remote MCP service in the
active V2 shape.
