# 1.1 OS Design (Core)

> Status: mounted hot detail (transitional)

## Purpose

- define the OS/core boundary of Entrance
- define which limits are enforced structurally rather than conversationally
- serve as the top mount point for capability, routing, and ownership constraints

## Mounted Root

- [machine.md](./machine.md)

## Confirmed Oracle Points

- This document preserves the OS/core hard-boundary cut beneath [machine.md](./machine.md): capability limits, runtime authority, sandboxing, and single-writer ownership.
- Entrance keeps an OS/core layer rather than collapsing directly into harness or prompt orchestration.
- Hard capability boundaries should be enforced by runtime structure rather than prompt wording alone.
- `Runtime truth` remains independent from semantic role slots and owns mechanical facts such as receipts, rejects, supervision events, taint, and admin actions.
- Runtime adjudication should stay registry-first and code-first, with the minimal OS registry cut anchored at `object_kind_registry / state_code_registry / control_policy_registry`.
- `control_policy_registry` carries writer, route, gate, sandbox, admission, and projection subcodes rather than scattering enforcement across row-local prose.
- `Human` is the final sovereignty source but has no direct project-internal canonical write path; `NOTA` is the only normal semantic ingress/egress.
- Break-glass power is limited to `observe / pause / stop / quarantine / revoke / replace`, and remains a runtime control path rather than a semantic authoring path.
- Any out-of-band human prompt intervention taints the touched lineage and blocks automatic promotion by default.
- Sandboxed execution and bounded worktrees remain the default direction for lower execution roles.
- `NOTA` must not directly mutate project-level policy state or project-level issues.
- Canonical truth follows single-writer ownership: `model-authored` fields are writable only in owned scope, `runtime-derived` fields are computed, and `runtime-only` fields remain exclusive to runtime.
- Hardening should prefer reducing writable row surface and increasing runtime derivation rather than adding new semantic layers by default.

## Current Boundary Reading

- lower execution roles should run inside bounded rooms or worktrees
- authority separation is a single-writer problem, not a courtesy rule
- Human may stop or replace inner execution, but cannot semantically steer a live inner instance through hidden in-band writes
- when a fact can be derived from lineage topology or registry context, runtime should derive it rather than trusting a row-local declaration

## Mounted Cold Docs

- [minimal_os_boundary.md](../cold/1.1-os-core/minimal_os_boundary.md)

## Mounted Chore Docs

- [agents_decommission_plan.md](../chore/1.1-os-core/agents_decommission_plan.md)
- [gitlab_connector_auth.md](../chore/1.1-os-core/gitlab_connector_auth.md)
- [harness_bootstrap_import.md](../chore/1.1-os-core/harness_bootstrap_import.md)
- [troubleshooting.md](../chore/1.1-os-core/troubleshooting.md)

## TODO(fill)

- keep the mounted summary aligned with the cold OS boundary cut
- push any later runtime, harness, or platform-specific wrinkles downward rather than regrowing this hot mount
