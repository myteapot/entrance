# Entrance

**Local control plane for coding automation.**

*One Rust binary for durable state, task ledgers, secrets, launchers, and a desktop bridge.*

> Entrance keeps project-side state close to the machine:
> persistent notes, a small task ledger, encrypted secrets, app indexing, and a GUI bridge over one runtime.

当前版本是 **V2 Microkernel Preview**：一个 `entrance` 程序提供 CLI 和后台 daemon；桌面端正在迁移到 Electron + SolidJS。Agent connector / MCP surface 仍在整理中。

---

## 一图看懂 / Architecture

![Entrance Architecture](./docs/entrance_architecture.png)

---

## 装完能干嘛？三个本地工作流 / Local Workflows

### 场景 1：把上下文落盘

重构做到一半，先把当前判断、剩余工作、踩过的坑写进 Drawer。下次继续时不用翻聊天记录。

用 Entrance：

```powershell
.\entrance.exe drawer memory import --title "登录页重构进度" --body "auth middleware 已修，下一步补集成测试"
.\entrance.exe drawer list
.\entrance.exe drawer history
```

*Durable notes and snapshots for long-running local work.*

### 场景 2：本地 secret vault

OpenAI key 在 `.env`，Anthropic key 在另一个 `.env`，Linear token 在浏览器里……

用 Entrance：

```powershell
# 所有 key 加密存在一个地方，agent 按需取用
.\entrance.exe drawer vault store --title "OpenAI" --secret "sk-..."
.\entrance.exe drawer vault list
```

*Encrypted local secrets without spreading tokens across project folders.*

### 场景 3：任务账本与验收回路

把一次修复、重构、实验记录成可查询的 run，保留状态、engine report 和 review 决策。

```powershell
# 派发任务
.\entrance.exe hive dispatch --title "修复登录页 500 错误"

# 查看任务账本
.\entrance.exe hive summary
.\entrance.exe hive engine 1

# 验收完毕
.\entrance.exe hive review 1 approve
```

*A small task ledger for dispatch, callbacks, and review state.*

---

## 快速开始 / Quick Start

### 当前推荐：从源码试用

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo build --workspace --release

.\target\release\entrance.exe status
```

### CLI smoke

```powershell
.\target\release\entrance.exe drawer add-note --title "Plan" --body "Ship README"
.\target\release\entrance.exe hive dispatch --title "Refactor pass"
.\target\release\entrance.exe launcher refresh
```

### 启动桌面端

```powershell
pnpm dev:electron
```

---

## 三个插件，各管一摊 / Plugins

| 插件 Plugin | 类比 Analogy | 状态 |
|---|---|---|
| **Drawer** | Durable storage: notes, imports, vault, snapshots | ✅ |
| **Hive** | Task ledger: dispatch, engine reports, callbacks, review | ✅ |
| **Launcher** | Local app index and launch surface | ✅ |

---

## 技术栈 / Tech Stack

Rust · Electron · SolidJS · SQLite · TOML

---

## 当前阶段 / Status

**V2 Microkernel Preview** — CLI 和 daemon bridge 可用，Electron GUI 迁移中；Agent connector / MCP surface 仍在整理中。

---

## 许可 / License

[Business Source License 1.1](./LICENSE) · [详情 LICENSES.md](./LICENSES.md) · [商标 TRADEMARKS.md](./TRADEMARKS.md)
