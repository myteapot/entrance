# Entrance

Entrance 是一个给 `agent loop` 加上 compiler/runtime 约束的本地控制平面。

普通 agent 很有趣，但也有一个明显问题：它可以执行很久，最后结果却和一开始的目标相差很远；执行过程中也可能逐渐偏离原本的意图。程序化语言之所以更稳定，是因为自然语言意图会先被编译成受约束的指令、类型、边界和错误反馈。Entrance 的核心想法，就是把类似的约束层引入 agent 行为。

换句话说，Entrance 不把 agent 当成一个无限自由的聊天执行者，而是把人的目标降低为可观测、可验收、可回滚的 `explore -> develop -> review` 串行循环。

## Core Idea

Entrance 的目标是把人的自然语言目标编译成一份 typed loop contract：

- 明确目标、边界、可尝试的方法集和验收标准。
- 明确角色边界，避免一个 agent 同时探索、执行、评价并自我放行。
- 明确状态流转，让每一步都有可追踪的 status、comment、证据和阻塞原因。
- 明确 reviewer 的 gates、score vector 和 keep/reject/block 判定。
- 明确人类决策面，让关键选择回到人，而不是让 agent 在模糊处越俎代庖。

这里的 `compiler` 不是传统意义上的源码编译器，而是 agent loop 的约束层：它把目标、策略、权限、输入输出、证据和状态转成 runtime 可以检查和执行的结构。

## Target Loop

一个 Entrance loop 由三个串行角色组成：

1. `Explorer`
   - 理解现状。
   - 读取代码、文档、issue 和已有证据。
   - 提出候选路线、约束、风险和验收方式。
   - 不直接修改产品状态。

2. `Developer`
   - 只执行已被接收的候选任务。
   - 在明确边界内做最小可用改动。
   - 产出命令、变更文件、日志、截图、指标等 evidence。

