# Entrance GitLab 规约

> 版本: v1
> 模式: 单项目 / 单主干 / 单 Bot / MR-Based

## 1. 目标

本规约用于约束 `Entrance` 在 GitLab 上的协作方式, 目标如下:

- 保持仓库结构简单, 先不引入 `arch` 或其他长期活跃分支。
- 以 `main` 作为唯一长期主干与唯一正式审计对象。
- 所有变更默认通过 Merge Request 进入 `main`, 不走直推。
- 将 AI 能力收口到一个 `bot` 账户, 避免多 Bot 权限切分失真。
- 将最终责任和发布决策收口到一个人类维护者账户。
- 保证后续即使发生问题, 真相仍能从 `main` 的提交历史、MR 讨论和 CI 记录中复盘出来。

## 2. 基本原则

- `main` 是唯一长期分支。
- 功能开发、试验、修复都从临时分支发起。
- 临时分支合并后应删除, 不保留长期并行开发干线。
- 所有进入 `main` 的改动都必须有对应 MR。
- 人类负责最终准入与节奏控制, Bot 负责实现、整理和提案。
- 当前不设置 `arch` 分支; 只有当 `main` 的审计压力、集成频率或 AI 自动合并风险明显上升时, 才升级为 `active -> main` 双层模型。

## 3. 账户模型

仓库只保留两类执行身份:

- `human`: 人类维护者账户, 项目角色为 `Maintainer`
- `bot`: 自动化账户, 项目角色为 `Developer`

说明:

- `NOTA`、`Arch`、`Dev` 均共享 `bot` 账户的 GitLab 权限。
- Persona 区分不通过 GitLab 权限建模, 而通过命名规范和 MR 元数据建模。
- 任何需要删除受保护分支、修改保护规则、处理发布或最终准入的动作, 都由 `human` 执行。

## 4. 分支模型

长期分支:

- `main`: 唯一长期主干, 受保护, 唯一发布与审计基线

临时分支:

- `dev/<topic>`
- `fix/<topic>`
- `chore/<topic>`
- `nota/<topic>`
- `arch/<topic>`

说明:

- 临时分支统一从最新 `main` 切出。
- `bot` 可以创建临时分支并向其推送。
- 临时分支不作为长期环境或长期事实来源。

## 5. GitLab 权限与 Branch Rule

项目采用 GitLab Free 可落地的最小配置。

### 5.1 `main`

- 设为 `Protected branch`
- `Allowed to merge = Maintainers`
- `Allowed to push and merge = No one`

效果:

- `human` 可以通过 MR 合并到 `main`
- `bot` 不能直接推送到 `main`
- `bot` 不能自行合并到 `main`
- 任何进入 `main` 的内容都必须经过 MR

### 5.2 临时分支

- 默认不设为 protected
- 允许 `bot` 正常创建、推送、更新
- MR 合并完成后删除源分支

## 6. MR 流程

统一流程如下:

1. `bot` 从 `main` 拉出临时分支。
2. `bot` 在临时分支上实现改动并推送。
3. `bot` 创建指向 `main` 的 Merge Request。
4. CI 通过后, `human` 审阅并决定是否合并。
5. 合并完成后删除源分支。

补充要求:

- 不允许 `bot` 直推 `main`
- 不允许绕过 MR 直接落主干
- 不保留长期悬挂分支
- 一个 MR 应尽量只承载一个清晰主题

## 7. Bot 命名与审计规范

由于 `NOTA`、`Arch`、`Dev` 共用一个 `bot` 账户, 必须通过文本约定保留可追踪性。

### 7.1 分支命名

- `nota/<topic>`
- `arch/<topic>`
- `dev/<topic>`
- `fix/<topic>`
- `chore/<topic>`

### 7.2 Commit 规范

建议提交信息前缀:

- `[NOTA]`
- `[ARCH]`
- `[DEV]`
- `[FIX]`
- `[CHORE]`

示例:

- `[DEV] implement forge task feed persistence`
- `[ARCH] refactor plugin boundary for vault commands`
- `[NOTA] sync spec wording with current release plan`

### 7.3 MR 标题规范

建议 MR 标题格式:

- `[DEV] <summary>`
- `[ARCH] <summary>`
- `[NOTA] <summary>`

### 7.4 MR 描述规范

每个 MR 至少回答以下问题:

- 此变更解决什么问题
- 为什么现在做
- 风险点是什么
- 如何验证
- 此次动作由哪个 persona 主导

推荐模板:

```md
## Summary

一句话说明本次改动

## Persona

DEV | ARCH | NOTA

## Why

说明动机与上下文

## Risk

说明潜在回归或不确定项

## Validation

- [ ] 本地运行
- [ ] 相关测试
- [ ] 人工验证
```

## 8. CI 与合并要求

建议在 GitLab 项目中开启以下策略:

- `Pipelines must succeed`
- `Delete source branch on merge`
- 禁止直接 push 到 `main`

如果当前 GitLab Free 实例允许配置, 建议再补充:

- squash merge 视团队习惯决定是否启用
- merge commit 规则保持一致, 不混用多种历史策略

本规约不强制要求 squash, 但要求:

- `main` 历史必须可解释
- MR 与最终提交之间应能互相映射

## 9. 人类维护者职责

`human` 的职责不是替 Bot 写代码, 而是把住主干。

`human` 负责:

- 审阅进入 `main` 的 MR
- 判断是否允许合并
- 处理发布、回滚和紧急封板
- 修改 branch rules 与仓库治理策略
- 定期审计 `main` 的提交历史与 MR 质量

定期审计重点:

- `main` 中是否存在无法解释的提交
- MR 标题、描述、验证记录是否完整
- 是否存在过大 MR 或混杂主题 MR
- Bot 是否持续遵守命名与说明规范

## 10. Bot 职责

`bot` 负责:

- 创建临时分支
- 在临时分支上提交代码与文档
- 发起指向 `main` 的 MR
- 在 MR 中提供足够的上下文、验证说明和风险说明

`bot` 不负责:

- 直接写入 `main`
- 自行放行进入 `main`
- 删除受保护分支
- 修改保护规则

## 11. 仓库初始化建议

在 `Pub` 群组中新建仓库时:

1. 默认分支名设置为 `main`
2. 创建仓库时勾选 `Initialize repository with a README`
3. 创建完成后配置 `main` 的 branch rule
4. 将 `human` 设为 `Maintainer`
5. 将 `bot` 设为 `Developer`

初始阶段不创建 `arch` 分支。

理由:

- 当前目标是先把事实收束到唯一主干
- 先观察单主干 MR-Based 流转是否足够稳定
- 避免过早引入双干线导致额外治理成本

## 12. 升级到 `active -> main` 的触发条件

仅在以下情况持续出现时, 才考虑引入 `active` 分支:

- `main` 上待审 MR 长期堆积
- AI 产出频率明显高于人类审阅频率
- 需要先批量集成再择机晋升到 `main`
- 需要将“自动合并区”与“人类签发区”严格分层

如果升级, 则采用:

- `main`: 人类签发主干
- `active`: AI 集成分支
- `dev/* -> active`
- `active -> main`

在未满足上述条件前, 默认坚持单主干方案。

## 13. 一句话治理结论

`Entrance` 当前采用 `单仓 + 单主干 main + 单 bot + MR-Based` 治理模式: Bot 负责提案与实现, Human 负责主干准入与审计, 所有真相以 `main` 的提交历史和 MR 记录为准。
