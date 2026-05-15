use anyhow::{bail, Result};
use entrance_core::LauncherQuery;

use crate::{app::AppServices, cli, print_json};

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance launcher refresh\n  entrance launcher list\n  entrance launcher search <query>\n  entrance launcher launch <command>\n  entrance launcher pin <command> <true|false>\n  entrance launcher hotkey"
            );
            Ok(())
        }
        [flag] if cli::is_help(flag) => run(services, &[]),
        [command] if command == "refresh" => {
            let count = services.launcher.refresh(&[])?;
            print_json(&serde_json::json!({ "indexed": count }))
        }
        [command] if command == "list" => print_json(&services.launcher.list()?),
        [command] if command == "hotkey" => {
            print_json(&serde_json::json!({ "hotkey": services.launcher.hotkey() }))
        }
        [command, query] if command == "search" => {
            let results = services.launcher.search(LauncherQuery {
                query: query.clone(),
                limit: 20,
            })?;
            print_json(&results)
        }
        [command, target] if command == "launch" => {
            services.launcher.launch(target, None, None)?;
            print_json(&serde_json::json!({ "launched": target }))
        }
        [command, target, pinned] if command == "pin" => {
            let pinned = matches!(pinned.as_str(), "true" | "1" | "yes" | "on");
            services.launcher.pin(target, pinned)?;
            print_json(&serde_json::json!({ "command": target, "pinned": pinned }))
        }
        _ => bail!("unsupported launcher command"),
    }
}
