use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::data_store::DataStore;

type JsonRow = Map<String, Value>;

const DECISION_COLUMNS: &[&str] = &[
    "title",
    "statement",
    "rationale",
    "decision_type",
    "decision_status",
    "scope_type",
    "scope_ref",
    "source_ref",
    "decided_by",
    "enforcement_level",
    "actor_scope",
    "confidence",
    "created_at",
    "updated_at",
];
const VISION_COLUMNS: &[&str] = &[
    "title",
    "statement",
    "horizon",
    "vision_status",
    "scope_type",
    "scope_ref",
    "source_ref",
    "confidence",
    "created_at",
    "updated_at",
];
const TODO_COLUMNS: &[&str] = &[
    "title",
    "status",
    "priority",
    "project",
    "created_at",
    "done_at",
    "temperature",
    "due_on",
    "remind_every_days",
    "remind_next_on",
    "last_reminded_at",
    "reminder_status",
];
const DOCUMENT_COLUMNS: &[&str] = &[
    "slug",
    "title",
    "content",
    "category",
    "created_at",
    "updated_at",
];
const MEMORY_FRAGMENT_COLUMNS: &[&str] = &[
    "title",
    "content",
    "kind",
    "source_type",
    "source_ref",
    "source_hash",
    "scope_type",
    "scope_ref",
    "target_table",
    "target_ref",
    "status",
    "triage_status",
    "temperature",
    "tags",
    "notes",
    "confidence",
    "created_at",
    "updated_at",
];
const INSTINCT_COLUMNS: &[&str] = &[
    "pattern",
    "action",
    "confidence",
    "source",
    "ref",
    "created_at",
    "status",
    "surfaced_to",
    "review_status",
    "origin_type",
    "lifecycle_status",
    "temperature",
    "updated_at",
];
const COFFEE_CHAT_COLUMNS: &[&str] = &[
    "project",
    "stage",
    "retro",
    "forward",
    "priorities",
    "created_at",
    "temperature",
];
const MEMORY_LINK_COLUMNS: &[&str] = &[
    "src_kind",
    "src_id",
    "dst_kind",
    "dst_id",
    "relation_type",
    "status",
    "created_at",
];

const DECISION_ALIASES: &[&str] = &["decision", "decisions"];
const VISION_ALIASES: &[&str] = &["vision", "visions"];
const TODO_ALIASES: &[&str] = &["todo", "todos"];
const DOCUMENT_ALIASES: &[&str] = &["document", "documents"];
const MEMORY_FRAGMENT_ALIASES: &[&str] = &["memory_fragment", "memory_fragments"];
const INSTINCT_ALIASES: &[&str] = &["instinct", "instincts"];
const COFFEE_CHAT_ALIASES: &[&str] = &["coffee_chat", "coffee_chats"];

