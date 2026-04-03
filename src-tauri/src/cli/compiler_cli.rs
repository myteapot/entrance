use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::core::compiler::registry::RegistryEntry;

use super::{bootstrap_cli_state, print_json};

pub(super) fn run_compiler_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command, subcommand] if command == "registry" && subcommand == "list" => {
            print_json(&startup.data_store().list_compiler_registry_snapshot()?)
        }
        [command, subcommand, rest @ ..] if command == "registry" && subcommand == "list" => {
            let format = parse_registry_list_format(rest)?;
            let entries = startup.data_store().list_compiler_registry_snapshot()?;
            print_registry_entries(&entries, format)
        }
        _ => bail!(
            "unsupported compiler command, expected `entrance compiler registry list [--format <json|table>]`"
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistryCliFormat {
    Json,
    Table,
}

fn parse_registry_list_format(args: &[String]) -> Result<RegistryCliFormat> {
    let mut format = RegistryCliFormat::Json;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                let value = args
                    .get(index + 1)
                    .context("`entrance compiler registry list --format` requires a value")?;
                format = match value.trim() {
                    "json" => RegistryCliFormat::Json,
                    "table" => RegistryCliFormat::Table,
                    other => bail!(
                        "unsupported compiler registry output format `{other}`, expected `json` or `table`"
                    ),
                };
                index += 2;
            }
            other => bail!("unsupported compiler registry list argument `{other}`"),
        }
    }

    Ok(format)
}

fn print_registry_entries(entries: &[RegistryEntry], format: RegistryCliFormat) -> Result<()> {
    match format {
        RegistryCliFormat::Json => print_json(entries),
        RegistryCliFormat::Table => print_registry_table(entries),
    }
}

fn print_registry_table(entries: &[RegistryEntry]) -> Result<()> {
    let headers = [
        "primitive",
        "object_kind",
        "flow_phase",
        "attention_state",
        "integrity_overlay",
        "control_policy",
        "writer_policy",
        "route_policy",
        "gate_policy",
        "sandbox_policy",
        "effect_kind",
        "supervision_scope",
        "allowed_roles",
        "allowed_rooms",
    ];

    let rows = entries
        .iter()
        .map(registry_entry_table_row)
        .collect::<Result<Vec<_>>>()?;
    let widths = column_widths(&headers, &rows);

    println!("{}", format_table_row(&headers, &widths));
    println!("{}", format_table_divider(&widths));
    for row in rows {
        println!("{}", format_table_row(&row, &widths));
    }

    Ok(())
}

fn registry_entry_table_row(entry: &RegistryEntry) -> Result<Vec<String>> {
    Ok(vec![
        display_scalar(&entry.primitive)?,
        display_scalar(&entry.object_kind)?,
        display_scalar(&entry.flow_phase)?,
        display_scalar(&entry.attention_state)?,
        display_scalar(&entry.integrity_overlay)?,
        display_scalar(&entry.control_policy)?,
        display_scalar(&entry.writer_policy)?,
        display_scalar(&entry.route_policy)?,
        display_scalar(&entry.gate_policy)?,
        display_scalar(&entry.sandbox_policy)?,
        display_scalar(&entry.effect_kind)?,
        display_scalar(&entry.supervision_scope)?,
        display_scalar(&entry.allowed_roles)?,
        display_scalar(&entry.allowed_rooms)?,
    ])
}

fn display_scalar<T: Serialize>(value: &T) -> Result<String> {
    let value =
        serde_json::to_value(value).context("failed to serialize compiler registry field")?;
    Ok(match value {
        Value::Null => "-".to_string(),
        Value::String(text) => text,
        other => other.to_string(),
    })
}

fn column_widths(headers: &[&str], rows: &[Vec<String>]) -> Vec<usize> {
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|value| value.len())
                .fold(header.len(), usize::max)
        })
        .collect()
}

fn format_table_row(columns: &[impl AsRef<str>], widths: &[usize]) -> String {
    columns
        .iter()
        .zip(widths.iter())
        .map(|(column, width)| format!("{:<width$}", column.as_ref(), width = width))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn format_table_divider(widths: &[usize]) -> String {
    widths
        .iter()
        .map(|width| "-".repeat(*width))
        .collect::<Vec<_>>()
        .join("-+-")
}
