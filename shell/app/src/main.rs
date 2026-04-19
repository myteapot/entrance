mod daemon;

use std::path::PathBuf;

use anyhow::{bail, Result};
use entrance_core::{boot, DrawerFilter, PluginContext, Store};
use entrance_drawer::DrawerPlugin;
use entrance_hive::{HiveDispatchRequest, HivePlugin};
use entrance_launcher::LauncherPlugin;
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
    let kernel = boot()?;
    let ctx = PluginContext {
        kernel: kernel.clone(),
    };

    let drawer = DrawerPlugin::new(&ctx);
    let hive = HivePlugin::new(&ctx);
    let launcher = LauncherPlugin::new(&ctx);

    match args.as_slice() {
        [] => {
            print_help();
            Ok(())
        }
        [flag] if is_help(flag) => {
            print_help();
            Ok(())
        }
        [command] if command == "status" => print_json(&kernel.store.app_status(&kernel.root)?),
        [command] if command == "daemon" => daemon::run_stdio(kernel, drawer, hive, launcher).await,
        [command, subcommand] if command == "mcp" && subcommand == "stdio" => {
            daemon::run_stdio(kernel, drawer, hive, launcher).await
        }
        [command, subcommand] if command == "mcp" && subcommand == "http" => {
            daemon::run_http(kernel, drawer, hive, launcher).await
        }
        [command, rest @ ..] if command == "drawer" => run_drawer_cli(&drawer, rest),
        [command, rest @ ..] if command == "hive" => run_hive_cli(&hive, rest),
        [command, rest @ ..] if command == "launcher" => run_launcher_cli(&launcher, &kernel.store, rest),
        _ => bail!("unsupported command; run `entrance --help`"),
    }
}

fn run_drawer_cli(drawer: &DrawerPlugin, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance drawer summary\n  entrance drawer list\n  entrance drawer add-note --title <text> --body <text>\n  entrance drawer import <path>"
            );
            Ok(())
        }
        [flag] if is_help(flag) => {
            println!(
                "Usage:\n  entrance drawer summary\n  entrance drawer list\n  entrance drawer add-note --title <text> --body <text>\n  entrance drawer import <path>"
            );
            Ok(())
        }
        [command] if command == "summary" => print_json(&drawer.summary()?),
        [command] if command == "list" => print_json(&drawer.list(DrawerFilter::default())?),
        [command, flag, title, flag2, body]
            if command == "add-note" && flag == "--title" && flag2 == "--body" =>
        {
            let id = drawer.add_note(title.clone(), body.clone(), vec!["ai-generated".to_string()])?;
            print_json(&serde_json::json!({ "id": id }))
        }
        [command, path] if command == "import" => {
            let id = drawer.import_path(PathBuf::from(path), vec!["imported".to_string()])?;
            print_json(&serde_json::json!({ "id": id }))
        }
        _ => bail!("unsupported drawer command"),
    }
}

fn run_hive_cli(hive: &HivePlugin, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]"
            );
            Ok(())
        }
        [flag] if is_help(flag) => {
            println!(
                "Usage:\n  entrance hive list\n  entrance hive dispatch --title <text> [--project <path>] [--summary <text>]"
            );
            Ok(())
        }
        [command] if command == "list" => print_json(&hive.list()?),
        [command, flag, title] if command == "dispatch" && flag == "--title" => {
            let report = hive.dispatch(HiveDispatchRequest {
                title: title.clone(),
                project_dir: None,
                summary: None,
                payload_json: "{}".to_string(),
            })?;
            print_json(&report)
        }
        [command, flag, title, flag2, project]
            if command == "dispatch" && flag == "--title" && flag2 == "--project" =>
        {
            let report = hive.dispatch(HiveDispatchRequest {
                title: title.clone(),
                project_dir: Some(project.clone()),
                summary: None,
                payload_json: "{}".to_string(),
            })?;
            print_json(&report)
        }
        _ => bail!("unsupported hive command"),
    }
}

fn run_launcher_cli(launcher: &LauncherPlugin, _store: &Store, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance launcher refresh\n  entrance launcher search <query>\n  entrance launcher launch <command>\n  entrance launcher pin <command> <true|false>\n  entrance launcher hotkey"
            );
            Ok(())
        }
        [flag] if is_help(flag) => {
            println!(
                "Usage:\n  entrance launcher refresh\n  entrance launcher search <query>\n  entrance launcher launch <command>\n  entrance launcher pin <command> <true|false>\n  entrance launcher hotkey"
            );
            Ok(())
        }
        [command] if command == "refresh" => {
            let count = launcher.refresh(&[])?;
            print_json(&serde_json::json!({ "indexed": count }))
        }
        [command] if command == "hotkey" => print_json(&serde_json::json!({ "hotkey": launcher.hotkey() })),
        [command, query] if command == "search" => {
            let results = launcher.search(entrance_core::LauncherQuery {
                query: query.clone(),
                limit: 20,
            })?;
            print_json(&results)
        }
        [command, target] if command == "launch" => {
            launcher.launch(target, None, None)?;
            print_json(&serde_json::json!({ "launched": target }))
        }
        [command, target, pinned] if command == "pin" => {
            let pinned = matches!(pinned.as_str(), "true" | "1" | "yes" | "on");
            launcher.pin(target, pinned)?;
            print_json(&serde_json::json!({ "command": target, "pinned": pinned }))
        }
        _ => bail!("unsupported launcher command"),
    }
}

fn is_help(value: &str) -> bool {
    matches!(value, "help" | "-h" | "--help")
}

fn print_help() {
    println!(
        "Entrance V2\n\nUsage:\n  entrance status\n  entrance drawer <subcommand>\n  entrance hive <subcommand>\n  entrance launcher <subcommand>\n  entrance daemon\n  entrance mcp stdio\n  entrance mcp http"
    );
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .try_init();
}
