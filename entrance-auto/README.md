# Entrance Automation

This directory contains reusable workflows, templates, fixtures, and ignored run
artifacts for the Entrance workspace.

## Tracked By Default

- `workflows/**/*.md`
- reusable workflow scripts
- `templates/**`
- small deterministic fixtures required for reproduction

## Ignored By Default

- `artifacts/`
- `reports/`
- `results/`
- `screenshots/`
- `traces/`
- `videos/`
- `logs/`
- `downloads/`
- `fixtures/private/`

If a workflow needs an ignored file, document how to recreate or provide it in
the workflow Markdown.

## Validation Workflows

- `workflows/validation/run-local-mvp-demo.sh` runs the local
  `Explorer -> Developer -> Reviewer` MVP loop plus the `remote-fixture:`
  external issue/status/comment dry-run from a clean app root. Use
  `--full-gates` to include Rust, frontend, formatting, and diff checks before
  generating the demo report. Use `--verify-golden` to compare normalized
  output contracts with `fixtures/golden/local-mvp-demo/`, and
  `--update-golden` only when the intended contract changes.
- `workflows/validation/capture-panel-screenshot.mjs` reuses the local MVP demo
  data, starts the local daemon and Vite, captures the Panel Issue board through
  Electron, and writes ignored screenshot plus metadata artifacts.
