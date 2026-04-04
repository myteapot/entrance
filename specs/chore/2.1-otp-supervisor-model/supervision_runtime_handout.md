# Supervision Runtime Handout

> Purpose: new-window continuation packet for supervision work

## What Is Already Landed

Hot files now contain OTP-derived supervision guidance:

- `A:/.agents/nota/identity.md`
- `A:/.agents/nota/rules.md`
- `A:/.agents/duet/SKILL.md`
- `A:/.agents/duet/roles/arch.md`

Cold files now contain supervision guidance:

- `specs/supervision_strategy.md`
- `specs/backend.md`
- `oracles/oracle.md`

Core code now contains supervision types:

- `src-tauri/src/core/supervision.rs`

Forge failure visibility has been improved:

- `src-tauri/src/plugins/forge/engine.rs`

It now appends system logs for:

- blocked credential failures
- spawn failures
- wait failures
- non-zero exits
- manual cancellation

## What Is Not Yet Landed

The system still does **not** have a real active supervision runtime.

Specifically missing:

- bounded retry execution
- retry/degraded runtime states in Forge storage/UI
- policy-driven restart decisions
- dispatch-pipeline supervision
- Connector/session-bundle supervision

## Current Architecture Direction

Use this order:

1. Forge agent process
   Apply `one_for_one`

2. Dispatch pipeline
   Apply `rest_for_one`

3. Connector session bundle
   Apply `one_for_all`

## Non-Negotiable Principle

The slogan is:

`max_retry + report + no_silent_failure`

This means:

- retries are bounded
- every failure remains visible
- recovery never erases incident history

## Best Next Discussion In New Window

Discuss the first executable slice only:

- how Forge should store retry count / degraded / blocked / escalation state
- whether retry metadata belongs in existing task rows or a separate incident table
- what the minimum UI/API visibility needs to be for v1

## Recommended Execution Order

1. Define Forge runtime state shape
2. Enforce retry budget in engine
3. Expose retry/degraded state through API/UI
4. Add tests
5. Only then move to dispatch-pipeline supervision
