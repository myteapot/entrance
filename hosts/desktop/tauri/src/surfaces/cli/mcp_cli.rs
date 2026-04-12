use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{bail, Context, Result};

use crate::{
    core::{
        action::ActorRole,
        event_bus::EventBus,
        StartupState,
    },
    hosts::plugins::{self, launcher::LauncherPlugin, vault::VaultPlugin},
    surfaces::mcp_server::{McpPluginSet, McpServer, McpTransport},
};

use super::bootstrap_headless;

pub(super) fn run_mcp_stdio(args: &[String]) -> Result<()> {
    let actor_role = parse_mcp_actor_role_args(args)?;
    let startup = bootstrap_headless()?;
    let server = build_mcp_server(&startup, McpTransport::Stdio, actor_role)?;
    server.serve_stdio()
}

pub(super) fn run_mcp_http(args: &[String]) -> Result<()> {
    let mut port = 9720u16;
    let mut endpoint = "/mcp".to_string();
    let mut actor_role = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--port" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --port` requires a value")?;
                port = value
                    .parse::<u16>()
                    .with_context(|| format!("invalid MCP HTTP port `{value}`"))?;
                index += 2;
            }
            "--endpoint" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --endpoint` requires a value")?;
                endpoint = normalize_http_endpoint(value)?;
                index += 2;
            }
            "--actor-role" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance mcp http --actor-role` requires a value")?;
                actor_role = Some(parse_mcp_actor_role(value)?);
                index += 2;
            }
            other => bail!("unsupported MCP HTTP argument `{other}`"),
        }
    }

    let startup = bootstrap_headless()?;
    let server = build_mcp_server(
        &startup,
        McpTransport::Http {
            endpoint: endpoint.clone(),
        },
        actor_role,
    )?;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime for MCP HTTP transport")?;

    runtime.block_on(server.serve_http(address))
}

fn build_mcp_server(
    startup: &StartupState,
    transport: McpTransport,
    actor_role: Option<ActorRole>,
) -> Result<McpServer> {
    let data_store = startup.data_store();
    let event_bus = EventBus::new();

    Ok(McpServer::with_actor_role(
        transport,
        McpPluginSet {
            core_data_store: Some(data_store.clone()),
            forge: startup
                .forge_enabled()
                .then(|| plugins::forge::ForgePlugin::new(data_store.clone(), event_bus.clone())),
            launcher: startup
                .launcher_enabled()
                .then(|| LauncherPlugin::new(data_store.clone())),
            vault: if startup.vault_enabled() {
                Some(VaultPlugin::new(data_store)?)
            } else {
                None
            },
        },
        actor_role,
    ))
}

fn normalize_http_endpoint(raw: &str) -> Result<String> {
    let endpoint = raw.trim();
    if endpoint.is_empty() {
        bail!("MCP HTTP endpoint must not be empty");
    }

    if endpoint.starts_with('/') {
        Ok(endpoint.to_string())
    } else {
        Ok(format!("/{endpoint}"))
    }
}

fn parse_mcp_actor_role_args(args: &[String]) -> Result<Option<ActorRole>> {
    match args {
        [] => Ok(None),
        [flag, value] if flag == "--actor-role" => Ok(Some(parse_mcp_actor_role(value)?)),
        [other, ..] => bail!("unsupported MCP stdio argument `{other}`"),
    }
}

fn parse_mcp_actor_role(value: &str) -> Result<ActorRole> {
    match value.trim() {
        "nota" => Ok(ActorRole::Nota),
        "arch" => Ok(ActorRole::Arch),
        "dev" => Ok(ActorRole::Dev),
        other => bail!("unsupported MCP actor role `{other}`, expected `nota`, `arch`, or `dev`"),
    }
}
