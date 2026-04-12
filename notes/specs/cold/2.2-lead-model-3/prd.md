# Entrance — PRD (产品需求文档) v2

> Arch 产出 | Phase 2 | 含 MCP Server + Connector 演进

## 产品定位

**Entrance** = 插件化桌面统一入口 + **数据真相源 (SSOT)**。

不仅是 GUI 入口, 更是所有工具/服务/配置的唯一数据源。外部服务 (OpenClaw, Obsidian 等) 连接 Entrance 读写数据, 卸载外部服务后数据仍在。

## 功能矩阵

### Core
PluginManager, EventBus, DataStore, ConfigStore, PermissionGuard, WindowManager, LoggingSystem, GlobalHotkeyManager, ThemeSystem, **MCPServer**, AutoUpdate

### 插件

| 插件 | 用户故事 | Stage |
|------|----------|-------|
| **Launcher** | 按快捷键 → 搜索 → 启动 | S0 |
| **Forge** | 提交 Agent 任务, 看实时状态, 外部可通过 HTTP/MCP 调用 | S1 |
| **Vault** | 所有 API token / MCP 配置 / Agent skill 统一管理 | S1 |
| **Board** | Linear 看板直接在 Entrance 里 | S2 |
| **Connector** | 连接外部服务 (OpenClaw/Obsidian/Zapier), 数据回流 Entrance | S3 |

### V2: Mesh, Seeker, Sync
