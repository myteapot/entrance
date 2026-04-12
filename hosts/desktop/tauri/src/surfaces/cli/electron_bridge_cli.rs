use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    sync::mpsc::{unbounded_channel, UnboundedSender},
};

use crate::{
    core::{
        bootstrap_for_paths,
        data_store::{DataStore, NewIssue},
        event_bus::{install_external_emitters, EventBus},
        hygiene::list_spec_hygiene_v0,
        landing::{
            import_linear_entrance_snapshot, list_landing_ingest_runs, list_landing_mirror_items,
            list_landing_planning_items, list_landing_unreconciled_items,
        },
        overview::{build_nota_runtime_overview, build_nota_runtime_status},
        parallel_budget::ParallelBudgetConfig,
        resolve_app_data_dir,
        system_heartbeat::{compute_pulse, run_system_heartbeat, AgentTier, HeartbeatConfig},
        AppPaths,
    },
    hosts::{
        desktop::{
            instance_manager::{InstanceManager, InstanceRole},
            logging::LoggingSystem,
        },
        plugins::{
            self,
            forge::{build_agent_task_request, CreateTaskRequest, ForgePlugin},
            launcher::LauncherPlugin,
            vault::VaultPlugin,
            AppContext, Plugin,
        },
    },
    surfaces::tauri::{self as tauri_surface, DashboardUiState, LauncherUiState},
};

