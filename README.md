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
| Hive | Persists the older dispatch/callback/review ledger and a loop ledger for `Explorer -> Developer -> Reviewer` rounds, stages, packets, admissions, evidence, verdicts, and linked issues. Legacy `Doer/Evaluator` rows remain audit-compatible. The SQLite ledger now exposes schema health with `PRAGMA user_version`, expected table/column/index checks, and loop-query indexes. Stage, stage-evidence, packet, admission, verdict, and issue-surface audit checks now catch basic replay and binding drift; admission audit also recomputes missing receipts, gate result, reason/result binding, and Developer `accepted_candidate` target binding instead of trusting the stored receipt. Verdict audit recomputes Reviewer invalid-round budget use/exhaustion from verdict history, so the 3-round `Blocked` fallback cannot be proven only by self-reported score/evidence fields. When no same-round operator decision has taken over, verdict audit also binds the terminal contract status back to the current-round verdict decision. Issue-surface audit binds operator transition admission receipts back to from/to status, policy resources, and allowed actions. Issue trace/operator summaries now expose the same transition admission proof, including action, gate, from/to status, policy registry resource, transition-policy resource, and confirmation requirement. Evidence rows can now be indexed through a derived `entrance.hive.evidence_manifest.v1` report with payload/receipt/transcript/artifact/path entries, digest coverage, path verification status, and next actions. | Durable loop ledger for repeated rounds, candidate history, evidence manifests, verdict history, schema migrations, and human decisions. |
| Compiler design | Compiler/action IR ideas exist in [`entrance-wiki/archive/legacy/agents/specs/compiler.md`](./entrance-wiki/archive/legacy/agents/specs/compiler.md). The active runtime now has versioned typed packet envelopes, a kernel `PREFLIGHT_PACKET` admitted by `runtime_policy_ready` before agent workers spawn, a first-class `entrance.hive.runtime_preflight.v1` report exposed through CLI/MCP/Panel, receipt-aware admission gates that reject failed worker receipts, versioned admission receipts, versioned verdict receipts bound back to stage evidence/admission facts, and a kernel-owned issue status transition policy registry exposed through `hive policy registry` and embedded into each `issue_transition_policy.v1` report. `runtime_policy_ready` now requires the config-aware capability preview to report `worker_spawn_ready=true`, so an unconfigured external review surface becomes a kernel `Blocked` issue instead of spawning agents without an observable board. That registry now includes a serialized status state machine for `Todo/Doing/Blocked/Needs Review/Done/Canceled`, including allowed actions, blocked actions, gates, confirmation requirements, terminal/human-decision classes, and conditional `run` / retryable `Canceled` behavior. This is still an MVP compiler cut rather than a full IR. | Active compiler IR with policy registry, richer admission gates, typed packets, receipts, verdicts, and runtime-owned routing. |
| Role separation | `hive loop run` records serial `Explorer`, `Developer`, and `Reviewer` stages with separate packet routes. The audit now rejects duplicate stage-role rows in a round and missing expected current-round stages while still accepting legacy `Doer/Evaluator` ledgers. Each stage carries a role worker receipt, and `hive loop worker-lifecycle <id>` exposes expected roles, observed workers, missing roles, receipt status, retry exhaustion, and a 3-round Reviewer invalid-budget fallback computed from consecutive invalid verdicts in the ledger rather than from the round number alone. Long-lived worker isolation and replacement are still future work. | `Explorer`, `Developer`, and `Reviewer` run as separate serial agents with clear write boundaries, review gates, fallback budgets, and replacement behavior. |
| Agent execution | MVP runtimes are `local` and `codex`; `codex` launches read-only `codex exec` role workers with configurable timeout, bounded attempts, and transcript evidence. Runtime preflight now rejects unsupported runtimes, probe failures, or unready configured review-surface connectors at the kernel gate before `Explorer -> Developer -> Reviewer` workers are started, recording a `Blocked` issue with `runtime_policy` audit detail instead of a fake worker failure. `hive loop preflight <id>` exposes the policy, runtime probe, config-aware `runtime_capability_preview.v1` for worker spawn readiness / sandbox / artifact mode / connector readiness / human boundary / worker context, current admission, blocker, failures, and next actions before/after a run. Worker lifecycle is now observable through `entrance.hive.worker_lifecycle.v1`, and evidence manifest is now observable through `entrance.hive.evidence_manifest.v1`; there is not yet durable heartbeat/resume/cancel/replacement handling, an enforced sandbox matrix, or real artifact capture/archive. | Runtime-managed workers with bounded permissions, receipts, retry policy, replacement behavior, and evidence manifests. |
| Review surface | The local Panel exposes issue/status/comment cards, trace chips, retry/review/cancel actions, linked loop state, Activity Timeline, connector freshness, provider-scoped connector queues, digest-bound publish/roundtrip plan-execute gates, and admission blockers backed by local config for `local-hive-panel`, `file`, and `remote-fixture`. Issue actions carry typed operator confirmation contracts; Panel, CLI, and MCP decisions write confirmation receipts into the same comment/evidence ledger. `hive issue transition-policy <id>` exposes `entrance.hive.issue_transition_policy.v1`, `hive issue timeline <id>` exposes `entrance.hive.issue_timeline.v1`, and operator events now include transition admission proof. The active `remote-fixture:` provider validates remote write/readback receipts locally, and `hive connector fixture-demo --compact` plus Panel `Run Fixture` run a complete local external-surface dry-run. | Issue-board-like surface where issue status and comments expose every loop stage, blocker, option, evidence item, and human decision across local Panel, MCP, file mirrors, and local fixture surfaces. |
| MCP surface | `entrance mcp stdio` exposes the local Hive issue/status/comment kernel as first MCP tools/resources/prompts: issue list/show/comment/control, loop create/run/retry/decide, review queue, loop control, connector control/queue/publish-plan/roundtrip-plan, transition policy, activity timeline, dashboard, evidence drilldown, evidence manifest, runtime preflight, worker lifecycle, per-tool permission annotations, actor identity context, and confirmation receipts. Connector execute tools require `human_confirmed=true` plus the current `plan_id`, and confirmed decisions persist typed receipts into the operator or connector execution ledger. It is still a local stdio surface, not a complete remote MCP server or verified identity system. | MCP-native control plane where agents and humans can inspect, advance, retry, block, and audit loops through typed tools, resources, prompts, permissions, issue status, comments, and local connector surfaces. |
| Review | Reviewer emits `keep`, `reject`, `needs-review`, or `blocked` with schema-versioned score vectors, gate results, evidence links, and human options. The 3-round reviewer-invalid budget is now ledger-backed: only consecutive invalid Reviewer verdicts advance the budget, and the third invalid round falls back to `Blocked`. Reviewer score/gate output now derives its MVP metrics from the current-round ledger: stage completeness, runtime readiness, prior evidence presence, admission integrity, target alignment, missing receipts, and gate failure reasons. Developer admission now uses `accepted_candidate_bound`: the `EXECUTION_PACKET.accepted_candidate` must match the same-round admitted Explorer candidate, or the compiler gate rejects it as target drift. Semantic target-drift checks and richer quality metrics are still shallow. | Reviewer produces structured verdicts from richer gates, metrics, evidence, target-drift checks, budget policy, and human preference boundaries. |
| GUI | GUI shows Runtime, Drawer, Hive, Launcher, and a minimal issue-board Panel for loop issues. The Panel has a Review Queue for `Blocked` / `Needs Review` issues, issue cards with round-aware packet/admission/evidence/verdict chips, connector target/write-plan/status-mapping/decision chips, selected-issue Reviewer Control, Transition Policy, Loop Dashboard, Evidence Drilldown, Evidence Manifest, Activity Timeline, Runtime Preflight, Worker Lifecycle, and action confirmation metadata. State-changing Panel actions refresh the board and re-read the selected issue control surfaces. The connector queue can plan/execute Publish or Roundtrip actions; unsupported provider publish/roundtrip buttons and execute plans surface adapter blockers instead of running. | Full loop dashboard for observing rounds, roles, status, evidence drilldown, verdicts, and human review points. |

