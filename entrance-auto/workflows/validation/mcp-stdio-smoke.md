# MCP Stdio Smoke Validation

## Purpose

Validate the current local MCP-native issue workbench through the real newline-delimited JSON-RPC stdio protocol, not only Rust unit tests.

The smoke target is now local only:

- negotiate `initialize` with `clientInfo`;
- list tools, prompts, resources, and resource templates;
- create an issue-bound loop through `entrance_loop_create` or `entrance_issue_create`;
- run the issue through `entrance_issue_run`;
- read `entrance_issue_control` and `entrance_loop_control` through tools/resources;
- verify `entrance_review_queue`;
- verify `entrance://policy/mcp-permissions` and `entrance://policy/actor-identity`;
- verify `entrance_issue_retry` refuses execution without `human_confirmed=true`.

Remote synchronization, external issue mirrors, publish/readback/roundtrip, and fixture demos are intentionally outside this validation target.

## Command

From the workspace root:

```bash
entrance-auto/workflows/validation/run-mcp-stdio-smoke.mjs
```

For a stricter release-style run:

```bash
entrance-auto/workflows/validation/run-mcp-stdio-smoke.mjs --full-gates
```

## Outputs

By default the script writes app data under `entrance-auto/tmp/mcp-stdio-smoke-<run-id>/` and reports under `entrance-auto/reports/`. Generated reports and databases are run artifacts and should stay ignored unless a human explicitly asks to publish a specific artifact.
