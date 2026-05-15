use anyhow::{bail, Result};

use crate::command::Command;

pub fn parse(args: &[String]) -> Result<Command> {
    match args {
        [] => Ok(Command::Help),
        [flag] if is_help(flag) => Ok(Command::Help),
        [command] if command == "status" => Ok(Command::Status),
        [command] if command == "daemon" => Ok(Command::Daemon),
        [command, subcommand] if command == "mcp" && subcommand == "stdio" => Ok(Command::McpStdio),
        [command, subcommand] if command == "mcp" && subcommand == "http" => Ok(Command::McpHttp),
        [command, rest @ ..] if command == "drawer" => Ok(Command::Drawer(rest.to_vec())),
        [command, rest @ ..] if command == "hive" => Ok(Command::Hive(rest.to_vec())),
        [command, rest @ ..] if command == "launcher" => Ok(Command::Launcher(rest.to_vec())),
        _ => bail!("unsupported command; run `entrance --help`"),
    }
}

pub fn is_help(value: &str) -> bool {
    matches!(value, "help" | "-h" | "--help")
}
