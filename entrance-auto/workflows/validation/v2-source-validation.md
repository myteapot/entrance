# V2 Source Validation

## Purpose

Verify the active Entrance V2 source tree after formatting, refactors, or
source-level fixes.

## Source Root

`entrance-src/`

## Prerequisites

- Rust toolchain available on `PATH`
- Node.js and pnpm available on `PATH`
- Dependencies installed with `pnpm install --frozen-lockfile`

## Steps

1. Change into the source root:

   ```bash
   cd entrance-src
   ```

2. Run Rust workspace validation:

   ```bash
   cargo check --workspace
   cargo test --workspace
   ```

3. Run frontend type/build validation:

   ```bash
   pnpm check
   pnpm build
   ```

## Evidence Policy

- Commit this workflow Markdown.
- Do not commit screenshots, logs, traces, videos, generated release artifacts,
  or run-specific materials.
- Save run evidence under ignored `entrance-auto/reports/`,
  `entrance-auto/screenshots/`, `entrance-auto/traces/`, or
  `entrance-auto/logs/`.