#[derive(Debug, Deserialize)]
struct StoreFile {
    #[allow(dead_code)]
    meta: Value,
    instincts: Vec<JsonRow>,
    documents: Vec<JsonRow>,
    coffee_chats: Vec<JsonRow>,
    todos: Vec<JsonRow>,
    memory_fragments: Vec<JsonRow>,
    decisions: Vec<JsonRow>,
    visions: Vec<JsonRow>,
    memory_links: Vec<JsonRow>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ImportReport {
    pub tables: Vec<TableReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableReport {
    pub table_name: String,
    pub inserted: usize,
    pub skipped: usize,
}

#[derive(Debug, Default)]
struct ImportState {
    source_id_map: HashMap<(String, i64), i64>,
}

impl ImportState {
    fn register(&mut self, aliases: &[&str], source_id: Option<i64>, target_id: i64) {
        let Some(source_id) = source_id else {
            return;
        };

        for alias in aliases {
            self.source_id_map
                .insert(((*alias).to_string(), source_id), target_id);
        }
    }

    fn resolve(&self, kind: &str, source_id: i64) -> Option<i64> {
        self.source_id_map
            .get(&(kind.trim().to_string(), source_id))
            .copied()
    }
}

pub fn import_store_json(conn: &Connection, path: &Path) -> Result<ImportReport> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read store.json `{}`", path.display()))?;
    let store: StoreFile =
        serde_json::from_str(&content).context("failed to parse NOTA store.json payload")?;

    let document_source_ids = infer_document_source_ids(&store.documents, &store.memory_links)?;
    let mut state = ImportState::default();
    let mut report = ImportReport::default();

    report.tables.push(import_table(
        conn,
        "decisions",
        DECISION_ALIASES,
        DECISION_COLUMNS,
        store
            .decisions
            .into_iter()
            .map(|row| Ok((read_optional_i64(&row, "id")?, row)))
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "todos",
        TODO_ALIASES,
        TODO_COLUMNS,
        store
            .todos
            .into_iter()
            .map(|row| Ok((read_optional_i64(&row, "id")?, row)))
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "instincts",
        INSTINCT_ALIASES,
        INSTINCT_COLUMNS,
        store
            .instincts
            .into_iter()
            .map(|row| Ok((read_optional_i64(&row, "id")?, row)))
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "coffee_chats",
        COFFEE_CHAT_ALIASES,
        COFFEE_CHAT_COLUMNS,
        store
            .coffee_chats
            .into_iter()
            .map(|row| Ok((read_optional_i64(&row, "id")?, row)))
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "documents",
        DOCUMENT_ALIASES,
        DOCUMENT_COLUMNS,
        store
            .documents
            .into_iter()
            .enumerate()
            .map(|(index, row)| (document_source_ids.get(&index).copied(), row))
            .collect(),
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "memory_fragments",
        MEMORY_FRAGMENT_ALIASES,
        MEMORY_FRAGMENT_COLUMNS,
        store
            .memory_fragments
            .into_iter()
            .map(|row| {
                let source_id = read_optional_i64(&row, "id")?;
                let translated = translate_memory_fragment_row(&row, &state)?;
                Ok((source_id, translated))
            })
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "visions",
        VISION_ALIASES,
        VISION_COLUMNS,
        store
            .visions
            .into_iter()
            .map(|row| {
                let source_id = read_optional_i64(&row, "id")?;
                let translated = translate_vision_row(&row, &state)?;
                Ok((source_id, translated))
            })
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    report.tables.push(import_table(
        conn,
        "memory_links",
        &[],
        MEMORY_LINK_COLUMNS,
        store
            .memory_links
            .into_iter()
            .map(|row| Ok((None, translate_memory_link_row(&row, &state)?)))
            .collect::<Result<Vec<_>>>()?,
        &mut state,
    )?);

    Ok(report)
}

pub fn import_store_json_into_data_store(
    data_store: &DataStore,
    path: impl AsRef<Path>,
) -> Result<ImportReport> {
    let path = path.as_ref();
    data_store.with_connection(|conn| import_store_json(conn, path))
}

fn import_table(
    conn: &Connection,
    table_name: &'static str,
    aliases: &[&str],
    insert_columns: &[&str],
    rows: Vec<(Option<i64>, JsonRow)>,
    state: &mut ImportState,
) -> Result<TableReport> {
    let mut inserted = 0;
    let mut skipped = 0;

    for (source_id, row) in rows {
        if let Some(existing_id) = find_existing_id(conn, table_name, &row)? {
            state.register(aliases, source_id, existing_id);
            skipped += 1;
            continue;
        }

        let target_id = insert_row(conn, table_name, insert_columns, &row)?;
        state.register(aliases, source_id, target_id);
        inserted += 1;
    }

    Ok(TableReport {
        table_name: table_name.to_string(),
        inserted,
        skipped,
    })
}

fn find_existing_id(conn: &Connection, table_name: &str, row: &JsonRow) -> Result<Option<i64>> {
    let key = dedupe_key(table_name, row)?;
    let predicates = key
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} IS ?{}", index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!("SELECT id FROM {table_name} WHERE {predicates} ORDER BY id ASC LIMIT 1");

    let mut statement = conn
        .prepare(&sql)
        .with_context(|| format!("failed to prepare dedupe query for `{table_name}`"))?;
    let mut rows = statement
        .query(params_from_iter(key.values))
        .with_context(|| format!("failed to query existing `{table_name}` rows"))?;

    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn insert_row(
    conn: &Connection,
    table_name: &str,
    insert_columns: &[&str],
    row: &JsonRow,
) -> Result<i64> {
    let placeholders = (1..=insert_columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {table_name} ({}) VALUES ({placeholders})",
        insert_columns.join(", ")
    );
    let values = insert_columns
        .iter()
        .map(|column| json_value_to_sql(row.get(*column).cloned().unwrap_or(Value::Null)))
        .collect::<Vec<_>>();

    conn.execute(&sql, params_from_iter(values))
        .with_context(|| format!("failed to insert row into `{table_name}`"))?;
    Ok(conn.last_insert_rowid())
}

fn dedupe_key(table_name: &str, row: &JsonRow) -> Result<DedupeKey> {
    match table_name {
        "decisions" => build_dedupe_key(row, &["title", "statement", "created_at"]),
        "visions" => build_dedupe_key(row, &["title", "statement", "created_at"]),
        "todos" => build_dedupe_key(row, &["title", "created_at", "project"]),
        "documents" => build_dedupe_key(row, &["slug", "category"]),
        "memory_fragments" => {
            let source_hash = read_required_string(row, "source_hash")?;
            if source_hash.trim().is_empty() {
                build_dedupe_key(row, &["title", "created_at"])
            } else {
                build_dedupe_key(row, &["source_hash", "title"])
            }
        }
        "memory_links" => build_dedupe_key(
            row,
            &["src_kind", "src_id", "dst_kind", "dst_id", "relation_type"],
        ),
        "instincts" => build_dedupe_key(row, &["pattern", "created_at"]),
        "coffee_chats" => build_dedupe_key(row, &["project", "stage", "created_at"]),
        other => bail!("unsupported import table `{other}`"),
    }
}

fn build_dedupe_key(row: &JsonRow, columns: &[&'static str]) -> Result<DedupeKey> {
    let values = columns
        .iter()
        .map(|column| {
            row.get(*column)
                .cloned()
                .map(json_value_to_sql)
                .ok_or_else(|| anyhow!("store row missing dedupe column `{column}`"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DedupeKey {
        columns: columns.to_vec(),
        values,
    })
}

fn translate_memory_fragment_row(row: &JsonRow, state: &ImportState) -> Result<JsonRow> {
    let mut translated = row.clone();
    let target_table = read_required_string(row, "target_table")?;
    if target_table.trim().is_empty() {
        return Ok(translated);
    }

    let current_ref = read_required_string(row, "target_ref")?;
    let new_ref = if target_table == "multiple" {
        translate_multiple_refs(&current_ref, state)?
    } else {
        translate_numeric_ref(&target_table, &current_ref, state)?.to_string()
    };

    translated.insert("target_ref".to_string(), Value::String(new_ref));
    Ok(translated)
}

fn translate_vision_row(row: &JsonRow, state: &ImportState) -> Result<JsonRow> {
    let mut translated = row.clone();
    let source_ref = read_required_string(row, "source_ref")?;
    if let Some((kind, raw_id)) = split_local_ref(&source_ref) {
        let target_id = translate_numeric_ref(kind, raw_id, state)?;
        translated.insert(
            "source_ref".to_string(),
            Value::String(format!("{kind}:{target_id}")),
        );
    }
    Ok(translated)
}

fn translate_memory_link_row(row: &JsonRow, state: &ImportState) -> Result<JsonRow> {
    let mut translated = row.clone();

    let src_kind = read_required_string(row, "src_kind")?;
    let src_source_id = read_required_i64(row, "src_id")?;
    let dst_kind = read_required_string(row, "dst_kind")?;
    let dst_source_id = read_required_i64(row, "dst_id")?;

    let src_target_id = state.resolve(&src_kind, src_source_id).with_context(|| {
        format!("failed to resolve memory link source `{src_kind}:{src_source_id}`")
    })?;
    let dst_target_id = state.resolve(&dst_kind, dst_source_id).with_context(|| {
        format!("failed to resolve memory link destination `{dst_kind}:{dst_source_id}`")
    })?;

    translated.insert("src_id".to_string(), json!(src_target_id));
    translated.insert("dst_id".to_string(), json!(dst_target_id));
    Ok(translated)
}

fn translate_multiple_refs(raw: &str, state: &ImportState) -> Result<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let (kind, raw_id) = split_local_ref(segment)
                .ok_or_else(|| anyhow!("invalid composite reference segment `{segment}`"))?;
            let target_id = translate_numeric_ref(kind, raw_id, state)?;
            Ok(format!("{kind}:{target_id}"))
        })
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join(","))
}

fn translate_numeric_ref(kind: &str, raw_id: &str, state: &ImportState) -> Result<i64> {
    let source_id = raw_id
        .trim()
        .parse::<i64>()
        .with_context(|| format!("invalid local reference id `{raw_id}` for `{kind}`"))?;
    state
        .resolve(kind, source_id)
        .with_context(|| format!("failed to resolve local reference `{kind}:{source_id}`"))
}

fn split_local_ref(value: &str) -> Option<(&str, &str)> {
    let (kind, raw_id) = value.split_once(':')?;
    if raw_id.trim().parse::<i64>().is_ok() {
        Some((kind.trim(), raw_id.trim()))
    } else {
        None
    }
}

fn infer_document_source_ids(
    documents: &[JsonRow],
    memory_links: &[JsonRow],
) -> Result<HashMap<usize, i64>> {
    let mut first_seen_by_source_id = BTreeMap::<i64, String>::new();

    for link in memory_links {
        let created_at = read_required_string(link, "created_at")?;
        capture_document_reference(
            link,
            "src_kind",
            "src_id",
            created_at.as_str(),
            &mut first_seen_by_source_id,
        )?;
        capture_document_reference(
            link,
            "dst_kind",
            "dst_id",
            created_at.as_str(),
            &mut first_seen_by_source_id,
        )?;
    }

    if first_seen_by_source_id.is_empty() {
        return Ok(HashMap::new());
    }

    let mut ids_by_created_at = BTreeMap::<String, BTreeSet<i64>>::new();
    for (source_id, created_at) in first_seen_by_source_id {
        ids_by_created_at
            .entry(created_at)
            .or_default()
            .insert(source_id);
    }

    let mut document_indexes_by_created_at = BTreeMap::<String, Vec<usize>>::new();
    for (index, row) in documents.iter().enumerate() {
        let created_at = read_required_string(row, "created_at")?;
        if !created_at.trim().is_empty() {
            document_indexes_by_created_at
                .entry(created_at)
                .or_default()
                .push(index);
        }
    }

    let mut inferred = HashMap::new();
    for (created_at, source_ids) in ids_by_created_at {
        let source_ids = source_ids.into_iter().collect::<Vec<_>>();
        let document_indexes = document_indexes_by_created_at
            .get(&created_at)
            .with_context(|| {
                format!(
                    "document links reference created_at `{created_at}` but no matching documents were found"
                )
            })?;

        if document_indexes.len() != source_ids.len() {
            bail!(
                "document source-id inference is ambiguous for created_at `{created_at}`: {} documents vs {} referenced ids",
                document_indexes.len(),
                source_ids.len()
            );
        }

        for (index, source_id) in document_indexes.iter().zip(source_ids.iter()) {
            inferred.insert(*index, *source_id);
        }
    }

    Ok(inferred)
}

fn capture_document_reference(
    row: &JsonRow,
    kind_field: &str,
    id_field: &str,
    created_at: &str,
    target: &mut BTreeMap<i64, String>,
) -> Result<()> {
    let kind = read_required_string(row, kind_field)?;
    if kind != "document" && kind != "documents" {
        return Ok(());
    }

    let source_id = read_required_i64(row, id_field)?;
    target
        .entry(source_id)
        .or_insert_with(|| created_at.to_string());
    Ok(())
}

fn read_optional_i64(row: &JsonRow, field: &str) -> Result<Option<i64>> {
    match row.get(field) {
        Some(Value::Number(number)) => number
            .as_i64()
            .map(Some)
            .ok_or_else(|| anyhow!("field `{field}` must be an integer")),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(None),
        Some(Value::String(text)) => text
            .trim()
            .parse::<i64>()
            .map(Some)
            .with_context(|| format!("field `{field}` must contain an integer")),
        Some(Value::Null) | None => Ok(None),
        Some(other) => Err(anyhow!(
            "field `{field}` must be an integer or null, got `{}`",
            other
        )),
    }
}

fn read_required_i64(row: &JsonRow, field: &str) -> Result<i64> {
    read_optional_i64(row, field)?.ok_or_else(|| anyhow!("field `{field}` must not be null"))
}

fn read_required_string(row: &JsonRow, field: &str) -> Result<String> {
    match row.get(field) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(Value::Null) => Ok(String::new()),
        Some(other) => Err(anyhow!(
            "field `{field}` must be a string-compatible value, got `{}`",
            other
        )),
        None => Err(anyhow!("field `{field}` is missing")),
    }
}

fn json_value_to_sql(value: Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(value)),
        Value::Number(number) => number
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| number.as_f64().map(SqlValue::Real))
            .unwrap_or_else(|| SqlValue::Text(number.to_string())),
        Value::String(value) => SqlValue::Text(value),
        other => SqlValue::Text(other.to_string()),
    }
}

