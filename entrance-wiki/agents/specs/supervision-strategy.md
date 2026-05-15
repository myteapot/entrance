# Entrance Supervision Strategy

> Owner: NOTA
> Status: Proposed
> Scope: OS core, Forge runtime, Connector runtime, failure visibility, retry semantics

## 1. Why This Spec Exists

Entrance has already accepted two strong ideas:

- cold/hot dual-track is the first guiding principle
- Entrance is a Next OS composed of OS core, hierarchical state machine, and compiler

If both are true, then failure handling cannot remain an ad hoc plugin detail.

It must become a first-class runtime contract.

The design source here is Erlang/OTP supervision thinking, adapted for Entrance rather than copied mechanically.

## 2. One-Line Principle

The supervision slogan for Entrance is:

`max_retry + report + no_silent_failure`

Meaning:

- retries are bounded
- every failure is surfaced
- no runtime is allowed to fail quietly and pretend nothing happened

## 3. Relation To Cold/Hot Dual-Track

Supervision itself must live on both tracks.

### 3.1 Cold layer owns the contract

Cold artifacts define:

- failure domain boundaries
- child type
- supervision strategy
- retry budget
- retry window
- escalation threshold
- owner role

This is canonical truth.

### 3.2 Hot layer owns the live operational surface

Hot artifacts show:

- current runtime child state
- retry count
- last error
- last restart timestamp
- degraded / blocked / escalated state
- active incident visibility

This is the active operational surface.

### 3.3 Write order

If one action lands both:

- supervision contract changes
- hot operational surface changes

then the write order must remain:

`cold -> hot`

## 4. OTP-Derived Principles

### 4.1 Failure domains must be explicit

A child should fail inside a known boundary.

Examples:

- one agent process
- one dispatch pipeline
- one connector session bundle
- one stage execution lane

### 4.2 Supervisors own restart decisions

Workers do work.
Supervisors decide restart, block, and escalate.

Children must not silently self-authorize topology changes after failure.

### 4.3 Restart is not success

A restart is an observed recovery event, not proof that the system was healthy.

Every restart should remain visible in runtime history.

### 4.4 Escalation is part of supervision

If retry budget is exhausted, the system must stop pretending the failure is local.

It must escalate upward.

## 5. Strategy Set

Entrance should adopt the OTP-derived strategy vocabulary below.

### 5.1 `one_for_one`

Use when one child can fail independently.

If one child fails:

- restart only that child

Typical Entrance use:

- one isolated Forge task
- one isolated agent process
- one isolated connector adapter worker

### 5.2 `rest_for_one`

Use when children form an ordered dependency chain.

If one upstream child fails:

- restart that child
- restart later children that depend on it

Typical Entrance use:

- dispatch pipeline
- staged runtime pipeline with downstream assumptions

### 5.3 `one_for_all`

Use when children are tightly coupled and must remain coherent as a bundle.

If one child fails:

- restart the whole bundle

Typical Entrance use:

- tightly coupled session bundle
- connector bridge bundle sharing one stateful session contract

## 6. Child Restart Policy

Each supervised child should also have a restart class:

- `permanent`
  always restart according to strategy
- `transient`
  restart only on abnormal exit / failure
- `temporary`
  never restart automatically

Recommended default for Forge agent execution:

- strategy: `one_for_one`
- restart class: `transient`

## 7. Retry Budget

Every supervised child or bundle should declare:

- `max_restarts`
- `window`

Examples:

- `3 restarts / 5 minutes`
- `1 restart / 1 minute`

Once the budget is exhausted:

- no hidden loops
- no silent reset to Running
- child or bundle moves to `Blocked` or `Failed`
- escalation becomes mandatory

## 8. Failure Visibility Contract

Failure visibility is mandatory.

At minimum, a failure event must:

1. update runtime status
2. append a system log
3. emit a visible event

If the failure crosses escalation threshold, it must also:

4. surface an escalation state

This applies to:

- spawn failures
- wait failures
- blocked credential failures
- repeated restart exhaustion
- connector runtime failures

## 9. Runtime State Vocabulary

Recommended hot runtime states:

- `Pending`
- `Running`
- `Retrying`
- `Degraded`
- `Blocked`
- `Failed`
- `Cancelled`
- `Done`

`Retrying` and `Degraded` are important because they make recovery attempts visible instead of flattening everything into Running/Failed only.

## 10. Entrance Role Mapping

### 10.1 NOTA

Owns:

- top-level visibility
- escalation surfacing
- continuity of incidents across surfaces

### 10.2 Arch

Owns:

- failure-domain design
- supervision topology
- strategy selection in specs

### 10.3 Dev

Owns:

- runtime enforcement
- failure visibility checks
- repair and integration after supervised failure

### 10.4 Agent

Owns:

- local execution only

Agent does not define supervision strategy.

## 11. Initial Entrance Mapping

### 11.1 Forge agent process

Recommended:

- strategy: `one_for_one`
- restart class: `transient`
- bounded retry budget
- always visible failure logs

### 11.2 Dispatch pipeline

Recommended:

- strategy: `rest_for_one`

Reason:

- prompt / environment / downstream execution often inherit assumptions from earlier setup steps

### 11.3 Connector session bundle

Recommended:

- strategy: `one_for_all`

Reason:

- stateful session bundles usually need coherence more than partial survival

## 12. Minimal Implementation Rule

The first implementation slice does not need full distributed supervision trees.

But it must at least guarantee:

- no silent failure
- visible blocked state
- visible failed state
- bounded retry design in the core model
- reusable supervision policy types in OS core

## 13. One-Line Conclusion

Entrance should adopt an OTP-derived supervision model in which cold files define supervision contracts, hot surfaces expose live failure state, retries remain bounded, and failures are never allowed to disappear silently.
