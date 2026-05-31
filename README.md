# Entrance

Entrance 是一个给 `agent loop` 加上 compiler/runtime 约束的本地控制平面。

普通 agent 很有趣，但也有一个明显问题：它可以执行很久，最后结果却和一开始的目标相差很远；执行过程中也可能逐渐偏离原本的意图。程序化语言之所以更稳定，是因为自然语言意图会先被编译成受约束的指令、类型、边界和错误反馈。Entrance 的核心想法，就是把类似的约束层引入 agent 行为。

换句话说，Entrance 不把 agent 当成一个无限自由的聊天执行者，而是把人的目标降低为可观测、可验收、可回滚的 `explore -> do -> evaluate` 串行循环。

## Core Idea

Entrance 的目标是把人的自然语言目标编译成一份 typed loop contract：

- 明确目标、边界、可尝试的方法集和验收标准。
- 明确角色边界，避免一个 agent 同时探索、执行、评价并自我放行。
- 明确状态流转，让每一步都有可追踪的 status、comment、证据和阻塞原因。
- 明确 evaluator 的 gates、score vector 和 keep/reject/block 判定。
- 明确人类决策面，让关键选择回到人，而不是让 agent 在模糊处越俎代庖。

这里的 `compiler` 不是传统意义上的源码编译器，而是 agent loop 的约束层：它把目标、策略、权限、输入输出、证据和状态转成 runtime 可以检查和执行的结构。

## Target Loop

一个 Entrance loop 由三个串行角色组成：

1. `Explorer`
   - 理解现状。
   - 读取代码、文档、issue 和已有证据。
   - 提出候选路线、约束、风险和验收方式。
   - 不直接修改产品状态。

2. `Doer`
   - 只执行已被接收的候选任务。
   - 在明确边界内做最小可用改动。
   - 产出命令、变更文件、日志、截图、指标等 evidence。

3. `Evaluator`
   - 只按 gate、score vector 和 evidence 判断结果。
   - 给出 `keep`、`reject`、`needs-review` 或 `blocked`。
   - 当需要人类偏好、外部信息或边界决策时，用 A/B/C 选项回到人类决策面。

每一轮 loop 都应投射到外部观察面：

- `issue` 表示任务、候选方向或阻塞点。
- `status` 表示当前阶段和可行动状态。
- `comment` 记录 agent 的输入、输出、证据、判定和人类决策。

最终目标是让 multi-agent 协作像一个可监督的编译执行系统，而不是一组彼此松散接力的自然语言会话。

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
| Hive | Persists the older dispatch/callback/review ledger and a minimal loop ledger for `Explorer -> Doer -> Evaluator` rounds, packets, admissions, evidence, verdicts, and linked issues. | Durable loop ledger for repeated rounds, candidate history, evidence manifests, verdict history, and human decisions. |
| Compiler design | Compiler/action IR ideas exist in [`entrance-wiki/archive/legacy/agents/specs/compiler.md`](./entrance-wiki/archive/legacy/agents/specs/compiler.md). The active runtime now has versioned typed packet envelopes, receipt-aware admission gates, versioned admission receipts, and versioned verdict receipts, but this is still an MVP compiler cut rather than a full IR. | Active compiler IR with policy registry, richer admission gates, typed packets, receipts, verdicts, and runtime-owned routing. |
| Role separation | `hive loop run` records serial `Explorer`, `Doer`, and `Evaluator` stages with separate packet routes. The roles are enforced by the Hive loop runner, not yet by isolated long-lived agent workers. | `Explorer`, `Doer`, and `Evaluator` run as separate serial agents with clear write boundaries and replacement behavior. |
| Agent execution | MVP runtimes are `local` and `codex`; `codex` launches a read-only `codex exec` worker with timeout and transcript evidence. There is not yet worker replacement, retry policy, sandbox matrix, or artifact manifest collection. | Runtime-managed workers with bounded permissions, receipts, retry policy, replacement behavior, and evidence manifests. |
| Review surface | The local Panel exposes issue/status/comment cards, trace chips, retry/review/cancel actions, and linked loop state. There is no active Linear/GitHub issue connector in the V2 runtime path yet. | External board where issue status and comments expose every loop stage, blocker, option, and decision across local and remote issue systems. |
| Evaluation | Evaluator emits `keep`, `reject`, `needs-review`, or `blocked` with schema-versioned score vectors, gate results, evidence links, and human options. Target-drift checks are still shallow. | Evaluator produces structured verdicts from richer gates, metrics, evidence, target-drift checks, and human preference boundaries. |
| GUI | GUI shows Runtime, Drawer, Hive, Launcher, and a minimal Linear-like Panel for loop issues. Issue cards show compact packet/admission/evidence/verdict trace chips. | Full loop dashboard for observing rounds, roles, status, evidence drilldown, verdicts, and human review points. |

## Validation

README-only changes do not require the full product validation suite. For source changes, run validation from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build
```
