# Entrance Runtime Contract

Last updated: 2026-06-09

## Local Issue Workbench

Entrance currently exposes a local issue/status/comment runtime. The primary surfaces are:

```bash
entrance hive issue create --title <text> --goal <text> [--runtime local|codex] [--compact]
entrance hive issue list [--compact]
entrance hive issue show <id> [--compact]
entrance hive issue claim <id> --agent <name> [--role developer|reviewer] [--compact]
entrance hive issue comment <id> --body <text> [--author <name>] [--compact]
entrance hive issue run <id> [--runtime local|codex] [--compact]
entrance hive issue review <id> --decision keep|reject|blocked [--body <text>] [--compact]
entrance hive issue retry <id> --human-confirmed [--body <text>] [--compact]
entrance hive issue decide <id> retry|request-review|cancel --human-confirmed [--body <text>] [--compact]
entrance hive issue control <id>
entrance hive review-queue
entrance hive loop create --title <text> --goal <text> [--runtime local|codex] [--compact]
entrance hive loop run <id> [--runtime local|codex] [--compact]
entrance hive loop control <id>
```

## MCP

`entrance mcp stdio` exposes local tools only:

- `entrance_issue_create`
- `entrance_issue_list`
- `entrance_issue_show`
- `entrance_issue_claim`
- `entrance_issue_comment`
- `entrance_issue_run`
- `entrance_issue_review`
- `entrance_issue_retry`
- `entrance_issue_decide`
- `entrance_issue_control`
- `entrance_loop_create`
- `entrance_loop_control`
- `entrance_review_queue`

Resources include `entrance://issues`, `entrance://issues/{issue_id}`, `entrance://issues/{issue_id}/control`, `entrance://loops/{loop_id}/control`, `entrance://review-queue`, `entrance://policy/issue-transitions`, `entrance://policy/mcp-permissions`, and `entrance://policy/actor-identity`.

## Human Boundary

Retry, request-review, and cancel require explicit human confirmation through CLI, Panel, or MCP. Developer runs implementation. Reviewer records verdict context and should not implement. Rejected or blocked outcomes require human retry before another automatic run.

## Removed Active Surface

External synchronization surfaces are not part of the current runtime contract. Do not rely on issue mirror, publish, readback, roundtrip, or remote fixture commands/resources/prompts in active workflows.
