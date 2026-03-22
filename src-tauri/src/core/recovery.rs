use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::core::data_store::{
    DataStore, NewPromotionRecord, NewSourceArtifact, NewSourceIngestRun, SourceIngestRunCompletion,
};

const RECOVERY_SEED_SOURCE_SYSTEM: &str = "recovery_seed";
const RECOVERY_SEED_SOURCE_WORKSPACE: &str = "repo_root";
const RECOVERY_SEED_SOURCE_PROJECT: &str = "Entrance";
const RECOVERY_SEED_TABLES: [&str; 10] = [
    "schema_meta",
    "chat_logs",
    "coffee_chats",
    "decisions",
    "documents",
    "instincts",
    "memory_fragments",
    "memory_links",
    "todos",
    "visions",
];

#[derive(Debug, Clone, Serialize)]
pub struct RecoverySeedImportReport {
    pub ingest_run_id: i64,
    pub source_system: String,
    pub source_workspace: String,
    pub source_project: String,
    pub artifact_path: String,
    pub artifact_sha256: String,
    pub imported_table_count: i64,
    pub imported_row_count: i64,
    pub imported_artifact_count: i64,
    pub manifest_artifact_id: i64,
    pub source_manifest_artifact_id: Option<i64>,
    pub table_row_counts: BTreeMap<String, i64>,
}

#[derive(Debug)]
struct RecoverySeedImportProgress {
    imported_row_count: i64,
    imported_artifact_count: i64,
}

#[derive(Debug)]
struct RecoverySeedSnapshot {
    table_row_counts: BTreeMap<String, i64>,
    tables: Vec<RecoverySeedTableDump>,
    source_manifest_path: Option<PathBuf>,
    source_manifest_payload: Option<String>,
}

#[derive(Debug)]
struct RecoverySeedTableDump {
    table_name: String,
    rows: Vec<Map<String, Value>>,
}

pub fn import_recovery_seed(
    data_store: &DataStore,
    artifact_path: impl AsRef<Path>,
) -> Result<RecoverySeedImportReport> {
    let artifact_path = artifact_path.as_ref();
    let bytes = fs::read(artifact_path).with_context(|| {
        format!(
            "failed to read recovery seed db `{}`",
            artifact_path.display()
        )
    })?;
    let artifact_sha256 = sha256_hex(&bytes);
    let snapshot = read_recovery_seed_snapshot(artifact_path)?;

    if snapshot.table_row_counts.is_empty() {
        return Err(anyhow!(
            "recovery seed db `{}` does not contain any recognized recovery tables",
            artifact_path.display()
        ));
    }

    let run = data_store.create_source_ingest_run(NewSourceIngestRun {
        source_system: RECOVERY_SEED_SOURCE_SYSTEM,
        source_workspace: RECOVERY_SEED_SOURCE_WORKSPACE,
        source_project: RECOVERY_SEED_SOURCE_PROJECT,
        artifact_path: Some(&artifact_path.to_string_lossy()),
        artifact_sha256: Some(&artifact_sha256),
        status: "running",
    })?;

    let mut progress = RecoverySeedImportProgress {
        imported_row_count: 0,
        imported_artifact_count: 0,
    };

    let import_result = import_recovery_seed_snapshot(
        data_store,
        run.id,
        artifact_path,
        &artifact_sha256,
        &snapshot,
        &mut progress,
    );

    match import_result {
        Ok((manifest_artifact_id, source_manifest_artifact_id)) => {
            let completed = data_store.complete_source_ingest_run(
                run.id,
                SourceIngestRunCompletion {
                    status: "completed",
                    imported_issue_count: 0,
                    imported_document_count: 0,
                    imported_milestone_count: 0,
                    imported_planning_item_count: 0,
                    error_message: None,
                },
            )?;

            Ok(RecoverySeedImportReport {
                ingest_run_id: completed.id,
                source_system: completed.source_system,
                source_workspace: completed.source_workspace,
                source_project: completed.source_project,
                artifact_path: completed
                    .artifact_path
                    .unwrap_or_else(|| artifact_path.to_string_lossy().to_string()),
                artifact_sha256: completed
                    .artifact_sha256
                    .unwrap_or_else(|| artifact_sha256.clone()),
                imported_table_count: snapshot.table_row_counts.len() as i64,
                imported_row_count: progress.imported_row_count,
                imported_artifact_count: progress.imported_artifact_count,
                manifest_artifact_id,
                source_manifest_artifact_id,
                table_row_counts: snapshot.table_row_counts,
            })
        }
        Err(error) => {
            let message = error.to_string();
            let _ = data_store.complete_source_ingest_run(
                run.id,
                SourceIngestRunCompletion {
                    status: "failed",
                    imported_issue_count: 0,
                    imported_document_count: 0,
                    imported_milestone_count: 0,
                    imported_planning_item_count: 0,
                    error_message: Some(&message),
                },
            );
            Err(error)
        }
    }
}

