# Entrance 仓库重构执行清单

> 目标：把当前单 crate 架构拆分为 core / harness / shell 三层
> 参考：`notes/human/architecture-target.md`
> 状态：待执行

## 当前结构 → 目标结构

```
当前:                                    目标:
entrance/                                entrance/
  hosts/desktop/tauri/                     core/           ← lib crate
    src/core/           →→→                  Cargo.toml
    src/hosts/plugins/  →→→                  src/...
    src/hosts/desktop/  →→→                harness/         ← lib crate
    src/surfaces/       →→→                  Cargo.toml
    src/lib.rs          →→→                  src/...
    src/main.rs         →→→                shell/
    Cargo.toml          →→→                  cli/           ← bin crate
  surfaces/gui/         →→→                  gui/           ← Tauri bin + SolidJS
  surfaces/contracts/   →→→                  mcp/           ← bin/lib crate
                                           notes/
                                           Cargo.toml       ← workspace root
```

## 前置知识

- 当前只有一个 Rust crate：`hosts/desktop/tauri/`（名为 `entrance`）
- 它同时产出 `lib`（`entrance_lib`）和 `bin`（`entrance`）
- 依赖：Tauri 2、rusqlite (bundled)、axum、tokio、serde、tracing
- 前端是 SolidJS，通过 Vite 6 构建，位于 `surfaces/gui/`
- Electron 宿主只有 3 个 mjs，位于 `hosts/desktop/electron/`
- 浏览器 E2E 测试位于 `hosts/desktop/browser/`

---

## 阶段 0：准备工作

- [ ] 0.1 确保 `main` 分支干净，创建重构分支
  ```bash
  git checkout -b refactor/core-harness-shell
  ```
- [ ] 0.2 记录当前通过的测试作为基线
  ```bash
  cargo test --manifest-path hosts/desktop/tauri/Cargo.toml --lib 2>&1 | tail -5
  pnpm check
  ```

---

## 阶段 1：创建 workspace 和 core crate

### 1.1 创建根 Cargo.toml workspace

- [ ] 在仓库根创建 `Cargo.toml`：
  ```toml
  [workspace]
  resolver = "2"
  members = [
      "core",
      "harness",
      "shell/cli",
      "shell/gui",
  ]
  
  [workspace.package]
  version = "1.0.0-rc.1"
  edition = "2021"
  license-file = "LICENSE"
  
  [workspace.dependencies]
  anyhow = "1"
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  tracing = "0.1"
  rusqlite = { version = "0.39", features = ["bundled"] }
  chrono = { version = "0.4", features = ["clock", "serde"] }
  tokio = { version = "1.50.0", features = ["io-std", "io-util", "net", "process", "sync"] }
  ```

### 1.2 创建 `core/` crate

- [ ] 创建 `core/Cargo.toml`：
  ```toml
  [package]
  name = "entrance-core"
  version.workspace = true
  edition.workspace = true
  
  [dependencies]
  anyhow.workspace = true
  serde.workspace = true
  serde_json.workspace = true
  tracing.workspace = true
  rusqlite.workspace = true
  chrono.workspace = true
  tokio.workspace = true
  dirs = "5"
  toml = "0.8"
  rand = "0.9"
  uuid = { version = "1", features = ["v4"] }
  ```

### 1.3 移动 core 源文件

- [ ] 将以下文件从 `hosts/desktop/tauri/src/core/` 移到 `core/src/`：
  ```
  core/src/
    lib.rs              ← 新文件，将当前 core/mod.rs 的 pub mod 声明提取到这里
    action.rs
    anti_zeno_runtime.rs
    bootstrap_mcp_cycle.rs
    chat_archive.rs
    cold_docs_runtime.rs
    compiler/           ← 整个目录移过来
      mod.rs
      admission.rs
      evidence.rs
      lowering.rs
      packet.rs
      registry.rs
      routing.rs
      semantics.rs
    config_store.rs
    data_store.rs
    design_governance.rs
    environment_runtime.rs
    event_bus.rs
    front_door.rs
    graph_events.rs
    hygiene.rs
    invariant_runtime.rs
    landing.rs
    memory_import.rs
    nota/               ← 整个目录移过来
      mod.rs
      helpers.rs
      policy.rs
      types.rs
    overview.rs
    parallel_budget.rs
    permission.rs
    projection_runtime.rs
    recovery.rs
    supervision.rs
    system_heartbeat.rs
  ```

