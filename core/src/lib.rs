pub mod action;
pub mod anti_zeno_runtime;
pub mod chat_archive;
pub mod cold_docs_runtime;
pub mod compiler;
pub mod data_store;
pub mod design_governance;
pub mod dispatch_host;
pub mod environment_runtime;
pub mod event_bus;
pub mod front_door;
pub mod graph_events;
pub mod hygiene;
pub mod invariant_runtime;
pub mod landing;
pub mod memory_import;
pub mod nota;
pub mod overview;
pub mod parallel_budget;
pub mod permission;
pub mod projection_runtime;
pub mod recovery;
pub mod supervision;
pub mod system_heartbeat;

pub use nota as nota_runtime;

pub mod core {
    pub use crate::action;
    pub use crate::anti_zeno_runtime;
    pub use crate::chat_archive;
    pub use crate::cold_docs_runtime;
    pub use crate::compiler;
    pub use crate::data_store;
    pub use crate::design_governance;
    pub use crate::dispatch_host;
    pub use crate::environment_runtime;
    pub use crate::event_bus;
    pub use crate::front_door;
    pub use crate::graph_events;
    pub use crate::hygiene;
    pub use crate::invariant_runtime;
    pub use crate::landing;
    pub use crate::memory_import;
    pub use crate::nota;
    pub use crate::nota_runtime;
    pub use crate::overview;
    pub use crate::parallel_budget;
    pub use crate::permission;
    pub use crate::projection_runtime;
    pub use crate::recovery;
    pub use crate::supervision;
    pub use crate::system_heartbeat;

    #[cfg(test)]
    pub use crate::AppPaths;
    #[cfg(test)]
    pub use crate::{bootstrap_for_paths, config_store};
}

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn test_env_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("test environment lock should not be poisoned")
}

#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
pub mod config_store {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct EntranceConfig {
        #[serde(default)]
        pub core: CoreConfig,
        #[serde(default)]
        pub paths: PathsConfig,
        #[serde(default)]
        pub plugins: PluginsConfig,
    }

