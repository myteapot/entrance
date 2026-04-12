# 2.2 Lead Model (3) Design

> Status: mounted hot detail (transitional)

## Purpose

- define the control-responsibility model that sits above execution details
- define which ownership boundaries belong to role semantics rather than to tooling accidents
- relate the future lead model to the currently active working role set

## Mounted Root

- [control.md](./control.md)

## Confirmed Oracle Points

- This document preserves the internal control-slot cut beneath [control.md](./control.md): slot meaning, authority separation, and the `Human / NOTA` boundary relation.
- If retained, the `3` should be read as `Policy / Operation / Execution` rather than `Leader / Manager / Agent`.
- `Arch / Dev / Agent` remain the SWE projection of `Policy / Operation / Execution`, not the OS primitive itself.
- `NOTA` sits above the internal three-slot model as the normal Human-facing semantic entry/exit surface and Human-facing cadence host.
- `NOTA` is not `Policy`, and detailed project decomposition or issue splitting does not belong to `NOTA`'s top role.
- Final sovereignty belongs to `Human intent`, but `Policy` is the highest internal strategy writer rather than a mutable superuser.
- Authority separation should compile as single-writer ownership across slots rather than overlapping edit rights.
- Stronger slots may decide, replace, block, or escalate across slots only through routed objects and runtime-owned control paths rather than direct foreign-slot mutation.
- Project-internal lineage begins only after runtime admits NOTA-authored boundary output into internal scope, with `Policy` as the default internal ingress target at v0.
- Human-facing wake and continuity stay hosted by `NOTA`, while internal escalation still travels through governed packets, verdicts, and runtime routing.

## Current Boundary Reading

- `NOTA` remains the normal Human-facing control surface rather than collapsing into `Policy`
- mixed permission designs should be treated as boundary failures rather than convenience features
- slot interaction should happen through routed objects and runtime delivery, not by direct foreign-slot mutation

## Mounted Cold Docs

- [minimal_control_slot_model.md](../cold/2.2-lead-model-3/minimal_control_slot_model.md)
- [prd.md](../cold/2.2-lead-model-3/prd.md)

## Mounted Chore Docs

- [milestones.md](../chore/2.2-lead-model-3/milestones.md)

## TODO(fill)

- keep the mounted summary aligned with the cold control-slot model
- treat the mounted cold PRD as historical reference only unless a specific lineage question requires it
