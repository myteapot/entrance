# Simulation Gate Todo

> Owner: NOTA
> Status: Todo
> Scope: mandatory simulation before upward handoff at every level

## Goal

Prevent avoidable upward interruptions.

No role should wake the next level just because it finished writing something.
It should first simulate, self-criticize, and locally repair what can be repaired.

The target rule is:

`simulation before handoff`

and

`if self-critique can solve it, do not disturb the upper level`

## Core Requirement

Every upward submission must include simulation evidence.

If simulation has not happened, the handoff is incomplete.

## Todo

1. Define a mandatory simulation gate for each role.

### Agent gate

Before reporting upward, Agent must:

- run bounded local verification
- self-criticize obvious mistakes
- repair locally if the issue is within its allowed room

Examples:

- compile/test/smoke
- prompt-scope sanity check
- obvious regression scan

### Dev gate

Before handing upward, Dev must:

- simulate integration behavior
- review the artifact as if it were being accepted
- fix locally if the problem is operational rather than strategic

Examples:

- integration smoke
- end-to-end path check
- merge/result review

### Arch gate

Before escalating upward, Arch must:

- simulate dependency topology
- simulate stage/parallelism implications
- run self-critique on whether the conflict is real or locally resolvable

Arch should not wake Human for contradictions that can be resolved by better decomposition or clearer contract writing.

### NOTA gate

Before waking Human, NOTA must:

- verify whether the issue can be resolved by lower-level re-loop
- avoid escalating because of missing local criticism

2. Require handoff payloads to include simulation evidence.

Minimum payload:

- what was simulated
- what passed
- what failed
- what was self-repaired locally
- what remains truly blocked

3. Reject upward handoff if simulation evidence is missing.

4. Add a local re-loop rule:

- if simulation finds a fixable issue inside the same role boundary, repair and rerun
- only escalate unresolved items that cross room/policy/spec boundaries

5. Make “avoidable Human wake-up” a tracked anti-pattern.

Examples:

- code written but not simulated
- obvious smoke failure pushed upward
- known local issue escalated without repair attempt

6. Add simulation checkpoints into future action/state design.

Possible directions for next discussion:

- add `simulate` as an explicit primitive
- or keep simulation as a required phase inside `make/review/update`

7. Add tests or policy checks later so runtime handoff cannot claim completion without simulation metadata.

## Constraints

- Simulation should be bounded, not endless perfectionism.
- The gate must improve throughput, not create ceremonial slowdown.
- The purpose is to reduce useless wake-ups and rework loops.

## Done Means

This todo is complete when:

- every role has a clear simulation gate
- upward handoff requires simulation evidence
- local self-repair is the default before escalation
- avoidable Human wake-ups are structurally reduced
