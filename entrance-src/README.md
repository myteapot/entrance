# Entrance

**Local control plane for coding automation.**

*One Rust binary for durable notes, task ledgers, local secrets, launchers, and a desktop bridge.*

> Entrance keeps project-side state close to the machine:
> persistent notes, a small task ledger, local AES-GCM vault records, app indexing, and a GUI bridge over one runtime.

当前版本是 **V2 Microkernel Preview**：一个 `entrance` 程序提供 CLI 和后台 daemon；桌面端使用 Electron + SolidJS，并通过同一个 daemon 协议调用 Rust runtime。

---

## 一图看懂 / Architecture

![Entrance Architecture](./docs/entrance_architecture.png)

---

## Runtime Surfaces

### Drawer

Durable local storage for notes, imported files, vault records, and version snapshots.

```powershell
.\entrance.exe drawer memory import --title "登录页重构进度" --body "auth middleware 已修，下一步补集成测试"
.\entrance.exe drawer list
.\entrance.exe drawer history
```

```powershell
.\entrance.exe drawer vault store --title "OpenAI" --secret "sk-..."
.\entrance.exe drawer vault list
```

### Hive

Task ledger for dispatch records, engine reports, callbacks, and review state.

```powershell
.\entrance.exe hive dispatch --title "修复登录页 500 错误"
.\entrance.exe hive summary
.\entrance.exe hive engine 1
.\entrance.exe hive review 1 approve
```

Local agent-loop MVP:

```powershell
.\entrance.exe hive loop create --title "README loop" --goal "Run a constrained agent loop" --runtime codex
.\entrance.exe hive loop run 1 --runtime codex
.\entrance.exe hive loop show 1
.\entrance.exe hive issue list
.\entrance.exe hive issue comment 1 --body "Reviewed from the local panel"
```

`hive loop run` records a minimal compiler path in SQLite: active policies,
typed packets, admission receipts, stage evidence, and the final verdict.
Supported MVP runtimes are `local` and `codex`; unknown runtimes return a
blocked verdict instead of being silently kept.

### Launcher

Local application index and launch surface.

```powershell
.\entrance.exe launcher refresh
.\entrance.exe launcher search code
.\entrance.exe launcher list
```

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
.\target\release\entrance.exe daemon http
```

### 启动桌面端

```powershell
pnpm dev:electron
```

---

## Plugin Status

| Surface | Responsibility | Status |
|---|---|---|
| **Drawer** | Durable storage: notes, imports, vault, snapshots | ✅ |
| **Hive** | Task ledger: dispatch, engine reports, callbacks, review | ✅ |
| **Launcher** | Local app index and launch surface | ✅ |

---

## 技术栈 / Tech Stack

Rust · Electron · SolidJS · SQLite · TOML

---

## 当前阶段 / Status

**V2 Microkernel Preview** — CLI、daemon bridge 和 Electron GUI 共用同一套 Rust runtime。当前没有独立 MCP server；外部集成应先走 daemon stdio/http invoke 协议。

---

## 许可 / License

[Business Source License 1.1](./LICENSE) · [详情 LICENSES.md](./LICENSES.md) · [商标 TRADEMARKS.md](./TRADEMARKS.md)
