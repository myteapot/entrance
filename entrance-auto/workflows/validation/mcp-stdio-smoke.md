# MCP Stdio Smoke Validation

## Purpose

Validate the current MCP-native control-plane surface through the real
newline-delimited JSON-RPC stdio protocol, not only Rust unit tests.

The smoke starts `entrance mcp stdio` from a clean app root and behaves like a
minimal MCP client:

- negotiate `initialize` with `clientInfo`;
- list tools, prompts, resources, and resource templates;
- fetch the loop contract prompt;
- create an `Explorer -> Developer -> Reviewer` loop through
  `entrance_loop_create`;
- run the loop through `entrance_issue_run`;
- read `entrance_loop_control` through both `tools/call` and
  `resources/read`;
- fetch the loop review prompt with the loop control resource embedded;
- create a `remote-fixture:` issue through MCP;
- read connector queue/control and the connector decision prompt;
- verify connector A/B/C options and digest-bound roundtrip `plan_id`;
- verify connector roundtrip execution refuses missing `human_confirmed=true`;
- execute the confirmed connector roundtrip and verify the queue becomes
  current;
- verify `entrance://policy/mcp-permissions` and
  `entrance://policy/actor-identity`;
- verify `entrance_issue_retry` refuses execution without
  `human_confirmed=true`.

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

By default the script writes:

- App data under `entrance-auto/tmp/mcp-stdio-smoke-<run-id>/`.
- JSON and Markdown reports under `entrance-auto/reports/`.

The report records protocol version, required tool/prompt/template presence,
the created issue and loop ids, loop control schema, Reviewer decision, fallback
budget, score names, loop A/B/C operator options, connector A/B/C operator
options, roundtrip completion, confirmation receipt client identity, and both
issue retry and connector roundtrip human-confirmation refusal checks. Generated
reports and databases are run artifacts and should stay ignored unless a human
explicitly asks to publish a specific artifact.