- [ ] 创建 `core/src/lib.rs`，内容从当前 `core/mod.rs` 提取 pub mod 声明：
  ```rust
  pub mod action;
  pub mod anti_zeno_runtime;
  pub mod bootstrap_mcp_cycle;
  pub mod chat_archive;
  pub mod cold_docs_runtime;
  pub mod compiler;
  pub mod config_store;
  pub mod data_store;
  pub mod design_governance;
  pub mod environment_runtime;
  pub mod event_bus;
  pub mod front_door;
  pub mod graph_events;
  pub mod hygiene;
  pub mod invariant_runtime;
  pub mod landing;
  pub mod memory_import;
  pub mod nota;
  pub use nota as nota_runtime;
  pub mod overview;
  pub mod parallel_budget;
  pub mod permission;
  pub mod projection_runtime;
  pub mod recovery;
  pub mod supervision;
  pub mod system_heartbeat;
  ```

- [ ] 注意：`core/src/data_store.rs` 中的 `migrations/` SQL 文件引用需要更新路径。当前使用 `include_str!("../../../migrations/xxx.sql")`，需要把 migration SQL 文件也移到 `core/` 下或者调整引用路径。

### 1.4 处理 migration SQL 文件

- [ ] 将 `hosts/desktop/tauri/migrations/` 移到 `core/migrations/`：
  ```bash
  git mv hosts/desktop/tauri/migrations core/migrations
  ```
  注意：Forge 插件也有自己的 migration 文件（0002, 0004, 0006），这些应该留在 harness 或跟着插件走。
  
  **推荐做法**：
  - `core/migrations/` 放 core 的 migrations (0001, 0003, 0005, 0007+)
  - `harness/migrations/` 放插件的 migrations (0002, 0004, 0006)
  - 在代码中调整 `include_str!` 路径

### 1.5 处理 core 对 Tauri 的依赖

> [!CAUTION]
> 当前 `core/` 代码中有对 Tauri 的隐式依赖需要消除。

- [ ] 搜索 core 文件中的 `tauri::` 引用：
  ```bash
  grep -rn 'tauri::' core/src/ --include='*.rs'
  ```
  在 `nota/mod.rs` 中，`run_nota_dispatch` 使用了 `tauri::async_runtime::spawn_blocking`。这些需要替换为纯 `tokio` 调用（`tokio::task::spawn_blocking`）。

- [ ] 搜索 `crate::hosts::` 和 `crate::surfaces::` 引用：
  ```bash
  grep -rn 'crate::hosts\|crate::surfaces' core/src/ --include='*.rs'
  ```
  core 不应该引用 hosts 或 surfaces。如果存在，需要重构为通过 trait 或回调注入。

- [ ] 消除 `graph_events.rs` 中的 Tauri emitter 依赖。当前它通过全局静态直接 emit Tauri 事件。需要改为通过 `EventBus` 或回调 trait。

---

## 阶段 2：创建 harness crate

### 2.1 创建 `harness/Cargo.toml`

- [ ] ```toml
  [package]
  name = "entrance-harness"
  version.workspace = true
  edition.workspace = true
  
  [dependencies]
  entrance-core = { path = "../core" }
  anyhow.workspace = true
  serde.workspace = true
  tracing.workspace = true
  ```

### 2.2 移动 harness 源文件

- [ ] 从当前 `core/mod.rs` 提取到 `harness/src/lib.rs`：
  - `AppPaths` 结构体及其 impl
  - `resolve_app_data_dir()`
  - `StartupState` 结构体及其 impl
  - `bootstrap_for_paths()`
  - `resolve_runtime_paths()`
  - `enabled_plugin_migrations()`
  - `resolve_owned_relative_path()`
  - `migrate_legacy_runtime_db()`

- [ ] 将插件系统移入 harness：
  ```
  harness/src/
    lib.rs              ← bootstrap 逻辑
    plugins/
      mod.rs            ← Plugin trait, AppContext, Manifest（来自 hosts/plugins/mod.rs）
      forge/            ← 来自 hosts/plugins/forge/
        mod.rs
        commands.rs
        engine.rs
        http.rs
      launcher/         ← 来自 hosts/plugins/launcher/
        mod.rs
        scanner.rs
        search.rs
      vault/            ← 来自 hosts/plugins/vault/
        mod.rs
        commands.rs
        crypto.rs
  ```

### 2.3 harness 对 core 的依赖方向

```
harness 依赖 core（正确）
core 不依赖 harness（正确）
shell 依赖 harness + core（正确）
```

---

## 阶段 3：创建 shell crates

### 3.1 `shell/cli/` — CLI shell

