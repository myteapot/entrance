# Entrance Current Architecture

Entrance V2 is a compact Rust workspace with one binary and three plugin crates.

## Active Source Shape

- `entrance-src/core/`: shared runtime primitives, boot, config, store, bus, versioning, filesystem, crypto, scheduler, and supervision.
- `entrance-src/plugins/drawer/`: durable notes, memory imports, vault records, organization plans, and drawer snapshots.
- `entrance-src/plugins/hive/`: local issue/status/comment ledger, loop contracts, worker receipts, reviewer verdicts, audit state, and issue control views. Hive now has thin module boundaries for model, kernel, policy, runner, worker, evidence, audit, timeline, and view.
- `entrance-src/plugins/launcher/`: local app indexing, search, pinning, and launch dispatch.
- `entrance-src/shell/app/`: the only Rust binary, exposing CLI, daemon transports, and the MCP stdio issue workbench.
- `entrance-src/shell/gui/`: Electron + SolidJS frontend that invokes `entrance daemon` and shows the local Linear-like issue board/detail surface.

## Deleted Shape

Do not reintroduce `harness/`, `shell/cli/`, `shell/mcp/`, `hosts/desktop/tauri/`, or Tauri product code. Historical documents may mention those paths, but they are archive-only context.

## Runtime Boundary

External GUI and automation callers should use `entrance daemon` over stdio or `entrance daemon http` over loopback HTTP. MCP clients can use `entrance mcp stdio`, implemented inside `shell/app`, which exposes local Hive issue tools/resources/prompts only.

The current control plane is local and issue-first:

- issue list/show/create/claim/comment/run/review/retry/decide/control;
- loop create/run/control;
- review queue for `Blocked` and `Needs Review`;
- policy resources for issue transitions, MCP permissions, and actor identity;
- Panel actions for local create/run/retry/review/cancel/comment.

Retry, request-review, and cancel remain human-confirmed decision boundaries. MCP and Panel confirmations are recorded as local audit context with self-reported, non-verified actor identity.

Remote synchronization, publish/readback/roundtrip, file mirrors, and external issue-surface demos are no longer part of the active architecture or user-facing runtime boundary.
