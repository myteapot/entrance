> Mirror source: `A:/my/obsidian/大吕/11-entrance/ENTRANCE HANDOFF.md`
> Imported into repo wiki: `2026-04-08`

# ENTRANCE HANDOFF

> **最后更新**: 2026-04-07 22:16
> **当前阶段**: v1.0.0 收尾 — 全部后端 + GUI + Issue Tracker 完成
> **代码库**: `A:\Publish\entrance` (branch: `main`, HEAD: `36b2fc3`)
> **DB**: `~/.entrance/data/entrance.db` (SQLite)
> **冷归档**: [ENTRANCE ROADMAP](./entrance-roadmap.md)

## 你是谁

你被 Human 召唤来推进 Entrance 项目。默认以 **Arch** 身份启动：分析现状、出方案、等 Human 批准后再行动。

## Entrance 是什么

面向智能体的操作系统 (OS for Agent)。Tauri 2 + Rust 后端 + SolidJS 前端。

五层种姓制：`Human（神灵）→ NOTA（祭司）→ Arch → Dev → Agent`

### 弹性层级架构

NOTA 不是固定的第四层，而是一个**特质 (trait)**——注入到最靠近 Human 的那个角色里。Settings 最显眼的配置项：

| Level | 配置 | Human 角色 | NOTA 载体 |
|-------|------|-----------|-----------|
| 1 | Human only | Pair coding | 无 |
| 2 | Dev(NOTA) → Agent | Agentic coding | Dev 自带 NOTA 特质 |
| 3 | Arch(NOTA) → Dev → Agent | PM，只看 MR | Arch 自带 NOTA 特质 |
| 4 | NOTA → Arch → Dev → Agent | 全自主 (实验) | NOTA 独立身份 |

`X(NOTA)` 表示主体是 X，但增强了 NOTA 的特质（主动汇报、心跳、Human 对话）。默认 Level 3。

核心架构 = **编译器管线**（V1-alpha 后已是唯一执行路径）：
```
ActionRecord → compile() → TypedActionPacket
  → lower_dispatch() → LoweredDispatch
    → admit_dispatch() → AdmittedDispatch
      → resolve_return_route() → ReturnRoute
```

## 关键文件

| 子系统 | 路径 | 说明 |
|--------|------|------|
| 编译器管线 | `src-tauri/src/core/compiler/` | packet, lowering, admission, routing, evidence |
| NOTA 运行时 | `src-tauri/src/core/nota/mod.rs` | ~8000 行，编译器管线驱动 |
| 数据层 | `src-tauri/src/core/data_store.rs` | SQLite，~7500 行 |
| 监督逻辑 | `src-tauri/src/core/supervision.rs` | signal + budget + restart |
| Forge 引擎 | `src-tauri/src/plugins/forge/` | task 管理 + worktree |
| 前端 | `src/pages/` | Chat, Forge, Dashboard, Vault, Issues (SolidJS) |
| Issue Tracker | `src-tauri/src/cli/issues_cli.rs` | CLI subcommands (list/get/create/status/comment) |
| Issue Tracker | `src/pages/Issues.tsx` | Kanban board UI |

## Remaining

> V0-min (16 MR) + V1-alpha (5 MR) + V1 Next (16 项) 全部完成。  
> 完整历史 → [ENTRANCE ROADMAP](./entrance-roadmap.md)，交付明细 → [ENTRANCE ROADMAP#Completed via HANDOFF](./entrance-roadmap.md#completed-via-handoff)。

| ID            | Task                                                               | Status |
| ------------- | ------------------------------------------------------------------ | ------ |
| **Linear 清理** | 移除 forge/mod.rs 中 Linear API types + resolve_linear_token fallback | ⬜      |
| **Electron**  | `electron` 分支：Tauri→Electron IPC 桥接适配                              | ⬜      |
| **v1.0.0**    | Release candidate                                                  | ⬜      |

## Current Status (2026-04-07 22:16)

- **后端**: 2A/2B/2C/2D + G2/G3 + ENT-4 全部完成。244 tests green
- **前端**: Board + Console + Do + Chat + Settings + Issues 全页面完成
- **De-Linearization (ENT-4)**: ✅ MR!26 (5 commits on `feat/de-linearize`)
  - `bba73bc` — Issue Tracker MVP: DB migration + 9 CRUD methods + IPC + Kanban UI
  - `9bec425` — Forge dispatch 去 Linear 化 (local DB first, Linear fallback)
  - `50a3336` — Issues + Board overlay → flex split-pane 布局
  - `72266f1` — `entrance issues` CLI 全套子命令
  - `36b2fc3` — MCP 暴露 5 个 issue CRUD tools
- **Issue Tracker 三面覆盖**: CLI (`entrance issues`) + MCP (`issues_list/get/create/update_status/add_comment`) + GUI (Kanban)
- **视觉迁移 (Carbon Flat)**: ✅ 包含 split-pane 布局修复
- **设计语言**: "Carbon" — grey-in-black + muted accent，零渐变、零 glow、零 shadowBlur
- **计算图**: 双模式 (⊤ Tree 层级图 / ◎ Force 力导向)，默认 Tree
- **下一步**: Linear 残留代码清理 → Electron 分支适配 → v1.0.0 release

## 编码规则

1. `cargo check` → `cargo test --lib` → commit → push
2. 所有 test 函数必须 `let _guard = crate::test_env_guard();`
3. 分支: `feat/<id>-<slug>`，squash merge to main
4. GitLab REST API + PAT — 参见 `gitlab-api` skill
5. **禁止 PowerShell 文件写入** — 用 Python 或内置工具
6. **并行 Agent 任务必须用 `git worktree`**: 多个 Codex 实例并行时，每个任务必须 `git worktree add ../entrance-<slug> -b feat/<branch> origin/main`，**绝不在同一个 worktree 里 checkout 多个分支**。Arch 生成 prompt 时必须包含 worktree 指令。
   - 事故记录: 2026-04-05，三个 Codex prompt 使用 `git checkout -b` 在同一目录，导致 commit 交叉污染。

## 快速启动

```bash
cargo check --manifest-path A:\Publish\entrance\src-tauri\Cargo.toml
cargo test --manifest-path A:\Publish\entrance\src-tauri\Cargo.toml --lib
```
	