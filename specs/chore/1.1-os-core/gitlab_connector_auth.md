# Entrance GitLab Connector Auth 设计

> Arch 设计稿
> 状态: Proposed
> 范围: Harness + Connector + Vault + Updater 对齐

## 1. 背景

当前 `Entrance` 已支持在 Vault 中保存 `gitlab-updater` token, 并在客户端启动时将其作为 `PRIVATE-TOKEN` 发送给 GitLab, 用于读取私有仓库内的 `updater.json` 与安装包。

这条链路已经解决了“私有仓库下的 Tauri 自动更新”问题, 但仍存在几个架构层面的不足:

- GitLab 访问仍以 updater 为中心, 还不是正式的 Connector 能力。
- GitLab host / project / branch / auth provider 等运行信息尚未抽象为统一配置。
- token 的职责边界不清晰, 未来容易把 `bot` token、updater token、触发 pipeline 的 token 混在一起。
- Harness 尚未成为“公开配置 + 本地 secret 引用”的统一入口。

因此, 需要将 GitLab 从“单点功能特例”升级为正式的系统级接入模型。

## 2. 目标

本设计的目标是:

- 将 GitLab 接入升级为 `Entrance` 的正式 connector/profile。
- 将 `GitLab host / project / branch / provider key` 作为公开配置管理。
- 将 `GitLab token` 继续保存在本地 Vault, 不进入源码、不进入仓库、不进入发布包。
- 让 updater 复用统一的 GitLab connector auth 解析链, 不再单独持有 token 逻辑。
- 为后续 Harness 层集中管理外部服务接入提供标准模式。

## 3. 非目标

本期不做:

- 不在源码仓库中保存任何 GitLab token 明文
- 不把 token 打进安装包或 `updater.json`
- 不实现完整 GitLab API 客户端
- 不处理多租户 / 多用户共享 token 分发问题
- 不在本期强制引入 `active` 分支发布模型

## 4. 核心原则

### 4.1 公开配置与秘密分离

公开配置:

- 可以进入 `entrance.toml` 或 Harness 配置
- 可以被导出、备份、版本化
- 不能包含 secret

秘密:

- 只能存在于本地 Vault
- 只在运行时按 provider key 解析
- 不写入 repo、不写入静态配置、不写入发布产物

### 4.2 配置存引用, Vault 存本体

GitLab connector 配置应只声明:

- 连接到哪个 GitLab
- 面向哪个 project
- 默认 branch 是什么
- 使用哪个 auth provider

真正 token 值由 Vault 提供。

### 4.3 Updater 是 GitLab consumer, 不是 auth owner

Updater 的职责只是:

- 读取版本元数据
- 下载签名安装包

Updater 不应该自己定义 GitLab 认证模型, 而应该向 GitLab connector 请求“给我可用 header / 可用下载上下文”。

## 5. 目标架构

```text
Harness / Config
    -> GitLab connector profile
        -> auth_provider = "gitlab-bot"
        -> updater_provider = "gitlab-updater"
    -> Vault
        -> gitlab-bot = <secret>
        -> gitlab-updater = <secret>
    -> GitLab connector runtime
        -> resolve provider from Vault
        -> build request headers
    -> consumers
        -> updater
        -> future git sync / release / pipeline trigger / MR helper
```

## 6. 配置模型

建议在系统配置中引入 GitLab connector profile。

示意:

```toml
[connectors.gitlab.pub]
enabled = true
base_url = "http://server:9311"
project = "pub/entrance"
default_branch = "main"
auth_provider = "gitlab-bot"
updater_provider = "gitlab-updater"
```

字段说明:

- `enabled`: 是否启用该 GitLab profile
- `base_url`: GitLab 实例根地址
- `project`: `namespace/project` 路径
- `default_branch`: 默认主干
- `auth_provider`: 通用 GitLab connector 使用的 Vault provider key
- `updater_provider`: updater 读取 raw 文件和安装包时使用的 Vault provider key

