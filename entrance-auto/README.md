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
