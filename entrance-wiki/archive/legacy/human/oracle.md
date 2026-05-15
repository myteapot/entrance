# Entrance — Oracle (定稿 v2)

> Phase 1 完成 + 架构演进。本文件是唯一真相源。

## 一句话

Entrance — "The last app you'll ever need to open." 插件化桌面统一入口 + **数据真相源**。

## 核心理念

| 原则 | 含义 |
|------|------|
| 冷热双轨优先 | 冷层承载 canonical truth，热层承载 active surface。任何设计都先分清事实层和操作层 |
| OTP-derived Supervision | max retry + report + no silent failure。冷层定义 supervision contract，热层承载 live retry / error / escalation state |
| Empty Core + Plugin Everything | Core 零业务逻辑, 所有功能由插件提供 |
| Single Source of Truth | Entrance 不仅是视图层, **是真实数据源**。外部服务 (OpenClaw 等) 读写 Entrance DB, 自身不持有数据 |
| 赛博洁癖 | TOML 配置, 单 SQLite, 无冗余文件 |
| 所有插件必须有 UI | "如果不能很方便使用 UI, 开发意义何在?" |
| 内容即界面 | 去 Chrome 化, 信息密度优先, 键盘优先, 即搜即得 |

## 安全模型

- **Entrance = 可信核心**, 跑系统服务 (不用 Docker)
- **外部 Agent (OpenClaw 等) = 非特权客户端**, 通过 MCP/HTTP 访问 Entrance, 无直接 DB 访问
- **Entrance 内部不跑 Agent** — Agent 权限太大, 必须在外部隔离运行

## 托管深度

| 层级 | 含义 | 示例 |
|------|------|------|
| 轻度 | 管理快捷方式 | Launcher |
| 中度 | 编排工作流 / 管理配置 | Vault, Connector |
| 重度 | 消化 API/数据 (替代 APP) | Forge, Board |

## 技术栈

| 维度 | 选择 |
|------|------|
| 语言 | Rust (后端) + TypeScript (前端) |
| 桌面框架 | Tauri 2.0 |
| 前端框架 | SolidJS (via Tauri webview) |
| 包管理 | pnpm |
| 项目结构 | Monorepo |
| 配置 | TOML only |
| 数据库 | 单 SQLite (entrance.db) — 所有数据的 SSOT |
| DB migration | crate (refinery / sqlx::migrate) |
| 插件机制 | V1 静态编译 (crate) → V2 WASM |
| 外部 API | MCP Server (AI 助手用) + HTTP REST (脚本/CI 用) |
| CI | 后续再定 |

## Core 职责

| 子系统 | 职责 |
|--------|------|
| PluginManager | 发现 / 加载 / 激活 / 停用 |
| EventBus | 插件间 pub/sub 通信, `{scope}:{action}` |
| DataStore | SQLite 抽象层, 每个插件独立表 |
| ConfigStore | TOML 读写 |
| PermissionGuard | 运行时权限校验 (L0~L4) |
| ActionCompiler | 将 `chat/learn/do` 与子角色动作原语编译为受约束 action records 和硬房间 |
| SupervisionKernel | 将 OTP supervision strategy 编译为 child policy、retry budget、failure visibility 和 escalation decision |
| WindowManager | Tauri 窗口生命周期 (多窗口) |
| LoggingSystem | 日志 |
| GlobalHotkeyManager | 全局快捷键注册 |
| ThemeSystem | 主题 / 样式 |
| **MCPServer** | 将所有插件能力统一暴露为 MCP tools, AI 助手专用通道 |
| AutoUpdate | 自动更新 |

## 插件架构 (5 插件)

| 插件 | 职责 | 托管深度 | Stage |
|------|------|----------|-------|
| **Launcher** | 全局快捷键呼出悬浮搜索栏, 模糊搜索, 即搜即启 | 轻度 | S0 |
| **Forge** | Agent 全生命周期: 启动/停止/状态/结果 + 管理 OpenClaw 实例 + 外部 MCP endpoint | 重度 | S1 |
| **Vault** | 统一凭证/配置: API tokens (加密) + MCP 配置 + Agent skills 注册表 | 中度 | S1 |
| **Board** | Kanban 看板, 与 Linear 同步 | 重度 | S2 |
| **Connector** | 连接外部服务: IM 镜像 (via OpenClaw), Obsidian, Zapier 式编排。All-in-one + Connect Everything | 中度 | S3 |

### V2 远期

| 插件 | 职责 |
|------|------|
| **Mesh** | 分布式注册中心 + 实例发现 + 跨实例调度 |
| **Seeker** | Raycast 式全屏搜索器 |
| **Sync** | Git 配置同步 |

## Connector 哲学 (原 Comm)

Connector ≠ 聊天插件。Connector 是 **Zapier-like 连接器**:

- 连接 OpenClaw (IM 桥接): Telegram/Slack 消息镜像到 Dashboard
- 连接 Obsidian: 笔记数据 + Obsidian CLI
- 连接任意外部服务: 通过 adapter 模式扩展
- 所有数据回流 entrance.db = 卸载外部服务后数据仍在

## 主界面

- Dashboard (主窗口, 常驻) + 侧边栏导航
- Launcher = 独立悬浮窗 (全局快捷键)
- 侧边栏切换 = 必做; 弹独立窗口 = V1 仅 Launcher

## 分布式 (V2)

- 中心化注册 + 离线回退
- headless 模式 (系统服务)
- 多实例跨调度

## 终极目标

> Entrance 成为 Duet 的执行层 — Human 只需看 Dashboard。
> NOTA 通过 MCP 连接 Entrance, 不再需要 Human 中转。
> 所有外部服务 (OpenClaw, Obsidian, Linear...) 的数据都在 Entrance 中, Entrance 是 SSOT。