说明:

- `auth_provider` 与 `updater_provider` 初期可以指向同一个 token
- 长期上建议保留拆分能力, 方便未来做“读仓库”和“写仓库”最小权限隔离

## 7. Vault 模型

GitLab token 继续使用现有 Vault 存储, 但语义上升级为 connector provider secret。

推荐 provider:

- `gitlab-bot`
- `gitlab-updater`

要求:

- token 只能保存在本地 app data 对应的 Vault 数据中
- token 不允许写入 `entrance.toml`
- token 不允许通过 export config 功能导出
- UI 中展示时必须遮罩

## 8. Updater 对齐方式

当前 updater 逻辑直接读取 Vault 中的 `gitlab-updater` provider。

目标改造方向:

1. Updater 读取 GitLab connector profile
2. 从 profile 中拿到 `updater_provider`
3. 通过统一 token resolver 读取 Vault secret
4. 组装 `PRIVATE-TOKEN` header
5. 请求 `updater.json` 与安装包

这样做的好处:

- updater 不再依赖硬编码 provider 语义
- 后续如果更换 provider 名称, 无需改 updater 代码
- Connector 成为唯一 GitLab 入口, 便于 Harness 集中治理

## 9. 发布与本地持久化原则

本设计要求:

- token 必须只存在于本机运行时数据目录
- 版本升级后本地 Vault 数据继续沿用
- 新 build / 新 release 不得重新打包 token
- 别人拿到安装包后, 无法从安装包中直接提取你的 GitLab token

这意味着:

- token 是“本机状态”
- 不是“应用资源”
- 不是“仓库配置”

## 10. 权限建议

最小权限建议:

- `gitlab-updater`: `read_repository`
- `gitlab-bot`: `write_repository`

仅在未来明确需要 GitLab API 自动化时, 再考虑额外 provider:

- `gitlab-api`
- `gitlab-trigger`

本期不建议为了方便而直接发一个大而全的 root 或 full-api token。

## 11. 实施拆分

建议拆成以下子任务:

### A. 配置层

- 在系统配置中引入 `connectors.gitlab.*` profile
- 定义 Rust 侧配置结构
- 为 profile 提供默认解析与校验

### B. Secret 解析层

- 提供通用 token resolver
- 支持按 provider key 从 Vault 读取 secret
- 统一错误语义: 未配置 / 解密失败 / provider 不存在

### C. Connector 层

- 新建 GitLab connector runtime
- 暴露统一 header 构造能力
- 为 future consumers 预留 raw file / release / repository helper 接口

### D. Updater 接入

- 让 updater 改为通过 GitLab connector 获取认证信息
- 去掉 updater 对固定 provider 名的直接依赖

### E. UI / Harness 层

- 在 Harness 或 Connector 页面中显示 GitLab profile 的公开配置
- 指示当前绑定的是哪个 provider
- 明确区分“配置已存在”和“secret 已配置”

## 12. 验收标准

- [ ] GitLab 连接信息可作为 profile 写入公开配置
- [ ] GitLab token 仅保存在本地 Vault, 不进入源码与发布产物
- [ ] updater 通过 connector profile + provider key 正常取 token
- [ ] token 更换只需更新本地 Vault, 无需修改源码
- [ ] 未来增加 GitLab 新用途时可复用统一 connector auth 模型

## 13. 与现有工作项关系

相关:

- `MYT-43`: 已完成 updater token 最小可用能力
- `MYT-46`: Harness Layer, 负责统一约束与配置管理
- `S3 Connector`: GitLab 应作为正式 connector/profile 纳入该方向

结论:

- `MYT-43` 解决的是“能用”
- 本设计解决的是“架构收口”

## 14. 一句话结论

`Entrance` 的 GitLab 接入应升级为“公开配置在 Harness / Connector, token 本体在本地 Vault, updater 通过统一 connector auth 解析”的标准模型, 从而保证长期可维护、可扩展、且不会把 bot token 混进源码或发布产物。
