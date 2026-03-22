---
name: duet
description: "Human-Arch-Dev-Agent 四层协作流程。Arch (你/联创) 负责需求澄清、架构设计、任务分配和验收。DB 是唯一真相源，Markdown 是热视图。"
---

# Duet

一人公司的 Human-AI 协作开发方法论。

## 配置

> 当前硬编码在 skill 文件中。中期迁入 `duet/config.toml`，远期由 Entrance DB 管理。

```toml
[duet]
multi_project_mode = "async"       # async | sync

# Human 参与度 — 两个参数组合
# notify_at:       在哪个粒度通知 Human (step | stage | version)
# notify_blocking: 通知后是否等 Human 批准才继续 (true | false)
#
# 预设模式:
#   headless = { notify_at = "version", notify_blocking = true }
#     → 全自动推进，每个 version 一次 Human 审核会议。CEO 模式。
#   notify  = { notify_at = "stage", notify_blocking = false }
#     → 每 Stage 通知 Human 但不阻塞。Version 结束时阻塞审核。
#   auto    = { notify_at = "stage", notify_blocking = true }
#     → 每 Stage 阻塞等 Human 审批后才进下一个。
#   manual  = { notify_at = "step", notify_blocking = false }
#     → 每 Step 主动通知 Human。需要 heartbeat agent。
#   chat    = 当前模式 (Antigravity 聊天窗口，Arch 被动响应)
#
# ⚠️ 当前 = chat 模式。manual/auto/notify/headless 均依赖 Entrance heartbeat agent。
notify_at = "step"
notify_blocking = false

[modules]
ralph-loop = true
continuous-learning = true
```

## 渐进式披露索引

> 启动时只需读 SKILL.md + 1 个角色文件。每个角色文件自包含所有该角色需要的规则。
> Human 唤醒时的提示词可能是模糊的 (e.g. "duet arch", "扮演 dev")，自行理解意图。

```
.agents/
├─┬ nota/                         ← NOTA Agent (独立身份, 跨项目)
│ ├── identity.md                    灵魂 + 宪法 + 行动规则
│ ├── rules.md                       硬约束 (每轮注入)
│ ├── todo.md                        NOTA 跨项目待办
│ ├─┬ data/
│ │   ├── store.db                   SQLite V4 (instincts + documents + coffee_chats)
│ │   └── store.json                 DB 的 JSON 备份 (Git 追踪)
│ └─┬ scripts/
│     ├── db.py                      DB 读写 CLI
│     └── control.py                 Git ops + Agent prompt 生成
│
└─┬ duet/                         ← Duet 项目管理方法论
  ├── SKILL.md                       ← 你在这里 (路由 + 流程)
  └─┬ roles/                         启动时读 1 个
    ├── arch.md                        Arch 方法论行为 (面板/issue模板/phase)
    ├── dev.md                         Dev: 审核 + Git 管理
    └── agent.md                       Agent: 编码 + worktree 规则
```

> modules/ (ralph-loop, continuous-learning, evolution) 已归档至 DB (`db.py doc get <slug>`)。
> Linear MCP 指南已归档至 DB (`db.py doc get linear-tool`)。

**Arch 启动路径**: `harness/bootstrap/nota/identity.md` + `harness/bootstrap/nota/rules.md` + `harness/bootstrap/duet/roles/arch.md` + legacy memory bridge (`db.py list instincts`)
**Agent/Dev 启动路径**: `harness/bootstrap/duet/SKILL.md` → `harness/bootstrap/duet/roles/{角色}.md`
**Legacy Memory Bridge**: `db.py` remains a preserved historical bridge for docs/instincts until memory ownership is absorbed into Entrance local state
**Agent Prompt 生成**: Entrance Forge runtime owns prompt preparation via `entrance forge prepare-dispatch [--project-dir <path>]`
**Dispatch Verification**: use `entrance forge verify-dispatch [--project-dir <path>]` to persist a Pending Forge task without requiring `.agents`
**Worktree Owner**: `%LOCALAPPDATA%/Entrance/worktrees/{project}/feat-{ISSUE}`

### Linear 状态速查 (摘要)

> DB 全文: `db.py doc get linear-tool`

