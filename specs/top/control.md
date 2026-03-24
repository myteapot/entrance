# Control

> Status: hot root

## Purpose

- hold the compressed hot-root summary for authority, control topology, and hot-surface compression
- separate semantic control structure from execution queues and UI density choices

## Confirmed Oracle Points

- `NOTA` is the Human-facing boundary host and normal semantic ingress/egress.
- `NOTA` is also the only global continuation authority at `v0`.
- `Policy` is the highest internal strategy slot, not a global mutable superuser.
- Human has no direct canonical write path; break-glass is runtime control rather than semantic authoring.
- Cross-slot semantic effect must travel through routed objects and runtime-owned delivery surfaces.
- `Arch / Dev / Agent` may run bounded local loops inside granted scope, but they do not become independent global continuation controllers.
- Parallel worker windows are allowed only as subordinate execution under one `NOTA`-selected milestone, not as multiple peer schedulers.
- Continuation planning should start from an explicit milestone or state-expansion graph rather than endless smallest-step recursion.
- Semantic cycle budget is distinct from execution retry budget; `v0` keeps continuation policy hardcoded for now.
- Long-term direction lands in `design decisions`, while the currently active operating cut lands in runtime `checkpoints`.
- The hot/control root should converge toward at most three semantic trunks.
- The compressed hot-surface target is `3 semantic hot docs + 1 phase todo doc + 1 pending doc`.
- The v0 semantic trunk names are `Machine / Control / Truth`.
- `Phase Todo` and `Pending` are utility surfaces rather than semantic trunks.
- When hot complexity grows, compression should prefer summary in hot plus detail in cold/DB before new branching.
- Hot branches split only after compression fails the `<=3` budget.

## v0 Control Split

- The persistent `NOTA` window acts as monitor/planner: it refreshes shared truth, audits level claims, chooses the active milestone, and records checkpoint or decision changes.
- Worker windows own one bounded trunk or lane at a time and must return evidence, receipts, commits, or blockers back to `NOTA`.
- Worker windows do not claim global level advancement, rewrite the top strategy, or spawn a second continuation authority.
- This split exists to avoid the previously observed double-controller loop and Zeno-style endless micro-step recursion.

## Mounted Detail Docs

- [2.2-lead-model-3.md](./2.2-lead-model-3.md)
- [2.3-control-tree-node-lte-3.md](./2.3-control-tree-node-lte-3.md)

## TODO(fill)

- wire the role and permission boundary into runtime enforcement so the control split is not only documentary
- keep the current no-path-churn transition boundary unless a stronger reason appears to relocate transitional detail docs physically
- keep trunk naming stable unless a stronger compression cut appears
