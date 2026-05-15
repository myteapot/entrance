use anyhow::{bail, Result};

use crate::command::Command;

pub fn parse(args: &[String]) -> Result<Command> {
    match args {
        [] => Ok(Command::Help),
        [flag] if is_help(flag) => Ok(Command::Help),
        [command] if command == "status" => Ok(Command::Status),
        [command] if command == "daemon" => Ok(Command::DaemonStdio),
        [command, subcommand] if command == "daemon" && subcommand == "stdio" => {
            Ok(Command::DaemonStdio)
        }
        [command, subcommand] if command == "daemon" && subcommand == "http" => {
            Ok(Command::DaemonHttp)
        }
        [command, rest @ ..] if command == "drawer" => Ok(Command::Drawer(rest.to_vec())),
        [command, rest @ ..] if command == "hive" => Ok(Command::Hive(rest.to_vec())),
        [command, rest @ ..] if command == "launcher" => Ok(Command::Launcher(rest.to_vec())),
        _ => bail!("unsupported command; run `entrance --help`"),
    }
}

pub fn is_help(value: &str) -> bool {
    matches!(value, "help" | "-h" | "--help")
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::command::Command;

    #[test]
    fn daemon_subcommands_are_the_only_bridge_entrypoints() {
        assert!(matches!(
            parse(&["daemon".to_string()]).unwrap(),
            Command::DaemonStdio
        ));
        assert!(matches!(
            parse(&["daemon".to_string(), "stdio".to_string()]).unwrap(),
            Command::DaemonStdio
        ));
        assert!(matches!(
            parse(&["daemon".to_string(), "http".to_string()]).unwrap(),
            Command::DaemonHttp
        ));
        assert!(parse(&["mcp".to_string(), "stdio".to_string()]).is_err());
    }
}