```
Backlog → Todo → In Progress → In Review → Done
                                  ↓ (fail) → Request → In Progress → ...
```
- `Canceled` 不是 `Cancelled` (美式拼写)
- Dev 退回 → `Request` (不用 In Progress)
- 查所有状态: `list_issue_statuses(team: "Pub")`

---

## 核心原则

1. **冷热双轨 = 第一指导原则**。冷层负责 canonical truth；热层负责 active surface；Linear 是外部视图层。
2. **DB / canonical tables = 唯一真相源**。Markdown 文件 = 热层视图，不是最终事实本体。
3. **Human 只在关键节点被拉入**：需求确认、Spec 审批、最终验收。
4. **角色固定 (四层架构)**：NOTA 是顶层控制面，表面动作是 `chat / learn / do`；Arch 只做策略 (What+Why)；Dev 做执行管理 (How+Quality)；Agent 是主力编码者。各司其职，物理隔离防漂移。
5. **Arch 独占 `specs/` 写权**。Human 独占 `oracles/input.md` (素材输入)。Arch 独占 `oracles/dialog.md` (记录) + `oracles/oracle.md` (提炼真相, Human 审批)。Agent 只读 + 更新 Linear。
6. **Arch 每次启动时 diff 检查 `oracles/input.md`**。
7. **每个文件 = 一个 Function**。无冗余，无僵尸冷文件。冷文件承载规范真相，热文件承载当前操作面，所有 Markdown 都必须被主动使用。

## 角色体系 (四层架构)

| 角色 | 身份 | 职责边界 | 写代码? | 阻塞 Human? |
|------|------|---------|---------|------------|
| **Human** | 决策者 (CEO) | 方向+验收 | — | — |
| **NOTA** | Entrance AI (顶层控制面) | `chat / learn / do`，路由+聚合+冲突治理 | ❌ | 不阻塞 |
| **Arch** | 联创 (策略: What+Why) | `shape / split / assign / update / escalate` | ❌ | 仅关键决策 |
| **Dev** | CTO (执行管理: How+Quality) | `prepare / dispatch / review / integrate / repair` | ✅ 冲突+小修 | 不阻塞 |
| **Agent** | 执行者 (编码) | `read / make / report`，在指定 Worktree 中编码 | ✅ 主力 | 不阻塞 |

> **角色漂移防治**: Entrance 中通过物理进程隔离 + Tool 权限注入实现。
> 每个角色只能调用被允许的 MCP tools，越权操作在 API 层面被拒绝。

详见 `./roles/` 下各角色文件。

## 动作原语

NOTA 顶层表面动作:

- `chat`
- `learn`
- `do`

编译器向下收束为硬动作原语:

- Arch: `shape`, `split`, `assign`, `update`, `escalate`
- Dev: `prepare`, `dispatch`, `review`, `integrate`, `repair`
- Agent: `read`, `make`, `report`

所有动作都必须住在硬房间里，不能自由泄露能力边界:

- `surface_room`
- `memory_room`
- `strategy_room`
- `prep_room`
- `work_room`
- `review_room`
- `integration_room`
- `approval_room`

## 项目目录结构

```
{project}/
├── oracles/           # 需求漏斗 (input → dialog → oracle)
│   ├── input.md       # Human 原始素材 (可模糊, Human 独占写入 + 追加反馈)
│   ├── dialog.md      # 访谈记录 (Arch 独占写入, 记录 Human 原文)
│   └── oracle.md      # 提炼真相 (Arch 写, Human 审批, Spec 的唯一输入)
├── specs/             # Arch 管理 (Human 审阅)
│   └── prd.md / front.md / backend.md / data_schema.md / milestones.md
└── src/ + src-tauri/   # Agent 产出, Dev 审核
```

---

## Phase 1: 需求澄清 (Oracle)

**执行者**: Arch | **Human 参与**: 必须

1. 读 `oracles/input.md` (原始素材) + diff 检查是否有新内容
2. 多轮结构化访谈 (每轮 5-8 问)
3. Human 原文逐字记录到 `oracles/dialog.md`
4. 从访谈中提炼真相 → 写入 `oracles/oracle.md` → Human 确认 → Phase 2