## Minimum Usable Unit

当前仓库已经具备一个可运行的本地闭环原型：可以用 `hive loop demo --runtime codex --compact` 启动默认 MVP 演示，也可以用 `hive loop start --runtime codex --compact` 自定义创建并运行 issue 绑定的 loop，或拆成 `loop create` 和 `issue run` 两步；运行时会先由 kernel 生成 `PREFLIGHT_PACKET` 并通过 `runtime_policy_ready` admission 证明 runtime 被 policy registry 支持且 probe ok，然后才串行跑 `Explorer -> Developer -> Reviewer`，把 packet、admission、worker receipt、evidence、verdict 和 issue comment 写入 SQLite，并在本地 Panel/CLI 中查看状态。unsupported runtime 会在 preflight 阶段直接变成 `Blocked` issue，带 `runtime_policy` audit detail，且不会伪造 worker failure。当前最小可用单元还包括 loop dashboard、evidence drilldown、evidence manifest、issue transition policy、activity timeline、worker lifecycle、operator transition admission proof、MCP stdio tools/resources/prompts，以及 `remote-fixture:` 本地外部面 dry-run。`hive policy registry --compact` 会暴露 issue status transition registry、状态机矩阵、connector admission required checks 和 `remote-fixture` status mapping；`hive connector fixture-demo --compact` 和 Panel `Run Fixture` 可以一键创建 `remote-fixture:` issue 并运行 publish -> readback -> admission -> final readback，把 connector readback/admission evidence 写回同一个 ledger。`entrance-auto/workflows/validation/run-local-mvp-demo.sh --full-gates --verify-golden` 可以从干净 app root 复现本地 MVP loop 与 `remote-fixture:` dry-run，输出机器可读报告，并比对已提交的稳定 contract golden fixtures；`capture-panel-screenshot.mjs --full-gates` 会用同一份数据捕获 Panel Issue board 截图和 metadata。`entrance mcp stdio` 已经提供最小 MCP control surface，让 MCP 客户端可以按 prompt contract 创建、运行、重试、评论、读取 issue-bound loops，并通过 review queue、loop control、connector control/queue/plan、single issue control packet、transition policy、timeline、dashboard、evidence、runtime preflight 和 worker lifecycle 看到 `Blocked` / `Needs Review` 的 human options、actions、blockers、Reviewer gates、score vector、3 轮 fallback budget、receipts 和 confirmation receipts。

