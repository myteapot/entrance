# Entrance Release Notes

## V2 Microkernel Preview

- The active product source is under `entrance-src/`.
- The runtime is one Rust binary: `entrance`.
- CLI and Electron GUI both route through the same Rust runtime.
- The daemon protocol is exposed as `entrance daemon` and
  `entrance daemon http`.
- Historical V0/V1, Tauri, harness, and GitLab promotion documents are retained
  under `entrance-wiki/archive/` for context only.

## Validation

```bash
cd entrance-src
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build
```