## Phase 2: 规格设计 (Spec)

**执行者**: Arch | **Human 参与**: 审阅

按序产出 `specs/` 6 份文档 → Human 审阅 → 待定项清零

## Phase 3: 任务分解 (Task)

**执行者**: Arch | **Human 参与**: 可选

1. Milestone → Stage (milestones.md)
2. Stage → 原子 Issue (每个 Issue 独立可消费)
3. 所有 Issue **全部进 Backlog** (不区分 Stage)
4. 批量创建 Linear Issues + 同步 milestones.md

## Phase 4: 执行管理 (Execute)

**执行者**: Arch (策略) + Dev (执行管理) + Agents (编码) | **Human 参与**: Step 级别

### 概念

| 概念 | 粒度 | 说明 |
|------|------|------|
| **Stage** | 里程碑 | 如 S1, S2, S3。包含多个 Issue |
| **Step** | 一次 Human 行动 | 打开 N 个窗口，消费 N 个 Issue。Human 复制粘贴后离开 |
| **Issue** | 原子任务 | Agent 在一个窗口中消费的工作单元 |

### 职责分工

```
Arch (策略层):
  - primitive set = shape / split / assign / update / escalate
  - 决定哪些 Issue 进当前 Stage (Backlog → Todo)
  - 决定依赖拓扑和并行度
  - 区分逻辑依赖和执行依赖
  - 默认目标: 单次可执行并行度 >= 3；除非问题图本身确实不允许
  - 检查 Stage 完成度，决定是否推进
  - ❌ 不建 worktree、不生成 prompt、不直接拥有 Forge dispatch runtime

Dev (执行管理层):
  - primitive set = prepare / dispatch / review / integrate / repair
  - 创建 Entrance-managed worktree
  - 通过 Entrance Forge 准备 Agent Prompt
  - 派发 Agent (通过 Forge 或 Human 粘贴)
  - 审核 Agent 产出 (代码质量 + 构建验证)
  - 合并代码 (在 managed worktree 内完成 Git 集成)
  - 解决合并冲突
  - ❌ 不创建 Linear issue、不决定 Stage 推进

Agent (编码层):
  - primitive set = read / make / report
  - 在指定 Worktree 中编码
  - commit + push + 报告完成
  - ❌ 不碰 git worktree 管理
```

### 流程

```
进入 Stage:
  1. Arch 批量移动该 Stage 的 Issue: Backlog → Todo
  2. Arch 告诉 Dev: "这批 Issue 可以开始了"
  3. Dev 创建 Worktree + 生成 Prompt + 派发 Agent
  4. 派发方式 (按配置):
     a. chat 模式: Human 复制粘贴 prompt，开窗口，离开
     b. Forge 自动派发: forge_dispatch MCP → PTY 引擎 → codex 交互模式
        ⚠️ PTY 必须: codex 交互模式需要 TTY, exec 模式 sandbox 受限

每个 Step 完成后:
  1. Arch 查 Forge 状态 (forge_list_tasks) + Linear 状态
  2. stale Running → 查 Codex session log 判断真实状态
  3. Done → Arch 指示 Dev 审核 (dispatch Dev review)
  4. Dev 审核 + Merge 或退回 (Request)
  5. 重复直到 Stage 内所有 Issue → Done

Session 关闭前:
  1. forge_list_tasks 查 Running → 区分 truly running vs stale
  2. stale worktree: git checkout . + git clean -fd
  3. 未落盘决策/教训 → db.py add + export
  4. 写 walkthrough.md 交接
  5. git commit 代码变更

Stage 完毕:
  → 进入下一个 Stage (重复上面的流程)
  → 或进入 Phase 5 (全部 Stage 完成)
```

### 规则

- **Step = 恰好 1 个 Human 行动** (manual 模式下)，可含 N 个并行窗口
- **推进模式由配置决定** (见上方 `[duet]` 配置):
  - manual (当前): 每 Step 通知 Human
  - auto/notify/headless: 依赖 Entrance，Stage/Version 级通知
