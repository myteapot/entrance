

## Workspace Layout

This repository is formatted as a Microt workspace:

- [`entrance-src/`](./entrance-src/) contains the Rust workspace, Electron shell, SolidJS renderer, product README, and source-level agent instructions.
- [`entrance-wiki/`](./entrance-wiki/) contains committed project knowledge. Current truth starts at [`entrance-wiki/current/`](./entrance-wiki/current/), while historical design material lives under `entrance-wiki/archive/`.
- [`entrance-auto/`](./entrance-auto/) contains reusable validation workflows, fixtures, templates, report output, screenshots, traces, logs, and generated release artifacts.

For current product usage, commands, and architecture, start with [`entrance-src/README.md`](./entrance-src/README.md).

## Current State vs Target State

| Area | Current State | Target State |
| --- | --- | --- |
| Product shape | V2 microkernel preview with one Rust binary, daemon, SQLite store, bus, scheduler, supervision, Drawer/Hive/Launcher plugins, and Electron GUI. | Agent-loop control plane with compiler/runtime constraints over multi-agent execution. |
| Hive | Persists dispatch, callback, and review state. This is a good starting point for a future loop ledger. | Durable loop ledger for `Explorer -> Doer -> Evaluator` rounds, candidates, evidence, verdicts, and human decisions. |
| Compiler design | Compiler/action IR ideas exist in [`entrance-wiki/archive/legacy/agents/specs/compiler.md`](./entrance-wiki/archive/legacy/agents/specs/compiler.md), but they are archived context, not current runtime truth. | Active compiler IR with policy registry, admission gates, typed packets, receipts, verdicts, and runtime-owned routing. |
| Role separation | Current Hive dispatch is a single task ledger, not a role-separated loop. | `Explorer`, `Doer`, and `Evaluator` run as separate serial stages with clear write boundaries. |
| Agent execution | No real agent worker launch, isolation, timeout, retry, replacement, or evidence collection is implemented. | Runtime-managed workers with bounded permissions, receipts, retry policy, replacement behavior, and evidence manifests. |
| Review surface | No active issue/status/comment integration, and no active Linear/GitHub issue connector in the V2 runtime path. | External board where issue status and comments expose every loop stage, blocker, option, and decision. |
| Evaluation | No evaluator gate model, score vector, or keep/reject/block decision contract. | Evaluator produces structured verdicts from gates, metrics, evidence, and target-drift checks. |
| GUI | GUI shows basic Runtime, Drawer, Hive, and Launcher surfaces. | Loop dashboard for observing rounds, roles, status, evidence, verdicts, and human review points. |

## Validation

README-only changes do not require the full product validation suite. For source changes, run validation from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build


```

