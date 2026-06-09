# Play-Fix Loop Record

Date: 2026-06-09

Goal: simulate a real user operating Entrance in auto mode with a "火车票购票系统" issue, then fix the bugs found in short committed rounds.

Method set:
- Play through the Panel/CLI as a user would: create issue, advance it, inspect control/dashboard/timeline surfaces.
- Prefer one concrete defect per round.
- Validate with targeted tests plus a fresh isolated app root.
- Commit every completed round.

Acceptance standard:
- The issue can be advanced and observed through the local-only workbench without audit drift.
- The UI/control surfaces expose trustworthy next state and evidence.
- Each fix keeps existing public command/tool/resource names stable.

## Round 1: Auto Advance Audit Drift

Play root: `/Users/mac/Documents/GitHub/entrance/entrance-auto/tmp/playfix-r1-train-ticket`

Observed:
- A new "火车票购票系统" issue advanced to `Done`.
- `issue control` still reported dashboard health as `audit_failed`.
- The failed detail was `issue_surface:comment:comment.payload.schema_version`.
- Root cause: `advance_issue` wrote typed comments with schema `entrance.hive.auto_advance.v1`, but the issue surface audit only allowed the older operator/system comment schemas.

Fix:
- Exported the auto-advance schema constant inside the hive crate.
- Taught issue surface audit to accept `source=kernel` comments with schema `entrance.hive.auto_advance.v1`.
- Added shape checks for the embedded `advance_step` and its linked `auto_advance` evidence.
- Added a regression assertion to `advance_one_step_records_step_and_stops` so auto-advance comments must pass issue surface audit.

Validation:
- `cargo build -q -p entrance-app --bin entrance`
- `cargo test -q -p entrance-hive advance_one_step_records_step_and_stops`
- `cargo test -q -p entrance-hive issue_surface_audit_rejects_untyped_comments`
- Fresh playthrough: create "火车票购票系统", run `issue advance --until-wait --runtime local`, then `issue control`.
- Result: issue status `Done`, dashboard health `ok`, failed checks `[]`, audit details `[]`.
- `git diff --check`

Commit: this round commit, `Fix auto advance issue surface audit`

## Round 2: Panel Space And Wrapping

Play root: `/Users/mac/Documents/GitHub/entrance/entrance-auto/tmp/playfix-r2-train-ticket`

Observed:
- A fresh "火车票购票系统" issue advanced to `Done`.
- Computer Use read the real Electron status page at `127.0.0.1:1420`.
- Long `App root` and database paths now need to stay inside narrow status cards instead of overflowing into adjacent cards.
- Earlier Panel play showed the board felt cropped at the default Electron width because the board used six fixed-width status columns inside a clipped board panel.
- Computer Use could read the Electron accessibility tree, but its click action repeatedly rejected the active session; Panel screenshot validation is therefore limited in this round.

Fix:
- Let status/detail metric text break anywhere inside its card.
- Changed board columns from six fixed `220px` columns to responsive `auto-fit` columns with a smaller stable minimum width.
- Removed hidden overflow from the board panel so content is not clipped by the outer panel.
- Added wrapping constraints for record titles, action buttons, doctor strip text, and trace pills.

Validation:
- `pnpm check`
- `pnpm build`
- Fresh playthrough data: create "火车票购票系统", run `issue advance --until-wait --runtime local`; result `Done`, stop reason `done`.
- Computer Use status-page read: real Electron window loaded the temporary app root and displayed wrapped long paths inside cards.
- `git diff --check`

Residual risk:
- Need one more UI round with a working click/DOM driver to screenshot the Panel board itself after navigation.

Commit: this round commit, `Improve panel board responsive layout`
