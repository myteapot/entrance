use std::{collections::HashMap, fs, path::Path};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::data_store::{
    DataStore, NewPlanningItemLink, NewPromotionRecord, NewSourceArtifact, NewSourceIngestRun,
    SourceIngestRunCompletion, StoredExternalIssueMirror, StoredPlanningItem,
    StoredPromotionRecord, StoredSourceIngestRun, UpsertExternalIssueMirror, UpsertPlanningItem,
};

#[derive(Debug, Clone, Serialize)]
pub struct LandingImportReport {
    pub ingest_run_id: i64,
    pub source_system: String,
    pub source_workspace: String,
    pub source_project: String,
    pub artifact_path: String,
    pub artifact_sha256: String,
    pub snapshot_artifact_id: i64,
    pub imported_issue_count: i64,
    pub imported_document_count: i64,
    pub imported_milestone_count: i64,
    pub imported_planning_item_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LandingMirrorSummary {
    #[serde(flatten)]
    pub mirror: StoredExternalIssueMirror,
    pub promotion_state: Option<String>,
    pub promotion_reason: Option<String>,
    pub promotion_recorded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LandingPlanningItemSummary {
    #[serde(flatten)]
    pub planning_item: StoredPlanningItem,
    pub promotion_state: Option<String>,
    pub promotion_reason: Option<String>,
    pub promotion_recorded_at: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct LandingImportProgress {
    imported_issue_count: i64,
    imported_document_count: i64,
    imported_milestone_count: i64,
    imported_planning_item_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinearEntranceSnapshot {
    pub generated_at: String,
    pub source: LinearSnapshotSource,
    pub project: LinearSnapshotProject,
    #[serde(default)]
    pub milestones: Vec<LinearSnapshotMilestone>,
    #[serde(default)]
    pub documents: Vec<LinearSnapshotDocument>,
    #[serde(default)]
    pub issues: Vec<LinearSnapshotIssue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinearSnapshotSource {
    pub system: String,
    pub workspace: String,
    pub project: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinearSnapshotProject {
    pub id: String,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub state: Option<String>,
    pub priority: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "targetDate")]
    pub target_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinearSnapshotMilestone {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "targetDate")]
    pub target_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinearSnapshotDocument {
    pub id: String,
    pub title: String,
    pub slug: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinearSnapshotIssue {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub state: Option<String>,
    pub priority: Option<String>,
    pub url: Option<String>,
    pub project: Option<String>,
    pub team: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<String>,
    #[serde(rename = "archivedAt")]
    pub archived_at: Option<String>,
    #[serde(rename = "dueDate")]
    pub due_date: Option<String>,
    #[serde(rename = "gitBranchName")]
    pub git_branch_name: Option<String>,
    #[serde(default)]
    pub relations: LinearSnapshotIssueRelations,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LinearSnapshotIssueRelations {
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default, rename = "blockedBy")]
    pub blocked_by: Vec<String>,
    #[serde(default, rename = "relatedTo")]
    pub related_to: Vec<String>,
    #[serde(default, rename = "duplicateOf")]
    pub duplicate_of: Option<String>,
}

pub fn import_linear_entrance_snapshot(
    data_store: &DataStore,
    artifact_path: impl AsRef<Path>,
) -> Result<LandingImportReport> {
    let artifact_path = artifact_path.as_ref();
    let bytes = fs::read(artifact_path)
        .with_context(|| format!("failed to read snapshot file `{}`", artifact_path.display()))?;
    let artifact_sha256 = sha256_hex(&bytes);
    let payload = std::str::from_utf8(&bytes)
        .with_context(|| {
            format!(
                "snapshot file `{}` is not valid UTF-8",
                artifact_path.display()
            )
        })?
        .to_string();
    let snapshot: LinearEntranceSnapshot = serde_json::from_str(&payload).with_context(|| {
        format!(
            "failed to parse snapshot file `{}`",
            artifact_path.display()
        )
    })?;

    validate_linear_entrance_snapshot(&snapshot)?;

    let run = data_store.create_source_ingest_run(NewSourceIngestRun {
        source_system: &snapshot.source.system,
        source_workspace: &snapshot.source.workspace,
        source_project: &snapshot.source.project,
        artifact_path: Some(&artifact_path.to_string_lossy()),
        artifact_sha256: Some(&artifact_sha256),
        status: "running",
    })?;

    let mut progress = LandingImportProgress::default();
    let import_result = import_snapshot_contents(
        data_store,
        &snapshot,
        artifact_path,
        &artifact_sha256,
        &payload,
        run.id,
        &mut progress,
    );

    match import_result {
        Ok(snapshot_artifact_id) => {
            let completed = data_store.complete_source_ingest_run(
                run.id,
                SourceIngestRunCompletion {
                    status: "completed",
                    imported_issue_count: progress.imported_issue_count,
                    imported_document_count: progress.imported_document_count,
                    imported_milestone_count: progress.imported_milestone_count,
                    imported_planning_item_count: progress.imported_planning_item_count,
                    error_message: None,
                },
            )?;

            Ok(LandingImportReport {
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
                snapshot_artifact_id,
                imported_issue_count: progress.imported_issue_count,
                imported_document_count: progress.imported_document_count,
                imported_milestone_count: progress.imported_milestone_count,
                imported_planning_item_count: progress.imported_planning_item_count,
            })
        }
        Err(error) => {
            let message = error.to_string();
            let _ = data_store.complete_source_ingest_run(
                run.id,
                SourceIngestRunCompletion {
                    status: "failed",
                    imported_issue_count: progress.imported_issue_count,
                    imported_document_count: progress.imported_document_count,
                    imported_milestone_count: progress.imported_milestone_count,
                    imported_planning_item_count: progress.imported_planning_item_count,
                    error_message: Some(&message),
                },
            );
            Err(error)
        }
    }
}

fn import_snapshot_contents(
    data_store: &DataStore,
    snapshot: &LinearEntranceSnapshot,
    artifact_path: &Path,
    artifact_sha256: &str,
    payload: &str,
    ingest_run_id: i64,
    progress: &mut LandingImportProgress,
) -> Result<i64> {
    let source_prefix = build_source_prefix(snapshot);
    let snapshot_artifact = data_store.insert_source_artifact(NewSourceArtifact {
        ingest_run_id,
        artifact_kind: "snapshot",
        artifact_key: &format!("snapshot:{artifact_sha256}"),
        title: Some(&snapshot.project.name),
        url: Some(&snapshot.project.url),
        payload_json: payload,
    })?;

    let project_payload = serde_json::to_string(&snapshot.project)
        .context("failed to serialize project artifact payload")?;
    data_store.insert_source_artifact(NewSourceArtifact {
        ingest_run_id,
        artifact_kind: "project",
        artifact_key: &format!("project:{}", snapshot.project.id),
        title: Some(&snapshot.project.name),
        url: Some(&snapshot.project.url),
        payload_json: &project_payload,
    })?;

    let artifact_path_string = artifact_path.to_string_lossy().to_string();
    data_store.insert_source_artifact(NewSourceArtifact {
        ingest_run_id,
        artifact_kind: "import_meta",
        artifact_key: &format!("import_meta:{artifact_sha256}"),
        title: Some(&artifact_path_string),
        url: None,
        payload_json: &serde_json::to_string(&serde_json::json!({
            "generated_at": snapshot.generated_at,
            "artifact_path": artifact_path_string,
            "artifact_sha256": artifact_sha256,
        }))
        .context("failed to serialize import metadata payload")?,
    })?;

    for milestone in &snapshot.milestones {
        let milestone_key = format!(
            "milestone:{}",
            stable_external_key(milestone.id.as_deref(), &milestone.name)
        );
        let milestone_payload = serde_json::to_string(milestone)
            .context("failed to serialize milestone artifact payload")?;
        data_store.insert_source_artifact(NewSourceArtifact {
            ingest_run_id,
            artifact_kind: "milestone",
            artifact_key: &milestone_key,
            title: Some(&milestone.name),
            url: None,
            payload_json: &milestone_payload,
        })?;

        let planning_item = data_store.upsert_planning_item(UpsertPlanningItem {
            canonical_key: Some(&format!("{source_prefix}:{milestone_key}")),
            item_type: "milestone_candidate",
            title: &milestone.name,
            description: milestone.description.as_deref(),
            status: "seeded",
            reconciliation_status: "unreconciled",
            source_system: Some(&snapshot.source.system),
            source_workspace: Some(&snapshot.source.workspace),
            source_project: Some(&snapshot.source.project),
            source_key: Some(&milestone_key),
            seeded_from_mirror_id: None,
        })?;
        progress.imported_planning_item_count += 1;
        progress.imported_milestone_count += 1;

        data_store.append_promotion_record(NewPromotionRecord {
            subject_kind: "planning_item",
            subject_id: planning_item.id,
            promotion_state: "storage_only",
            reason: Some("seeded from external milestone artifact"),
            source_ingest_run_id: Some(ingest_run_id),
        })?;
    }

    for document in &snapshot.documents {
        let document_payload = serde_json::to_string(document)
            .context("failed to serialize document artifact payload")?;
        data_store.insert_source_artifact(NewSourceArtifact {
            ingest_run_id,
            artifact_kind: "document",
            artifact_key: &format!("document:{}", document.id),
            title: Some(&document.title),
            url: document.slug.as_deref(),
            payload_json: &document_payload,
        })?;
        progress.imported_document_count += 1;
    }

    let mut planning_item_by_issue = HashMap::new();
    for issue in &snapshot.issues {
        let issue_key = format!("{source_prefix}:issue:{}", issue.id);
        let relations_json = serde_json::to_string(&issue.relations)
            .context("failed to serialize issue relations payload")?;
        let labels_json = serde_json::to_string(&issue.labels)
            .context("failed to serialize issue labels payload")?;
        let issue_payload =
            serde_json::to_string(issue).context("failed to serialize issue mirror payload")?;

        let mirror = data_store.upsert_external_issue_mirror(UpsertExternalIssueMirror {
            ingest_run_id,
            mirror_key: &issue_key,
            source_system: &snapshot.source.system,
            source_workspace: &snapshot.source.workspace,
            source_project: &snapshot.source.project,
            external_issue_id: &issue.id,
            project_name: issue.project.as_deref().or(Some(&snapshot.project.name)),
            team_name: issue.team.as_deref(),
            parent_external_issue_id: issue.parent_id.as_deref(),
            title: &issue.title,
            description: issue.description.as_deref(),
            state: issue.state.as_deref(),
            priority: issue.priority.as_deref(),
            url: issue.url.as_deref(),
            labels_json: &labels_json,
            relations_json: &relations_json,
            payload_json: &issue_payload,
            git_branch_name: issue.git_branch_name.as_deref(),
            due_date: issue.due_date.as_deref(),
            created_at: issue.created_at.as_deref(),
            updated_at: issue.updated_at.as_deref(),
            completed_at: issue.completed_at.as_deref(),
            archived_at: issue.archived_at.as_deref(),
        })?;

        data_store.append_promotion_record(NewPromotionRecord {
            subject_kind: "external_issue_mirror",
            subject_id: mirror.id,
            promotion_state: "storage_only",
            reason: Some("captured from external issue snapshot"),
            source_ingest_run_id: Some(ingest_run_id),
        })?;

        let planning_item = data_store.upsert_planning_item(UpsertPlanningItem {
            canonical_key: Some(&issue_key),
            item_type: "issue",
            title: &issue.title,
            description: issue.description.as_deref(),
            status: "seeded",
            reconciliation_status: "unreconciled",
            source_system: Some(&snapshot.source.system),
            source_workspace: Some(&snapshot.source.workspace),
            source_project: Some(&snapshot.source.project),
            source_key: Some(&issue.id),
            seeded_from_mirror_id: Some(mirror.id),
        })?;

        data_store.ensure_planning_item_link(NewPlanningItemLink {
            planning_item_id: planning_item.id,
            link_type: "mirrors",
            target_planning_item_id: None,
            target_external_issue_mirror_id: Some(mirror.id),
            metadata_json: r#"{"seed":"external_issue_mirror"}"#,
        })?;

        data_store.append_promotion_record(NewPromotionRecord {
            subject_kind: "planning_item",
            subject_id: planning_item.id,
            promotion_state: "storage_only",
            reason: Some("seeded from external issue mirror"),
            source_ingest_run_id: Some(ingest_run_id),
        })?;

        planning_item_by_issue.insert(issue.id.clone(), planning_item.id);
        progress.imported_issue_count += 1;
        progress.imported_planning_item_count += 1;
    }

    for issue in &snapshot.issues {
        let Some(source_item_id) = planning_item_by_issue.get(&issue.id).copied() else {
            continue;
        };

        if let Some(parent_id) = issue.parent_id.as_deref() {
            if let Some(target_id) = planning_item_by_issue.get(parent_id).copied() {
                data_store.ensure_planning_item_link(NewPlanningItemLink {
                    planning_item_id: source_item_id,
                    link_type: "parent",
                    target_planning_item_id: Some(target_id),
                    target_external_issue_mirror_id: None,
                    metadata_json: "{}",
                })?;
            }
        }

        import_relation_links(
            data_store,
            source_item_id,
            "blocks",
            &issue.relations.blocks,
            &planning_item_by_issue,
        )?;
        import_relation_links(
            data_store,
            source_item_id,
            "blocked_by",
            &issue.relations.blocked_by,
            &planning_item_by_issue,
        )?;
        import_relation_links(
            data_store,
            source_item_id,
            "related_to",
            &issue.relations.related_to,
            &planning_item_by_issue,
        )?;

        if let Some(duplicate_of) = issue.relations.duplicate_of.as_deref() {
            if let Some(target_id) = planning_item_by_issue.get(duplicate_of).copied() {
                data_store.ensure_planning_item_link(NewPlanningItemLink {
                    planning_item_id: source_item_id,
                    link_type: "duplicate_of",
                    target_planning_item_id: Some(target_id),
                    target_external_issue_mirror_id: None,
                    metadata_json: "{}",
                })?;
            }
        }
    }

    Ok(snapshot_artifact.id)
}

pub fn list_landing_ingest_runs(data_store: &DataStore) -> Result<Vec<StoredSourceIngestRun>> {
    data_store.list_source_ingest_runs()
}

pub fn list_landing_mirror_items(data_store: &DataStore) -> Result<Vec<LandingMirrorSummary>> {
    let mirrors = data_store.list_external_issue_mirrors()?;
    let latest_promotions = latest_promotion_map(data_store.list_promotion_records()?);

    Ok(mirrors
        .into_iter()
        .map(|mirror| {
            let promotion =
                latest_promotions.get(&("external_issue_mirror".to_string(), mirror.id));
            LandingMirrorSummary {
                mirror,
                promotion_state: promotion.map(|record| record.promotion_state.clone()),
                promotion_reason: promotion.and_then(|record| record.reason.clone()),
                promotion_recorded_at: promotion.map(|record| record.created_at.clone()),
            }
        })
        .collect())
}

pub fn list_landing_planning_items(
    data_store: &DataStore,
) -> Result<Vec<LandingPlanningItemSummary>> {
    let items = data_store.list_planning_items()?;
    let latest_promotions = latest_promotion_map(data_store.list_promotion_records()?);

    Ok(items
        .into_iter()
        .map(|planning_item| {
            let promotion = latest_promotions.get(&("planning_item".to_string(), planning_item.id));
            LandingPlanningItemSummary {
                planning_item,
                promotion_state: promotion.map(|record| record.promotion_state.clone()),
                promotion_reason: promotion.and_then(|record| record.reason.clone()),
                promotion_recorded_at: promotion.map(|record| record.created_at.clone()),
            }
        })
        .collect())
}

pub fn list_landing_unreconciled_items(
    data_store: &DataStore,
) -> Result<Vec<LandingPlanningItemSummary>> {
    let items = data_store.list_unreconciled_planning_items()?;
    let latest_promotions = latest_promotion_map(data_store.list_promotion_records()?);

    Ok(items
        .into_iter()
        .map(|planning_item| {
            let promotion = latest_promotions.get(&("planning_item".to_string(), planning_item.id));
            LandingPlanningItemSummary {
                planning_item,
                promotion_state: promotion.map(|record| record.promotion_state.clone()),
                promotion_reason: promotion.and_then(|record| record.reason.clone()),
                promotion_recorded_at: promotion.map(|record| record.created_at.clone()),
            }
        })
        .collect())
}

fn import_relation_links(
    data_store: &DataStore,
    planning_item_id: i64,
    link_type: &str,
    related_issue_ids: &[String],
    planning_item_by_issue: &HashMap<String, i64>,
) -> Result<()> {
    for related_issue_id in related_issue_ids {
        if let Some(target_id) = planning_item_by_issue.get(related_issue_id).copied() {
            data_store.ensure_planning_item_link(NewPlanningItemLink {
                planning_item_id,
                link_type,
                target_planning_item_id: Some(target_id),
                target_external_issue_mirror_id: None,
                metadata_json: "{}",
            })?;
        }
    }

    Ok(())
}

fn validate_linear_entrance_snapshot(snapshot: &LinearEntranceSnapshot) -> Result<()> {
    if snapshot.source.system.trim() != "linear" {
        return Err(anyhow!(
            "expected `source.system` to be `linear`, got `{}`",
            snapshot.source.system
        ));
    }

    if snapshot.source.workspace.trim().is_empty() {
        return Err(anyhow!("`source.workspace` must not be empty"));
    }

    if snapshot.source.project.trim() != "Entrance" {
        return Err(anyhow!(
            "expected `source.project` to be `Entrance`, got `{}`",
            snapshot.source.project
        ));
    }

    if snapshot.project.name.trim() != "Entrance" {
        return Err(anyhow!(
            "expected `project.name` to be `Entrance`, got `{}`",
            snapshot.project.name
        ));
    }

    if snapshot.project.id.trim().is_empty() {
        return Err(anyhow!("`project.id` must not be empty"));
    }

    for issue in &snapshot.issues {
        if issue.id.trim().is_empty() {
            return Err(anyhow!("snapshot contains an issue with an empty `id`"));
        }
        if issue.title.trim().is_empty() {
            return Err(anyhow!(
                "snapshot issue `{}` has an empty `title`",
                issue.id
            ));
        }
    }

    Ok(())
}

fn build_source_prefix(snapshot: &LinearEntranceSnapshot) -> String {
    format!(
        "{}:{}:{}",
        snapshot.source.system.to_lowercase(),
        snapshot.source.workspace.trim(),
        snapshot.source.project.trim()
    )
}

fn stable_external_key(id: Option<&str>, fallback_name: &str) -> String {
    match id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(id) => id.to_string(),
        None => slugify(fallback_name),
    }
}

fn slugify(raw: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;

    for character in raw.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }

    output.trim_matches('-').to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn latest_promotion_map(
    records: Vec<StoredPromotionRecord>,
) -> HashMap<(String, i64), StoredPromotionRecord> {
    let mut latest = HashMap::new();

    for record in records {
        let key = (record.subject_kind.clone(), record.subject_id);
        latest.entry(key).or_insert(record);
    }

    latest
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::core::data_store::MigrationPlan;

    #[test]
    fn imports_linear_snapshot_into_landing_tables() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;
        let snapshot_path = write_test_snapshot()?;

        let report = import_linear_entrance_snapshot(&store, &snapshot_path)?;
        let runs = store.list_source_ingest_runs()?;
        let artifacts = store.list_source_artifacts(report.ingest_run_id)?;
        let mirrors = store.list_external_issue_mirrors()?;
        let planning_items = store.list_planning_items()?;
        let links = store.list_planning_item_links()?;
        let promotions = store.list_promotion_records()?;
        let mirror_summaries = list_landing_mirror_items(&store)?;
        let planning_summaries = list_landing_planning_items(&store)?;
        let unreconciled_summaries = list_landing_unreconciled_items(&store)?;

        assert_eq!(report.imported_issue_count, 2);
        assert_eq!(report.imported_document_count, 1);
        assert_eq!(report.imported_milestone_count, 1);
        assert_eq!(report.imported_planning_item_count, 3);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(artifacts.len(), 5);
        assert_eq!(mirrors.len(), 2);
        assert_eq!(planning_items.len(), 3);
        assert!(links.iter().any(|link| link.link_type == "mirrors"));
        assert!(links.iter().any(|link| link.link_type == "blocks"));
        assert_eq!(promotions.len(), 5);
        assert_eq!(mirror_summaries.len(), 2);
        assert!(mirror_summaries
            .iter()
            .all(|summary| summary.promotion_state.as_deref() == Some("storage_only")));
        assert_eq!(planning_summaries.len(), 3);
        assert!(planning_summaries
            .iter()
            .all(|summary| summary.promotion_state.as_deref() == Some("storage_only")));
        assert_eq!(unreconciled_summaries.len(), 3);

        let _ = fs::remove_file(snapshot_path);
        Ok(())
    }

    fn write_test_snapshot() -> Result<PathBuf> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("failed to compute test snapshot nonce")?
            .as_nanos();
        let path = env::temp_dir().join(format!("entrance-landing-test-{nonce}.json"));
        let payload = r##"{
  "generated_at": "2026-03-22T10:37:32.223Z",
  "source": {
    "system": "linear",
    "workspace": "microt",
    "project": "Entrance"
  },
  "project": {
    "id": "project-1",
    "name": "Entrance",
    "url": "https://linear.app/microt/project/entrance",
    "description": "Entrance project",
    "summary": "",
    "state": "Backlog",
    "priority": "High",
    "startDate": null,
    "targetDate": null
  },
  "milestones": [
    {
      "id": "milestone-1",
      "name": "Bootstrap Ownership",
      "description": "First candidate milestone",
      "targetDate": null
    }
  ],
  "documents": [
    {
      "id": "doc-1",
      "title": "Landing Notes",
      "slug": "landing-notes",
      "updatedAt": "2026-03-22T10:00:00.000Z",
      "content": "# Notes"
    }
  ],
  "issues": [
    {
      "id": "MYT-100",
      "title": "Seed landing layer",
      "description": "Import the first snapshot",
      "state": "Todo",
      "priority": "High",
      "url": "https://linear.app/microt/issue/MYT-100",
      "project": "Entrance",
      "team": "Pub",
      "parentId": null,
      "labels": ["Feature"],
      "createdAt": "2026-03-22T10:00:00.000Z",
      "updatedAt": "2026-03-22T10:10:00.000Z",
      "completedAt": null,
      "archivedAt": null,
      "dueDate": null,
      "gitBranchName": "kc2003/myt-100",
      "relations": {
        "blocks": ["MYT-101"],
        "blockedBy": [],
        "relatedTo": [],
        "duplicateOf": null
      }
    },
    {
      "id": "MYT-101",
      "title": "Read landing layer",
      "description": "List imported planning items",
      "state": "Backlog",
      "priority": "Medium",
      "url": "https://linear.app/microt/issue/MYT-101",
      "project": "Entrance",
      "team": "Pub",
      "parentId": "MYT-100",
      "labels": [],
      "createdAt": "2026-03-22T10:05:00.000Z",
      "updatedAt": "2026-03-22T10:15:00.000Z",
      "completedAt": null,
      "archivedAt": null,
      "dueDate": null,
      "gitBranchName": null,
      "relations": {
        "blocks": [],
        "blockedBy": ["MYT-100"],
        "relatedTo": [],
        "duplicateOf": null
      }
    }
  ]
}"##;

        fs::write(&path, payload)
            .with_context(|| format!("failed to write test snapshot to `{}`", path.display()))?;
        Ok(path)
    }
}
