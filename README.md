# Entrance

`Entrance` 当前公开姿态是 `V0 HEADLESS ALPHA`。它是一个以数据库为真相源的连续性运行时，也是一个 headless `NOTA` 宿主。

> English: `Entrance` is currently published as a `V0 HEADLESS ALPHA`: a DB-first continuity runtime and headless `NOTA` host.

## 安装指南

### 方式 A：直接使用发布二进制

推荐优先使用发布页提供的 Windows zip。

1. 从发布页下载 `v0.3.1-headless-alpha.1`。
2. 解压后运行 `entrance.exe`。
3. 在终端中先读取运行时状态：

```powershell
.\entrance.exe nota status
.\entrance.exe nota overview
.\entrance.exe nota checkpoints
```

> English: Preferred path: download the Windows zip release, unzip it, run `entrance.exe`, and start by reading runtime state through `nota status`, `nota overview`, and `nota checkpoints`.

### 方式 B：从源码构建

如果你希望自己构建：

1. 安装 Node.js、pnpm、Rust stable toolchain 和 Windows C++ build environment。
2. 安装前端依赖：

```powershell
pnpm install --frozen-lockfile
```

3. 构建前端资源：

```powershell
pnpm build
```

4. 构建 Windows 二进制：

```powershell
cargo build --manifest-path src-tauri/Cargo.toml --release
```

5. 读取运行时：

```powershell
.\src-tauri\target\release\entrance.exe nota status
.\src-tauri\target\release\entrance.exe nota overview
```

> English: Source builds are supported. Install Node.js, pnpm, Rust, and the Windows build toolchain, then run `pnpm install`, `pnpm build`, and `cargo build --manifest-path src-tauri/Cargo.toml --release`.

## 版权与许可

当前仓库采用收紧的 source-available 许可模式。

- 默认代码许可见 [LICENSE](./LICENSE)
- 简要许可说明见 [LICENSES.md](./LICENSES.md)
- 名称与标识的使用边界见 [TRADEMARKS.md](./TRADEMARKS.md)

> English: The repository uses a tight source-available license model. See `LICENSE`, `LICENSES.md`, and `TRADEMARKS.md` for the current boundaries.