fn import_recovery_seed_snapshot(
    data_store: &DataStore,
    ingest_run_id: i64,
    artifact_path: &Path,
    artifact_sha256: &str,
    snapshot: &RecoverySeedSnapshot,
    progress: &mut RecoverySeedImportProgress,
) -> Result<(i64, Option<i64>)> {
    let source_db_path = artifact_path.to_string_lossy().to_string();
    let mut source_manifest_artifact_id = None;

    if let (Some(manifest_path), Some(manifest_payload)) = (
        snapshot.source_manifest_path.as_ref(),
        snapshot.source_manifest_payload.as_ref(),
    ) {
        let manifest_payload_json = wrap_source_manifest_payload(manifest_path, manifest_payload)?;
        let manifest_artifact = data_store.insert_source_artifact(NewSourceArtifact {
            ingest_run_id,
            artifact_kind: "recovery_seed_file_manifest",
            artifact_key: &format!("manifest:{artifact_sha256}"),
            title: Some(&manifest_path.to_string_lossy()),
            url: None,
            payload_json: &manifest_payload_json,
        })?;
        append_storage_only_promotion(
            data_store,
            manifest_artifact.id,
            "preserved adjacent recovery seed manifest file",
            ingest_run_id,
        )?;
        progress.imported_artifact_count += 1;
        source_manifest_artifact_id = Some(manifest_artifact.id);
    }

    for table in &snapshot.tables {
        for (index, row) in table.rows.iter().enumerate() {
            let artifact_payload = serde_json::to_string(&json!({
                "source_system": RECOVERY_SEED_SOURCE_SYSTEM,
                "source_workspace": RECOVERY_SEED_SOURCE_WORKSPACE,
                "source_project": RECOVERY_SEED_SOURCE_PROJECT,
                "source_db_path": source_db_path,
                "source_db_sha256": artifact_sha256,
                "source_table": table.table_name,
                "source_row_key": artifact_row_key(row, index),
                "source_row": row,
            }))
            .context("failed to serialize recovery seed row payload")?;

            let title = artifact_title(&table.table_name, row);
            let artifact_key = format!("{}:{}", table.table_name, artifact_row_key(row, index));
            let artifact = data_store.insert_source_artifact(NewSourceArtifact {
                ingest_run_id,
                artifact_kind: "recovery_seed_row",
                artifact_key: &artifact_key,
                title: title.as_deref(),
                url: None,
                payload_json: &artifact_payload,
            })?;
            append_storage_only_promotion(
                data_store,
                artifact.id,
                "imported recovery seed row into runtime storage truth",
                ingest_run_id,
            )?;
            progress.imported_row_count += 1;
            progress.imported_artifact_count += 1;
        }
    }

    let manifest_payload = serde_json::to_string(&json!({
        "source_system": RECOVERY_SEED_SOURCE_SYSTEM,
        "source_workspace": RECOVERY_SEED_SOURCE_WORKSPACE,
        "source_project": RECOVERY_SEED_SOURCE_PROJECT,
        "source_db_path": source_db_path,
        "source_db_sha256": artifact_sha256,
        "table_row_counts": snapshot.table_row_counts,
        "recognized_tables": snapshot
            .tables
            .iter()
            .map(|table| table.table_name.as_str())
            .collect::<Vec<_>>(),
        "imported_row_count": progress.imported_row_count,
        "source_manifest_path": snapshot
            .source_manifest_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
    }))
    .context("failed to serialize recovery seed import manifest payload")?;

    let manifest_artifact = data_store.insert_source_artifact(NewSourceArtifact {
        ingest_run_id,
        artifact_kind: "recovery_seed_manifest",
        artifact_key: &format!("db:{artifact_sha256}"),
        title: Some(&source_db_path),
        url: None,
        payload_json: &manifest_payload,
    })?;
    append_storage_only_promotion(
        data_store,
        manifest_artifact.id,
        "recorded recovery seed import manifest in runtime storage truth",
        ingest_run_id,
    )?;
    progress.imported_artifact_count += 1;

    Ok((manifest_artifact.id, source_manifest_artifact_id))
}

