# Entrance 目标架构

> Status: 确认的方向，尚未执行
> Date: 2026-04-13

## 一句话

Entrance = core（Linux kernel）+ harness（NixOS rebuild）+ shell（CLI/GUI/MCP）。

## 仓库结构

```
entrance/
  core/              ← lib crate，OS 内核，纯运行时逻辑，零宿主假设
  harness/           ← lib crate，bootloader + 声明式插件装配
  shell/
    cli/             ← bin crate，命令行 shell
    gui/             ← Tauri/Electron bin + SolidJS 前端，图形 shell
    mcp/             ← bin/lib，AI agent 的 shell
  notes/             ← 脚手架，项目交流用（非产品代码）
```

不需要更多顶层目录。

## 核心原则

### core = Linux kernel

- 一组运行时工具集，本质上是一个 Rust lib crate
- 不知道自己被 CLI、GUI 还是 MCP 调用
- 不做 IO 假设（不自己读 TOML、不自己打开 DB）
- 所有外部依赖通过构造时注入
- `pub` API 边界就是 contract——不需要单独的 contract 目录

### harness = NixOS rebuild

- 读 `~/.entrance/entrance.toml`（声明式配置清单）
- 按声明组装 core 实例：开 DB、注册启用的插件、配置日志
- 插件（Forge/Launcher/Vault/Board/Connector）在 harness 内管理
- 把组装好的 core 交给调用者，自己不持有运行时状态

### shell = 用户界面

- CLI：最薄的 shell，`harness::boot()` → `core.execute(args)` → stdout
- GUI：Tauri/Electron bin，通过 IPC 暴露 core 给 SolidJS 前端
- MCP：AI agent 专用 shell，通过 HTTP/stdio 暴露 core
- 所有 shell 链接同一个 core lib crate，不走 subprocess

### notes = 临时脚手架

- 给人看的（human/）、给 AI 看的（agents/）、归档的（archive/）
- 随着 core 的 data_store + checkpoint 机制成熟，交流内容会被吸收进 DB
- 最终会缩小或自然消亡

## 声明式配置

```toml
# ~/.entrance/entrance.toml

[core]
db_path = "~/.entrance/data/entrance.db"
log_level = "info"

[plugins]
launcher = { enabled = true }
forge = { enabled = true, http_port = 9315 }
vault = { enabled = true }
board = { enabled = false }
connector = { enabled = false }

[shell.cli]
default_output = "json"

[shell.gui]
theme = "dark"
global_hotkey = "Alt+Space"

[shell.mcp]
mode = "stdio"
```

## 与现状的差距

现在 core 被困在 `hosts/desktop/tauri/src/core/`，
harness 逻辑散落在 `core/mod.rs` 和 `lib.rs`，
shell 混在 `src/surfaces/` 和 `surfaces/gui/`。

重构路径（大致）：

1. 把 `hosts/desktop/tauri/src/core/` 提取为独立 workspace crate `core/`
2. 把 bootstrap 逻辑从 `core/mod.rs` + `lib.rs` 提取到 `harness/`
3. 把 `src/surfaces/cli/` 提取为 `shell/cli/`
4. 把 `surfaces/gui/` 移到 `shell/gui/`
5. 把 MCP 相关代码提取为 `shell/mcp/`
6. 把 `hosts/desktop/tauri/` 重构为 `shell/gui/` 的 Tauri 后端部分
7. 删除层级冲突的 `hosts/` 和 `surfaces/` 目录
