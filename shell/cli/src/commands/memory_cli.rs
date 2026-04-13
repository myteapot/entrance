use anyhow::{bail, Result};

use crate::core::memory_import::import_store_json_into_data_store;

use super::{bootstrap_cli_state, print_json};

pub(super) fn run_memory_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command, flag, value] if command == "import" && flag == "--source" => print_json(
            &import_store_json_into_data_store(&startup.data_store(), value)?,
        ),
        [command, value] if command == "import" => print_json(&import_store_json_into_data_store(
            &startup.data_store(),
            value,
        )?),
        _ => bail!("unsupported memory command, expected `entrance memory import --source <path>`"),
    }
}
