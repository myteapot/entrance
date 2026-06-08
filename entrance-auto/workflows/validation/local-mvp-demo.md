# Local MVP Demo Validation

## Purpose

Validate the current minimum usable Entrance control-plane unit from a clean
local app root:

- A local `Explorer -> Developer -> Reviewer` loop reaches `Done` with reviewer
  decision `keep`.
- The loop records all three local worker receipts without missing receipts.
- The `remote-fixture:` external issue/status/comment dry-run reaches a current
  connector state after publish, readback, admission, and final readback.
- A machine-readable report and a short Markdown summary are written to ignored
  report paths.

## Command

From the workspace root:

```bash
entrance-auto/workflows/validation/run-local-mvp-demo.sh
```

For a stricter release-style run:

```bash
entrance-auto/workflows/validation/run-local-mvp-demo.sh --full-gates
```

## Outputs

By default the script writes:

- App data and raw command outputs under
  `entrance-auto/tmp/local-mvp-demo-<run-id>/`.
- JSON and Markdown summaries under `entrance-auto/reports/`.

The committed workflow is reusable. The generated reports, database, connector
mirrors, and logs are run artifacts and should stay ignored unless a human
explicitly asks to publish a specific artifact.

## Panel Handoff

The generated report includes the daemon and Vite commands needed to inspect the
same app root in the local Panel. Browser screenshots are intentionally kept as
separate ignored artifacts so the CLI workflow remains deterministic and can run
without a browser session.
