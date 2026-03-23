use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::core::data_store::{
    DataStore, NewPromotionRecord, NewSourceArtifact, NewSourceIngestRun,
    SourceIngestRunCompletion, StoredPromotionRecord, StoredSourceArtifact, StoredSourceIngestRun,
    UpsertCoffeeChatRecord, UpsertDecisionRecord, UpsertDocumentRecord, UpsertInstinctRecord,
    UpsertMemoryFragmentRecord, UpsertMemoryLinkRecord, UpsertTodoRecord, UpsertVisionRecord,
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
const SAFE_RECOVERY_PROMOTION_TABLES: [&str; 4] =
    ["coffee_chats", "documents", "instincts", "todos"];
const REMAINING_RECOVERY_PROMOTION_TABLES: [&str; 4] =
    ["decisions", "memory_fragments", "memory_links", "visions"];

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

#[derive(Debug, Clone, Serialize)]
pub struct RecoverySeedRunSummary {
    #[serde(flatten)]
    pub run: StoredSourceIngestRun,
    pub imported_table_count: i64,
    pub imported_row_count: i64,
    pub imported_artifact_count: i64,
    pub recognized_tables: Vec<String>,
    pub table_row_counts: BTreeMap<String, i64>,
    pub source_manifest_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoverySeedRowSummary {
    #[serde(flatten)]
    pub artifact: StoredSourceArtifact,
    pub promotion_state: Option<String>,
    pub promotion_reason: Option<String>,
    pub promotion_recorded_at: Option<String>,
    pub source_table: String,
    pub source_row_key: String,
    pub source_row: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoverySeedRowsReport {
    pub ingest_run: RecoverySeedRunSummary,
    pub requested_table: Option<String>,
    pub limit: usize,
    pub total_matching_rows: usize,
    pub rows: Vec<RecoverySeedRowSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoverySeedPromotionReport {
    pub ingest_run: RecoverySeedRunSummary,
    pub requested_table: Option<String>,
    pub promoted_tables: Vec<String>,
    pub total_candidate_rows: i64,
    pub upserted_row_count: i64,
    pub new_promotion_record_count: i64,
    pub rows_by_table: BTreeMap<String, i64>,
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

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedManifestArtifactPayload {
    #[serde(default)]
    imported_row_count: i64,
    #[serde(default)]
    recognized_tables: Vec<String>,
    #[serde(default)]
    table_row_counts: BTreeMap<String, i64>,
    #[serde(default)]
    source_manifest_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedRowArtifactPayload {
    source_table: String,
    source_row_key: String,
    source_row: Value,
}

#[derive(Debug, Clone, Default)]
pub struct RecoverySeedRowsQuery {
    pub ingest_run_id: Option<i64>,
    pub table_name: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct RecoverySeedPromotionQuery {
    pub ingest_run_id: Option<i64>,
    pub table_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedDocumentRow {
    id: i64,
    slug: String,
    title: String,
    content: String,
    category: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedTodoRow {
    id: i64,
    title: String,
    status: String,
    priority: i64,
    project: String,
    created_at: String,
    #[serde(default)]
    done_at: Option<String>,
    #[serde(default = "default_warm")]
    temperature: String,
    #[serde(default)]
    due_on: String,
    #[serde(default)]
    remind_every_days: i64,
    #[serde(default)]
    remind_next_on: String,
    #[serde(default)]
    last_reminded_at: String,
    #[serde(default = "default_none")]
    reminder_status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedInstinctRow {
    id: i64,
    pattern: String,
    action: String,
    confidence: f64,
    #[serde(default)]
    source: String,
    #[serde(default, rename = "ref")]
    reference: String,
    created_at: String,
    #[serde(default = "default_active")]
    status: String,
    #[serde(default)]
    surfaced_to: String,
    #[serde(default)]
    review_status: String,
    #[serde(default)]
    origin_type: String,
    #[serde(default = "default_active")]
    lifecycle_status: String,
    #[serde(default = "default_warm")]
    temperature: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedCoffeeChatRow {
    id: i64,
    project: String,
    stage: String,
    retro: String,
    forward: String,
    priorities: String,
    created_at: String,
    #[serde(default = "default_warm")]
    temperature: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedDecisionRow {
    id: i64,
    title: String,
    statement: String,
    #[serde(default)]
    rationale: String,
    #[serde(default)]
    decision_type: String,
    #[serde(default = "default_accepted")]
    decision_status: String,
    #[serde(default)]
    scope_type: String,
    #[serde(default)]
    scope_ref: String,
    #[serde(default)]
    source_ref: String,
    #[serde(default)]
    decided_by: String,
    #[serde(default)]
    enforcement_level: String,
    #[serde(default)]
    actor_scope: String,
    #[serde(default = "default_unit_confidence")]
    confidence: f64,
    created_at: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedVisionRow {
    id: i64,
    title: String,
    statement: String,
    #[serde(default)]
    horizon: String,
    #[serde(default = "default_active")]
    vision_status: String,
    #[serde(default)]
    scope_type: String,
    #[serde(default)]
    scope_ref: String,
    #[serde(default)]
    source_ref: String,
    #[serde(default = "default_unit_confidence")]
    confidence: f64,
    created_at: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedMemoryFragmentRow {
    id: i64,
    title: String,
    content: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    source_type: String,
    #[serde(default)]
    source_ref: String,
    #[serde(default)]
    source_hash: String,
    #[serde(default)]
    scope_type: String,
    #[serde(default)]
    scope_ref: String,
    #[serde(default)]
    target_table: String,
    #[serde(default)]
    target_ref: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    triage_status: String,
    #[serde(default = "default_warm")]
    temperature: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    notes: String,
    #[serde(default = "default_unit_confidence")]
    confidence: f64,
    created_at: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RecoverySeedMemoryLinkRow {
    id: i64,
    src_kind: String,
    src_id: i64,
    dst_kind: String,
    dst_id: i64,
    relation_type: String,
    #[serde(default = "default_active")]
    status: String,
    created_at: String,
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

pub fn list_recovery_seed_runs(data_store: &DataStore) -> Result<Vec<RecoverySeedRunSummary>> {
    let runs = data_store.list_source_ingest_runs()?;
    let mut summaries = Vec::new();

    for run in runs
        .into_iter()
        .filter(|run| run.source_system == RECOVERY_SEED_SOURCE_SYSTEM)
    {
        let artifacts = data_store.list_source_artifacts(run.id)?;
        let manifest = artifacts
            .iter()
            .find(|artifact| artifact.artifact_kind == "recovery_seed_manifest")
            .map(parse_recovery_seed_manifest_payload)
            .transpose()?
            .unwrap_or_else(|| RecoverySeedManifestArtifactPayload {
                imported_row_count: 0,
                recognized_tables: Vec::new(),
                table_row_counts: BTreeMap::new(),
                source_manifest_path: None,
            });

        summaries.push(RecoverySeedRunSummary {
            run,
            imported_table_count: manifest.table_row_counts.len() as i64,
            imported_row_count: manifest.imported_row_count,
            imported_artifact_count: artifacts.len() as i64,
            recognized_tables: manifest.recognized_tables,
            table_row_counts: manifest.table_row_counts,
            source_manifest_path: manifest.source_manifest_path,
        });
    }

    Ok(summaries)
}

pub fn list_recovery_seed_rows(
    data_store: &DataStore,
    query: RecoverySeedRowsQuery,
) -> Result<RecoverySeedRowsReport> {
    let ingest_run = resolve_recovery_seed_run(data_store, query.ingest_run_id)?;

    let limit = query.limit.unwrap_or(50);
    if limit == 0 {
        return Err(anyhow!("recovery rows `limit` must be >= 1"));
    }

    let requested_table = normalized_optional_table_name(query.table_name.as_deref());
    let mut rows = collect_recovery_seed_rows(data_store, ingest_run.run.id, requested_table)?;

    let total_matching_rows = rows.len();
    rows.truncate(limit);

    Ok(RecoverySeedRowsReport {
        ingest_run,
        requested_table: requested_table.map(str::to_string),
        limit,
        total_matching_rows,
        rows,
    })
}

pub fn promote_safe_recovery_seed_v0(
    data_store: &DataStore,
    query: RecoverySeedPromotionQuery,
) -> Result<RecoverySeedPromotionReport> {
    let ingest_run = resolve_recovery_seed_run(data_store, query.ingest_run_id)?;
    let requested_table = normalized_optional_table_name(query.table_name.as_deref());
    promote_recovery_seed_tables(
        data_store,
        ingest_run,
        requested_table,
        &SAFE_RECOVERY_PROMOTION_TABLES,
        "safe recovery promotion",
    )
}

pub fn promote_remaining_recovery_seed_v0(
    data_store: &DataStore,
    query: RecoverySeedPromotionQuery,
) -> Result<RecoverySeedPromotionReport> {
    let ingest_run = resolve_recovery_seed_run(data_store, query.ingest_run_id)?;
    let requested_table = normalized_optional_table_name(query.table_name.as_deref());
    promote_recovery_seed_tables(
        data_store,
        ingest_run,
        requested_table,
        &REMAINING_RECOVERY_PROMOTION_TABLES,
        "remaining recovery promotion",
    )
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

fn promote_recovery_seed_tables(
    data_store: &DataStore,
    ingest_run: RecoverySeedRunSummary,
    requested_table: Option<&str>,
    allowed_tables: &[&str],
    promotion_label: &str,
) -> Result<RecoverySeedPromotionReport> {
    if let Some(requested_table) = requested_table {
        if !allowed_tables.contains(&requested_table) {
            return Err(anyhow!(
                "unsupported {promotion_label} table `{requested_table}`; use one of: {}",
                allowed_tables.join(", ")
            ));
        }
    }

    let rows = collect_recovery_seed_rows(data_store, ingest_run.run.id, requested_table)?
        .into_iter()
        .filter(|row| allowed_tables.contains(&row.source_table.as_str()))
        .collect::<Vec<_>>();
    let latest_promotions = latest_promotion_map(data_store.list_promotion_records()?);
    let mut promoted_tables = Vec::new();
    let mut rows_by_table = BTreeMap::new();
    let mut new_promotion_record_count = 0i64;

    for row in &rows {
        promote_recovery_seed_row(data_store, row)?;

        let latest_state = latest_promotions
            .get(&("source_artifact".to_string(), row.artifact.id))
            .map(|record| record.promotion_state.as_str());
        if latest_state != Some("cold_promoted") {
            data_store.append_promotion_record(NewPromotionRecord {
                subject_kind: "source_artifact",
                subject_id: row.artifact.id,
                promotion_state: "cold_promoted",
                reason: Some(match row.source_table.as_str() {
                    "documents" => "promoted recovery seed row into canonical documents table",
                    "todos" => "promoted recovery seed row into canonical todos table",
                    "instincts" => "promoted recovery seed row into canonical instincts table",
                    "coffee_chats" => {
                        "promoted recovery seed row into canonical coffee_chats table"
                    }
                    "decisions" => "promoted recovery seed row into canonical decisions table",
                    "visions" => "promoted recovery seed row into canonical visions table",
                    "memory_fragments" => {
                        "promoted recovery seed row into canonical memory_fragments table"
                    }
                    "memory_links" => {
                        "promoted recovery seed row into canonical memory_links table"
                    }
                    _ => "promoted recovery seed row into canonical runtime memory table",
                }),
                source_ingest_run_id: Some(ingest_run.run.id),
            })?;
            new_promotion_record_count += 1;
        }

        *rows_by_table.entry(row.source_table.clone()).or_insert(0) += 1;
        if !promoted_tables
            .iter()
            .any(|table| table == &row.source_table)
        {
            promoted_tables.push(row.source_table.clone());
        }
    }

    Ok(RecoverySeedPromotionReport {
        ingest_run,
        requested_table: requested_table.map(str::to_string),
        promoted_tables,
        total_candidate_rows: rows.len() as i64,
        upserted_row_count: rows.len() as i64,
        new_promotion_record_count,
        rows_by_table,
    })
}

fn promote_recovery_seed_row(data_store: &DataStore, row: &RecoverySeedRowSummary) -> Result<()> {
    match row.source_table.as_str() {
        "documents" => {
            let payload: RecoverySeedDocumentRow =
                serde_json::from_value(row.source_row.clone())
                    .context("failed to parse recovery document row")?;
            data_store.upsert_document_record(UpsertDocumentRecord {
                id: payload.id,
                slug: &payload.slug,
                title: &payload.title,
                content: &payload.content,
                category: &payload.category,
                created_at: &payload.created_at,
                updated_at: &payload.updated_at,
            })?;
        }
        "todos" => {
            let payload: RecoverySeedTodoRow = serde_json::from_value(row.source_row.clone())
                .context("failed to parse recovery todo row")?;
            data_store.upsert_todo_record(UpsertTodoRecord {
                id: payload.id,
                title: &payload.title,
                status: &payload.status,
                priority: payload.priority,
                project: &payload.project,
                created_at: &payload.created_at,
                done_at: payload.done_at.as_deref(),
                temperature: &payload.temperature,
                due_on: &payload.due_on,
                remind_every_days: payload.remind_every_days,
                remind_next_on: &payload.remind_next_on,
                last_reminded_at: &payload.last_reminded_at,
                reminder_status: &payload.reminder_status,
            })?;
        }
        "instincts" => {
            let payload: RecoverySeedInstinctRow =
                serde_json::from_value(row.source_row.clone())
                    .context("failed to parse recovery instinct row")?;
            let updated_at = if payload.updated_at.trim().is_empty() {
                payload.created_at.as_str()
            } else {
                payload.updated_at.as_str()
            };
            data_store.upsert_instinct_record(UpsertInstinctRecord {
                id: payload.id,
                pattern: &payload.pattern,
                action: &payload.action,
                confidence: payload.confidence,
                source: &payload.source,
                reference: &payload.reference,
                created_at: &payload.created_at,
                status: &payload.status,
                surfaced_to: &payload.surfaced_to,
                review_status: &payload.review_status,
                origin_type: &payload.origin_type,
                lifecycle_status: &payload.lifecycle_status,
                temperature: &payload.temperature,
                updated_at,
            })?;
        }
        "coffee_chats" => {
            let payload: RecoverySeedCoffeeChatRow = serde_json::from_value(row.source_row.clone())
                .context("failed to parse recovery coffee chat row")?;
            data_store.upsert_coffee_chat_record(UpsertCoffeeChatRecord {
                id: payload.id,
                project: &payload.project,
                stage: &payload.stage,
                retro: &payload.retro,
                forward: &payload.forward,
                priorities: &payload.priorities,
                created_at: &payload.created_at,
                temperature: &payload.temperature,
            })?;
        }
        "decisions" => {
            let payload: RecoverySeedDecisionRow =
                serde_json::from_value(row.source_row.clone())
                    .context("failed to parse recovery decision row")?;
            let updated_at = if payload.updated_at.trim().is_empty() {
                payload.created_at.as_str()
            } else {
                payload.updated_at.as_str()
            };
            data_store.upsert_decision_record(UpsertDecisionRecord {
                id: payload.id,
                title: &payload.title,
                statement: &payload.statement,
                rationale: &payload.rationale,
                decision_type: &payload.decision_type,
                decision_status: &payload.decision_status,
                scope_type: &payload.scope_type,
                scope_ref: &payload.scope_ref,
                source_ref: &payload.source_ref,
                decided_by: &payload.decided_by,
                enforcement_level: &payload.enforcement_level,
                actor_scope: &payload.actor_scope,
                confidence: payload.confidence,
                created_at: &payload.created_at,
                updated_at,
            })?;
        }
        "visions" => {
            let payload: RecoverySeedVisionRow = serde_json::from_value(row.source_row.clone())
                .context("failed to parse recovery vision row")?;
            let updated_at = if payload.updated_at.trim().is_empty() {
                payload.created_at.as_str()
            } else {
                payload.updated_at.as_str()
            };
            data_store.upsert_vision_record(UpsertVisionRecord {
                id: payload.id,
                title: &payload.title,
                statement: &payload.statement,
                horizon: &payload.horizon,
                vision_status: &payload.vision_status,
                scope_type: &payload.scope_type,
                scope_ref: &payload.scope_ref,
                source_ref: &payload.source_ref,
                confidence: payload.confidence,
                created_at: &payload.created_at,
                updated_at,
            })?;
        }
        "memory_fragments" => {
            let payload: RecoverySeedMemoryFragmentRow =
                serde_json::from_value(row.source_row.clone())
                    .context("failed to parse recovery memory fragment row")?;
            let updated_at = if payload.updated_at.trim().is_empty() {
                payload.created_at.as_str()
            } else {
                payload.updated_at.as_str()
            };
            data_store.upsert_memory_fragment_record(UpsertMemoryFragmentRecord {
                id: payload.id,
                title: &payload.title,
                content: &payload.content,
                kind: &payload.kind,
                source_type: &payload.source_type,
                source_ref: &payload.source_ref,
                source_hash: &payload.source_hash,
                scope_type: &payload.scope_type,
                scope_ref: &payload.scope_ref,
                target_table: &payload.target_table,
                target_ref: &payload.target_ref,
                status: &payload.status,
                triage_status: &payload.triage_status,
                temperature: &payload.temperature,
                tags: &payload.tags,
                notes: &payload.notes,
                confidence: payload.confidence,
                created_at: &payload.created_at,
                updated_at,
            })?;
        }
        "memory_links" => {
            let payload: RecoverySeedMemoryLinkRow = serde_json::from_value(row.source_row.clone())
                .context("failed to parse recovery memory link row")?;
            data_store.upsert_memory_link_record(UpsertMemoryLinkRecord {
                id: payload.id,
                src_kind: &payload.src_kind,
                src_id: payload.src_id,
                dst_kind: &payload.dst_kind,
                dst_id: payload.dst_id,
                relation_type: &payload.relation_type,
                status: &payload.status,
                created_at: &payload.created_at,
            })?;
        }
        other => {
            return Err(anyhow!(
                "recovery promotion does not support source table `{other}`"
            ));
        }
    }

    Ok(())
}

fn resolve_recovery_seed_run(
    data_store: &DataStore,
    ingest_run_id: Option<i64>,
) -> Result<RecoverySeedRunSummary> {
    let runs = list_recovery_seed_runs(data_store)?;
    match ingest_run_id {
        Some(ingest_run_id) => runs
            .into_iter()
            .find(|run| run.run.id == ingest_run_id)
            .ok_or_else(|| anyhow!("recovery seed ingest run `{ingest_run_id}` does not exist")),
        None => runs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no recovery seed ingest runs have been imported yet")),
    }
}

fn collect_recovery_seed_rows(
    data_store: &DataStore,
    ingest_run_id: i64,
    requested_table: Option<&str>,
) -> Result<Vec<RecoverySeedRowSummary>> {
    let promotions = latest_promotion_map(data_store.list_promotion_records()?);
    let artifacts = data_store.list_source_artifacts(ingest_run_id)?;
    let mut rows = Vec::new();

    for artifact in artifacts
        .into_iter()
        .filter(|artifact| artifact.artifact_kind == "recovery_seed_row")
    {
        let row_payload = parse_recovery_seed_row_payload(&artifact)?;
        if let Some(requested_table) = requested_table {
            if row_payload.source_table != requested_table {
                continue;
            }
        }

        let promotion = promotions.get(&("source_artifact".to_string(), artifact.id));
        rows.push(RecoverySeedRowSummary {
            artifact,
            promotion_state: promotion.map(|record| record.promotion_state.clone()),
            promotion_reason: promotion.and_then(|record| record.reason.clone()),
            promotion_recorded_at: promotion.map(|record| record.created_at.clone()),
            source_table: row_payload.source_table,
            source_row_key: row_payload.source_row_key,
            source_row: row_payload.source_row,
        });
    }

    Ok(rows)
}

fn normalized_optional_table_name(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn parse_recovery_seed_manifest_payload(
    artifact: &StoredSourceArtifact,
) -> Result<RecoverySeedManifestArtifactPayload> {
    serde_json::from_str(&artifact.payload_json).with_context(|| {
        format!(
            "failed to parse recovery seed manifest artifact `{}` payload",
            artifact.artifact_key
        )
    })
}

fn parse_recovery_seed_row_payload(
    artifact: &StoredSourceArtifact,
) -> Result<RecoverySeedRowArtifactPayload> {
    serde_json::from_str(&artifact.payload_json).with_context(|| {
        format!(
            "failed to parse recovery seed row artifact `{}` payload",
            artifact.artifact_key
        )
    })
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

fn latest_promotion_map(
    records: Vec<StoredPromotionRecord>,
) -> BTreeMap<(String, i64), StoredPromotionRecord> {
    let mut latest = BTreeMap::new();

    for record in records {
        let key = (record.subject_kind.clone(), record.subject_id);
        latest.entry(key).or_insert(record);
    }

    latest
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

fn default_active() -> String {
    "active".to_string()
}

fn default_warm() -> String {
    "warm".to_string()
}

fn default_none() -> String {
    "none".to_string()
}

fn default_accepted() -> String {
    "accepted".to_string()
}

fn default_unit_confidence() -> f64 {
    1.0
}