3. `Reviewer`
   - 只按 gate、score vector 和 evidence 判断结果。
   - 给出 `keep`、`reject`、`needs-review` 或 `blocked`。
   - 如果连续 3 轮预算已经用完，并且 reviewer 仍判断候选无效，则 fallback 到 `Blocked` issue 状态，等待人类决策。
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
| Hive | Persists the older dispatch/callback/review ledger and a loop ledger for `Explorer -> Developer -> Reviewer` rounds, stages, packets, admissions, evidence, verdicts, and linked issues. Legacy `Doer/Evaluator` rows remain audit-compatible. The SQLite ledger now exposes schema health with `PRAGMA user_version`, expected table/column/index checks, and loop-query indexes. Stage, stage-evidence, packet, admission, verdict, and issue-surface audit checks now catch basic replay and binding drift. | Durable loop ledger for repeated rounds, candidate history, evidence manifests, verdict history, schema migrations, and human decisions. |
| Compiler design | Compiler/action IR ideas exist in [`entrance-wiki/archive/legacy/agents/specs/compiler.md`](./entrance-wiki/archive/legacy/agents/specs/compiler.md). The active runtime now has versioned typed packet envelopes, receipt-aware admission gates that reject failed worker receipts, versioned admission receipts, and versioned verdict receipts bound back to stage evidence/admission facts, but this is still an MVP compiler cut rather than a full IR. | Active compiler IR with policy registry, richer admission gates, typed packets, receipts, verdicts, and runtime-owned routing. |
| Role separation | `hive loop run` records serial `Explorer`, `Developer`, and `Reviewer` stages with separate packet routes. The audit now rejects duplicate stage-role rows in a round and missing expected current-round stages while still accepting legacy `Doer/Evaluator` ledgers. Each stage carries a role worker receipt, while long-lived worker isolation and replacement are still future work. | `Explorer`, `Developer`, and `Reviewer` run as separate serial agents with clear write boundaries, review gates, fallback budgets, and replacement behavior. |
| Agent execution | MVP runtimes are `local` and `codex`; `codex` launches read-only `codex exec` role workers with configurable timeout, bounded attempts, and transcript evidence. There is not yet worker replacement, sandbox matrix, or artifact manifest collection. | Runtime-managed workers with bounded permissions, receipts, retry policy, replacement behavior, and evidence manifests. |
| Review surface | The local Panel exposes issue/status/comment cards, trace chips, retry/review/cancel actions, linked loop state, connector freshness, a provider-scoped connector publish queue, two-step publish plan/execute gates, one-click issue mirror roundtrip, queue-level digest-bound roundtrip plan/execute gates, and provider-specific admission blockers backed by local config for local/file plus Linear/GitHub providers. Issue actions now carry a typed operator confirmation contract, and Panel retry/review/cancel decisions write `source=panel` confirmation receipts into the same operator decision comment/evidence ledger used by MCP. Publish and queue roundtrip execution record typed issue comments/evidence before writing connector mirrors, plans are gated by provider writer-adapter/readback/admission capability, admission previews expose typed check vectors, and connector status now carries compact remote diagnostics so write/readback retry and rate-limit signals can be shown in the Panel with expandable operation-attempt drilldown. The active `remote-fixture:` provider validates remote write/readback receipts locally. GitHub can now expose a guarded REST publish/readback connector when `entrance.toml` enables it and a token env is present: publish updates issues and upserts the latest comment with an issue-stable idempotency marker, comment lookup/readback follows GitHub `Link` pagination, transient `5xx` responses become typed retry/backoff attempts, `403/429` rate limits become typed blockers, and admission stays blocked until the typed readback checks pass. Linear can now expose a guarded GraphQL publish/readback connector when configured: it reads the issue UUID by Linear identifier, updates title/description, upserts the latest comment with the same issue-stable marker strategy, retries transient GraphQL HTTP `5xx`, classifies `403/429` and GraphQL rate-limit errors as typed blockers, and gates admission on typed readback. GitHub/Linear providers parse provider-specific `remote_target` values such as `github:owner/repo#123` and `linear:TEAM-123`, then expose `entrance.hive.connector_remote_write_plan.v1` request envelopes so invalid targets or inactive providers become typed blockers before any remote write path. `hive policy registry --compact` now exposes the GitHub/Linear connector retry budgets that the write/readback paths actually use and the connector admission `required_checks` compatibility list plus structured `check_registry` entries with severity, owner, required evidence, and summary; actual admission check rows inherit that metadata so CLI and Panel can tie a failed check back to its policy owner and evidence contract. `retry_policy_bound` rejects remote diagnostics whose observed attempts exceed the active policy budget. There is not yet a complete Linear/GitHub connector with production drift handling, richer Linear state mapping, configurable/adaptive retry policy, and real-token coverage. | External board where issue status and comments expose every loop stage, blocker, option, and decision across local and remote issue systems. |
| MCP surface | `entrance mcp stdio` exposes the local Hive issue/status/comment kernel as first MCP tools/resources/prompts: list/show/comment issues, read a typed single-issue control packet, read the `Blocked`/`Needs Review` review queue, create/run/retry/decide issue-bound loops, read status/schema/policy/issue resources, and fetch prompt contracts for loop creation, issue advancement, and blocked human decisions. `tools/list` now annotates each tool with `entrance.mcp.tool_permission.v1`, while `entrance://policy/mcp-permissions` exposes the same per-tool registry plus `entrance://policy/actor-identity` for self-reported MCP actors and local Panel audit actors. `entrance_issue_control` and `entrance://issues/{id}/control` aggregate status, actions, blockers, recent evidence, operator confirmation receipts, actor identity context, and human decision call templates for one issue. MCP retry/review/cancel tools require `human_confirmed=true`, and confirmed decisions record a typed `entrance.hive.operator_confirmation_receipt.v1` in the operator decision comment/evidence payload, including `initialize.clientInfo` and a non-verified actor record when provided, while preserving a readable MCP confirmation marker in the note. It is still a local stdio surface, not a complete remote MCP/Linear connector product or verified identity system. | MCP-native control plane where agents and humans can inspect, advance, retry, block, and audit loops through typed tools, resources, prompts, permissions, and issue connectors. |
| Review | Reviewer emits `keep`, `reject`, `needs-review`, or `blocked` with schema-versioned score vectors, gate results, evidence links, and human options. A reject at or after the 3-round reviewer-invalid budget falls back to `Blocked`. Target-drift checks are still shallow. | Reviewer produces structured verdicts from richer gates, metrics, evidence, target-drift checks, budget policy, and human preference boundaries. |
| GUI | GUI shows Runtime, Drawer, Hive, Launcher, and a minimal Linear-like Panel for loop issues. The Panel now has a first-class Review Queue band for `Blocked` / `Needs Review` issues, using the same issue actions, doctor summaries, verdict reason codes, and evidence links as the board. Issue cards show round-aware packet/admission/evidence/verdict trace chips, connector target chips for parsed remote issue targets, remote write-plan chips for the provider request envelope, remote diagnostic chips for retry/rate-limit signals, selected-issue attempt drilldown for remote write/readback operations, and action confirmation metadata on decision buttons. The connector queue can plan/execute Publish or Roundtrip actions; unsupported provider publish/roundtrip buttons and execute plans surface adapter blockers instead of running. | Full loop dashboard for observing rounds, roles, status, evidence drilldown, verdicts, and human review points. |

