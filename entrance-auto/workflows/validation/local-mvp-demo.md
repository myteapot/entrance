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

To compare normalized output contracts with the committed golden fixtures:

```bash
entrance-auto/workflows/validation/run-local-mvp-demo.sh --verify-golden
```

When the intended contract changes, update the fixtures explicitly:

```bash
entrance-auto/workflows/validation/run-local-mvp-demo.sh --update-golden
```

## Outputs

By default the script writes:

- App data and raw command outputs under
  `entrance-auto/tmp/local-mvp-demo-<run-id>/`.
- JSON and Markdown summaries under `entrance-auto/reports/`.
- Normalized run snapshots under
  `entrance-auto/tmp/local-mvp-demo-<run-id>/normalized/`.

The tracked golden fixtures live under
`entrance-auto/fixtures/golden/local-mvp-demo/`. They intentionally preserve
stable contract fields only, such as role/stage status, reviewer decision,
connector readiness, issue board status, and action labels. Run-specific
timestamps, paths, ids from external services, hashes, and raw logs stay out of
the committed fixtures.

The committed workflow is reusable. The generated reports, database, connector
mirrors, and logs are run artifacts and should stay ignored unless a human
explicitly asks to publish a specific artifact.

## Panel Handoff

The generated report includes the daemon and Vite commands needed to inspect the
same app root in the local Panel. Browser screenshots are intentionally kept as
separate ignored artifacts so the CLI workflow remains deterministic and can run
without a browser session.

To capture the Panel Issue board with the same local MVP data:

```bash
entrance-auto/workflows/validation/capture-panel-screenshot.mjs
```

For a release-style screenshot run that also verifies the source gates and
golden CLI contracts:

```bash
entrance-auto/workflows/validation/capture-panel-screenshot.mjs --full-gates
```

The screenshot workflow writes PNG files under `entrance-auto/screenshots/` and
metadata/summary reports under `entrance-auto/reports/`. It validates that the
Panel exposes the local MVP issue, the `remote-fixture:` issue, connector queue,
`Run Fixture` actions, `Todo`/`Done` columns, and reviewer keep evidence.
