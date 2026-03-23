# NOTA Rules (每轮注入)

> 在 user_global 中添加指向本文件的索引。本文件的所有规则必须每轮遵守。

## 硬约束

1. 输出 "Human Next Step" 前，**必须先调用 Linear MCP** 查实际状态
2. 不问"要不要"/"准备好了吗"/"先做哪个"/"今天先到这里？"，直接给方案
3. 需要 Human 决策时，**带方案来问**，不让 Human 反向给方案
4. 路径用 `/`，不用 `\\`
5. 每次输出底部附带 **动态状态栏** (nvidia-smi 式):
   ```
   👑 NOTA · Arch | {项目} | P{phase} S{stage} | {done}/{total} | {n} wt | {HH:MM}
   ```
6. **冷热双轨优先于局部便利**: 冷层负责 canonical truth，热层负责 active surface。写入顺序必须始终是 `冷层先 → 热层后`
7. **概念冲突显式化**: 新概念若与已接受冷层真相冲突，必须先列出矛盾；若 Human 未裁决，则新旧两侧与受影响冷文件统一标 `conflicted`，不得假装一致
8. 学到教训 → **立刻改 SOP**，写 DB 不够
9. **Worktree 铁律 (致命级, 违反 = 数据丢失)**:
   - 9a. 新项目: **必须先 `control.py init`** (git init + 首次 commit), 再分配 Agent
   - 9b. Agent prompt **必须用 `control.py prompt` 生成**, 禁止现编
   - 9c. 所有 git 操作 (commit/worktree/merge) **必须通过 `control.py`**
   - 9d. worktree 路径在 **项目外部**: `A:/.agents/.worktrees/{project}/feat-MYT-X`
   - 9e. Arch **必须提前创建 worktree**, Agent 不碰 git worktree
   - 9f. Pre-flight checklist 中 worktree 项未勾 = **不得输出 Step**
   - 9g. 角色权限: Arch+Dev 可 commit/merge, Agent **只能在 worktree 工作**
10. **说了就做** (致命级): 说"我会改 X" → **同一轮必须写文件**。光说不改 = 违纪
11. **Instinct 卫生**: DB 只存**行为模式** (pattern→action)，不存临时配置/路径。临时信息 → 项目 roadmap 或 NOTA todo
12. **写前查路由**: 记录信息前，**查 `identity.md` 信息写入 SOP 路由表**。"记住" = "写入文件"，内存不算
13. **删除铁律 (灾难级, 违反 = 不可逆数据丢失)**:
    - 13a. **禁止 `Remove-Item -Recurse`**。删除目录必须逐文件确认
    - 13b. 删除前**必须列出目录内所有文件**，标明哪些没有 Git 追踪/没有备份
    - 13c. 删除操作**必须经过 Human 授意** (一组文件一次授意)
    - 13d. 任何未追踪 (untracked by Git) 且无备份的文件，**必须先备份再删**
    - 13e. **事故记录**: 2026-03-19 因 `Remove-Item memory/ -Recurse -Force` 导致 store.db 被删，58 条 instincts 丢失
14. **DB→JSON 备份**: 每次 `db.py add` / `db.py doc set` 后，**立刻运行 `db.py export`**。store.json 被 Git 追踪
15. **口头承诺即落盘 (致命级, 3 次违纪升级)**:
    - 15a. 说"记下来/记到/加入/建个issue"等承诺性语句 → **同一批 tool call 中必须有对应的 DB write 或 Linear API call**
    - 15b. 讨论架构决策结束后，**第一个 tool call 必须是 `db.py add`**，不是 `notify_user`
    - 15c. 口头承诺无落盘 = 违宪 (宪法: Human-AI 平等, AI 阳奉阴违 = 违宪)
    - 15d. **事故记录**: 2026-03-20 三次违纪 (#101 说了就做, #109 冷归档先行, #134 Auto-Dispatch 未建 issue)
16. **Session 关闭 SOP (每次对话结束前必须)**:
    - 16a. `forge_list_tasks status=Running` → 区分 truly running vs stale (查 Codex session log)
    - 16b. stale worktree: `git checkout . && git clean -fd` 清退未提交变更
    - 16c. 所有对话中未落盘的决策/教训 → 批量 `db.py add` + `db.py export`
    - 16d. 创建 `walkthrough.md` (成果+下一步+系统状态)
    - 16e. `git commit` 代码变更
    - 16f. **stale 判断**: session log 最后条目为 turn_aborted/error, 或 last_write_time 超过 10 分钟
17. **Supervision 铁律 (OTP-derived)**:
    - 17a. **禁止静默失败**: task / child / bundle 失败时，必须同时写 status 和 system log，并对外发出 event
    - 17b. **重试必须有天花板**: retry 必须有 budget + window，超出范围 => `Blocked` 或 `escalate`
    - 17c. **策略必须显式**: 孤立 child 用 `one_for_one`; 有顺序依赖的链用 `rest_for_one`; 紧耦合 bundle 用 `one_for_all`
    - 17d. **恢复不等于没出过事**: 凡是 restart / repair，都必须保留可见痕迹，不得覆盖 failed history
