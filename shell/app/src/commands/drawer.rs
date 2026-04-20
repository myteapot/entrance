use std::path::PathBuf;

use anyhow::{bail, Result};
use entrance_core::DrawerFilter;
use entrance_drawer::VaultSecret;

use crate::{app::AppServices, cli, print_json};

pub fn run(services: &AppServices, args: &[String]) -> Result<()> {
    match args {
        [] => {
            println!(
                "Usage:\n  entrance drawer summary\n  entrance drawer list\n  entrance drawer add-note --title <text> --body <text>\n  entrance drawer import <path>\n  entrance drawer memory import --title <text> --body <text>\n  entrance drawer organize plan\n  entrance drawer organize apply\n  entrance drawer history\n  entrance drawer snapshot <summary>\n  entrance drawer rollback <commit>\n  entrance drawer vault store --title <text> --secret <text>\n  entrance drawer vault list"
            );
            Ok(())
        }
        [flag] if cli::is_help(flag) => run(services, &[]),
        [command] if command == "summary" => print_json(&services.drawer.summary()?),
        [command] if command == "list" => print_json(&services.drawer.list(DrawerFilter::default())?),
        [command, flag, title, flag2, body]
            if command == "add-note" && flag == "--title" && flag2 == "--body" =>
        {
            let id = services
                .drawer
                .add_note(title.clone(), body.clone(), vec!["ai-generated".to_string()])?;
            print_json(&serde_json::json!({ "id": id }))
        }
        [command, path] if command == "import" => {
            let report = services
                .drawer
                .import_path_report(PathBuf::from(path), vec!["imported".to_string()])?;
            print_json(&report)
        }
        [domain, command, flag, title, flag2, body]
            if domain == "memory" && command == "import" && flag == "--title" && flag2 == "--body" =>
        {
            print_json(&services.drawer.import_memory(
                title.clone(),
                body.clone(),
                vec!["ai-generated".to_string()],
            )?)
        }
        [command, subcommand] if command == "organize" && subcommand == "plan" => {
            print_json(&services.drawer.plan_reorganization()?)
        }
        [command, subcommand] if command == "organize" && subcommand == "apply" => {
            let applied = services
                .drawer
                .apply_reorganization(services.drawer.plan_reorganization()?)?;
            print_json(&serde_json::json!({ "applied": applied }))
        }
        [command] if command == "history" => print_json(&services.drawer.history(20)?),
        [command, summary] if command == "snapshot" => print_json(&services.drawer.snapshot(summary)?),
        [command, target] if command == "rollback" => {
            services.drawer.rollback(target)?;
            print_json(&serde_json::json!({ "rolled_back_to": target }))
        }
        [domain, command, flag, title, flag2, secret]
            if domain == "vault" && command == "store" && flag == "--title" && flag2 == "--secret" =>
        {
            print_json(&services.drawer.store_secret(VaultSecret {
                title: title.clone(),
                secret: secret.clone(),
                tags: vec![],
            })?)
        }
        [domain, command] if domain == "vault" && command == "list" => {
            print_json(&services.drawer.list_secrets()?)
        }
        _ => bail!("unsupported drawer command"),
    }
}