    impl Default for EntranceConfig {
        fn default() -> Self {
            Self {
                core: CoreConfig::default(),
                paths: PathsConfig::default(),
                plugins: PluginsConfig::default(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct CoreConfig {
        #[serde(default = "default_theme")]
        pub theme: String,
        #[serde(default = "default_log_level")]
        pub log_level: String,
        #[serde(default = "default_true")]
        pub mcp_enabled: bool,
    }

    impl Default for CoreConfig {
        fn default() -> Self {
            Self {
                theme: default_theme(),
                log_level: default_log_level(),
                mcp_enabled: default_true(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PathsConfig {
        #[serde(default = "default_runtime_db_path")]
        pub runtime_db: String,
        #[serde(default = "default_logs_path")]
        pub logs: String,
        #[serde(default = "default_cache_path")]
        pub cache: String,
        #[serde(default = "default_exports_path")]
        pub exports: String,
        #[serde(default = "default_snapshots_path")]
        pub snapshots: String,
        #[serde(default = "default_worktrees_path")]
        pub worktrees: String,
    }

    impl Default for PathsConfig {
        fn default() -> Self {
            Self {
                runtime_db: default_runtime_db_path(),
                logs: default_logs_path(),
                cache: default_cache_path(),
                exports: default_exports_path(),
                snapshots: default_snapshots_path(),
                worktrees: default_worktrees_path(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PluginsConfig {
        #[serde(default)]
        pub launcher: LauncherConfig,
        #[serde(default)]
        pub forge: ForgeConfig,
        #[serde(default)]
        pub vault: TogglePluginConfig,
    }

    impl Default for PluginsConfig {
        fn default() -> Self {
            Self {
                launcher: LauncherConfig::default(),
                forge: ForgeConfig::default(),
                vault: TogglePluginConfig { enabled: false },
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct LauncherConfig {
        #[serde(default = "default_true")]
        pub enabled: bool,
        #[serde(default = "default_launcher_hotkey")]
        pub hotkey: String,
        #[serde(default)]
        pub scan_paths: Vec<String>,
    }

    impl Default for LauncherConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                hotkey: default_launcher_hotkey(),
                scan_paths: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ForgeConfig {
        #[serde(default = "default_true")]
        pub enabled: bool,
        #[serde(default = "default_forge_http_port")]
        pub http_port: u16,
        #[serde(default)]
        pub project_dir: Option<String>,
        #[serde(default)]
        pub agent_command: Option<String>,
    }

    impl Default for ForgeConfig {
        fn default() -> Self {
            Self {
                enabled: true,
                http_port: default_forge_http_port(),
                project_dir: None,
                agent_command: None,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
    pub struct TogglePluginConfig {
        #[serde(default)]
        pub enabled: bool,
    }

    fn default_theme() -> String {
        "dark".to_string()
    }

    fn default_log_level() -> String {
        "info".to_string()
    }

    fn default_true() -> bool {
        true
    }

    fn default_runtime_db_path() -> String {
        "data/entrance.db".to_string()
    }

    fn default_logs_path() -> String {
        "logs".to_string()
    }

    fn default_cache_path() -> String {
        "cache".to_string()
    }

    fn default_exports_path() -> String {
        "exports".to_string()
    }

    fn default_snapshots_path() -> String {
        "snapshots".to_string()
    }

    fn default_worktrees_path() -> String {
        "worktrees".to_string()
    }

    fn default_launcher_hotkey() -> String {
        "Alt+Space".to_string()
    }

    fn default_forge_http_port() -> u16 {
        9315
    }

    pub fn render_config(config: &EntranceConfig) -> Result<String> {
        Ok(toml::to_string_pretty(config)?)
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct TestStartupState {
    data_store: data_store::DataStore,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    app_data_dir: PathBuf,
    config_path: PathBuf,
    data_dir: PathBuf,
    db_path: PathBuf,
    log_dir: PathBuf,
    cache_dir: PathBuf,
    exports_dir: PathBuf,
    snapshots_dir: PathBuf,
    worktrees_dir: PathBuf,
}

#[cfg(test)]
impl AppPaths {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        let app_data_dir = app_data_dir.into();
        Self {
            config_path: app_data_dir.join("entrance.toml"),
            data_dir: app_data_dir.join("data"),
            db_path: app_data_dir.join("data").join("entrance.db"),
            log_dir: app_data_dir.join("logs"),
            cache_dir: app_data_dir.join("cache"),
            exports_dir: app_data_dir.join("exports"),
            snapshots_dir: app_data_dir.join("snapshots"),
            worktrees_dir: app_data_dir.join("worktrees"),
            app_data_dir,
        }
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn ensure_layout(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.app_data_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.log_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.exports_dir)?;
        std::fs::create_dir_all(&self.snapshots_dir)?;
        std::fs::create_dir_all(&self.worktrees_dir)?;
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

#[cfg(test)]
impl TestStartupState {
    pub fn data_store(&self) -> data_store::DataStore {
        self.data_store.clone()
    }
}

#[cfg(test)]
pub fn bootstrap_for_paths(paths: AppPaths) -> anyhow::Result<TestStartupState> {
    paths.ensure_layout()?;
    let migration_plan = data_store::MigrationPlan::new(hosts::plugins::forge::migrations());
    let data_store = data_store::DataStore::open(paths.db_path(), migration_plan)?;
    compiler::registry::seed_registry_snapshot(&data_store)?;
    Ok(TestStartupState { data_store })
}

#[cfg(test)]
pub mod hosts {
    pub mod plugins {
        pub mod forge {
            use anyhow::Result;

            use crate::{
                action::ActorRole,
                data_store::{DataStore, MigrationStep, StoredForgeTask},
                dispatch_host::{CreateTaskRequest, DispatchHost, PreparedDispatch},
                event_bus::EventBus,
            };

            static MIGRATIONS: [MigrationStep; 3] = [
                MigrationStep {
                    name: "0002_create_plugin_forge_tasks",
                    sql: include_str!("../../harness/src/plugins/forge/schema/0002_create_plugin_forge_tasks.sql"),
                },
                MigrationStep {
                    name: "0004_create_plugin_forge_task_logs",
                    sql: include_str!("../../harness/src/plugins/forge/schema/0004_create_plugin_forge_task_logs.sql"),
                },
                MigrationStep {
                    name: "0006_create_plugin_forge_dispatch_receipts",
                    sql: include_str!("../../harness/src/plugins/forge/schema/0006_create_plugin_forge_dispatch_receipts.sql"),
                },
            ];

            pub fn migrations() -> &'static [MigrationStep] {
                &MIGRATIONS
            }

            #[derive(Clone)]
            pub struct ForgePlugin {
                data_store: DataStore,
                _event_bus: EventBus,
            }

            impl ForgePlugin {
                pub fn new(data_store: DataStore, event_bus: EventBus) -> Self {
                    Self {
                        data_store,
                        _event_bus: event_bus,
                    }
                }
            }

            impl DispatchHost for ForgePlugin {
                fn prepare_agent_dispatch(
                    &self,
                    _data_store: &DataStore,
                    project_dir: Option<String>,
                ) -> Result<PreparedDispatch> {
                    Ok(stub_dispatch(ActorRole::Agent, project_dir))
                }

                fn prepare_dev_dispatch(
                    &self,
                    _data_store: &DataStore,
                    project_dir: Option<String>,
                ) -> Result<PreparedDispatch> {
                    Ok(stub_dispatch(ActorRole::Dev, project_dir))
                }

                fn build_agent_task_request(
                    &self,
                    dispatch: &PreparedDispatch,
                    model: String,
                    agent_command: Option<String>,
                ) -> Result<CreateTaskRequest> {
                    Ok(stub_task_request("Agent", dispatch, model, agent_command))
                }

                fn build_dev_task_request(
                    &self,
                    dispatch: &PreparedDispatch,
                    model: String,
                    agent_command: Option<String>,
                ) -> Result<CreateTaskRequest> {
                    Ok(stub_task_request("Dev", dispatch, model, agent_command))
                }

                fn create_task(&self, request: CreateTaskRequest) -> Result<i64> {
                    self.data_store.insert_forge_task(
                        &request.name,
                        &request.command,
                        &request.args,
                        request.working_dir.as_deref(),
                        request.stdin_text.as_deref(),
                        &request.required_tokens,
                        &request.metadata,
                    )
                }

                fn get_task(&self, task_id: i64) -> Result<Option<StoredForgeTask>> {
                    self.data_store.get_forge_task(task_id)
                }

                fn spawn_task(&self, _task_id: i64) -> Result<()> {
                    anyhow::bail!("test stub does not execute forge tasks")
                }
            }

            fn stub_dispatch(role: ActorRole, project_dir: Option<String>) -> PreparedDispatch {
                let project_root = project_dir.unwrap_or_else(|| ".".to_string());
                let dispatch_tool_name = match role {
                    ActorRole::Agent => "forge_dispatch_agent",
                    ActorRole::Dev => "forge_dispatch_dev",
                    _ => "forge_dispatch_agent",
                };

                PreparedDispatch {
                    dispatch_role: role,
                    dispatch_tool_name: dispatch_tool_name.to_string(),
                    issue_id: "MYT-TEST".to_string(),
                    issue_status: "Todo".to_string(),
                    issue_status_source: "test".to_string(),
                    issue_title: Some("Test issue".to_string()),
                    worktree_path: project_root.clone(),
                    project_root,
                    prompt_source: "test".to_string(),
                    prompt: "test prompt".to_string(),
                }
            }

            fn stub_task_request(
                prefix: &str,
                dispatch: &PreparedDispatch,
                model: String,
                agent_command: Option<String>,
            ) -> CreateTaskRequest {
                let metadata = serde_json::json!({
                    "kind": format!("{}_dispatch", prefix.to_ascii_lowercase()),
                    "issue_id": dispatch.issue_id,
                    "worktree_path": dispatch.worktree_path,
                    "model": model,
                })
                .to_string();

                CreateTaskRequest {
                    name: format!("{prefix} {}", dispatch.issue_id),
                    command: agent_command.unwrap_or_else(|| "codex".to_string()),
                    args: "[]".to_string(),
                    working_dir: Some(dispatch.worktree_path.clone()),
                    stdin_text: Some(dispatch.prompt.clone()),
                    required_tokens: "[]".to_string(),
                    metadata,
                    dispatch_receipt: None,
                }
            }
        }
    }
}
