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
  generating the demo report.