pub(super) fn run_electron_bridge_stdio(args: &[String]) -> Result<()> {
    if let Some(argument) = args.first() {
        bail!("unsupported Electron bridge argument `{argument}`");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime for Electron bridge")?;

    runtime.block_on(run_electron_bridge_stdio_async())
}

async fn run_electron_bridge_stdio_async() -> Result<()> {
    let runtime = ElectronBridgeRuntime::bootstrap()?;
    let (outbound_tx, mut outbound_rx) = unbounded_channel::<BridgeOutboundMessage>();

    runtime.install_emitters(outbound_tx.clone());
    runtime.forward_backend_events(outbound_tx.clone());

    let writer = tokio::spawn(async move {
        let mut stdout = BufWriter::new(tokio::io::stdout());
        while let Some(message) = outbound_rx.recv().await {
            let line = match serde_json::to_string(&message) {
                Ok(line) => line,
                Err(error) => {
                    tracing::warn!(?error, "failed to serialize Electron bridge message");
                    continue;
                }
            };

            if stdout.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if stdout.write_all(b"\n").await.is_err() {
                break;
            }
            if stdout.flush().await.is_err() {
                break;
            }
        }
    });

    let _ = outbound_tx.send(BridgeOutboundMessage::Ready);

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next_line().await? {
        let payload = line.trim();
        if payload.is_empty() {
            continue;
        }

        let request = match serde_json::from_str::<BridgeInvokeRequest>(payload) {
            Ok(request) => request,
            Err(error) => {
                tracing::warn!(?error, payload, "failed to parse Electron bridge request");
                continue;
            }
        };

        if request.kind != "invoke" {
            tracing::warn!(kind = request.kind, "unsupported Electron bridge request kind");
            continue;
        }

        let response = match runtime.handle_invoke(&request.command, request.args).await {
            Ok(result) => BridgeOutboundMessage::Response {
                id: request.id,
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => BridgeOutboundMessage::Response {
                id: request.id,
                ok: false,
                result: None,
                error: Some(error),
            },
        };

        let _ = outbound_tx.send(response);
    }

    drop(outbound_tx);
    let _ = writer.await;
    Ok(())
}

struct ElectronBridgeRuntime {
    _logging_system: LoggingSystem,
    launcher_ui: LauncherUiState,
    dashboard_ui: DashboardUiState,
    data_store: DataStore,
    event_bus: EventBus,
    instance_manager: InstanceManager,
    launcher: Option<LauncherPlugin>,
    forge: Option<ForgePlugin>,
    vault: Option<VaultPlugin>,
}

impl ElectronBridgeRuntime {
    fn bootstrap() -> Result<Self> {
        let app_paths = AppPaths::new(resolve_app_data_dir()?);
        let startup = bootstrap_for_paths(app_paths)?;
        let data_store = startup.data_store();
        let launcher_hotkey = startup.launcher_hotkey().map(str::to_owned);
        let logging_system = LoggingSystem::init(
            startup.paths().log_dir(),
            startup.log_level(),
            Some(data_store.clone()),
        )?;
        let event_bus = EventBus::new();
        let enabled_plugin_count = [
            startup.launcher_enabled(),
            startup.forge_enabled(),
            startup.vault_enabled(),
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();

        let launcher = startup
            .launcher_enabled()
            .then(|| LauncherPlugin::new(data_store.clone()));
        let forge = startup.forge_enabled().then(|| {
            let forge = ForgePlugin::new(data_store.clone(), event_bus.clone());
            if let Err(error) = forge.start_http_server(startup.forge_http_port()) {
                tracing::warn!(
                    ?error,
                    "Forge HTTP server failed to start (port may be in use), continuing without it"
                );
            }
            forge
        });
        let vault = if startup.vault_enabled() {
            Some(VaultPlugin::new(data_store.clone())?)
        } else {
            None
        };

        let app_context = AppContext::new(data_store.clone(), event_bus.clone());
        if let Some(plugin) = launcher.as_ref() {
            plugin.init(&app_context)?;
        }
        if let Some(plugin) = forge.as_ref() {
            plugin.init(&app_context)?;
        }
        if let Some(plugin) = vault.as_ref() {
            plugin.init(&app_context)?;
        }

        tauri::async_runtime::spawn({
            let data_store = data_store.clone();
            let event_bus = event_bus.clone();
            async move {
                run_system_heartbeat(data_store, event_bus, HeartbeatConfig::default()).await;
            }
        });

        Ok(Self {
            _logging_system: logging_system,
            launcher_ui: LauncherUiState {
                hotkey: launcher_hotkey.clone(),
            },
            dashboard_ui: DashboardUiState {
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                launcher_hotkey,
                enabled_plugin_count,
                launcher_enabled: startup.launcher_enabled(),
                forge_enabled: startup.forge_enabled(),
                vault_enabled: startup.vault_enabled(),
            },
            data_store: data_store.clone(),
            event_bus: event_bus.clone(),
            instance_manager: InstanceManager::new(data_store, event_bus),
            launcher,
            forge,
            vault,
        })
    }

    fn install_emitters(&self, outbound_tx: UnboundedSender<BridgeOutboundMessage>) {
        let graph_tx = outbound_tx.clone();
        install_external_emitters(
            move |event| match serde_json::to_string(event) {
                Ok(payload) => {
                    let _ = graph_tx.send(BridgeOutboundMessage::Event {
                        topic: "graph:update".to_string(),
                        payload,
                    });
                }
                Err(error) => {
                    tracing::warn!(?error, "failed to serialize graph update event");
                }
            },
            move |event| match serde_json::to_string(event) {
                Ok(payload) => {
                    let _ = outbound_tx.send(BridgeOutboundMessage::Event {
                        topic: "nota:dialog".to_string(),
                        payload,
                    });
                }
                Err(error) => {
                    tracing::warn!(?error, "failed to serialize nota dialog event");
                }
            },
        );
    }

    fn forward_backend_events(&self, outbound_tx: UnboundedSender<BridgeOutboundMessage>) {
        let mut events = self.event_bus.subscribe();
        tauri::async_runtime::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => {
                        let _ = outbound_tx.send(BridgeOutboundMessage::Event {
                            topic: event.topic,
                            payload: event.payload,
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "Electron bridge event stream lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    async fn handle_invoke(&self, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "launcher_hotkey" => to_json_value(self.launcher_ui.hotkey.clone()),
            "dashboard_summary" => to_json_value(tauri_surface::build_dashboard_summary(
                &self.dashboard_ui,
                &self.data_store,
            )?),
            "list_agent_instances" => {
                to_json_value(self.data_store.list_agent_instances().map_err(stringify_error)?)
            }
            "get_system_pulse" => to_json_value(
                compute_pulse(&self.data_store, &HeartbeatConfig::default())
                    .map_err(stringify_error)?,
            ),
            "get_parallel_budget_config" => to_json_value(ParallelBudgetConfig::default()),
            "create_agent_instance" => {
                let args = parse_args::<CreateAgentInstanceArgs>(args)?;
                let role: InstanceRole = args.role.parse().map_err(stringify_error)?;
                to_json_value(
                    self.instance_manager
                        .create_instance(
                            role,
                            args.parent_instance_id,
                            &args.display_name,
                            &args.config_json,
                            None,
                            AgentTier::ArchNota,
                        )
                        .map_err(stringify_error)?,
                )
            }
            "stop_agent_instance" => {
                let args = parse_args::<StopAgentInstanceArgs>(args)?;
                self.instance_manager
                    .stop_instance(args.id)
                    .map_err(stringify_error)?;
                to_json_value(())
            }
            "spawn_child_instances" => {
                let args = parse_args::<SpawnChildInstancesArgs>(args)?;
                to_json_value(
                    self.instance_manager
                        .spawn_children(args.parent_id, args.count)
                        .map_err(stringify_error)?,
                )
            }
            "launcher_search" => {
                let args = parse_args::<LauncherSearchArgs>(args)?;
                to_json_value(
                    self.launcher()?
                        .search(&args.query, args.limit.unwrap_or(20))
                        .map_err(stringify_error)?,
                )
            }
            "launcher_launch" => {
                let args = parse_args::<LauncherLaunchArgs>(args)?;
                self.launcher()?
                    .launch(
                        &args.path,
                        args.arguments.as_deref(),
                        args.working_dir.as_deref(),
                    )
                    .map_err(stringify_error)?;
                to_json_value(())
            }
            "launcher_pin" => {
                let args = parse_args::<LauncherPinArgs>(args)?;
                self.launcher()?
                    .pin(&args.path, args.pinned)
                    .map_err(stringify_error)?;
                to_json_value(())
            }
            "nota_approve_prayer" => {
                let args = parse_args::<ApprovePrayerArgs>(args)?;
                to_json_value(tauri_surface::nota_prayer::approve_prayer(
                    &self.data_store,
                    args.allocation_id,
                )?)
            }
            "nota_reject_prayer" => {
                let args = parse_args::<RejectPrayerArgs>(args)?;
                to_json_value(tauri_surface::nota_prayer::reject_prayer(
                    &self.data_store,
                    args.allocation_id,
                    args.reason,
                )?)
            }
            "issue_list" => {
                let args = parse_args::<IssueListArgs>(args)?;
                to_json_value(
                    self.data_store
                        .list_issues(args.status.as_deref())
                        .map_err(stringify_error)?,
                )
            }
            "issue_get" => {
                let args = parse_args::<IssueKeyArgs>(args)?;
                to_json_value(
                    self.data_store
                        .get_issue_by_key(&args.issue_key)
                        .map_err(stringify_error)?,
                )
            }
            "issue_create" => {
                let args = parse_args::<IssueCreateArgs>(args)?;
                to_json_value(
                    self.data_store
                        .create_issue(NewIssue {
                            title: &args.title,
                            description: args.description.as_deref().unwrap_or(""),
                            status: "todo",
                            priority: args.priority.as_deref().unwrap_or("none"),
                            labels: args.labels.as_deref().unwrap_or("[]"),
                            assignee: args.assignee.as_deref().unwrap_or(""),
                        })
                        .map_err(stringify_error)?,
                )
            }
            "issue_update_status" => {
                let args = parse_args::<IssueUpdateStatusArgs>(args)?;
                to_json_value(
                    self.data_store
                        .update_issue_status(&args.issue_key, &args.status)
                        .map_err(stringify_error)?,
                )
            }
            "issue_update" => {
                let args = parse_args::<IssueUpdateArgs>(args)?;
                to_json_value(
                    self.data_store
                        .update_issue(
                            &args.issue_key,
                            args.title.as_deref(),
                            args.description.as_deref(),
                            args.priority.as_deref(),
                            args.labels.as_deref(),
                            args.assignee.as_deref(),
                        )
                        .map_err(stringify_error)?,
                )
            }
            "issue_delete" => {
                let args = parse_args::<IssueKeyArgs>(args)?;
                self.data_store
                    .delete_issue(&args.issue_key)
                    .map_err(stringify_error)?;
                to_json_value(())
            }
            "issue_add_comment" => {
                let args = parse_args::<IssueAddCommentArgs>(args)?;
                to_json_value(
                    self.data_store
                        .add_issue_comment(&args.issue_key, &args.author, &args.body)
                        .map_err(stringify_error)?,
                )
            }
            "issue_list_comments" => {
                let args = parse_args::<IssueKeyArgs>(args)?;
                to_json_value(
                    self.data_store
                        .list_issue_comments(&args.issue_key)
                        .map_err(stringify_error)?,
                )
            }
            "vault_list_tokens" => {
                to_json_value(self.vault()?.list_tokens().map_err(stringify_error)?)
            }
            "vault_add_token" => {
                let args = parse_args::<VaultTokenArgs>(args)?;
                to_json_value(
                    self.vault()?
                        .add_token(&args.name, &args.provider, &args.value)
                        .map_err(stringify_error)?,
                )
            }
            "vault_upsert_token" => {
                let args = parse_args::<VaultTokenArgs>(args)?;
                to_json_value(
                    self.vault()?
                        .upsert_token(&args.name, &args.provider, &args.value)
                        .map_err(stringify_error)?,
                )
            }
            "vault_delete_token" => {
                let args = parse_args::<VaultTokenIdArgs>(args)?;
                self.vault()?
                    .delete_token(args.id)
                    .map_err(stringify_error)?;
                to_json_value(())
            }
            "vault_get_token" => {
                let args = parse_args::<VaultTokenIdArgs>(args)?;
                to_json_value(self.vault()?.get_token(args.id).map_err(stringify_error)?)
            }
            "vault_get_token_by_provider" => {
                let args = parse_args::<VaultProviderArgs>(args)?;
                to_json_value(
                    self.vault()?
                        .get_token_by_provider(&args.provider)
                        .map_err(stringify_error)?,
                )
            }
            "vault_list_mcp" => {
                to_json_value(self.vault()?.list_mcp_configs().map_err(stringify_error)?)
            }
            "vault_update_mcp" => {
                let args = parse_args::<VaultUpdateMcpArgs>(args)?;
                to_json_value(
                    self.vault()?
                        .update_mcp_config(
                            args.id,
                            &args.name,
                            &args.transport,
                            &args.endpoint,
                            args.enabled,
                        )
                        .map_err(stringify_error)?,
                )
            }
            "landing_import_snapshot" => {
                let args = parse_args::<LandingImportArgs>(args)?;
                to_json_value(
                    import_linear_entrance_snapshot(&self.data_store, args.path)
                        .map_err(stringify_error)?,
                )
            }
            "landing_list_ingest_runs" => {
                to_json_value(list_landing_ingest_runs(&self.data_store).map_err(stringify_error)?)
            }
            "landing_list_mirror_items" => {
                to_json_value(list_landing_mirror_items(&self.data_store).map_err(stringify_error)?)
            }
            "landing_list_planning_items" => to_json_value(
                list_landing_planning_items(&self.data_store).map_err(stringify_error)?,
            ),
            "landing_list_unreconciled_items" => to_json_value(
                list_landing_unreconciled_items(&self.data_store).map_err(stringify_error)?,
            ),
            "hygiene_list_spec_v0" => {
                to_json_value(list_spec_hygiene_v0(&self.data_store).map_err(stringify_error)?)
            }
            "nota_runtime_overview" => to_json_value(
                build_nota_runtime_overview(&self.data_store).map_err(stringify_error)?,
            ),
            "nota_runtime_status" => {
                to_json_value(build_nota_runtime_status(&self.data_store).map_err(stringify_error)?)
            }
            "forge_create_task" => {
                let args = parse_args::<ForgeCreateTaskArgs>(args)?;
                let forge = self.forge()?;
                let required_tokens = serde_json::to_string(&args.required_tokens.unwrap_or_default())
                    .map_err(stringify_error)?;
                let id = forge
                    .create_task(CreateTaskRequest {
                        name: args.name,
                        command: args.command,
                        args: args.args,
                        working_dir: None,
                        stdin_text: None,
                        required_tokens,
                        metadata: "{}".to_string(),
                        dispatch_receipt: None,
                    })
                    .map_err(stringify_error)?;
                forge.engine().spawn_task(id).map_err(stringify_error)?;
                to_json_value(id)
            }
            "forge_dispatch_agent" => {
                let args = parse_args::<ForgeDispatchAgentArgs>(args)?;
                let forge = self.forge()?;
                let request = build_agent_task_request(
                    args.issue_id,
                    args.worktree_path,
                    args.model,
                    args.prompt,
                    args.required_tokens.unwrap_or_default(),
                    args.agent_command,
                )
                .map_err(stringify_error)?;
                let id = forge.create_task(request).map_err(stringify_error)?;
                forge.engine().spawn_task(id).map_err(stringify_error)?;
                to_json_value(id)
            }
            "forge_prepare_agent_dispatch" => {
                let args = parse_args::<ForgePrepareDispatchArgs>(args)?;
                to_json_value(
                    plugins::forge::prepare_agent_dispatch(self.data_store.clone(), args.project_dir)
                        .await?,
                )
            }
            "forge_list_tasks" => {
                to_json_value(self.forge()?.list_tasks().map_err(stringify_error)?)
            }
            "forge_get_task" => {
                let args = parse_args::<ForgeTaskIdArgs>(args)?;
                to_json_value(self.forge()?.get_task(args.id).map_err(stringify_error)?)
            }
            "forge_get_task_details" => {
                let args = parse_args::<ForgeTaskIdArgs>(args)?;
                to_json_value(
                    self.forge()?
                        .get_task_details(args.id)
                        .map_err(stringify_error)?,
                )
            }
            "forge_cancel_task" => {
                let args = parse_args::<ForgeTaskIdArgs>(args)?;
                self.forge()?.cancel_task(args.id).map_err(stringify_error)?;
                to_json_value(())
            }
            other => Err(format!("unsupported Electron bridge command `{other}`")),
        }
    }

    fn launcher(&self) -> Result<&LauncherPlugin, String> {
        self.launcher
            .as_ref()
            .ok_or_else(|| "launcher plugin is not enabled".to_string())
    }

    fn forge(&self) -> Result<&ForgePlugin, String> {
        self.forge
            .as_ref()
            .ok_or_else(|| "forge plugin is not enabled".to_string())
    }

    fn vault(&self) -> Result<&VaultPlugin, String> {
        self.vault
            .as_ref()
            .ok_or_else(|| "vault plugin is not enabled".to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum BridgeOutboundMessage {
    Ready,
    Response {
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Event {
        topic: String,
        payload: String,
    },
}

#[derive(Debug, Deserialize)]
struct BridgeInvokeRequest {
    kind: String,
    id: String,
    command: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAgentInstanceArgs {
    role: String,
    #[serde(default, alias = "parent_instance_id")]
    parent_instance_id: Option<i64>,
    display_name: String,
    #[serde(default = "default_config_json", alias = "config_json")]
    config_json: String,
}

#[derive(Debug, Deserialize)]
struct StopAgentInstanceArgs {
    id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpawnChildInstancesArgs {
    #[serde(alias = "parent_id")]
    parent_id: i64,
    count: usize,
}

#[derive(Debug, Deserialize)]
struct LauncherSearchArgs {
    query: String,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LauncherLaunchArgs {
    path: String,
    arguments: Option<String>,
    #[serde(default, alias = "working_dir")]
    working_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LauncherPinArgs {
    path: String,
    pinned: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovePrayerArgs {
    #[serde(alias = "allocation_id")]
    allocation_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RejectPrayerArgs {
    #[serde(alias = "allocation_id")]
    allocation_id: i64,
    reason: String,
}

#[derive(Debug, Default, Deserialize)]
struct IssueListArgs {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueKeyArgs {
    #[serde(alias = "issue_key")]
    issue_key: String,
}

#[derive(Debug, Deserialize)]
struct IssueCreateArgs {
    title: String,
    description: Option<String>,
    priority: Option<String>,
    labels: Option<String>,
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueUpdateStatusArgs {
    #[serde(alias = "issue_key")]
    issue_key: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueUpdateArgs {
    #[serde(alias = "issue_key")]
    issue_key: String,
    title: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    labels: Option<String>,
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueAddCommentArgs {
    #[serde(alias = "issue_key")]
    issue_key: String,
    author: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct VaultTokenArgs {
    name: String,
    provider: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct VaultTokenIdArgs {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct VaultProviderArgs {
    provider: String,
}

#[derive(Debug, Deserialize)]
struct VaultUpdateMcpArgs {
    id: Option<i64>,
    name: String,
    transport: String,
    endpoint: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct LandingImportArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct ForgeTaskIdArgs {
    id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForgePrepareDispatchArgs {
    #[serde(default, alias = "project_dir")]
    project_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForgeCreateTaskArgs {
    name: String,
    command: String,
    args: String,
    #[serde(default, alias = "required_tokens")]
    required_tokens: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForgeDispatchAgentArgs {
    #[serde(alias = "issue_id")]
    issue_id: String,
    #[serde(alias = "worktree_path")]
    worktree_path: String,
    model: String,
    prompt: String,
    #[serde(default, alias = "required_tokens")]
    required_tokens: Option<Vec<String>>,
    #[serde(default, alias = "agent_command")]
    agent_command: Option<String>,
}

fn default_config_json() -> String {
    "{}".to_string()
}

fn parse_args<T>(args: Value) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let args = if args.is_null() {
        Value::Object(Default::default())
    } else {
        args
    };
    serde_json::from_value(args).map_err(|error| format!("invalid Electron bridge arguments: {error}"))
}

fn stringify_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn to_json_value<T>(value: T) -> Result<Value, String>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|error| error.to_string())
}
