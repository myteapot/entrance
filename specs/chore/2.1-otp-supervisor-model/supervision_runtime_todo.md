# Supervision Runtime Todo

> Owner: NOTA
> Status: Todo
> Scope: turn OTP-derived supervision from docs/core types into active runtime behavior

## Goal

Make the current supervision design executable inside Entrance rather than only documented.

The current baseline already exists:

- cold/hot dual-track is codified
- OTP-derived supervision principles are documented
- core supervision types exist
- Forge failures are now visible instead of silently disappearing

The next step is runtime enforcement.

## Todo

1. Wire `src-tauri/src/core/supervision.rs` into Forge runtime instead of leaving it as detached core types.

2. Add bounded retry execution for Forge agent processes.

Current target:

- strategy: `one_for_one`
- restart class: `transient`
- retry budget: use the default agent-process policy first

3. Make retry attempts visible in runtime state rather than silently folding them into `Running`.

Target states:

- `Retrying`
- `Degraded`
- `Blocked`

4. Add retry counters and last-failure metadata to Forge runtime surfaces.

Minimum visible fields:

- retry count
- last error
- last restart time
- escalation flag or blocked reason

5. Prevent hidden infinite restart loops.

Acceptance:

- retry budget exhaustion stops restart attempts
- task moves to `Blocked` or `Failed`
- event + system log remain visible

6. Design dispatch-pipeline supervision as the second runtime slice.

Current intended strategy:

- `rest_for_one`

Reason:

- prompt generation, env binding, and downstream execution carry ordered assumptions

7. Leave Connector session-bundle supervision for the third slice.

Current intended strategy:

- `one_for_all`

8. Add tests that prove:

- retry budget is honored
- no silent failure occurs on exhausted retries
- runtime state exposes retry/degraded transitions
- policy defaults match intended Forge/dispatch/session mappings

## Constraints

- Do not introduce a fake supervision tree that looks complete but does nothing.
- Prefer minimal enforcement slices over giant architecture-only refactors.
- Keep Forge as the first proving ground.
- Do not erase failure history when recovery succeeds.

## Done Means

This todo is complete when:

- Forge runtime actually uses supervision policy
- retry is bounded
- failure visibility is guaranteed
- retry/degraded/blocked behavior can be observed in tests and UI/API state