fn read_recovery_seed_snapshot(artifact_path: &Path) -> Result<RecoverySeedSnapshot> {
    let connection = Connection::open_with_flags(
        artifact_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| {
        format!(
            "failed to open recovery seed sqlite db `{}` read-only",
            artifact_path.display()
        )
    })?;

    let mut table_row_counts = BTreeMap::new();
    let mut tables = Vec::new();

    for table_name in RECOVERY_SEED_TABLES {
        if !table_exists(&connection, table_name)? {
            continue;
        }

        let rows = read_table_rows(&connection, table_name)?;
        table_row_counts.insert(table_name.to_string(), rows.len() as i64);
        tables.push(RecoverySeedTableDump {
            table_name: table_name.to_string(),
            rows,
        });
    }

    let source_manifest_path = adjacent_manifest_path(artifact_path).filter(|path| path.exists());
    let source_manifest_payload = source_manifest_path
        .as_ref()
        .map(|path| {
            fs::read_to_string(path).with_context(|| {
                format!("failed to read recovery seed manifest `{}`", path.display())
            })
        })
        .transpose()?;

    Ok(RecoverySeedSnapshot {
        table_row_counts,
        tables,
        source_manifest_path,
        source_manifest_payload,
    })
}

fn read_table_rows(connection: &Connection, table_name: &str) -> Result<Vec<Map<String, Value>>> {
    let query = if table_has_column(connection, table_name, "id")? {
        format!("SELECT * FROM {table_name} ORDER BY id ASC")
    } else {
        format!("SELECT * FROM {table_name}")
    };
    let mut statement = connection
        .prepare(&query)
        .with_context(|| format!("failed to prepare recovery table query for `{table_name}`"))?;
    let column_names = statement
        .column_names()
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut rows = statement
        .query([])
        .with_context(|| format!("failed to query recovery table `{table_name}`"))?;
    let mut output = Vec::new();

    while let Some(row) = rows.next()? {
        let mut object = Map::new();
        for (index, column_name) in column_names.iter().enumerate() {
            object.insert(
                column_name.clone(),
                sqlite_value_to_json(row.get_ref(index)?),
            );
        }
        output.push(object);
    }

    Ok(output)
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => Value::String(hex_encode(value)),
    }
}

fn append_storage_only_promotion(
    data_store: &DataStore,
    artifact_id: i64,
    reason: &str,
    ingest_run_id: i64,
) -> Result<()> {
    data_store.append_promotion_record(NewPromotionRecord {
        subject_kind: "source_artifact",
        subject_id: artifact_id,
        promotion_state: "storage_only",
        reason: Some(reason),
        source_ingest_run_id: Some(ingest_run_id),
    })?;
    Ok(())
}

fn artifact_row_key(row: &Map<String, Value>, index: usize) -> String {
    row.get("id")
        .map(json_key_fragment)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("row-{}", index + 1))
}

fn artifact_title(table_name: &str, row: &Map<String, Value>) -> Option<String> {
    let candidates = [
        "title",
        "slug",
        "pattern",
        "project",
        "relation_type",
        "version",
        "session_id",
    ];
    for key in candidates {
        let Some(value) = row.get(key) else {
            continue;
        };
        let fragment = json_key_fragment(value);
        if !fragment.is_empty() {
            return Some(format!("{table_name}:{fragment}"));
        }
    }

    row.get("id")
        .map(json_key_fragment)
        .filter(|value| !value.is_empty())
        .map(|value| format!("{table_name}:{value}"))
}

fn json_key_fragment(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.trim().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn wrap_source_manifest_payload(manifest_path: &Path, manifest_payload: &str) -> Result<String> {
    let manifest_json = serde_json::from_str::<Value>(manifest_payload)
        .unwrap_or_else(|_| Value::String(manifest_payload.to_string()));
    serde_json::to_string(&json!({
        "manifest_path": manifest_path.to_string_lossy().to_string(),
        "manifest": manifest_json,
    }))
    .context("failed to serialize adjacent recovery seed manifest payload")
}

fn adjacent_manifest_path(db_path: &Path) -> Option<PathBuf> {
    let file_name = db_path.file_name()?.to_string_lossy();
    Some(db_path.with_file_name(format!("{file_name}.manifest.json")))
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table_name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists != 0)
}

fn table_has_column(connection: &Connection, table_name: &str, column_name: &str) -> Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|column| column == column_name))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = std::fmt::Write::write_fmt(&mut output, format_args!("{byte:02x}"));
    }
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}