- [ ] 创建 `shell/cli/Cargo.toml`：
  ```toml
  [package]
  name = "entrance"
  version.workspace = true
  edition.workspace = true
  
  [[bin]]
  name = "entrance"
  path = "src/main.rs"
  
  [dependencies]
  entrance-core = { path = "../../core" }
  entrance-harness = { path = "../../harness" }
  anyhow.workspace = true
  serde_json.workspace = true
  tracing.workspace = true
  ```

- [ ] 移动文件：
  ```
  shell/cli/src/
    main.rs             ← 来自 hosts/desktop/tauri/src/main.rs（仅 CLI 部分）
    mod.rs
    forge_cli.rs        ← 来自 surfaces/cli/forge_cli.rs
    nota_cli.rs         ← 来自 surfaces/cli/nota_cli.rs
    issues_cli.rs
    memory_cli.rs
    compiler_cli.rs
    mcp_cli.rs
  ```

- [ ] main.rs 逻辑简化为：
  ```rust
  fn main() {
      let startup = entrance_harness::bootstrap()?;
      entrance_cli::dispatch(startup, std::env::args());
  }
  ```

### 3.2 `shell/gui/` — GUI shell (Tauri + SolidJS)

- [ ] 创建 `shell/gui/Cargo.toml`：
  ```toml
  [package]
  name = "entrance-gui"
  version.workspace = true
  edition.workspace = true
  
  [[bin]]
  name = "entrance-gui"
  path = "src/main.rs"
  
  [lib]
  name = "entrance_gui_lib"
  crate-type = ["staticlib", "cdylib", "rlib"]
  
  [dependencies]
  entrance-core = { path = "../../core" }
  entrance-harness = { path = "../../harness" }
  tauri = { version = "2", features = [] }
  tauri-plugin-dialog = "2"
  tauri-plugin-process = "2"
  tauri-plugin-opener = "2"
  tauri-plugin-global-shortcut = "2"
  # ... 其他 Tauri 依赖
  ```

- [ ] 移动 Tauri 专属代码：
  ```
  shell/gui/src/
    main.rs             ← 来自 hosts/desktop/tauri/src/main.rs（GUI 入口）
    lib.rs              ← 来自 hosts/desktop/tauri/src/lib.rs（setup_application + run_tauri_app）
    desktop/
      hotkey.rs         ← 来自 hosts/desktop/
      instance_manager.rs
      logging.rs
      plugin_manager.rs
      theme.rs
      updater.rs
      window.rs
    tauri_commands/      ← 来自 surfaces/tauri/
      mod.rs
      issues.rs
      nota_prayer.rs
  ```

- [ ] 移动前端文件：
  ```
  shell/gui/
    renderer/           ← 来自 surfaces/gui/renderer/（整个目录）
    contracts/          ← 来自 surfaces/contracts/desktop/
    vite.config.ts      ← 来自 surfaces/gui/vite.config.ts
    public/             ← 来自 surfaces/gui/public/
  ```

- [ ] 移动 Tauri 配置文件：
  ```bash
  git mv hosts/desktop/tauri/tauri.conf.json shell/gui/tauri.conf.json
  git mv hosts/desktop/tauri/capabilities shell/gui/capabilities
  git mv hosts/desktop/tauri/icons shell/gui/icons
  git mv hosts/desktop/tauri/gen shell/gui/gen
  git mv hosts/desktop/tauri/build.rs shell/gui/build.rs
  ```

- [ ] Electron 宿主文件：
  ```bash
  git mv hosts/desktop/electron shell/gui/electron
  ```

### 3.3 `shell/mcp/` — MCP shell

- [ ] 创建 `shell/mcp/Cargo.toml`（可以先作为 lib crate 嵌入 cli）
- [ ] 移动文件：
  ```
  shell/mcp/src/
    lib.rs
    server.rs          ← 来自 surfaces/mcp_server.rs
    stdio_client.rs    ← 来自 surfaces/mcp_stdio_client.rs
  ```
- [ ] 或者暂时保留在 `shell/cli/` 内作为子模块，等独立需求明确后再拆。

---

## 阶段 4：更新前端构建配置

- [ ] 4.1 更新 `package.json` 中的路径：
  ```json
  "scripts": {
    "dev": "vite --config shell/gui/vite.config.ts",
    "build": "pnpm check && vite build --config shell/gui/vite.config.ts",
    "tauri": "tauri -c shell/gui/tauri.conf.json",
    "test:e2e": "npx playwright test -c shell/gui/browser/playwright.config.ts"
  }
  ```

- [ ] 4.2 更新 `vite.config.ts` 中的 `root` 路径

- [ ] 4.3 更新 `tauri.conf.json` 中的 `frontendDist` 和其他路径