struct DedupeKey {
    columns: Vec<&'static str>,
    values: Vec<SqlValue>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::core::data_store::MigrationPlan;

    fn write_store_fixture(label: &str, value: &Value) -> Result<PathBuf> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "entrance-memory-import-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)?;
        let path = dir.join("store.json");
        fs::write(&path, serde_json::to_string_pretty(value)?)?;
        Ok(path)
    }

    fn count_rows(conn: &Connection, table: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        Ok(conn.query_row(&sql, [], |row| row.get(0))?)
    }

    fn report_map(report: &ImportReport) -> BTreeMap<String, (usize, usize)> {
        report
            .tables
            .iter()
            .map(|table| (table.table_name.clone(), (table.inserted, table.skipped)))
            .collect()
    }

    fn source_store_path() -> Option<PathBuf> {
        [
            PathBuf::from("/mnt/a/Publish/myagents/id/nota/data/store.json"),
            PathBuf::from("A:/Publish/myagents/id/nota/data/store.json"),
        ]
        .into_iter()
        .find(|path| path.exists())
    }

    #[test]
    fn test_import_store_json_roundtrip() -> Result<()> {
        let _guard = crate::test_env_guard();
        let fixture = json!({
            "meta": {"version": "test"},
            "instincts": [{
                "id": 3,
                "pattern": "Escalate destructive cleanup",
                "action": "Use controlled delete.",
                "confidence": 1.0,
                "source": "chat",
                "ref": "chat:cleanup",
                "created_at": "2026-04-01 08:00:00",
                "status": "active",
                "surfaced_to": "",
                "review_status": "approved",
                "origin_type": "discussion",
                "lifecycle_status": "active",
                "temperature": "hot",
                "updated_at": "2026-04-01 08:00:00"
            }],
            "documents": [
                {
                    "slug": "architecture/control-plane",
                    "title": "Control plane",
                    "content": "One control surface.",
                    "category": "architecture",
                    "created_at": "2026-04-01 12:00:00",
                    "updated_at": "2026-04-01 12:00:00"
                },
                {
                    "slug": "architecture/runtime",
                    "title": "Runtime",
                    "content": "Runtime shape.",
                    "category": "architecture",
                    "created_at": "2026-04-01 12:05:00",
                    "updated_at": "2026-04-01 12:05:00"
                }
            ],
            "coffee_chats": [{
                "id": 7,
                "project": "Entrance",
                "stage": "recovery",
                "retro": "Need one truth source.",
                "forward": "Import store into Entrance DB.",
                "priorities": "1) import 2) verify",
                "created_at": "2026-04-01 07:45:00",
                "temperature": "warm"
            }],
            "todos": [{
                "id": 10,
                "title": "Wire importer",
                "status": "open",
                "priority": 1,
                "project": "Entrance",
                "created_at": "2026-04-01 09:00:00",
                "done_at": null,
                "temperature": "warm",
                "due_on": "",
                "remind_every_days": 0,
                "remind_next_on": "",
                "last_reminded_at": "",
                "reminder_status": "none"
            }],
            "memory_fragments": [
                {
                    "id": 30,
                    "kind": "todo",
                    "title": "Importer todo fragment",
                    "content": "Track the importer task.",
                    "source_type": "session-log",
                    "source_ref": "session:1",
                    "source_hash": "",
                    "confidence": 0.9,
                    "status": "triaged",
                    "triage_status": "triaged",
                    "temperature": "warm",
                    "scope_type": "project",
                    "scope_ref": "Entrance",
                    "target_table": "todos",
                    "target_ref": "10",
                    "tags": "todo",
                    "notes": "",
                    "created_at": "2026-04-01 09:05:00",
                    "updated_at": "2026-04-01 09:05:00"
                },
                {
                    "id": 31,
                    "kind": "decision",
                    "title": "Importer decision fragment",
                    "content": "Track the import pipeline decision.",
                    "source_type": "session-log",
                    "source_ref": "session:2",
                    "source_hash": "fragment-hash",
                    "confidence": 0.95,
                    "status": "triaged",
                    "triage_status": "promoted",
                    "temperature": "hot",
                    "scope_type": "project",
                    "scope_ref": "Entrance",
                    "target_table": "",
                    "target_ref": "",
                    "tags": "decision",
                    "notes": "",
                    "created_at": "2026-04-01 09:06:00",
                    "updated_at": "2026-04-01 09:06:00"
                }
            ],
            "decisions": [{
                "id": 20,
                "title": "Import is copy-only",
                "statement": "The source store must remain untouched.",
                "rationale": "Preserve recovery evidence.",
                "decision_type": "safety",
                "actor_scope": "system",
                "enforcement_level": "hard",
                "scope_type": "system",
                "scope_ref": "storage",
                "decision_status": "accepted",
                "confidence": 1.0,
                "decided_by": "Human",
                "source_ref": "chat:copy-only",
                "created_at": "2026-04-01 08:30:00",
                "updated_at": "2026-04-01 08:30:00"
            }],
            "visions": [{
                "id": 40,
                "title": "One runtime truth",
                "statement": "Entrance owns the unified runtime DB.",
                "scope_type": "system",
                "scope_ref": "runtime-db",
                "vision_status": "active",
                "horizon": "long",
                "confidence": 0.9,
                "source_ref": "memory_fragments:30",
                "created_at": "2026-04-01 10:00:00",
                "updated_at": "2026-04-01 10:00:00"
            }],
            "memory_links": [
                {
                    "id": 1,
                    "src_kind": "decision",
                    "src_id": 20,
                    "dst_kind": "memory_fragments",
                    "dst_id": 30,
                    "relation_type": "derived_from",
                    "status": "active",
                    "created_at": "2026-04-01 12:00:00"
                },
                {
                    "id": 2,
                    "src_kind": "decision",
                    "src_id": 20,
                    "dst_kind": "document",
                    "dst_id": 19,
                    "relation_type": "references",
                    "status": "active",
                    "created_at": "2026-04-01 12:00:00"
                },
                {
                    "id": 3,
                    "src_kind": "vision",
                    "src_id": 40,
                    "dst_kind": "document",
                    "dst_id": 20,
                    "relation_type": "references",
                    "status": "active",
                    "created_at": "2026-04-01 12:05:00"
                }
            ]
        });
        let fixture_path = write_store_fixture("roundtrip", &fixture)?;
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let first_report = import_store_json_into_data_store(&store, &fixture_path)?;
        let second_report = import_store_json_into_data_store(&store, &fixture_path)?;

        store.with_connection(|conn| {
            assert_eq!(count_rows(conn, "decisions")?, 1);
            assert_eq!(count_rows(conn, "visions")?, 1);
            assert_eq!(count_rows(conn, "todos")?, 1);
            assert_eq!(count_rows(conn, "documents")?, 2);
            assert_eq!(count_rows(conn, "memory_fragments")?, 2);
            assert_eq!(count_rows(conn, "instincts")?, 1);
            assert_eq!(count_rows(conn, "coffee_chats")?, 1);
            assert_eq!(count_rows(conn, "memory_links")?, 3);

            let todo_id: i64 = conn.query_row(
                "SELECT id FROM todos WHERE title = 'Wire importer'",
                [],
                |row| row.get(0),
            )?;
            let fragment_id: i64 = conn.query_row(
                "SELECT id FROM memory_fragments WHERE title = 'Importer todo fragment'",
                [],
                |row| row.get(0),
            )?;
            let vision_source_ref: String = conn.query_row(
                "SELECT source_ref FROM visions WHERE title = 'One runtime truth'",
                [],
                |row| row.get(0),
            )?;
            let fragment_target_ref: String = conn.query_row(
                "SELECT target_ref FROM memory_fragments WHERE title = 'Importer todo fragment'",
                [],
                |row| row.get(0),
            )?;
            let document_id: i64 = conn.query_row(
                "SELECT id FROM documents WHERE slug = 'architecture/control-plane'",
                [],
                |row| row.get(0),
            )?;
            let decision_id: i64 = conn.query_row(
                "SELECT id FROM decisions WHERE title = 'Import is copy-only'",
                [],
                |row| row.get(0),
            )?;
            let linked_document_id: i64 = conn.query_row(
                "SELECT dst_id FROM memory_links WHERE relation_type = 'references' AND src_kind = 'decision'",
                [],
                |row| row.get(0),
            )?;

            assert_eq!(fragment_target_ref, todo_id.to_string());
            assert_eq!(vision_source_ref, format!("memory_fragments:{fragment_id}"));
            assert_eq!(linked_document_id, document_id);

            let fragment_link_src_id: i64 = conn.query_row(
                "SELECT src_id FROM memory_links WHERE relation_type = 'derived_from'",
                [],
                |row| row.get(0),
            )?;
            let fragment_link_dst_id: i64 = conn.query_row(
                "SELECT dst_id FROM memory_links WHERE relation_type = 'derived_from'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(fragment_link_src_id, decision_id);
            assert_eq!(fragment_link_dst_id, fragment_id);

            Ok(())
        })?;

        assert_eq!(
            report_map(&first_report),
            BTreeMap::from([
                ("coffee_chats".to_string(), (1, 0)),
                ("decisions".to_string(), (1, 0)),
                ("documents".to_string(), (2, 0)),
                ("instincts".to_string(), (1, 0)),
                ("memory_fragments".to_string(), (2, 0)),
                ("memory_links".to_string(), (3, 0)),
                ("todos".to_string(), (1, 0)),
                ("visions".to_string(), (1, 0)),
            ])
        );
        assert_eq!(
            report_map(&second_report),
            BTreeMap::from([
                ("coffee_chats".to_string(), (0, 1)),
                ("decisions".to_string(), (0, 1)),
                ("documents".to_string(), (0, 2)),
                ("instincts".to_string(), (0, 1)),
                ("memory_fragments".to_string(), (0, 2)),
                ("memory_links".to_string(), (0, 3)),
                ("todos".to_string(), (0, 1)),
                ("visions".to_string(), (0, 1)),
            ])
        );

        Ok(())
    }

    #[test]
    fn test_import_store_json_real_fixture_when_available() -> Result<()> {
        let _guard = crate::test_env_guard();
        let Some(path) = source_store_path() else {
            return Ok(());
        };

        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;
        let first_report = import_store_json_into_data_store(&store, &path)?;
        let second_report = import_store_json_into_data_store(&store, &path)?;

        store.with_connection(|conn| {
            assert_eq!(count_rows(conn, "decisions")?, 19);
            assert_eq!(count_rows(conn, "visions")?, 5);
            assert_eq!(count_rows(conn, "todos")?, 20);
            assert_eq!(count_rows(conn, "documents")?, 15);
            assert_eq!(count_rows(conn, "memory_fragments")?, 39);
            assert_eq!(count_rows(conn, "memory_links")?, 81);
            assert_eq!(count_rows(conn, "instincts")?, 3);
            assert_eq!(count_rows(conn, "coffee_chats")?, 3);
            Ok(())
        })?;

        let first_map = report_map(&first_report);
        assert_eq!(first_map.get("decisions"), Some(&(19, 0)));
        assert_eq!(first_map.get("visions"), Some(&(5, 0)));
        assert_eq!(first_map.get("todos"), Some(&(20, 0)));
        assert_eq!(first_map.get("documents"), Some(&(15, 0)));
        assert_eq!(first_map.get("memory_fragments"), Some(&(39, 0)));
        assert_eq!(first_map.get("memory_links"), Some(&(81, 0)));
        assert_eq!(first_map.get("instincts"), Some(&(3, 0)));
        assert_eq!(first_map.get("coffee_chats"), Some(&(3, 0)));

        let second_map = report_map(&second_report);
        assert_eq!(second_map.get("decisions"), Some(&(0, 19)));
        assert_eq!(second_map.get("visions"), Some(&(0, 5)));
        assert_eq!(second_map.get("todos"), Some(&(0, 20)));
        assert_eq!(second_map.get("documents"), Some(&(0, 15)));
        assert_eq!(second_map.get("memory_fragments"), Some(&(0, 39)));
        assert_eq!(second_map.get("memory_links"), Some(&(0, 81)));
        assert_eq!(second_map.get("instincts"), Some(&(0, 3)));
        assert_eq!(second_map.get("coffee_chats"), Some(&(0, 3)));

        Ok(())
    }
}
