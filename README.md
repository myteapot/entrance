# Entrance

`Entrance` 已切到 V2 微内核重构：`core / plugins / shell`。仓库不再维护 `harness`、独立 CLI shell、独立 MCP shell 或任何 Tauri 代码。

> English: `Entrance` now uses a V2 microkernel layout: `core / plugins / shell`, with one Rust binary and one Electron GUI.

## 安装指南

### 方式 A：运行统一 binary

当前 Rust 入口只有一个：

- `entrance`：CLI + daemon + MCP

常用命令：

```bash
./target/release/entrance status
./target/release/entrance drawer list
./target/release/entrance drawer history
./target/release/entrance hive list
./target/release/entrance hive summary
./target/release/entrance launcher search code
./target/release/entrance launcher list
./target/release/entrance daemon
./target/release/entrance mcp http
```

### 方式 B：从源码构建

如果你希望自己构建：

1. 安装 Node.js、pnpm、Rust stable toolchain 和对应平台的桌面构建环境。
2. 安装前端依赖：

```bash
pnpm install
```

3. 构建前端资源：

```bash
pnpm build
```

4. 构建统一 Rust binary：

```bash
cargo build --workspace --release
```

5. 运行入口：

```bash
./target/release/entrance status
./target/release/entrance daemon
./target/release/entrance mcp stdio
pnpm dev:electron
```

> English: Build the frontend with `pnpm build`, then build the Rust workspace and run `entrance`.

## Runtime Operations

当前运行时主契约：

- `entrance`：唯一 Rust binary
- `entrance daemon`：Electron GUI backend
- `entrance mcp stdio`：stdio MCP
- `entrance mcp http`：HTTP MCP

常用命令：

```bash
./target/release/entrance status
./target/release/entrance drawer add-note --title "Plan" --body "V2 cutover"
./target/release/entrance drawer vault store --title "GitLab" --secret "token"
./target/release/entrance hive dispatch --title "Refactor pass"
./target/release/entrance hive review 1 approve
./target/release/entrance launcher refresh
./target/release/entrance daemon
./target/release/entrance mcp http
```

## Phase 4 Gate

最小验证顺序：

```bash
cargo check --workspace
cargo test --workspace
pnpm build
./target/release/entrance status
./target/release/entrance daemon
./target/release/entrance mcp stdio
```

## 版权与许可

当前仓库采用收紧的 source-available 许可模式。

- 默认代码许可见 [LICENSE](./LICENSE)
- 简要许可说明见 [LICENSES.md](./LICENSES.md)
- 名称与标识的使用边界见 [TRADEMARKS.md](./TRADEMARKS.md)
