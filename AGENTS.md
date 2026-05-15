# Entrance Workspace

This is the Entrance project workspace root, not the product source root.

## Directory Contract

- `entrance-src/`: product source code. Run source inspection, builds, tests,
  commits, and product Git operations here unless the user says otherwise.
- `entrance-wiki/`: committed Markdown project knowledge and current-state
  documentation. Use it for orientation, then verify implementation claims
  against `entrance-src/`.
- `entrance-auto/`: automation workflows, fixtures, templates, report output,
  screenshots, traces, logs, and release artifacts.

## Default Behavior

- Stay at this workspace root for overview, planning, and project-management
  discussion.
- Switch to `entrance-src/` before implementation, code review, build, unit
  test, frontend validation, or product source inspection.
- Write reusable validation and release workflows under `entrance-auto/workflows/`.
- Write run reports, screenshots, traces, logs, downloads, and generated release
  artifacts under ignored paths in `entrance-auto/`.
- Keep durable project knowledge as Markdown under `entrance-wiki/`.
- Do not create product code, build outputs, or one-off test artifacts directly
  in this workspace root.

## Source Validation

Run product validation from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build
```
