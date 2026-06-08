mod app;
mod cli;
mod command;
mod commands;
mod daemon;
mod mcp;

use anyhow::Result;
use command::Command;
use serde::Serialize;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    init_tracing();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let services = app::boot_services()?;

    match cli::parse(&args)? {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Status => print_json(&services.kernel.store.app_status(&services.kernel.root)?),
        Command::DaemonStdio => daemon::run_stdio(services).await,
        Command::DaemonHttp => daemon::run_http(services).await,
        Command::McpStdio => mcp::run_stdio(services).await,
        Command::Drawer(args) => commands::drawer::run(&services, &args),
        Command::Hive(args) => commands::hive::run(&services, &args),
        Command::Launcher(args) => commands::launcher::run(&services, &args),
    }
}

fn print_help() {
    println!(
        "Entrance V2\n\nUsage:\n  entrance status\n  entrance drawer <subcommand>\n  entrance hive <subcommand>\n  entrance launcher <subcommand>\n  entrance daemon\n  entrance daemon stdio\n  entrance daemon http\n  entrance mcp\n  entrance mcp stdio"
    );
}

pub(crate) fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();
}