- [ ] 4.4 更新 `tsconfig.json` 中的 `include` 路径

- [ ] 4.5 移动浏览器测试：
  ```bash
  git mv hosts/desktop/browser shell/gui/browser
  ```

---

## 阶段 5：更新 import 路径

> [!WARNING]
> 这是最耗时的步骤。所有 `use crate::core::` 引用需要变为 `use entrance_core::`。

- [ ] 5.1 在 core crate 中：
  - 所有 `use crate::core::` → 移除 `core::` 前缀（因为现在已经在 core crate 内部了）
  - 例如 `use crate::core::data_store::DataStore` → `use crate::data_store::DataStore`

- [ ] 5.2 在 harness crate 中：
  - `use crate::core::` → `use entrance_core::`
  - `use crate::hosts::plugins::` → `use crate::plugins::`

- [ ] 5.3 在 shell/gui crate 中：
  - `use crate::core::` → `use entrance_core::`
  - `use crate::hosts::` → `use entrance_harness::` 或 `use crate::desktop::`
  - `use crate::surfaces::` → `use crate::tauri_commands::`

- [ ] 5.4 在 shell/cli crate 中：
  - 类似处理

- [ ] 5.5 全局搜索验证无残留：
  ```bash
  grep -rn 'crate::core::' core/src/ harness/src/ shell/ --include='*.rs'
  grep -rn 'hosts::plugins' core/src/ --include='*.rs'
  ```

---

## 阶段 6：处理测试

- [ ] 6.1 `core/` 内的单元测试（`#[cfg(test)]` 块）随文件迁移，不需要额外处理

- [ ] 6.2 集成测试需要迁移：
  ```
  当前 hosts/desktop/tauri/tests/ 下的测试文件按依赖关系分配：
  - bootstrap_assets.rs    → shell/gui/tests/ 或 harness/tests/
  - forge_cli.rs           → shell/cli/tests/
  - mcp_stdio.rs           → shell/mcp/tests/ 或 shell/cli/tests/
  - mcp_http.rs            → shell/mcp/tests/ 或 shell/cli/tests/
  - nota_cli.rs            → shell/cli/tests/
  - hygiene_cli.rs         → shell/cli/tests/
  ```

- [ ] 6.3 `src/tests.rs` 中的测试按实际依赖拆分

---

## 阶段 7：清理和验证

- [ ] 7.1 删除旧的空目录：
  ```bash
  rm -rf hosts/desktop/tauri/src/
  # 保留 hosts/desktop/tauri/ 只剩 Cargo.toml stub（如果需要）
  # 或完全删除 hosts/ 目录
  ```

- [ ] 7.2 删除或重定向旧目录：
  ```bash
  rm -rf surfaces/   # 已全部迁移到 shell/gui/
  ```

- [ ] 7.3 保留的文件/目录：
  - `hosts/release/` → 移到 `shell/gui/release/` 或根目录 `release/`
  - `hosts/platform/windows/` → 移到 `shell/gui/platform/windows/`

- [ ] 7.4 编译验证：
  ```bash
  cargo check --workspace
  cargo test --workspace --lib
  pnpm check
  pnpm build
  ```

- [ ] 7.5 更新 `.gitlab-ci.yml` 中的路径

- [ ] 7.6 更新 `AGENTS.md` 中的目录说明

---

## 风险和注意事项

> [!CAUTION]
> **Tauri 的 `tauri.conf.json` 对目录结构有硬要求。** Tauri 期望 Cargo.toml 和 tauri.conf.json 在特定相对位置。移动后需要仔细检查 `tauri.conf.json` 中的 `identifier`、`frontendDist`、`devUrl` 等路径配置。

> [!WARNING]
> **`graph_events.rs` 使用全局静态 Tauri AppHandle 发送事件。** 这是 core 对 Tauri 的最强耦合点。解耦方式：core 只 emit 到 EventBus，GUI shell 负责订阅 EventBus 并转发给 Tauri emitter。当前 `lib.rs` 已经部分有这个模式（L106-114）。

> [!WARNING]
> **`include_str!` 宏在编译期解析相对路径。** Migration SQL 文件的 `include_str!` 路径必须相对于源文件所在位置。移动文件后所有 `include_str!` 都要更新。

> [!IMPORTANT]
> **建议分多个 PR 执行，不要一个 PR 做完所有事。** 推荐顺序：
> 1. PR1: 创建 workspace + 提取 core crate（最大最难的一步）
> 2. PR2: 提取 harness crate
> 3. PR3: 拆分 shell/cli + shell/gui
> 4. PR4: 清理旧目录 + 更新 CI