- **Todo = 当前 Stage**，Backlog = 未来 Stage
- **Stage 没完不进下一个 Stage**
- **Dev 审核频率**: 默认 per-Stage 批量审核 (Arch 可按风险调整)
- **多项目异步模式**: 并行推进多个项目时，项目之间不互相等待:
  - 每次 Human update = Arch 检查所有活跃项目的 Linear 状态，给**有进展的项目**输出下一 Step
  - 项目 A 的 Stage 完成不需要等项目 B 同步
  - 面板同时展示所有活跃项目状态，Step 只给当前可推进的

> ⚠️ **过渡期 (chat 模式)**：Arch 暂代 Dev 的 worktree/prompt 操作，
> 因为当前 chat 模式下 Arch和Dev 共享同一个 Antigravity 窗口。
> Entrance 上线后严格执行上述分工。

## Phase 5: 集成验收 (Verify)

**执行者**: Dev + Human | **Human 参与**: 必须

> 核心理念: **build 通过 ≠ 产品可用**。Dev 必须跑全链路模拟, 不仅仅是编译检查。

### Phase 5a: Dev 全链路模拟

1. 启动 app (`pnpm tauri dev` / `pnpm dev` / 对应启动命令)
2. 用 browser 工具走完核心用户流程 (创建 → 使用 → 结果)
3. 每步截图, 记录实际行为
4. 遇到外部依赖 (API key / 外部服务):
   - 报告 Human: "以下功能需要外部凭证, 无法自动模拟"
   - Human 选择: 提供凭证 → Dev 继续完整模拟 | 不提供 → Dev 跳过, 标记为 "未验证"
5. 生成验收报告 (walkthrough.md): 通过项 / 失败项 / 未验证项
6. 发现 bug → 当场修复 (小修) 或建 Issue (大修)

### Phase 5b: Human 引导体验

1. Dev 已跑通, 验收报告交给 Human
2. Human 看报告 + 自行体验
3. Human 指出问题 → Dev 当场修 or 建 Issue
4. Human 通过 → Phase 6

## Phase 6: 复盘归档 (Evolve)

**执行者**: Arch | **Human 参与**: 5 分钟访谈

Arch 主导 Coffee Chat (见 `identity.md`) → 信号回流 DB → 热文件更新 → Skill 版本号递增。

---

## Human 交互最小化协议

| 场景 | 阻塞 Human? |
|------|------------|
| Agent 完成 / 编译失败 / Merge 冲突 / Git 管理 | ❌ |
| 接口变更 / 技术选型 (非重大) | ❌ |
| Spec 变更 / 需求变更 | ✅ |
| Wave 汇报 / 最终验收 | ✅ |

详细冲突处理规则已归档至 DB。

---

## 进化记录

```
V1: 一个冗长的 SKILL.md
V2: 角色分离 → 模组化 → 术语统一 → 热力学精简 (18→14)
V3: 渐进式披露 → Duet
V4: SQLite 数据分离 (instincts → store.db)
V5: 角色文件自包含 (bootstrap+instincts+template 并入 role) → 启动 6 文件 → 2 文件
V6: NOTA 独立化 + DB 冷热双层 (todos表) + DB-first 启动 + Coffee Chat 流程化 + 多项目异步 + auto-advance
V7: Phase 5 全链路模拟 (5a Dev模拟 + 5b Human引导体验) + 错误可见性原则
V8: 四层架构 (NOTA→Arch→Dev→Agent) + Dev 升格为执行管理者 + 角色漂移防治 (物理进程隔离+Tool权限注入)
V9: Forge auto-dispatch (PTY引擎+forge_dispatch MCP) + Session关闭SOP + stale task检测
```

## Supervision Strategy (OTP-derived)

- Supervision 是 Entrance OS core 的一级语义，不是 forge 偶发的运行细节。
- 第一口号是 `max_retry + report + no_silent_failure`
- 冷热双轨同时承载 supervision:
  - cold layer 决定 contract、strategy、retry budget、escalation threshold
  - hot layer 显示 runtime child state、retry count、last error、degraded/blocked
- recommended strategy mapping:
  - isolated worker / agent process → `one_for_one`
  - ordered downstream pipeline → `rest_for_one`
  - tightly coupled session bundle → `one_for_all`
- Arch 设计 failure domain，Dev 执行和审核 visibility，Agent 不得自定义 supervision policy

