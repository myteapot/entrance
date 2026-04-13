use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::core::compiler::{registry::RegistryEntry, semantics::EffectiveControlSemantics};

use super::{bootstrap_cli_state, print_json};

pub(super) fn run_compiler_cli(args: &[String]) -> Result<()> {
    let startup = bootstrap_cli_state()?;

    match args {
        [command, subcommand] if command == "registry" && subcommand == "list" => {
            print_json(&startup.data_store().list_compiler_registry_snapshot()?)
        }
        [command, subcommand, rest @ ..] if command == "registry" && subcommand == "list" => {
            let options = parse_registry_list_options(rest)?;
            let entries = startup.data_store().list_compiler_registry_snapshot()?;
            print_registry_entries(&entries, options)
        }
        _ => bail!(
            "unsupported compiler command, expected `entrance compiler registry list [--format <json|table>] [--include-semantics]`"
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistryCliFormat {
    Json,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegistryListOptions {
    format: RegistryCliFormat,
    include_semantics: bool,
}

#[derive(Debug, Clone, Serialize)]
struct RegistryEntryWithSemantics {
    #[serde(flatten)]
    entry: RegistryEntry,
    effective_semantics: EffectiveControlSemantics,
}

impl RegistryEntryWithSemantics {
    fn from_entry(entry: &RegistryEntry) -> Self {
        Self {
            entry: entry.clone(),
            effective_semantics: entry.effective_semantics(),
        }
    }
}

fn parse_registry_list_options(args: &[String]) -> Result<RegistryListOptions> {
    let mut format = RegistryCliFormat::Json;
    let mut include_semantics = false;
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
            "--include-semantics" => {
                include_semantics = true;
                index += 1;
            }
            other => bail!("unsupported compiler registry list argument `{other}`"),
        }
    }

    Ok(RegistryListOptions {
        format,
        include_semantics,
    })
}

fn print_registry_entries(entries: &[RegistryEntry], options: RegistryListOptions) -> Result<()> {
    match (options.format, options.include_semantics) {
        (RegistryCliFormat::Json, true) => {
            let entries = entries
                .iter()
                .map(RegistryEntryWithSemantics::from_entry)
                .collect::<Vec<_>>();
            print_json(&entries)
        }
        (RegistryCliFormat::Json, false) => print_json(entries),
        (RegistryCliFormat::Table, include_semantics) => {
            print_registry_table(entries, include_semantics)
        }
    }
}

fn print_registry_table(entries: &[RegistryEntry], include_semantics: bool) -> Result<()> {
    let mut headers = vec![
        "primitive".to_string(),
        "object_kind".to_string(),
        "flow_phase".to_string(),
        "attention_state".to_string(),
        "integrity_overlay".to_string(),
        "control_policy".to_string(),
        "writer_policy".to_string(),
        "route_policy".to_string(),
        "gate_policy".to_string(),
        "sandbox_policy".to_string(),
        "effect_kind".to_string(),
        "supervision_scope".to_string(),
        "allowed_roles".to_string(),
        "allowed_rooms".to_string(),
    ];
    if include_semantics {
        headers.extend([
            "requires_admission_gate".to_string(),
            "writes_truth".to_string(),
            "requires_supervision".to_string(),
            "sandbox_requirement".to_string(),
            "hot_projection_allowed".to_string(),
            "requires_human_approval".to_string(),
            "is_read_only".to_string(),
            "routing_constraint".to_string(),
        ]);
    }

    let rows = entries
        .iter()
        .map(|entry| registry_entry_table_row(entry, include_semantics))
        .collect::<Result<Vec<_>>>()?;
    let widths = column_widths(&headers, &rows);

    println!("{}", format_table_row(&headers, &widths));
    println!("{}", format_table_divider(&widths));
    for row in rows {
        println!("{}", format_table_row(&row, &widths));
    }

    Ok(())
}

fn registry_entry_table_row(entry: &RegistryEntry, include_semantics: bool) -> Result<Vec<String>> {
    let mut row = vec![
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
    ];

    if include_semantics {
        let semantics = entry.effective_semantics();
        row.extend([
            display_scalar(&semantics.requires_admission_gate)?,
            display_scalar(&semantics.writes_truth)?,
            display_scalar(&semantics.requires_supervision)?,
            display_scalar(&semantics.sandbox_requirement)?,
            display_scalar(&semantics.hot_projection_allowed)?,
            display_scalar(&semantics.requires_human_approval)?,
            display_scalar(&semantics.is_read_only)?,
            display_scalar(&semantics.routing_constraint)?,
        ]);
    }

    Ok(row)
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

fn column_widths(headers: &[impl AsRef<str>], rows: &[Vec<String>]) -> Vec<usize> {
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|value| value.len())
                .fold(header.as_ref().len(), usize::max)
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
