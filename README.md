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

**V2 Microkernel Preview** — CLI 和 daemon bridge 可用，Electron GUI 迁移中；Agent connector / MCP surface 仍在整理中。

---

## 许可 / License

[Business Source License 1.1](./LICENSE) · [详情 LICENSES.md](./LICENSES.md) · [商标 TRADEMARKS.md](./TRADEMARKS.md)
