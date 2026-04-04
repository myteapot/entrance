# Entrance — Backend 架构 v2

> Arch 产出 | 含 MCP Server + Connector + SSOT

## 1. 项目结构

```
entrance/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── core/
│   │   │   ├── plugin_manager.rs
│   │   │   ├── event_bus.rs
│   │   │   ├── data_store.rs
│   │   │   ├── action.rs         # cold/hot dual-track + action primitives + hard rooms
│   │   │   ├── supervision.rs    # OTP-derived supervision strategy + retry budgets + visibility
│   │   │   ├── config_store.rs
│   │   │   ├── permission.rs
│   │   │   ├── window.rs
│   │   │   ├── logging.rs
│   │   │   ├── hotkey.rs
│   │   │   ├── theme.rs
│   │   │   ├── mcp_server.rs     ← NEW
│   │   │   └── updater.rs
│   │   ├── plugins/
│   │   │   ├── mod.rs            # Plugin trait
│   │   │   ├── launcher/
│   │   │   ├── forge/
│   │   │   ├── vault/
│   │   │   ├── board/
│   │   │   └── connector/       ← RENAMED from comm
│   │   └── commands/
│   ├── migrations/
│   └── tauri.conf.json
├── src/                          # SolidJS 前端
├── entrance.toml
├── oracles/
└── specs/
```

## 2. Plugin Trait (updated)

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn init(&self, ctx: &AppContext) -> Result<()>;
    fn on_event(&self, event: &Event) -> Result<()>;
    fn register_commands(&self) -> Vec<TauriCommand>;
    fn mcp_tools(&self) -> Vec<MCPToolDefinition>;  // NEW: 声明 MCP tools
    fn shutdown(&self) -> Result<()>;
}
```

## 3. MCPServer (Core 新增)

```rust
/// Core 能力, 非插件。聚合所有插件的 MCP tools 统一暴露。
pub struct MCPServer {
    transport: MCPTransport,  // stdio | HTTP+SSE
}

impl MCPServer {
    /// 启动时: 遍历所有插件的 mcp_tools() → 注册到 MCP server
    pub fn init(&self, plugins: &[Box<dyn Plugin>]);

    /// 暴露的 tools 示例:
    /// - vault.list_tokens, vault.get_skill
    /// - forge.run_agent, forge.task_status
    /// - board.list_issues, board.update_issue
    /// - connector.list_services, connector.send_message
    /// - launcher.search, launcher.launch
}
```

**与 Forge HTTP API 的关系:**
- MCP = AI 助手专用通道 (Gemini/Claude/Codex)
- HTTP = 脚本/CI/外部服务通用接口
- 两者共存, scope 不同

## 4. 安全模型

```
Entrance (system service, 可信)
    ↕ MCP/HTTP (受控 API, PermissionGuard)
外部 Agent/服务 (Docker/进程, 非特权)
    ↕ 各自协议
外部世界 (Telegram, Linear, etc.)
```

- Entrance 内部不运行 Agent
- Agent 权限在外部隔离, 通过 API 访问 Entrance 数据
- Entrance = SSOT, 外部服务 = 数据视图

## 4.1 第一指导原则

- `cold_hot_dual_track` 是第一指导原则
- cold layer 承载 canonical truth
- hot layer 承载 active surface / operational view
- 若一次编译动作同时触达 cold 与 hot，写入顺序必须是 `cold -> hot`
- 动作编译不得绕过硬房间与角色边界

## 4.2 Supervision Strategy

- supervision strategy 基于 Erlang / OTP 的原理，但平移到 Entrance 自身的 OS core
- core 的目标不是“尽快重试”，而是 `max_retry + report + no_silent_failure`
- cold layer 定义:
  - child type
  - supervision strategy
  - retry budget / window
  - escalation threshold
- hot layer 显示:
  - child runtime state
  - retry count
  - last error
  - degraded / blocked / escalated state
- recommended mapping:
  - isolated child => `one_for_one`
  - ordered dependency chain => `rest_for_one`
  - tightly coupled bundle => `one_for_all`
- Forge / Connector / future runtime workers 都应通过这层语义被建模，而不是各自私有 error handling

## 5. Core 其余设计

(EventBus, DataStore, ConfigStore, PermissionGuard 等同 v1, 见 data_schema.md)

## 6. 启动流程

```
1. Tauri main / system service 启动
2. Core 初始化 (ConfigStore → DataStore → Logging → Theme → EventBus → Hotkey → Window → Permission)
3. PluginManager.init_all()
4. MCPServer.init(plugins) ← NEW: 注册所有插件 MCP tools
5. 前端渲染 Dashboard
6. AutoUpdate 后台检查
```