当前验证也把 loop control 纳入稳定合同：`run-local-mvp-demo.sh --verify-golden` 会比对 `loop-control-summary.json`，`capture-panel-screenshot.mjs --full-gates` 会断言 Panel Reviewer Control、`loop_control.v1`、score vector、fallback budget 和 A/B/C operator options 可见。

MCP stdio 也有协议级 smoke：`entrance-auto/workflows/validation/run-mcp-stdio-smoke.mjs` 会启动真实 `entrance mcp stdio`，通过 newline-delimited JSON-RPC 调 `initialize`、tools/prompts/resource templates、MCP loop create/run、loop control tool/resource、loop review prompt、connector queue/control/decision prompt 和 digest-bound connector roundtrip plan/execute，并验证未设置 `human_confirmed=true` 的 retry 或 connector roundtrip 都会被拒绝。

但如果把“最小可用单元”定义为 Entrance 这个项目真正想交付的东西，也就是一个通过外部 `issue(status) + comment` 面板约束 multi-agent loop 的 compiler/runtime 控制平面，那么当前还没有完成。还差这些最小闭环能力：

- 正式的 compiler IR 和更稳定的 policy registry 生命周期，而不是主要散落在 Hive 命令路径里的 MVP 数据结构；当前已有 runtime preflight gate、issue transition policy registry、evidence manifest report，以及会阻止 unready connector spawn worker 的配置感知 `runtime_capability_preview.v1`，但还缺 policy 生命周期、版本迁移、完整 capability gate，以及真实 sandbox/artifact 执行约束。
- 外部 issue surface 产品化：当前目标收敛为本地 issue-board、file mirror 和 `remote-fixture` dry-run；还缺更稳定的 connector policy lifecycle、drift repair、真实 operator identity、远程 MCP server 形态，以及把外部面接入真实人类工作流的交互设计。
- MCP-native 产品化：当前已有本地 stdio tools/resources/prompts、本地 per-tool permission registry、本地 actor identity audit policy、本地 typed confirmation receipt 和协议级 stdio smoke，还缺真实 MCP 客户端配置、verified 身份/权限边界、named MCP client 兼容测试、远程连接器绑定和面向 human review 的交互设计。
- 更严格的 worker 生命周期管理：当前已有可观察的 `worker_lifecycle.v1` 报告和派生 `evidence_manifest.v1` 报告，但还缺隔离、替换、超时后恢复、真实 artifact 捕获/归档，以及跨进程/跨轮次的 durable 失败归因。
- 更完整的 Reviewer gates：当前已有 ledger-derived MVP score vector 和 Developer accepted-candidate binding；还缺语义级目标漂移检测、真实质量指标、keep/reject/block 的更强证据要求，以及需要人类偏好时的选项生成。
- 更完整的 evidence 产品化：当前已有 `evidence_drilldown.v1`、`evidence_manifest.v1` 和 Panel 聚焦视图，能展示 worker receipt、transcript/payload excerpt、remote receipt 摘要、artifact/path hint、payload key diff、digest coverage、path verification state、evidence/loop-level blocker 和绑定到 blocker 的 retry/review/cancel/comment 决策面；还缺可展开的完整 transcript、真实远端 receipt 归档、真实 artifact manifest 生成/内容校验、payload schema diff 和更完整的 blocker decision workflow。

## Validation

README-only changes do not require the full product validation suite. For source changes, run validation from `entrance-src/`:

```bash
cargo check --workspace
cargo test --workspace
pnpm check
pnpm build
```
