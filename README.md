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
| Hive | Persists the older dispatch/callback/review ledger and a minimal loop ledger for `Explorer -> Doer -> Evaluator` rounds, stages, packets, admissions, evidence, verdicts, and linked issues. Stage, stage-evidence, packet, admission, verdict, and issue-surface audit checks now catch basic replay and binding drift. | Durable loop ledger for repeated rounds, candidate history, evidence manifests, verdict history, and human decisions. |
| Compiler design | Compiler/action IR ideas exist in [`entrance-wiki/archive/legacy/agents/specs/compiler.md`](./entrance-wiki/archive/legacy/agents/specs/compiler.md). The active runtime now has versioned typed packet envelopes, receipt-aware admission gates that reject failed worker receipts, versioned admission receipts, and versioned verdict receipts bound back to stage evidence/admission facts, but this is still an MVP compiler cut rather than a full IR. | Active compiler IR with policy registry, richer admission gates, typed packets, receipts, verdicts, and runtime-owned routing. |
| Role separation | `hive loop run` records serial `Explorer`, `Doer`, and `Evaluator` stages with separate packet routes. The audit now rejects duplicate stage-role rows in a round and missing expected current-round stages. Each stage carries a role worker receipt, while long-lived worker isolation and replacement are still future work. | `Explorer`, `Doer`, and `Evaluator` run as separate serial agents with clear write boundaries and replacement behavior. |
| Agent execution | MVP runtimes are `local` and `codex`; `codex` launches read-only `codex exec` role workers with configurable timeout, bounded attempts, and transcript evidence. There is not yet worker replacement, richer retry policy, sandbox matrix, or artifact manifest collection. | Runtime-managed workers with bounded permissions, receipts, retry policy, replacement behavior, and evidence manifests. |
| Review surface | The local Panel exposes issue/status/comment cards, trace chips, retry/review/cancel actions, linked loop state, connector freshness, a provider-scoped connector publish queue, two-step publish plan/execute gates, one-click issue mirror roundtrip, queue-level digest-bound roundtrip plan/execute gates, and provider-specific admission blockers backed by local config for local/file plus Linear/GitHub providers. Publish and queue roundtrip execution record typed issue comments/evidence before writing connector mirrors, plans are gated by provider writer-adapter/readback/admission capability, admission previews expose typed check vectors, and the active `remote-fixture:` provider validates remote write/readback receipts locally. GitHub can now expose a guarded REST publish/readback connector when `entrance.toml` enables it and a token env is present: publish updates issues and upserts the latest comment with an issue-stable idempotency marker, comment lookup/readback follows GitHub `Link` pagination, transient `5xx` responses become typed retry/backoff attempts, `403/429` rate limits become typed blockers, and admission stays blocked until the typed readback checks pass. Linear can now expose a guarded GraphQL publish/readback connector when configured: it reads the issue UUID by Linear identifier, updates title/description, upserts the latest comment with the same issue-stable marker strategy, retries transient GraphQL HTTP `5xx`, classifies `403/429` and GraphQL rate-limit errors as typed blockers, and gates admission on typed readback. GitHub/Linear providers parse provider-specific `remote_target` values such as `github:owner/repo#123` and `linear:TEAM-123`, then expose `entrance.hive.connector_remote_write_plan.v1` request envelopes so invalid targets or inactive providers become typed blockers before any remote write path. There is not yet a complete Linear/GitHub connector with production drift handling, richer Linear state mapping, broader retry policy, and real-token coverage. | External board where issue status and comments expose every loop stage, blocker, option, and decision across local and remote issue systems. |
| Evaluation | Evaluator emits `keep`, `reject`, `needs-review`, or `blocked` with schema-versioned score vectors, gate results, evidence links, and human options. Target-drift checks are still shallow. | Evaluator produces structured verdicts from richer gates, metrics, evidence, target-drift checks, and human preference boundaries. |
| GUI | GUI shows Runtime, Drawer, Hive, Launcher, and a minimal Linear-like Panel for loop issues. Issue cards show round-aware packet/admission/evidence/verdict trace chips, connector target chips for parsed remote issue targets, remote write-plan chips for the provider request envelope, and the connector queue can plan/execute Publish or Roundtrip actions; unsupported provider publish/roundtrip buttons and execute plans surface adapter blockers instead of running. | Full loop dashboard for observing rounds, roles, status, evidence drilldown, verdicts, and human review points. |

## Minimum Usable Unit

当前仓库已经具备一个可运行的本地闭环原型：可以创建 issue 绑定的 loop，用 `local` 或 `codex` runtime 串行跑 `Explorer -> Doer -> Evaluator`，把 packet、admission、worker receipt、evidence、verdict 和 issue comment 写入 SQLite，并在本地 Panel/CLI 中查看状态。

但如果把“最小可用单元”定义为 Entrance 这个项目真正想交付的东西，也就是一个通过外部 `issue(status) + comment` 面板约束 multi-agent loop 的 compiler/runtime 控制平面，那么当前还没有完成。还差这些最小闭环能力：

- 正式的 compiler IR 和更稳定的 policy registry 生命周期，而不是主要散落在 Hive 命令路径里的 MVP 数据结构。
- 完整的 GitHub/Linear issue connector，包括幂等 receipt、失败重试和本地/远端漂移校验；GitHub 目前已有受配置和 token gate 保护的 publish/readback/admission gate 切片，并能用 issue-stable idempotency marker upsert 最新 comment、按 GitHub `Link` header 读取分页 comments、对瞬时 `5xx` 做 typed retry/backoff、对 `403/429` rate limit 给出 typed blocker；Linear 目前已有受配置和 token gate 保护的 GraphQL publish/readback/admission gate 切片，可用 issue identifier 读取 UUID、更新标题/描述并用同一 issue-stable marker 更新最新 comment，也能对 GraphQL HTTP `5xx` 做 typed retry/backoff、对 `403/429` 和 GraphQL rate limit 给出 typed blocker，但还缺生产级漂移处理、更完整的重试策略、真实 token 覆盖和 Linear 状态映射。
- 更严格的 worker 生命周期管理：隔离、替换、超时后恢复、artifact/evidence manifest 收集，以及跨轮次的失败归因。
- 更完整的 Evaluator gates：目标漂移检测、score vector 计算、keep/reject/block 的证据要求，以及需要人类偏好时的选项生成。
- 真正面向 loop 的 dashboard：按 round、role、issue、comment、evidence、verdict 和 blocker 组织，而不只是当前 Runtime/Drawer/Hive/Launcher 加一个最小 Panel。

## Validation

README-only changes do not require the full product validation suite. For source changes, run validation from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build
```
