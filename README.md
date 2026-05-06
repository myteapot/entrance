# Entrance

**你的 AI 编程助手的「操作系统」。**

*The "operating system" for your AI coding agents.*

> 如果 Codex CLI 是一个干活的工人，Entrance 就是他的工具箱 + 记忆宫殿 + 保险柜。
> 工人下班了再上班，打开 Entrance，上次做到哪、密钥在哪、下一步该干嘛 —— 全都还在。

---

## 一图看懂 / Architecture

![Entrance Architecture](./docs/entrance_architecture.png)

---

## 装完能干嘛？三个真实场景 / Real Examples

### 场景 1：给 Codex CLI 装上「记忆」

你用 Codex CLI 重构了一半代码，关掉终端。第二天打开，Codex 什么都不记得了。

用 Entrance：

```powershell
# Entrance 作为 MCP server 启动，Codex CLI 连上它
.\entrance.exe mcp stdio

# Codex 现在能读到昨天的进度、决策记录、待办事项
# 不用你再复述一遍 "昨天我们改到哪了"
```

*Codex CLI forgets everything after you close the terminal. Entrance gives it persistent memory via MCP.*

### 场景 2：不再到处翻 API Key

OpenAI key 在 `.env`，Anthropic key 在另一个 `.env`，Linear token 在浏览器里……

用 Entrance：

```powershell
# 所有 key 加密存在一个地方，agent 按需取用
.\entrance.exe drawer vault store --title "OpenAI" --secret "sk-..."
.\entrance.exe drawer vault list
```

*All API keys encrypted in one place. Agents fetch them on demand through Vault.*

### 场景 3：一条命令派活、全程监管

你想让 agent 去修一个 bug，但想知道它在干嘛、干完没、结果怎么样。

```powershell
# 派发任务
.\entrance.exe hive dispatch --title "修复登录页 500 错误"

# 查看进度
.\entrance.exe hive summary

# 验收完毕
.\entrance.exe hive review 1 approve
```

*Dispatch a task, monitor progress, and review the result from the CLI.*

---

## 快速开始 / Quick Start

### 下载即用

1. 从 [Releases](https://github.com/myteapot/Entrance/releases) 下载最新版本
2. 解压，运行 `entrance.exe`
3. 试一下：`.\entrance.exe status`

### 接入 AI Agent

```powershell
# 让 Codex CLI / Claude Code 通过 MCP 连接 Entrance
.\entrance.exe mcp stdio

# 或者用 HTTP（适合脚本和 CI）
.\entrance.exe mcp http
```

### 从源码构建

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo build --workspace --release
```

---

## 三个插件，各管一摊 / Plugins

| 插件 Plugin | 类比 Analogy | 状态 |
|---|---|---|
| **Drawer** | 记忆抽屉 + 保险柜 —— 笔记、文件、密钥、快照 | ✅ |
| **Hive** | 工头 —— 派活、盯梢、验收 | ✅ |
| **Launcher** | Spotlight / Raycast —— 本地启动项搜索 | ✅ |

---

## 技术栈 / Tech Stack

Rust · Electron · SolidJS · SQLite · TOML · MCP

---

## 当前阶段 / Status

**V2 Microkernel Preview** — CLI、daemon、MCP Server 可用，Electron GUI 迁移中。

---

## 许可 / License

[Business Source License 1.1](./LICENSE) · [详情 LICENSES.md](./LICENSES.md) · [商标 TRADEMARKS.md](./TRADEMARKS.md)
