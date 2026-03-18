# Entrance — Milestones + Roadmap v2

> 含 MCPServer (S1), Connector (S3)

## V1

### S0: 骨架 + Launcher (~15h, 9 issues)

MYT-19~27 (全部 Todo)

验收: 全局快捷键 → 搜索 → 启动应用

### S1: Forge + Vault + MCP Server (~22h, 11 issues)

含 MYT-19~27 之后的新 issue (待 S0 完成后创建):
- Forge: Agent 任务 CRUD + 子进程管理 + HTTP API
- Vault: Token/MCP/Skill 管理 + 加密
- **MCPServer**: Core 初始化 MCP server, 注册所有插件 tools
- Dashboard widgets

验收: Forge 调度 Agent + Vault 管 tokens + 外部 AI 通过 MCP 连接

### S2: Board (~12h, 5 issues)

Linear 集成 + Kanban UI

### S3: Connector (~12h, 5 issues)

- Connector 框架 (adapter 模式)
- OpenClaw adapter (IM 桥接)
- Obsidian adapter (笔记 + CLI)
- 消息/数据镜像到 entrance.db

验收: OpenClaw 消息在 Dashboard 可见, Obsidian 笔记可搜索

## V2 远景

| 插件 | 方向 |
|------|------|
| Mesh | 分布式实例 + 跨调度 |
| Seeker | Raycast 全屏搜索 |
| Sync | Git 配置同步 |
| WASM | 插件动态加载 |
| Zapier | Connector 内的编排引擎 |
| **Duet 迁移** | **control.py / db.py → Entrance MCP tools。Entrance 成为 Duet 唯一基础设施，AI 通过 MCP 读写 skills/specs/config/worktree，不再需要本地脚本** |

## 总估算

| Stage | Issues | 工时 |
|-------|--------|------|
| S0 | 9 | ~15h |
| S1 | 11 | ~22h |
| S2 | 5 | ~12h |
| S3 | 5 | ~12h |
| **V1** | **30** | **~61h** |
