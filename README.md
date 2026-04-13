# Entrance

`Entrance` 当前公开姿态是 `V1 RELEASE CANDIDATE`。仓库现已收敛为 Rust workspace，包含 `core`、`harness`、`shell/cli`、`shell/gui`、`shell/mcp` 四类最终形态入口。

> English: `Entrance` is currently published as a `V1 RELEASE CANDIDATE`, and the repository now ships as a Rust workspace with dedicated CLI, GUI, desktop bridge, and MCP shells.

## 安装指南

### 方式 A：直接使用发布二进制

推荐优先使用发布页提供的 Windows zip。当前发布包会按构建结果携带这些二进制：

- `entrance-gui.exe`：桌面 GUI
- `entrance.exe`：CLI
- `entrance-mcp.exe`：MCP shell
- `entrance-desktop-bridge.exe`：Electron 桥接 sidecar

启动桌面 GUI：

```powershell
.\entrance-gui.exe
```

读取运行时状态：

```powershell
.\entrance.exe nota status
.\entrance.exe nota overview
.\entrance.exe nota checkpoints
```

### 方式 B：从源码构建

如果你希望自己构建：

1. 安装 Node.js、pnpm、Rust stable toolchain 和对应平台的桌面构建环境。
2. 安装前端依赖：

```powershell
pnpm install --frozen-lockfile
```

3. 构建前端资源：

```powershell
pnpm build
```

4. 构建全部 Rust 二进制：

```powershell
cargo build --workspace --release
```

5. 运行入口：

```powershell
.\target\release\entrance-gui.exe
.\target\release\entrance.exe nota status
.\target\release\entrance-mcp.exe stdio
```

> English: Source builds are supported. Install Node.js, pnpm, Rust, and the desktop build toolchain, then run `pnpm install`, `pnpm build`, and `cargo build --workspace --release`.

## Runtime Operations

当前运行时主契约：

- `entrance`：纯 CLI，不再无参启动 GUI
- `entrance-gui`：Tauri 桌面 GUI
- `entrance-desktop-bridge`：Electron 使用的独立桌面桥接二进制
- `entrance-mcp`：MCP shell，支持 `stdio` 与 `http`

常用命令：

```powershell
.\target\release\entrance.exe nota status
.\target\release\entrance.exe nota overview
.\target\release\entrance.exe nota invariants
.\target\release\entrance.exe nota repair
.\target\release\entrance.exe nota rebuild-projections --project-dir <repo>
.\target\release\entrance.exe recovery status
.\target\release\entrance-mcp.exe http --port 9720 --endpoint /mcp
```

## Release Gate

发布级自洽与双端验证入口：

```bash
./shell/gui/release/verify-v1-self-consistency.sh
```

```powershell
./shell/gui/release/verify-v1-self-consistency.ps1
```

对应检查清单见：`notes/human/release-checklist.md`。

双端补充验证命令：

```bash
pnpm test:electron-smoke
```

```powershell
./shell/gui/release/run-windows-native-smoke.ps1 -Configuration Release
```

## 版权与许可

当前仓库采用收紧的 source-available 许可模式。

- 默认代码许可见 [LICENSE](./LICENSE)
- 简要许可说明见 [LICENSES.md](./LICENSES.md)
- 名称与标识的使用边界见 [TRADEMARKS.md](./TRADEMARKS.md)
