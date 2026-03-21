# Simulation Gate Handout

> Purpose: new-window continuation packet for simulation-before-handoff work

## Problem Statement

The current architecture already separates roles, but it still needs a stronger upward-submission rule.

The missing rule is:

- no upward handoff without simulation
- no avoidable wake-up of the next level

In plain terms:

- Agent should not hand noisy unfinished work to Dev
- Dev should not hand operationally unverified work to Arch or Human
- Arch should not wake Human for issues that better local criticism could solve

## Intended Principle

`simulation before handoff`

and

`if self-critique can solve it, do not disturb the upper level`

## Why This Matters

Without this gate, the system loses efficiency in the worst possible way:

- Human gets awakened by avoidable issues
- upper levels act as smoke tests for lower levels
- rework loops increase
- role separation exists on paper but not in quality behavior

## Current Direction

The simulation gate should apply at every level.

### Agent

Must do local verification and self-repair before reporting upward.

### Dev

Must do integration-level simulation before upward submission.

### Arch

Must simulate dependency, topology, and conceptual consequences before escalating upward.

### NOTA

Must avoid waking Human when the issue should first be sent back into lower-level loops.

## Best Next Discussion In New Window

Discuss these two design questions first:

1. Should `simulate` become an explicit action primitive?

2. What is the minimum simulation evidence payload that must accompany every upward handoff?

## Recommended Starting Shape

For v1, do not overcomplicate it.

Start with:

- simulation as a required gate, even if not yet a separate primitive
- evidence attached to upward handoff
- rejection of handoff if simulation evidence is absent

## Suggested Follow-Up Execution Order

1. Define the evidence schema
2. Define per-role simulation minimums
3. Decide whether simulation is a primitive or a phase
4. Thread the rule into Forge/Harness handoff flow