## Minimum Usable Unit

当前仓库已经具备一个可运行的本地闭环原型：可以用 `hive loop demo --runtime codex --compact` 启动默认 MVP 演示，也可以用 `hive loop start --runtime codex --compact` 自定义创建并运行 issue 绑定的 loop，或拆成 `loop create` 和 `issue run` 两步；运行时会串行跑 `Explorer -> Developer -> Reviewer`，把 packet、admission、worker receipt、evidence、verdict 和 issue comment 写入 SQLite，并在本地 Panel/CLI 中查看状态。Panel 的 retry/review/cancel 按钮现在会按 typed action confirmation contract 写入 operator decision receipt。`entrance mcp stdio` 也已经提供最小 MCP tool/resource/prompt 面，让 MCP 客户端可以按 prompt contract 创建、运行、重试、评论和读取这些 issue-bound loops，并通过 review queue 或单 issue control packet 直接看到 `Blocked` / `Needs Review` 的 human options、actions、blockers、recent evidence 和 receipts；每个 MCP tool 都带 per-tool permission annotation，MCP human decisions 需要显式确认，并把确认上下文、可选 MCP clientInfo 写入 operator decision note 和 typed confirmation receipt。`loop demo --compact` 会给出 Panel 启动信息，`loop start --compact` 同时给出失败恢复摘要，包括 retry command、failed checks、missing receipts 和 failed worker rows。

但如果把“最小可用单元”定义为 Entrance 这个项目真正想交付的东西，也就是一个通过外部 `issue(status) + comment` 面板约束 multi-agent loop 的 compiler/runtime 控制平面，那么当前还没有完成。还差这些最小闭环能力：

- 正式的 compiler IR 和更稳定的 policy registry 生命周期，而不是主要散落在 Hive 命令路径里的 MVP 数据结构。
- 完整的 GitHub/Linear issue connector，包括幂等 receipt、失败重试和本地/远端漂移校验；GitHub 目前已有受配置和 token gate 保护的 publish/readback/admission gate 切片，并能用 issue-stable idempotency marker upsert 最新 comment、按 GitHub `Link` header 读取分页 comments、对瞬时 `5xx` 做 typed retry/backoff、对 `403/429` rate limit 给出 typed blocker；Linear 目前已有受配置和 token gate 保护的 GraphQL publish/readback/admission gate 切片，可用 issue identifier 读取 UUID、更新标题/描述并用同一 issue-stable marker 更新最新 comment，也能对 GraphQL HTTP `5xx` 做 typed retry/backoff、对 `403/429` 和 GraphQL rate limit 给出 typed blocker；GitHub/Linear retry budget 已进入 policy registry，admission preview 也会用 `retry_policy_bound` 校验观测到的远端 attempt budget，但还缺生产级漂移处理、可配置/自适应重试策略、真实 token 覆盖和 Linear 状态映射。
- MCP-native 产品化：当前只有本地 stdio tools/resources/prompts、本地 per-tool permission registry、本地 actor identity audit policy 和本地 typed confirmation receipt，还缺真实 MCP 客户端配置、verified 身份/权限边界、协议兼容测试、远程连接器绑定和面向 human review 的交互设计。
- 更严格的 worker 生命周期管理：隔离、替换、超时后恢复、artifact/evidence manifest 收集，以及跨轮次的失败归因。
- 更完整的 Reviewer gates：目标漂移检测、score vector 计算、keep/reject/block 的证据要求，以及需要人类偏好时的选项生成。
- 真正面向 loop 的 dashboard：按 round、role、issue、comment、evidence、verdict 和 blocker 组织，而不只是当前 Runtime/Drawer/Hive/Launcher 加一个最小 Panel。

## Validation

README-only changes do not require the full product validation suite. For source changes, run validation from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build
```
