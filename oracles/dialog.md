# Entrance — Dialog

> Human 原话逐字记录。Arch 不改写, 只归类。

## Session 1 — 2026-03-18 (冷启动 + 架构演进)

### Round 1-5: Phase 1 完整访谈 (17 问)

详见 oracle.md 定稿。所有决策已确认。

### Round 6: 架构演进 (MCP + SSOT + Connector)

**Entrance MCP Server:**
> 如果 skill 和 specs 都让 Entrance 管理, 使用 MCP 方式注入会好。Entrance 抛出 MCP, 在 Antigravity 中继续聊天, 使用 Entrance 所有能力。不再需要 patch review。

**NOTA 身份 vs Arch 角色:**
> NOTA 是身份, Arch 是工作流角色。你有时候会混淆吗?

**Entrance = Single Source of Truth:**
> OpenClaw 的所有数据包括启停都在 Entrance 管理, 读取 Entrance DB。它管理的只是数据的视图, 数据本身由 Entrance 管理。卸了 Docker 数据仍在 Entrance 里。

**安全模型:**
> 不会用 Docker 管理 Entrance, 也不会让 OpenClaw 有 Entrance 这么高的权限。Entrance 始终是静态的, 里面不应该有 agent, agent 权限太大。Entrance 跑系统服务。

**Comm → Connector:**
> 可能叫 connector 更合适。不一定只管 OpenClaw, 还要管别的。各种配置、服务都想放进来。Obsidian 数据也在里面, Obsidian CLI 也通过 Entrance 访问。所有东西 connect 在一块, 类似 Zapier 思想 — all in one + connector。Zapier 本身也只是 Entrance 中的一个 module。

**S3 确认:**
> OpenClaw 集成放 S3。
