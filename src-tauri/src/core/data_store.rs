use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use chrono::Utc;
#[cfg(test)]
use rusqlite::OpenFlags;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::plugins::launcher::scanner::DiscoveredApp;

const CORE_MIGRATION: MigrationStep = MigrationStep {
    name: "0000_create_core_tables",
    sql: include_str!("../../migrations/0000_create_core_tables.sql"),
};

const CORE_LANDING_MIGRATION: MigrationStep = MigrationStep {
    name: "0005_create_core_landing_tables",
    sql: include_str!("../../migrations/0005_create_core_landing_tables.sql"),
};

const CORE_NOTA_RUNTIME_MIGRATION: MigrationStep = MigrationStep {
    name: "0007_create_core_nota_runtime_tables",
    sql: include_str!("../../migrations/0007_create_core_nota_runtime_tables.sql"),
};

const CORE_NOTA_DO_RUNTIME_MIGRATION: MigrationStep = MigrationStep {
    name: "0008_create_core_nota_do_runtime_tables",
    sql: include_str!("../../migrations/0008_create_core_nota_do_runtime_tables.sql"),
};

const CORE_DECISION_LINKS_MIGRATION: MigrationStep = MigrationStep {
    name: "0009_create_core_decision_links",
    sql: include_str!("../../migrations/0009_create_core_decision_links.sql"),
};

const CORE_CHAT_ARCHIVE_MIGRATION: MigrationStep = MigrationStep {
    name: "0010_create_core_chat_archive_tables",
    sql: include_str!("../../migrations/0010_create_core_chat_archive_tables.sql"),
};

const CORE_NOTA_RUNTIME_ALLOCATIONS_MIGRATION: MigrationStep = MigrationStep {
    name: "0011_create_core_nota_runtime_allocations",
    sql: include_str!("../../migrations/0011_create_core_nota_runtime_allocations.sql"),
};

const CORE_MIGRATIONS: [MigrationStep; 7] = [
    CORE_MIGRATION,
    CORE_LANDING_MIGRATION,
    CORE_NOTA_RUNTIME_MIGRATION,
    CORE_NOTA_DO_RUNTIME_MIGRATION,
    CORE_DECISION_LINKS_MIGRATION,
    CORE_CHAT_ARCHIVE_MIGRATION,
    CORE_NOTA_RUNTIME_ALLOCATIONS_MIGRATION,
];

#[derive(Debug, Clone, Copy)]
pub struct MigrationStep {
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct MigrationPlan<'a> {
    pub core: &'a [MigrationStep],
    pub plugins: &'a [MigrationStep],
}

impl<'a> MigrationPlan<'a> {
    pub fn new(plugins: &'a [MigrationStep]) -> Self {
        Self {
            core: core_migrations(),
            plugins,
        }
    }
}

pub fn core_migrations() -> &'static [MigrationStep] {
    &CORE_MIGRATIONS
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredLauncherApp {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub path: String,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub icon_path: Option<String>,
    pub source: String,
    pub launch_count: i64,
    pub last_used: Option<String>,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredForgeTask {
    pub id: i64,
    pub name: String,
    pub command: String,
    pub args: String, // JSON
    pub working_dir: Option<String>,
    pub stdin_text: Option<String>,
    pub required_tokens: String, // JSON
    pub metadata: String,        // JSON
    pub status: String,
    pub status_message: Option<String>,
    pub exit_code: Option<i64>,
    pub created_at: String,
    pub heartbeat_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredForgeTaskLog {
    pub id: i64,
    pub task_id: i64,
    pub stream: String,
    pub line: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredForgeDispatchReceipt {
    pub id: i64,
    pub parent_task_id: i64,
    pub child_task_id: i64,
    pub supervision_scope: String,
    pub supervision_strategy: String,
    pub child_dispatch_role: String,
    pub child_dispatch_tool_name: String,
    pub child_slot: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVaultToken {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct EncryptedVaultToken {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub encrypted_value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVaultTokenSecret {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVaultMcpConfig {
    pub id: i64,
    pub name: String,
    pub transport: String,
    pub endpoint: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredSourceIngestRun {
    pub id: i64,
    pub source_system: String,
    pub source_workspace: String,
    pub source_project: String,
    pub artifact_path: Option<String>,
    pub artifact_sha256: Option<String>,
    pub status: String,
    pub imported_issue_count: i64,
    pub imported_document_count: i64,
    pub imported_milestone_count: i64,
    pub imported_planning_item_count: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredSourceArtifact {
    pub id: i64,
    pub ingest_run_id: i64,
    pub artifact_kind: String,
    pub artifact_key: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredExternalIssueMirror {
    pub id: i64,
    pub mirror_key: String,
    pub source_system: String,
    pub source_workspace: String,
    pub source_project: String,
    pub external_issue_id: String,
    pub project_name: Option<String>,
    pub team_name: Option<String>,
    pub parent_external_issue_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub state: Option<String>,
    pub priority: Option<String>,
    pub url: Option<String>,
    pub labels_json: String,
    pub relations_json: String,
    pub payload_json: String,
    pub git_branch_name: Option<String>,
    pub due_date: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
    pub archived_at: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub last_ingest_run_id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredPlanningItem {
    pub id: i64,
    pub canonical_key: Option<String>,
    pub item_type: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub reconciliation_status: String,
    pub source_system: Option<String>,
    pub source_workspace: Option<String>,
    pub source_project: Option<String>,
    pub source_key: Option<String>,
    pub seeded_from_mirror_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredPlanningItemLink {
    pub id: i64,
    pub planning_item_id: i64,
    pub link_type: String,
    pub target_planning_item_id: Option<i64>,
    pub target_external_issue_mirror_id: Option<i64>,
    pub metadata_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredPromotionRecord {
    pub id: i64,
    pub subject_kind: String,
    pub subject_id: i64,
    pub promotion_state: String,
    pub reason: Option<String>,
    pub source_ingest_run_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredCadenceObject {
    pub id: i64,
    pub cadence_kind: String,
    pub title: String,
    pub summary: String,
    pub payload_json: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub source_type: String,
    pub source_ref: String,
    pub admission_policy: String,
    pub projection_policy: String,
    pub status: String,
    pub is_current: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredCadenceLink {
    pub id: i64,
    pub src_cadence_object_id: i64,
    pub dst_cadence_object_id: i64,
    pub relation_type: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredNotaRuntimeTransaction {
    pub id: i64,
    pub actor_role: String,
    pub surface_action: String,
    pub transaction_kind: String,
    pub title: String,
    pub payload_json: String,
    pub status: String,
    pub forge_task_id: Option<i64>,
    pub cadence_checkpoint_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredNotaRuntimeReceipt {
    pub id: i64,
    pub transaction_id: i64,
    pub receipt_kind: String,
    pub payload_json: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredNotaRuntimeAllocation {
    pub id: i64,
    pub allocator_role: String,
    pub allocator_surface: String,
    pub allocation_kind: String,
    pub source_transaction_id: i64,
    pub lineage_ref: String,
    pub child_execution_kind: String,
    pub child_execution_ref: String,
    pub return_target_kind: String,
    pub return_target_ref: String,
    pub escalation_target_kind: String,
    pub escalation_target_ref: String,
    pub status: String,
    pub payload_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredDecisionRecord {
    pub id: i64,
    pub title: String,
    pub statement: String,
    pub rationale: String,
    pub decision_type: String,
    pub decision_status: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub source_ref: String,
    pub decided_by: String,
    pub enforcement_level: String,
    pub actor_scope: String,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredDecisionLink {
    pub id: i64,
    pub src_decision_id: i64,
    pub dst_decision_id: i64,
    pub relation_type: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredChatArchiveSetting {
    pub id: i64,
    pub scope_type: String,
    pub scope_ref: String,
    pub archive_policy: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredChatCaptureRecord {
    pub id: i64,
    pub session_ref: String,
    pub role: String,
    pub capture_mode: String,
    pub archive_policy: String,
    pub content: String,
    pub summary: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub linked_decision_id: Option<i64>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredTodoRecord {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub priority: i64,
    pub project: String,
    pub created_at: String,
    pub done_at: Option<String>,
    pub temperature: String,
    pub due_on: String,
    pub remind_every_days: i64,
    pub remind_next_on: String,
    pub last_reminded_at: String,
    pub reminder_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredVisionRecord {
    pub id: i64,
    pub title: String,
    pub statement: String,
    pub horizon: String,
    pub vision_status: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub source_ref: String,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredMemoryFragment {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub source_type: String,
    pub source_ref: String,
    pub source_hash: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub target_table: String,
    pub target_ref: String,
    pub status: String,
    pub triage_status: String,
    pub temperature: String,
    pub tags: String,
    pub notes: String,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredMemoryLink {
    pub id: i64,
    pub src_kind: String,
    pub src_id: i64,
    pub dst_kind: String,
    pub dst_id: i64,
    pub relation_type: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct UpsertDocumentRecord<'a> {
    pub id: i64,
    pub slug: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub category: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertTodoRecord<'a> {
    pub id: i64,
    pub title: &'a str,
    pub status: &'a str,
    pub priority: i64,
    pub project: &'a str,
    pub created_at: &'a str,
    pub done_at: Option<&'a str>,
    pub temperature: &'a str,
    pub due_on: &'a str,
    pub remind_every_days: i64,
    pub remind_next_on: &'a str,
    pub last_reminded_at: &'a str,
    pub reminder_status: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertInstinctRecord<'a> {
    pub id: i64,
    pub pattern: &'a str,
    pub action: &'a str,
    pub confidence: f64,
    pub source: &'a str,
    pub reference: &'a str,
    pub created_at: &'a str,
    pub status: &'a str,
    pub surfaced_to: &'a str,
    pub review_status: &'a str,
    pub origin_type: &'a str,
    pub lifecycle_status: &'a str,
    pub temperature: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertCoffeeChatRecord<'a> {
    pub id: i64,
    pub project: &'a str,
    pub stage: &'a str,
    pub retro: &'a str,
    pub forward: &'a str,
    pub priorities: &'a str,
    pub created_at: &'a str,
    pub temperature: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertDecisionRecord<'a> {
    pub id: i64,
    pub title: &'a str,
    pub statement: &'a str,
    pub rationale: &'a str,
    pub decision_type: &'a str,
    pub decision_status: &'a str,
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub source_ref: &'a str,
    pub decided_by: &'a str,
    pub enforcement_level: &'a str,
    pub actor_scope: &'a str,
    pub confidence: f64,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewDecisionRecord<'a> {
    pub title: &'a str,
    pub statement: &'a str,
    pub rationale: &'a str,
    pub decision_type: &'a str,
    pub decision_status: &'a str,
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub source_ref: &'a str,
    pub decided_by: &'a str,
    pub enforcement_level: &'a str,
    pub actor_scope: &'a str,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct NewDecisionLink<'a> {
    pub src_decision_id: i64,
    pub dst_decision_id: i64,
    pub relation_type: &'a str,
    pub status: &'a str,
}

#[derive(Debug, Clone)]
pub struct ChatArchiveSettingRecord<'a> {
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub archive_policy: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewChatCaptureRecord<'a> {
    pub session_ref: &'a str,
    pub role: &'a str,
    pub capture_mode: &'a str,
    pub archive_policy: &'a str,
    pub content: &'a str,
    pub summary: &'a str,
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub linked_decision_id: Option<i64>,
    pub status: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertVisionRecord<'a> {
    pub id: i64,
    pub title: &'a str,
    pub statement: &'a str,
    pub horizon: &'a str,
    pub vision_status: &'a str,
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub source_ref: &'a str,
    pub confidence: f64,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertMemoryFragmentRecord<'a> {
    pub id: i64,
    pub title: &'a str,
    pub content: &'a str,
    pub kind: &'a str,
    pub source_type: &'a str,
    pub source_ref: &'a str,
    pub source_hash: &'a str,
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub target_table: &'a str,
    pub target_ref: &'a str,
    pub status: &'a str,
    pub triage_status: &'a str,
    pub temperature: &'a str,
    pub tags: &'a str,
    pub notes: &'a str,
    pub confidence: f64,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertMemoryLinkRecord<'a> {
    pub id: i64,
    pub src_kind: &'a str,
    pub src_id: i64,
    pub dst_kind: &'a str,
    pub dst_id: i64,
    pub relation_type: &'a str,
    pub status: &'a str,
    pub created_at: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewSourceIngestRun<'a> {
    pub source_system: &'a str,
    pub source_workspace: &'a str,
    pub source_project: &'a str,
    pub artifact_path: Option<&'a str>,
    pub artifact_sha256: Option<&'a str>,
    pub status: &'a str,
}

#[derive(Debug, Clone)]
pub struct SourceIngestRunCompletion<'a> {
    pub status: &'a str,
    pub imported_issue_count: i64,
    pub imported_document_count: i64,
    pub imported_milestone_count: i64,
    pub imported_planning_item_count: i64,
    pub error_message: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct NewSourceArtifact<'a> {
    pub ingest_run_id: i64,
    pub artifact_kind: &'a str,
    pub artifact_key: &'a str,
    pub title: Option<&'a str>,
    pub url: Option<&'a str>,
    pub payload_json: &'a str,
}

#[derive(Debug, Clone)]
pub struct UpsertExternalIssueMirror<'a> {
    pub ingest_run_id: i64,
    pub mirror_key: &'a str,
    pub source_system: &'a str,
    pub source_workspace: &'a str,
    pub source_project: &'a str,
    pub external_issue_id: &'a str,
    pub project_name: Option<&'a str>,
    pub team_name: Option<&'a str>,
    pub parent_external_issue_id: Option<&'a str>,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub state: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub url: Option<&'a str>,
    pub labels_json: &'a str,
    pub relations_json: &'a str,
    pub payload_json: &'a str,
    pub git_branch_name: Option<&'a str>,
    pub due_date: Option<&'a str>,
    pub created_at: Option<&'a str>,
    pub updated_at: Option<&'a str>,
    pub completed_at: Option<&'a str>,
    pub archived_at: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct UpsertPlanningItem<'a> {
    pub canonical_key: Option<&'a str>,
    pub item_type: &'a str,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub status: &'a str,
    pub reconciliation_status: &'a str,
    pub source_system: Option<&'a str>,
    pub source_workspace: Option<&'a str>,
    pub source_project: Option<&'a str>,
    pub source_key: Option<&'a str>,
    pub seeded_from_mirror_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewPlanningItemLink<'a> {
    pub planning_item_id: i64,
    pub link_type: &'a str,
    pub target_planning_item_id: Option<i64>,
    pub target_external_issue_mirror_id: Option<i64>,
    pub metadata_json: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewPromotionRecord<'a> {
    pub subject_kind: &'a str,
    pub subject_id: i64,
    pub promotion_state: &'a str,
    pub reason: Option<&'a str>,
    pub source_ingest_run_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewCadenceObject<'a> {
    pub cadence_kind: &'a str,
    pub title: &'a str,
    pub summary: &'a str,
    pub payload_json: &'a str,
    pub scope_type: &'a str,
    pub scope_ref: &'a str,
    pub source_type: &'a str,
    pub source_ref: &'a str,
    pub admission_policy: &'a str,
    pub projection_policy: &'a str,
    pub status: &'a str,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct NewCadenceLink<'a> {
    pub src_cadence_object_id: i64,
    pub dst_cadence_object_id: i64,
    pub relation_type: &'a str,
    pub status: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewNotaRuntimeTransaction<'a> {
    pub actor_role: &'a str,
    pub surface_action: &'a str,
    pub transaction_kind: &'a str,
    pub title: &'a str,
    pub payload_json: &'a str,
    pub status: &'a str,
    pub forge_task_id: Option<i64>,
    pub cadence_checkpoint_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NotaRuntimeTransactionUpdate<'a> {
    pub status: &'a str,
    pub forge_task_id: Option<i64>,
    pub cadence_checkpoint_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewNotaRuntimeReceipt<'a> {
    pub transaction_id: i64,
    pub receipt_kind: &'a str,
    pub payload_json: &'a str,
    pub status: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewNotaRuntimeAllocation<'a> {
    pub allocator_role: &'a str,
    pub allocator_surface: &'a str,
    pub allocation_kind: &'a str,
    pub source_transaction_id: i64,
    pub lineage_ref: &'a str,
    pub child_execution_kind: &'a str,
    pub child_execution_ref: &'a str,
    pub return_target_kind: &'a str,
    pub return_target_ref: &'a str,
    pub escalation_target_kind: &'a str,
    pub escalation_target_ref: &'a str,
    pub status: &'a str,
    pub payload_json: &'a str,
}

#[derive(Debug, Clone)]
pub struct NotaRuntimeAllocationUpdate<'a> {
    pub status: &'a str,
    pub payload_json: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct NewForgeDispatchReceipt<'a> {
    pub parent_task_id: i64,
    pub supervision_scope: &'a str,
    pub supervision_strategy: &'a str,
    pub child_dispatch_role: &'a str,
    pub child_dispatch_tool_name: &'a str,
    pub child_slot: Option<&'a str>,
}

#[derive(Clone)]
pub struct DataStore {
    connection: Arc<Mutex<Connection>>,
    path: Arc<PathBuf>,
}

impl std::fmt::Debug for DataStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl DataStore {
    pub fn open(path: impl AsRef<Path>, migration_plan: MigrationPlan<'_>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(&path)?;

        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            path: Arc::new(path),
        };
        store.migrate(migration_plan)?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn open_read_only(
        path: impl AsRef<Path>,
        migration_plan: MigrationPlan<'_>,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            path: Arc::new(path),
        };
        store.migrate(migration_plan)?;
        Ok(store)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn in_memory(migration_plan: MigrationPlan<'_>) -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        let store = Self {
            connection: Arc::new(Mutex::new(connection)),
            path: Arc::new(PathBuf::from(":memory:")),
        };
        store.migrate(migration_plan)?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn launcher_app_count(&self) -> Result<i64> {
        self.with_connection(|connection| {
            let count =
                connection.query_row("SELECT COUNT(*) FROM plugin_launcher_apps", [], |row| {
                    row.get(0)
                })?;
            Ok(count)
        })
    }

    pub fn append_core_event_log(&self, topic: &str, payload: Option<&str>) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute(
                r#"
                INSERT INTO core_event_log (topic, payload)
                VALUES (?1, ?2)
                "#,
                params![topic, payload],
            )?;
            Ok(())
        })
    }

    pub fn upsert_launcher_apps(&self, apps: &[DiscoveredApp]) -> Result<()> {
        if apps.is_empty() {
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;

        for app in apps {
            transaction.execute(
                r#"
                INSERT INTO plugin_launcher_apps (
                    name,
                    normalized_name,
                    path,
                    arguments,
                    working_dir,
                    icon_path,
                    source,
                    launch_count,
                    last_used,
                    pinned,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, 0, ?8, ?8)
                ON CONFLICT(path) DO UPDATE SET
                    name = excluded.name,
                    normalized_name = excluded.normalized_name,
                    arguments = COALESCE(excluded.arguments, plugin_launcher_apps.arguments),
                    working_dir = COALESCE(excluded.working_dir, plugin_launcher_apps.working_dir),
                    icon_path = COALESCE(excluded.icon_path, plugin_launcher_apps.icon_path),
                    source = excluded.source,
                    updated_at = excluded.updated_at
                "#,
                params![
                    app.name,
                    app.normalized_name,
                    app.path,
                    app.arguments,
                    app.working_dir,
                    app.icon_path,
                    app.source,
                    now,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn list_launcher_apps(&self) -> Result<Vec<StoredLauncherApp>> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                r#"
                SELECT
                    id,
                    name,
                    normalized_name,
                    path,
                    arguments,
                    working_dir,
                    icon_path,
                    source,
                    launch_count,
                    last_used,
                    pinned,
                    created_at,
                    updated_at
                FROM plugin_launcher_apps
                ORDER BY pinned DESC, launch_count DESC, name ASC
                "#,
            )?;

            let rows = statement.query_map([], map_launcher_row)?;
            let apps = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(apps)
        })
    }

    pub fn get_launcher_app_by_path(&self, path: &str) -> Result<Option<StoredLauncherApp>> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    r#"
                    SELECT
                        id,
                        name,
                        normalized_name,
                        path,
                        arguments,
                        working_dir,
                        icon_path,
                        source,
                        launch_count,
                        last_used,
                        pinned,
                        created_at,
                        updated_at
                    FROM plugin_launcher_apps
                    WHERE path = ?1
                    "#,
                    [path],
                    map_launcher_row,
                )
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn record_launcher_launch(&self, path: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        self.with_connection(|connection| {
            let changed = connection.execute(
                r#"
                UPDATE plugin_launcher_apps
                SET launch_count = launch_count + 1,
                    last_used = ?2,
                    updated_at = ?2
                WHERE path = ?1
                "#,
                params![path, now],
            )?;

            if changed == 0 {
                connection.execute(
                    r#"
                    INSERT INTO plugin_launcher_apps (
                        name,
                        normalized_name,
                        path,
                        arguments,
                        working_dir,
                        icon_path,
                        source,
                        launch_count,
                        last_used,
                        pinned,
                        created_at,
                        updated_at
                    ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, 'manual', 1, ?4, 0, ?4, ?4)
                    "#,
                    params![
                        fallback_app_name(path),
                        crate::plugins::launcher::search::normalize_text(path),
                        path,
                        now,
                    ],
                )?;
            }

            Ok(())
        })
    }

    pub fn set_launcher_pinned(&self, path: &str, pinned: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let changed = self.with_connection(|connection| {
            Ok(connection.execute(
                r#"
                UPDATE plugin_launcher_apps
                SET pinned = ?2,
                    updated_at = ?3
                WHERE path = ?1
                "#,
                params![path, if pinned { 1 } else { 0 }, now],
            )?)
        })?;

        if changed == 0 {
            return Err(anyhow!("launcher app `{path}` does not exist in the index"));
        }

        Ok(())
    }

    pub fn insert_forge_task(
        &self,
        name: &str,
        command: &str,
        args: &str,
        working_dir: Option<&str>,
        stdin_text: Option<&str>,
        required_tokens: &str,
        metadata: &str,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_forge_tasks (
                    name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, heartbeat_at, finished_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'Pending', NULL, NULL, ?8, NULL, NULL)
                "#,
                params![
                    name,
                    command,
                    args,
                    working_dir,
                    stdin_text,
                    required_tokens,
                    metadata,
                    now
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn insert_forge_task_with_dispatch_receipt(
        &self,
        name: &str,
        command: &str,
        args: &str,
        working_dir: Option<&str>,
        stdin_text: Option<&str>,
        required_tokens: &str,
        metadata: &str,
        dispatch_receipt: &NewForgeDispatchReceipt<'_>,
    ) -> Result<(i64, StoredForgeDispatchReceipt)> {
        let now = Utc::now().to_rfc3339();
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            r#"
            INSERT INTO plugin_forge_tasks (
                name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, heartbeat_at, finished_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'Pending', NULL, NULL, ?8, NULL, NULL)
            "#,
            params![
                name,
                command,
                args,
                working_dir,
                stdin_text,
                required_tokens,
                metadata,
                now
            ],
        )?;
        let child_task_id = transaction.last_insert_rowid();
        transaction.execute(
            r#"
            INSERT INTO plugin_forge_dispatch_receipts (
                parent_task_id,
                child_task_id,
                supervision_scope,
                supervision_strategy,
                child_dispatch_role,
                child_dispatch_tool_name,
                child_slot,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                dispatch_receipt.parent_task_id,
                child_task_id,
                dispatch_receipt.supervision_scope,
                dispatch_receipt.supervision_strategy,
                dispatch_receipt.child_dispatch_role,
                dispatch_receipt.child_dispatch_tool_name,
                dispatch_receipt.child_slot,
                now,
            ],
        )?;
        let receipt_id = transaction.last_insert_rowid();
        transaction.commit()?;

        Ok((
            child_task_id,
            StoredForgeDispatchReceipt {
                id: receipt_id,
                parent_task_id: dispatch_receipt.parent_task_id,
                child_task_id,
                supervision_scope: dispatch_receipt.supervision_scope.to_string(),
                supervision_strategy: dispatch_receipt.supervision_strategy.to_string(),
                child_dispatch_role: dispatch_receipt.child_dispatch_role.to_string(),
                child_dispatch_tool_name: dispatch_receipt.child_dispatch_tool_name.to_string(),
                child_slot: dispatch_receipt.child_slot.map(str::to_string),
                created_at: now,
            },
        ))
    }

    pub fn update_forge_task_status(
        &self,
        id: i64,
        status: &str,
        exit_code: Option<i32>,
        status_message: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            if status == "Running" {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3, status_message = ?4, heartbeat_at = ?5, finished_at = NULL
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code, status_message, now],
                )?;
            } else if matches!(status, "Done" | "Failed" | "Cancelled" | "Blocked") {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3, status_message = ?4, finished_at = ?5
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code, status_message, now],
                )?;
            } else {
                conn.execute(
                    r#"
                    UPDATE plugin_forge_tasks
                    SET status = ?2, exit_code = ?3, status_message = ?4
                    WHERE id = ?1
                    "#,
                    params![id, status, exit_code, status_message],
                )?;
            }
            Ok(())
        })
    }

    pub fn touch_forge_task_heartbeat(&self, id: i64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE plugin_forge_tasks
                SET heartbeat_at = ?2
                WHERE id = ?1
                  AND status = 'Running'
                "#,
                params![id, now],
            )?;
            Ok(())
        })
    }

    pub fn update_pending_forge_task_request(
        &self,
        id: i64,
        command: &str,
        args: &str,
        working_dir: Option<&str>,
        stdin_text: Option<&str>,
        required_tokens: &str,
        metadata: &str,
    ) -> Result<()> {
        let changed = self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                UPDATE plugin_forge_tasks
                SET command = ?2,
                    args = ?3,
                    working_dir = ?4,
                    stdin_text = ?5,
                    required_tokens = ?6,
                    metadata = ?7
                WHERE id = ?1
                  AND status = 'Pending'
                "#,
                params![
                    id,
                    command,
                    args,
                    working_dir,
                    stdin_text,
                    required_tokens,
                    metadata,
                ],
            )?)
        })?;

        if changed == 0 {
            return Err(anyhow!(
                "forge task `{id}` does not exist or is no longer Pending"
            ));
        }

        Ok(())
    }

    pub fn list_forge_tasks(&self) -> Result<Vec<StoredForgeTask>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, heartbeat_at, finished_at FROM plugin_forge_tasks ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map([], map_forge_row)?;
            let tasks = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(tasks)
        })
    }

    pub fn get_forge_task(&self, id: i64) -> Result<Option<StoredForgeTask>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT id, name, command, args, working_dir, stdin_text, required_tokens, metadata, status, status_message, exit_code, created_at, heartbeat_at, finished_at FROM plugin_forge_tasks WHERE id = ?1",
                [id],
                map_forge_row,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn append_forge_task_log(
        &self,
        task_id: i64,
        stream: &str,
        line: &str,
    ) -> Result<StoredForgeTaskLog> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_forge_task_logs (
                    task_id, stream, line, created_at
                ) VALUES (?1, ?2, ?3, ?4)
                "#,
                params![task_id, stream, line, now],
            )?;
            conn.execute(
                r#"
                UPDATE plugin_forge_tasks
                SET heartbeat_at = ?2
                WHERE id = ?1
                  AND status = 'Running'
                "#,
                params![task_id, now],
            )?;
            Ok(StoredForgeTaskLog {
                id: conn.last_insert_rowid(),
                task_id,
                stream: stream.to_string(),
                line: line.to_string(),
                created_at: now,
            })
        })
    }

    pub fn list_forge_task_logs(&self, task_id: i64) -> Result<Vec<StoredForgeTaskLog>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, task_id, stream, line, created_at
                FROM plugin_forge_task_logs
                WHERE task_id = ?1
                ORDER BY id ASC
                "#,
            )?;
            let rows = stmt.query_map([task_id], map_forge_log_row)?;
            let logs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(logs)
        })
    }

    pub fn get_forge_dispatch_parent_receipt(
        &self,
        child_task_id: i64,
    ) -> Result<Option<StoredForgeDispatchReceipt>> {
        self.with_connection(|conn| {
            match conn
                .query_row(
                    r#"
                    SELECT
                        id,
                        parent_task_id,
                        child_task_id,
                        supervision_scope,
                        supervision_strategy,
                        child_dispatch_role,
                        child_dispatch_tool_name,
                        child_slot,
                        created_at
                    FROM plugin_forge_dispatch_receipts
                    WHERE child_task_id = ?1
                    "#,
                    [child_task_id],
                    map_forge_dispatch_receipt_row,
                )
                .optional()
            {
                Ok(receipt) => Ok(receipt),
                Err(rusqlite::Error::SqliteFailure(_, Some(message)))
                    if message.contains("no such table: plugin_forge_dispatch_receipts") =>
                {
                    Ok(None)
                }
                Err(error) => Err(error.into()),
            }
        })
    }

    pub fn list_forge_dispatch_child_receipts(
        &self,
        parent_task_id: i64,
    ) -> Result<Vec<StoredForgeDispatchReceipt>> {
        self.with_connection(|conn| {
            let mut statement = match conn.prepare(
                r#"
                SELECT
                    id,
                    parent_task_id,
                    child_task_id,
                    supervision_scope,
                    supervision_strategy,
                    child_dispatch_role,
                    child_dispatch_tool_name,
                    child_slot,
                    created_at
                FROM plugin_forge_dispatch_receipts
                WHERE parent_task_id = ?1
                ORDER BY id ASC
                "#,
            ) {
                Ok(statement) => statement,
                Err(rusqlite::Error::SqliteFailure(_, Some(message)))
                    if message.contains("no such table: plugin_forge_dispatch_receipts") =>
                {
                    return Ok(Vec::new());
                }
                Err(error) => return Err(error.into()),
            };
            let rows = statement.query_map([parent_task_id], map_forge_dispatch_receipt_row)?;
            let receipts = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(receipts)
        })
    }

    pub fn insert_vault_token(
        &self,
        name: &str,
        provider: &str,
        encrypted_value: &str,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO plugin_vault_tokens (
                    name, provider, encrypted_value, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?4)
                "#,
                params![name, provider, encrypted_value, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn update_vault_token(
        &self,
        id: i64,
        name: &str,
        provider: &str,
        encrypted_value: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let changed = self.with_connection(|conn| {
            Ok(conn.execute(
                r#"
                UPDATE plugin_vault_tokens
                SET name = ?2,
                    provider = ?3,
                    encrypted_value = ?4,
                    updated_at = ?5
                WHERE id = ?1
                "#,
                params![id, name, provider, encrypted_value, now],
            )?)
        })?;

        if changed == 0 {
            return Err(anyhow!("vault token `{id}` does not exist"));
        }

        Ok(())
    }

    pub fn list_vault_tokens(&self) -> Result<Vec<StoredVaultToken>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, provider, created_at, updated_at
                FROM plugin_vault_tokens
                ORDER BY provider ASC, name ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_vault_token_row)?;
            let tokens = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(tokens)
        })
    }

    pub fn get_vault_token(&self, id: i64) -> Result<Option<EncryptedVaultToken>> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT id, name, provider, encrypted_value, created_at, updated_at
                FROM plugin_vault_tokens
                WHERE id = ?1
                "#,
                [id],
                map_encrypted_vault_token_row,
            )
            .optional()
            .map_err(Into::into)
        })
    }

    pub fn get_vault_token_by_provider(
        &self,
        provider: &str,
    ) -> Result<Option<EncryptedVaultToken>> {
        self.with_connection(|conn| {
            match conn
                .query_row(
                    r#"
                    SELECT id, name, provider, encrypted_value, created_at, updated_at
                    FROM plugin_vault_tokens
                    WHERE LOWER(provider) = LOWER(?1)
                    ORDER BY updated_at DESC, id DESC
                    LIMIT 1
                    "#,
                    [provider],
                    map_encrypted_vault_token_row,
                )
                .optional()
            {
                Ok(token) => Ok(token),
                Err(rusqlite::Error::SqliteFailure(_, Some(message)))
                    if message.contains("no such table: plugin_vault_tokens") =>
                {
                    Ok(None)
                }
                Err(error) => Err(error.into()),
            }
        })
    }

    pub fn delete_vault_token(&self, id: i64) -> Result<()> {
        let changed = self.with_connection(|conn| {
            Ok(conn.execute("DELETE FROM plugin_vault_tokens WHERE id = ?1", [id])?)
        })?;

        if changed == 0 {
            return Err(anyhow!("vault token `{id}` does not exist"));
        }

        Ok(())
    }

    pub fn list_vault_mcp_configs(&self) -> Result<Vec<StoredVaultMcpConfig>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, transport, endpoint, enabled, created_at, updated_at
                FROM plugin_vault_mcp_configs
                ORDER BY enabled DESC, name ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_vault_mcp_row)?;
            let configs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(configs)
        })
    }

    pub fn upsert_vault_mcp_config(
        &self,
        id: Option<i64>,
        name: &str,
        transport: &str,
        endpoint: &str,
        enabled: bool,
    ) -> Result<StoredVaultMcpConfig> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            let id = if let Some(id) = id {
                let changed = conn.execute(
                    r#"
                    UPDATE plugin_vault_mcp_configs
                    SET name = ?2,
                        transport = ?3,
                        endpoint = ?4,
                        enabled = ?5,
                        updated_at = ?6
                    WHERE id = ?1
                    "#,
                    params![
                        id,
                        name,
                        transport,
                        endpoint,
                        if enabled { 1 } else { 0 },
                        now
                    ],
                )?;

                if changed == 0 {
                    return Err(anyhow!("vault MCP config `{id}` does not exist"));
                }

                id
            } else {
                conn.execute(
                    r#"
                    INSERT INTO plugin_vault_mcp_configs (
                        name, transport, endpoint, enabled, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                    "#,
                    params![name, transport, endpoint, if enabled { 1 } else { 0 }, now],
                )?;
                conn.last_insert_rowid()
            };

            fetch_vault_mcp_config(conn, id)?
                .ok_or_else(|| anyhow!("vault MCP config `{id}` could not be reloaded"))
        })
    }

    pub fn create_source_ingest_run(
        &self,
        new_run: NewSourceIngestRun<'_>,
    ) -> Result<StoredSourceIngestRun> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO source_ingest_runs (
                    source_system,
                    source_workspace,
                    source_project,
                    artifact_path,
                    artifact_sha256,
                    status,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    new_run.source_system,
                    new_run.source_workspace,
                    new_run.source_project,
                    new_run.artifact_path,
                    new_run.artifact_sha256,
                    new_run.status,
                    now,
                ],
            )?;

            fetch_source_ingest_run(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("source ingest run disappeared after creation"))
        })
    }

    pub fn complete_source_ingest_run(
        &self,
        id: i64,
        completion: SourceIngestRunCompletion<'_>,
    ) -> Result<StoredSourceIngestRun> {
        let completed_at = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            let changed = conn.execute(
                r#"
                UPDATE source_ingest_runs
                SET status = ?2,
                    imported_issue_count = ?3,
                    imported_document_count = ?4,
                    imported_milestone_count = ?5,
                    imported_planning_item_count = ?6,
                    error_message = ?7,
                    completed_at = ?8
                WHERE id = ?1
                "#,
                params![
                    id,
                    completion.status,
                    completion.imported_issue_count,
                    completion.imported_document_count,
                    completion.imported_milestone_count,
                    completion.imported_planning_item_count,
                    completion.error_message,
                    completed_at,
                ],
            )?;

            if changed == 0 {
                return Err(anyhow!("source ingest run `{id}` does not exist"));
            }

            fetch_source_ingest_run(conn, id)?
                .ok_or_else(|| anyhow!("source ingest run `{id}` could not be reloaded"))
        })
    }

    pub fn list_source_ingest_runs(&self) -> Result<Vec<StoredSourceIngestRun>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    source_system,
                    source_workspace,
                    source_project,
                    artifact_path,
                    artifact_sha256,
                    status,
                    imported_issue_count,
                    imported_document_count,
                    imported_milestone_count,
                    imported_planning_item_count,
                    error_message,
                    created_at,
                    completed_at
                FROM source_ingest_runs
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_source_ingest_run_row)?;
            let runs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(runs)
        })
    }

    pub fn insert_source_artifact(
        &self,
        artifact: NewSourceArtifact<'_>,
    ) -> Result<StoredSourceArtifact> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO source_artifacts (
                    ingest_run_id,
                    artifact_kind,
                    artifact_key,
                    title,
                    url,
                    payload_json,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(ingest_run_id, artifact_kind, artifact_key) DO UPDATE SET
                    title = excluded.title,
                    url = excluded.url,
                    payload_json = excluded.payload_json,
                    created_at = excluded.created_at
                "#,
                params![
                    artifact.ingest_run_id,
                    artifact.artifact_kind,
                    artifact.artifact_key,
                    artifact.title,
                    artifact.url,
                    artifact.payload_json,
                    now,
                ],
            )?;

            fetch_source_artifact(
                conn,
                artifact.ingest_run_id,
                artifact.artifact_kind,
                artifact.artifact_key,
            )?
            .ok_or_else(|| anyhow!("source artifact disappeared after insert"))
        })
    }

    pub fn list_source_artifacts(&self, ingest_run_id: i64) -> Result<Vec<StoredSourceArtifact>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    ingest_run_id,
                    artifact_kind,
                    artifact_key,
                    title,
                    url,
                    payload_json,
                    created_at
                FROM source_artifacts
                WHERE ingest_run_id = ?1
                ORDER BY artifact_kind ASC, artifact_key ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([ingest_run_id], map_source_artifact_row)?;
            let artifacts = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(artifacts)
        })
    }

    pub fn upsert_external_issue_mirror(
        &self,
        mirror: UpsertExternalIssueMirror<'_>,
    ) -> Result<StoredExternalIssueMirror> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO external_issue_mirrors (
                    mirror_key,
                    source_system,
                    source_workspace,
                    source_project,
                    external_issue_id,
                    project_name,
                    team_name,
                    parent_external_issue_id,
                    title,
                    description,
                    state,
                    priority,
                    url,
                    labels_json,
                    relations_json,
                    payload_json,
                    git_branch_name,
                    due_date,
                    created_at,
                    updated_at,
                    completed_at,
                    archived_at,
                    first_seen_at,
                    last_seen_at,
                    last_ingest_run_id
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?23, ?24
                )
                ON CONFLICT(mirror_key) DO UPDATE SET
                    source_system = excluded.source_system,
                    source_workspace = excluded.source_workspace,
                    source_project = excluded.source_project,
                    external_issue_id = excluded.external_issue_id,
                    project_name = excluded.project_name,
                    team_name = excluded.team_name,
                    parent_external_issue_id = excluded.parent_external_issue_id,
                    title = excluded.title,
                    description = excluded.description,
                    state = excluded.state,
                    priority = excluded.priority,
                    url = excluded.url,
                    labels_json = excluded.labels_json,
                    relations_json = excluded.relations_json,
                    payload_json = excluded.payload_json,
                    git_branch_name = excluded.git_branch_name,
                    due_date = excluded.due_date,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at,
                    completed_at = excluded.completed_at,
                    archived_at = excluded.archived_at,
                    last_seen_at = excluded.last_seen_at,
                    last_ingest_run_id = excluded.last_ingest_run_id
                "#,
                params![
                    mirror.mirror_key,
                    mirror.source_system,
                    mirror.source_workspace,
                    mirror.source_project,
                    mirror.external_issue_id,
                    mirror.project_name,
                    mirror.team_name,
                    mirror.parent_external_issue_id,
                    mirror.title,
                    mirror.description,
                    mirror.state,
                    mirror.priority,
                    mirror.url,
                    mirror.labels_json,
                    mirror.relations_json,
                    mirror.payload_json,
                    mirror.git_branch_name,
                    mirror.due_date,
                    mirror.created_at,
                    mirror.updated_at,
                    mirror.completed_at,
                    mirror.archived_at,
                    now,
                    mirror.ingest_run_id,
                ],
            )?;

            fetch_external_issue_mirror_by_key(conn, mirror.mirror_key)?
                .ok_or_else(|| anyhow!("external issue mirror disappeared after upsert"))
        })
    }

    pub fn list_external_issue_mirrors(&self) -> Result<Vec<StoredExternalIssueMirror>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    mirror_key,
                    source_system,
                    source_workspace,
                    source_project,
                    external_issue_id,
                    project_name,
                    team_name,
                    parent_external_issue_id,
                    title,
                    description,
                    state,
                    priority,
                    url,
                    labels_json,
                    relations_json,
                    payload_json,
                    git_branch_name,
                    due_date,
                    created_at,
                    updated_at,
                    completed_at,
                    archived_at,
                    first_seen_at,
                    last_seen_at,
                    last_ingest_run_id
                FROM external_issue_mirrors
                ORDER BY external_issue_id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_external_issue_mirror_row)?;
            let mirrors = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(mirrors)
        })
    }

    pub fn upsert_planning_item(&self, item: UpsertPlanningItem<'_>) -> Result<StoredPlanningItem> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            if let Some(canonical_key) = item.canonical_key {
                conn.execute(
                    r#"
                    INSERT INTO planning_items (
                        canonical_key,
                        item_type,
                        title,
                        description,
                        status,
                        reconciliation_status,
                        source_system,
                        source_workspace,
                        source_project,
                        source_key,
                        seeded_from_mirror_id,
                        created_at,
                        updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                    ON CONFLICT(canonical_key) DO UPDATE SET
                        item_type = excluded.item_type,
                        title = excluded.title,
                        description = excluded.description,
                        status = excluded.status,
                        reconciliation_status = excluded.reconciliation_status,
                        source_system = excluded.source_system,
                        source_workspace = excluded.source_workspace,
                        source_project = excluded.source_project,
                        source_key = excluded.source_key,
                        seeded_from_mirror_id = excluded.seeded_from_mirror_id,
                        updated_at = excluded.updated_at
                    "#,
                    params![
                        canonical_key,
                        item.item_type,
                        item.title,
                        item.description,
                        item.status,
                        item.reconciliation_status,
                        item.source_system,
                        item.source_workspace,
                        item.source_project,
                        item.source_key,
                        item.seeded_from_mirror_id,
                        now,
                    ],
                )?;

                fetch_planning_item_by_canonical_key(conn, canonical_key)?
                    .ok_or_else(|| anyhow!("planning item disappeared after upsert"))
            } else {
                conn.execute(
                    r#"
                    INSERT INTO planning_items (
                        canonical_key,
                        item_type,
                        title,
                        description,
                        status,
                        reconciliation_status,
                        source_system,
                        source_workspace,
                        source_project,
                        source_key,
                        seeded_from_mirror_id,
                        created_at,
                        updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
                    "#,
                    params![
                        Option::<&str>::None,
                        item.item_type,
                        item.title,
                        item.description,
                        item.status,
                        item.reconciliation_status,
                        item.source_system,
                        item.source_workspace,
                        item.source_project,
                        item.source_key,
                        item.seeded_from_mirror_id,
                        now,
                    ],
                )?;

                fetch_planning_item(conn, conn.last_insert_rowid())?
                    .ok_or_else(|| anyhow!("planning item disappeared after insert"))
            }
        })
    }

    pub fn list_planning_items(&self) -> Result<Vec<StoredPlanningItem>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    canonical_key,
                    item_type,
                    title,
                    description,
                    status,
                    reconciliation_status,
                    source_system,
                    source_workspace,
                    source_project,
                    source_key,
                    seeded_from_mirror_id,
                    created_at,
                    updated_at
                FROM planning_items
                ORDER BY item_type ASC, title ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_planning_item_row)?;
            let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(items)
        })
    }

    pub fn list_unreconciled_planning_items(&self) -> Result<Vec<StoredPlanningItem>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    canonical_key,
                    item_type,
                    title,
                    description,
                    status,
                    reconciliation_status,
                    source_system,
                    source_workspace,
                    source_project,
                    source_key,
                    seeded_from_mirror_id,
                    created_at,
                    updated_at
                FROM planning_items
                WHERE reconciliation_status = 'unreconciled'
                ORDER BY item_type ASC, title ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_planning_item_row)?;
            let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(items)
        })
    }

    pub fn ensure_planning_item_link(
        &self,
        link: NewPlanningItemLink<'_>,
    ) -> Result<StoredPlanningItemLink> {
        self.with_connection(|conn| {
            if let Some(existing) = fetch_planning_item_link(
                conn,
                link.planning_item_id,
                link.link_type,
                link.target_planning_item_id,
                link.target_external_issue_mirror_id,
            )? {
                return Ok(existing);
            }

            let now = Utc::now().to_rfc3339();
            conn.execute(
                r#"
                INSERT INTO planning_item_links (
                    planning_item_id,
                    link_type,
                    target_planning_item_id,
                    target_external_issue_mirror_id,
                    metadata_json,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    link.planning_item_id,
                    link.link_type,
                    link.target_planning_item_id,
                    link.target_external_issue_mirror_id,
                    link.metadata_json,
                    now,
                ],
            )?;

            fetch_planning_item_link_by_id(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("planning item link disappeared after insert"))
        })
    }

    pub fn list_planning_item_links(&self) -> Result<Vec<StoredPlanningItemLink>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    planning_item_id,
                    link_type,
                    target_planning_item_id,
                    target_external_issue_mirror_id,
                    metadata_json,
                    created_at
                FROM planning_item_links
                ORDER BY planning_item_id ASC, link_type ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_planning_item_link_row)?;
            let links = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(links)
        })
    }

    pub fn append_promotion_record(
        &self,
        record: NewPromotionRecord<'_>,
    ) -> Result<StoredPromotionRecord> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO promotion_records (
                    subject_kind,
                    subject_id,
                    promotion_state,
                    reason,
                    source_ingest_run_id,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    record.subject_kind,
                    record.subject_id,
                    record.promotion_state,
                    record.reason,
                    record.source_ingest_run_id,
                    now,
                ],
            )?;

            fetch_promotion_record(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("promotion record disappeared after insert"))
        })
    }

    pub fn list_promotion_records(&self) -> Result<Vec<StoredPromotionRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    subject_kind,
                    subject_id,
                    promotion_state,
                    reason,
                    source_ingest_run_id,
                    created_at
                FROM promotion_records
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_promotion_record_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
        })
    }

    pub fn insert_cadence_object(
        &self,
        record: NewCadenceObject<'_>,
    ) -> Result<StoredCadenceObject> {
        let now = Utc::now().to_rfc3339();
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;

        if record.is_current {
            transaction.execute(
                r#"
                UPDATE cadence_objects
                SET is_current = 0,
                    status = CASE
                        WHEN status = 'active' THEN 'superseded'
                        ELSE status
                    END,
                    updated_at = ?2
                WHERE cadence_kind = ?1
                  AND is_current != 0
                "#,
                params![record.cadence_kind, now],
            )?;
        }

        transaction.execute(
            r#"
            INSERT INTO cadence_objects (
                cadence_kind,
                title,
                summary,
                payload_json,
                scope_type,
                scope_ref,
                source_type,
                source_ref,
                admission_policy,
                projection_policy,
                status,
                is_current,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
            "#,
            params![
                record.cadence_kind,
                record.title,
                record.summary,
                record.payload_json,
                record.scope_type,
                record.scope_ref,
                record.source_type,
                record.source_ref,
                record.admission_policy,
                record.projection_policy,
                record.status,
                if record.is_current { 1 } else { 0 },
                now,
            ],
        )?;
        let row_id = transaction.last_insert_rowid();
        let cadence_object = fetch_cadence_object_by_id(&transaction, row_id)?
            .ok_or_else(|| anyhow!("cadence object disappeared after insert"))?;
        transaction.commit()?;
        Ok(cadence_object)
    }

    pub fn list_cadence_objects(&self) -> Result<Vec<StoredCadenceObject>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    cadence_kind,
                    title,
                    summary,
                    payload_json,
                    scope_type,
                    scope_ref,
                    source_type,
                    source_ref,
                    admission_policy,
                    projection_policy,
                    status,
                    is_current,
                    created_at,
                    updated_at
                FROM cadence_objects
                ORDER BY cadence_kind ASC, is_current DESC, id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_cadence_object_row)?;
            let objects = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(objects)
        })
    }

    pub fn list_cadence_objects_by_kind(
        &self,
        cadence_kind: &str,
    ) -> Result<Vec<StoredCadenceObject>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    cadence_kind,
                    title,
                    summary,
                    payload_json,
                    scope_type,
                    scope_ref,
                    source_type,
                    source_ref,
                    admission_policy,
                    projection_policy,
                    status,
                    is_current,
                    created_at,
                    updated_at
                FROM cadence_objects
                WHERE cadence_kind = ?1
                ORDER BY is_current DESC, id DESC
                "#,
            )?;
            let rows = stmt.query_map([cadence_kind], map_cadence_object_row)?;
            let objects = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(objects)
        })
    }

    pub fn insert_cadence_link(&self, record: NewCadenceLink<'_>) -> Result<StoredCadenceLink> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO cadence_links (
                    src_cadence_object_id,
                    dst_cadence_object_id,
                    relation_type,
                    status,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(src_cadence_object_id, dst_cadence_object_id, relation_type) DO UPDATE SET
                    status = excluded.status,
                    created_at = excluded.created_at
                "#,
                params![
                    record.src_cadence_object_id,
                    record.dst_cadence_object_id,
                    record.relation_type,
                    record.status,
                    now,
                ],
            )?;

            fetch_cadence_link(
                conn,
                record.src_cadence_object_id,
                record.dst_cadence_object_id,
                record.relation_type,
            )?
            .ok_or_else(|| anyhow!("cadence link disappeared after upsert"))
        })
    }

    pub fn list_cadence_links(&self) -> Result<Vec<StoredCadenceLink>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    src_cadence_object_id,
                    dst_cadence_object_id,
                    relation_type,
                    status,
                    created_at
                FROM cadence_links
                ORDER BY id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_cadence_link_row)?;
            let links = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(links)
        })
    }

    pub fn insert_nota_runtime_transaction(
        &self,
        record: NewNotaRuntimeTransaction<'_>,
    ) -> Result<StoredNotaRuntimeTransaction> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO nota_runtime_transactions (
                    actor_role,
                    surface_action,
                    transaction_kind,
                    title,
                    payload_json,
                    status,
                    forge_task_id,
                    cadence_checkpoint_id,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                "#,
                params![
                    record.actor_role,
                    record.surface_action,
                    record.transaction_kind,
                    record.title,
                    record.payload_json,
                    record.status,
                    record.forge_task_id,
                    record.cadence_checkpoint_id,
                    now,
                ],
            )?;

            fetch_nota_runtime_transaction(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("nota runtime transaction disappeared after insert"))
        })
    }

    pub fn update_nota_runtime_transaction(
        &self,
        id: i64,
        update: NotaRuntimeTransactionUpdate<'_>,
    ) -> Result<StoredNotaRuntimeTransaction> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE nota_runtime_transactions
                SET status = ?2,
                    forge_task_id = COALESCE(?3, forge_task_id),
                    cadence_checkpoint_id = COALESCE(?4, cadence_checkpoint_id),
                    updated_at = ?5
                WHERE id = ?1
                "#,
                params![
                    id,
                    update.status,
                    update.forge_task_id,
                    update.cadence_checkpoint_id,
                    now,
                ],
            )?;

            fetch_nota_runtime_transaction(conn, id)?
                .ok_or_else(|| anyhow!("nota runtime transaction `{id}` does not exist"))
        })
    }

    pub fn get_nota_runtime_transaction(
        &self,
        id: i64,
    ) -> Result<Option<StoredNotaRuntimeTransaction>> {
        self.with_connection(|conn| fetch_nota_runtime_transaction(conn, id))
    }

    pub fn list_nota_runtime_transactions(&self) -> Result<Vec<StoredNotaRuntimeTransaction>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    actor_role,
                    surface_action,
                    transaction_kind,
                    title,
                    payload_json,
                    status,
                    forge_task_id,
                    cadence_checkpoint_id,
                    created_at,
                    updated_at
                FROM nota_runtime_transactions
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_nota_runtime_transaction_row)?;
            let transactions = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(transactions)
        })
    }

    pub fn append_nota_runtime_receipt(
        &self,
        record: NewNotaRuntimeReceipt<'_>,
    ) -> Result<StoredNotaRuntimeReceipt> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO nota_runtime_receipts (
                    transaction_id,
                    receipt_kind,
                    payload_json,
                    status,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    record.transaction_id,
                    record.receipt_kind,
                    record.payload_json,
                    record.status,
                    now,
                ],
            )?;

            fetch_nota_runtime_receipt(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("nota runtime receipt disappeared after insert"))
        })
    }

    pub fn list_nota_runtime_receipts(
        &self,
        transaction_id: Option<i64>,
    ) -> Result<Vec<StoredNotaRuntimeReceipt>> {
        self.with_connection(|conn| {
            let mut stmt = if transaction_id.is_some() {
                conn.prepare(
                    r#"
                    SELECT
                        id,
                        transaction_id,
                        receipt_kind,
                        payload_json,
                        status,
                        created_at
                    FROM nota_runtime_receipts
                    WHERE transaction_id = ?1
                    ORDER BY id ASC
                    "#,
                )?
            } else {
                conn.prepare(
                    r#"
                    SELECT
                        id,
                        transaction_id,
                        receipt_kind,
                        payload_json,
                        status,
                        created_at
                    FROM nota_runtime_receipts
                    ORDER BY id ASC
                    "#,
                )?
            };

            let rows = if let Some(transaction_id) = transaction_id {
                stmt.query_map([transaction_id], map_nota_runtime_receipt_row)?
            } else {
                stmt.query_map([], map_nota_runtime_receipt_row)?
            };
            let receipts = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(receipts)
        })
    }

    pub fn insert_nota_runtime_allocation(
        &self,
        record: NewNotaRuntimeAllocation<'_>,
    ) -> Result<StoredNotaRuntimeAllocation> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO nota_runtime_allocations (
                    allocator_role,
                    allocator_surface,
                    allocation_kind,
                    source_transaction_id,
                    lineage_ref,
                    child_execution_kind,
                    child_execution_ref,
                    return_target_kind,
                    return_target_ref,
                    escalation_target_kind,
                    escalation_target_ref,
                    status,
                    payload_json,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
                "#,
                params![
                    record.allocator_role,
                    record.allocator_surface,
                    record.allocation_kind,
                    record.source_transaction_id,
                    record.lineage_ref,
                    record.child_execution_kind,
                    record.child_execution_ref,
                    record.return_target_kind,
                    record.return_target_ref,
                    record.escalation_target_kind,
                    record.escalation_target_ref,
                    record.status,
                    record.payload_json,
                    now,
                ],
            )?;

            fetch_nota_runtime_allocation(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("nota runtime allocation disappeared after insert"))
        })
    }

    pub fn update_nota_runtime_allocation(
        &self,
        id: i64,
        update: NotaRuntimeAllocationUpdate<'_>,
    ) -> Result<StoredNotaRuntimeAllocation> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE nota_runtime_allocations
                SET status = ?2,
                    payload_json = COALESCE(?3, payload_json),
                    updated_at = ?4
                WHERE id = ?1
                "#,
                params![id, update.status, update.payload_json, now],
            )?;

            fetch_nota_runtime_allocation(conn, id)?
                .ok_or_else(|| anyhow!("nota runtime allocation `{id}` does not exist"))
        })
    }

    pub fn list_nota_runtime_allocations(&self) -> Result<Vec<StoredNotaRuntimeAllocation>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    allocator_role,
                    allocator_surface,
                    allocation_kind,
                    source_transaction_id,
                    lineage_ref,
                    child_execution_kind,
                    child_execution_ref,
                    return_target_kind,
                    return_target_ref,
                    escalation_target_kind,
                    escalation_target_ref,
                    status,
                    payload_json,
                    created_at,
                    updated_at
                FROM nota_runtime_allocations
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_nota_runtime_allocation_row)?;
            let allocations = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(allocations)
        })
    }

    pub fn list_memory_fragment_records(&self) -> Result<Vec<StoredMemoryFragment>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    title,
                    content,
                    kind,
                    source_type,
                    source_ref,
                    source_hash,
                    scope_type,
                    scope_ref,
                    target_table,
                    target_ref,
                    status,
                    triage_status,
                    temperature,
                    tags,
                    notes,
                    confidence,
                    created_at,
                    updated_at
                FROM memory_fragments
                ORDER BY kind ASC, target_ref ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_memory_fragment_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
        })
    }

    pub fn list_memory_link_records(&self) -> Result<Vec<StoredMemoryLink>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    src_kind,
                    src_id,
                    dst_kind,
                    dst_id,
                    relation_type,
                    status,
                    created_at
                FROM memory_links
                ORDER BY id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_memory_link_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
        })
    }

    pub fn upsert_document_record(&self, record: UpsertDocumentRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO documents (
                    id, slug, title, content, category, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(id) DO UPDATE SET
                    slug = excluded.slug,
                    title = excluded.title,
                    content = excluded.content,
                    category = excluded.category,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                "#,
                params![
                    record.id,
                    record.slug,
                    record.title,
                    record.content,
                    record.category,
                    record.created_at,
                    record.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn upsert_todo_record(&self, record: UpsertTodoRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO todos (
                    id, title, status, priority, project, created_at, done_at, temperature,
                    due_on, remind_every_days, remind_next_on, last_reminded_at, reminder_status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    status = excluded.status,
                    priority = excluded.priority,
                    project = excluded.project,
                    created_at = excluded.created_at,
                    done_at = excluded.done_at,
                    temperature = excluded.temperature,
                    due_on = excluded.due_on,
                    remind_every_days = excluded.remind_every_days,
                    remind_next_on = excluded.remind_next_on,
                    last_reminded_at = excluded.last_reminded_at,
                    reminder_status = excluded.reminder_status
                "#,
                params![
                    record.id,
                    record.title,
                    record.status,
                    record.priority,
                    record.project,
                    record.created_at,
                    record.done_at,
                    record.temperature,
                    record.due_on,
                    record.remind_every_days,
                    record.remind_next_on,
                    record.last_reminded_at,
                    record.reminder_status,
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_todo_records(&self) -> Result<Vec<StoredTodoRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    title,
                    status,
                    priority,
                    project,
                    created_at,
                    done_at,
                    temperature,
                    due_on,
                    remind_every_days,
                    remind_next_on,
                    last_reminded_at,
                    reminder_status
                FROM todos
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_todo_record_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
        })
    }

    pub fn upsert_instinct_record(&self, record: UpsertInstinctRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO instincts (
                    id, pattern, action, confidence, source, ref, created_at, status,
                    surfaced_to, review_status, origin_type, lifecycle_status, temperature, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ON CONFLICT(id) DO UPDATE SET
                    pattern = excluded.pattern,
                    action = excluded.action,
                    confidence = excluded.confidence,
                    source = excluded.source,
                    ref = excluded.ref,
                    created_at = excluded.created_at,
                    status = excluded.status,
                    surfaced_to = excluded.surfaced_to,
                    review_status = excluded.review_status,
                    origin_type = excluded.origin_type,
                    lifecycle_status = excluded.lifecycle_status,
                    temperature = excluded.temperature,
                    updated_at = excluded.updated_at
                "#,
                params![
                    record.id,
                    record.pattern,
                    record.action,
                    record.confidence,
                    record.source,
                    record.reference,
                    record.created_at,
                    record.status,
                    record.surfaced_to,
                    record.review_status,
                    record.origin_type,
                    record.lifecycle_status,
                    record.temperature,
                    record.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn upsert_coffee_chat_record(&self, record: UpsertCoffeeChatRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO coffee_chats (
                    id, project, stage, retro, forward, priorities, created_at, temperature
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO UPDATE SET
                    project = excluded.project,
                    stage = excluded.stage,
                    retro = excluded.retro,
                    forward = excluded.forward,
                    priorities = excluded.priorities,
                    created_at = excluded.created_at,
                    temperature = excluded.temperature
                "#,
                params![
                    record.id,
                    record.project,
                    record.stage,
                    record.retro,
                    record.forward,
                    record.priorities,
                    record.created_at,
                    record.temperature,
                ],
            )?;
            Ok(())
        })
    }

    pub fn upsert_decision_record(&self, record: UpsertDecisionRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO decisions (
                    id, title, statement, rationale, decision_type, decision_status, scope_type,
                    scope_ref, source_ref, decided_by, enforcement_level, actor_scope,
                    confidence, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    statement = excluded.statement,
                    rationale = excluded.rationale,
                    decision_type = excluded.decision_type,
                    decision_status = excluded.decision_status,
                    scope_type = excluded.scope_type,
                    scope_ref = excluded.scope_ref,
                    source_ref = excluded.source_ref,
                    decided_by = excluded.decided_by,
                    enforcement_level = excluded.enforcement_level,
                    actor_scope = excluded.actor_scope,
                    confidence = excluded.confidence,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                "#,
                params![
                    record.id,
                    record.title,
                    record.statement,
                    record.rationale,
                    record.decision_type,
                    record.decision_status,
                    record.scope_type,
                    record.scope_ref,
                    record.source_ref,
                    record.decided_by,
                    record.enforcement_level,
                    record.actor_scope,
                    record.confidence,
                    record.created_at,
                    record.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn insert_decision_record(
        &self,
        record: NewDecisionRecord<'_>,
    ) -> Result<StoredDecisionRecord> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO decisions (
                    title,
                    statement,
                    rationale,
                    decision_type,
                    decision_status,
                    scope_type,
                    scope_ref,
                    source_ref,
                    decided_by,
                    enforcement_level,
                    actor_scope,
                    confidence,
                    created_at,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
                "#,
                params![
                    record.title,
                    record.statement,
                    record.rationale,
                    record.decision_type,
                    record.decision_status,
                    record.scope_type,
                    record.scope_ref,
                    record.source_ref,
                    record.decided_by,
                    record.enforcement_level,
                    record.actor_scope,
                    record.confidence,
                    now,
                ],
            )?;

            fetch_decision_record(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("decision disappeared after insert"))
        })
    }

    pub fn get_decision_record(&self, id: i64) -> Result<Option<StoredDecisionRecord>> {
        self.with_connection(|conn| fetch_decision_record(conn, id))
    }

    pub fn list_decision_records(&self) -> Result<Vec<StoredDecisionRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    title,
                    statement,
                    rationale,
                    decision_type,
                    decision_status,
                    scope_type,
                    scope_ref,
                    source_ref,
                    decided_by,
                    enforcement_level,
                    actor_scope,
                    confidence,
                    created_at,
                    updated_at
                FROM decisions
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_decision_record_row)?;
            let decisions = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(decisions)
        })
    }

    pub fn insert_decision_link(&self, record: NewDecisionLink<'_>) -> Result<StoredDecisionLink> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO decision_links (
                    src_decision_id,
                    dst_decision_id,
                    relation_type,
                    status,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(src_decision_id, dst_decision_id, relation_type) DO UPDATE SET
                    status = excluded.status,
                    created_at = excluded.created_at
                "#,
                params![
                    record.src_decision_id,
                    record.dst_decision_id,
                    record.relation_type,
                    record.status,
                    now,
                ],
            )?;

            fetch_decision_link(
                conn,
                record.src_decision_id,
                record.dst_decision_id,
                record.relation_type,
            )?
            .ok_or_else(|| anyhow!("decision link disappeared after upsert"))
        })
    }

    pub fn list_decision_links(&self) -> Result<Vec<StoredDecisionLink>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    src_decision_id,
                    dst_decision_id,
                    relation_type,
                    status,
                    created_at
                FROM decision_links
                ORDER BY id ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_decision_link_row)?;
            let links = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(links)
        })
    }

    pub fn upsert_chat_archive_setting(
        &self,
        record: ChatArchiveSettingRecord<'_>,
    ) -> Result<StoredChatArchiveSetting> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO chat_archive_settings (
                    scope_type,
                    scope_ref,
                    archive_policy,
                    updated_at
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(scope_type, scope_ref) DO UPDATE SET
                    archive_policy = excluded.archive_policy,
                    updated_at = excluded.updated_at
                "#,
                params![
                    record.scope_type,
                    record.scope_ref,
                    record.archive_policy,
                    now,
                ],
            )?;

            fetch_chat_archive_setting(conn, record.scope_type, record.scope_ref)?
                .ok_or_else(|| anyhow!("chat archive setting disappeared after upsert"))
        })
    }

    pub fn get_chat_archive_setting(
        &self,
        scope_type: &str,
        scope_ref: &str,
    ) -> Result<Option<StoredChatArchiveSetting>> {
        self.with_connection(|conn| fetch_chat_archive_setting(conn, scope_type, scope_ref))
    }

    pub fn list_chat_archive_settings(&self) -> Result<Vec<StoredChatArchiveSetting>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    scope_type,
                    scope_ref,
                    archive_policy,
                    updated_at
                FROM chat_archive_settings
                ORDER BY scope_type ASC, scope_ref ASC
                "#,
            )?;
            let rows = stmt.query_map([], map_chat_archive_setting_row)?;
            let settings = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(settings)
        })
    }

    pub fn insert_chat_capture_record(
        &self,
        record: NewChatCaptureRecord<'_>,
    ) -> Result<StoredChatCaptureRecord> {
        let now = Utc::now().to_rfc3339();
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO chat_capture_records (
                    session_ref,
                    role,
                    capture_mode,
                    archive_policy,
                    content,
                    summary,
                    scope_type,
                    scope_ref,
                    linked_decision_id,
                    status,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                "#,
                params![
                    record.session_ref,
                    record.role,
                    record.capture_mode,
                    record.archive_policy,
                    record.content,
                    record.summary,
                    record.scope_type,
                    record.scope_ref,
                    record.linked_decision_id,
                    record.status,
                    now,
                ],
            )?;

            fetch_chat_capture_record(conn, conn.last_insert_rowid())?
                .ok_or_else(|| anyhow!("chat capture disappeared after insert"))
        })
    }

    pub fn list_chat_capture_records(&self) -> Result<Vec<StoredChatCaptureRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    session_ref,
                    role,
                    capture_mode,
                    archive_policy,
                    content,
                    summary,
                    scope_type,
                    scope_ref,
                    linked_decision_id,
                    status,
                    created_at
                FROM chat_capture_records
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_chat_capture_record_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
        })
    }

    pub fn upsert_vision_record(&self, record: UpsertVisionRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO visions (
                    id, title, statement, horizon, vision_status, scope_type, scope_ref,
                    source_ref, confidence, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    statement = excluded.statement,
                    horizon = excluded.horizon,
                    vision_status = excluded.vision_status,
                    scope_type = excluded.scope_type,
                    scope_ref = excluded.scope_ref,
                    source_ref = excluded.source_ref,
                    confidence = excluded.confidence,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                "#,
                params![
                    record.id,
                    record.title,
                    record.statement,
                    record.horizon,
                    record.vision_status,
                    record.scope_type,
                    record.scope_ref,
                    record.source_ref,
                    record.confidence,
                    record.created_at,
                    record.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_vision_records(&self) -> Result<Vec<StoredVisionRecord>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                    id,
                    title,
                    statement,
                    horizon,
                    vision_status,
                    scope_type,
                    scope_ref,
                    source_ref,
                    confidence,
                    created_at,
                    updated_at
                FROM visions
                ORDER BY id DESC
                "#,
            )?;
            let rows = stmt.query_map([], map_vision_record_row)?;
            let records = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(records)
        })
    }

    pub fn upsert_memory_fragment_record(
        &self,
        record: UpsertMemoryFragmentRecord<'_>,
    ) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO memory_fragments (
                    id, title, content, kind, source_type, source_ref, source_hash, scope_type,
                    scope_ref, target_table, target_ref, status, triage_status, temperature,
                    tags, notes, confidence, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    content = excluded.content,
                    kind = excluded.kind,
                    source_type = excluded.source_type,
                    source_ref = excluded.source_ref,
                    source_hash = excluded.source_hash,
                    scope_type = excluded.scope_type,
                    scope_ref = excluded.scope_ref,
                    target_table = excluded.target_table,
                    target_ref = excluded.target_ref,
                    status = excluded.status,
                    triage_status = excluded.triage_status,
                    temperature = excluded.temperature,
                    tags = excluded.tags,
                    notes = excluded.notes,
                    confidence = excluded.confidence,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                "#,
                params![
                    record.id,
                    record.title,
                    record.content,
                    record.kind,
                    record.source_type,
                    record.source_ref,
                    record.source_hash,
                    record.scope_type,
                    record.scope_ref,
                    record.target_table,
                    record.target_ref,
                    record.status,
                    record.triage_status,
                    record.temperature,
                    record.tags,
                    record.notes,
                    record.confidence,
                    record.created_at,
                    record.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn upsert_memory_link_record(&self, record: UpsertMemoryLinkRecord<'_>) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO memory_links (
                    id, src_kind, src_id, dst_kind, dst_id, relation_type, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO UPDATE SET
                    src_kind = excluded.src_kind,
                    src_id = excluded.src_id,
                    dst_kind = excluded.dst_kind,
                    dst_id = excluded.dst_id,
                    relation_type = excluded.relation_type,
                    status = excluded.status,
                    created_at = excluded.created_at
                "#,
                params![
                    record.id,
                    record.src_kind,
                    record.src_id,
                    record.dst_kind,
                    record.dst_id,
                    record.relation_type,
                    record.status,
                    record.created_at,
                ],
            )?;
            Ok(())
        })
    }

    fn migrate(&self, migration_plan: MigrationPlan<'_>) -> Result<()> {
        self.with_connection(|connection| {
            for migration in migration_plan
                .core
                .iter()
                .chain(migration_plan.plugins.iter())
            {
                let _ = migration.name;
                connection.execute_batch(migration.sql)?;
            }
            ensure_forge_task_columns(connection)?;
            ensure_curated_memory_tables(connection)?;
            Ok(())
        })
    }

    fn with_connection<T, F>(&self, callback: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let connection = self.lock_connection()?;
        callback(&connection)
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| anyhow!("database connection lock poisoned"))
    }
}

fn map_launcher_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredLauncherApp> {
    Ok(StoredLauncherApp {
        id: row.get(0)?,
        name: row.get(1)?,
        normalized_name: row.get(2)?,
        path: row.get(3)?,
        arguments: row.get(4)?,
        working_dir: row.get(5)?,
        icon_path: row.get(6)?,
        source: row.get(7)?,
        launch_count: row.get(8)?,
        last_used: row.get(9)?,
        pinned: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_forge_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredForgeTask> {
    Ok(StoredForgeTask {
        id: row.get(0)?,
        name: row.get(1)?,
        command: row.get(2)?,
        args: row.get(3)?,
        working_dir: row.get(4)?,
        stdin_text: row.get(5)?,
        required_tokens: row.get(6)?,
        metadata: row.get(7)?,
        status: row.get(8)?,
        status_message: row.get(9)?,
        exit_code: row.get(10)?,
        created_at: row.get(11)?,
        heartbeat_at: row.get(12)?,
        finished_at: row.get(13)?,
    })
}

fn map_forge_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredForgeTaskLog> {
    Ok(StoredForgeTaskLog {
        id: row.get(0)?,
        task_id: row.get(1)?,
        stream: row.get(2)?,
        line: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn map_forge_dispatch_receipt_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredForgeDispatchReceipt> {
    Ok(StoredForgeDispatchReceipt {
        id: row.get(0)?,
        parent_task_id: row.get(1)?,
        child_task_id: row.get(2)?,
        supervision_scope: row.get(3)?,
        supervision_strategy: row.get(4)?,
        child_dispatch_role: row.get(5)?,
        child_dispatch_tool_name: row.get(6)?,
        child_slot: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn map_vault_token_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVaultToken> {
    Ok(StoredVaultToken {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_encrypted_vault_token_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncryptedVaultToken> {
    Ok(EncryptedVaultToken {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        encrypted_value: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn map_vault_mcp_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVaultMcpConfig> {
    Ok(StoredVaultMcpConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        transport: row.get(2)?,
        endpoint: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn map_source_ingest_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSourceIngestRun> {
    Ok(StoredSourceIngestRun {
        id: row.get(0)?,
        source_system: row.get(1)?,
        source_workspace: row.get(2)?,
        source_project: row.get(3)?,
        artifact_path: row.get(4)?,
        artifact_sha256: row.get(5)?,
        status: row.get(6)?,
        imported_issue_count: row.get(7)?,
        imported_document_count: row.get(8)?,
        imported_milestone_count: row.get(9)?,
        imported_planning_item_count: row.get(10)?,
        error_message: row.get(11)?,
        created_at: row.get(12)?,
        completed_at: row.get(13)?,
    })
}

fn map_source_artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSourceArtifact> {
    Ok(StoredSourceArtifact {
        id: row.get(0)?,
        ingest_run_id: row.get(1)?,
        artifact_kind: row.get(2)?,
        artifact_key: row.get(3)?,
        title: row.get(4)?,
        url: row.get(5)?,
        payload_json: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn map_external_issue_mirror_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredExternalIssueMirror> {
    Ok(StoredExternalIssueMirror {
        id: row.get(0)?,
        mirror_key: row.get(1)?,
        source_system: row.get(2)?,
        source_workspace: row.get(3)?,
        source_project: row.get(4)?,
        external_issue_id: row.get(5)?,
        project_name: row.get(6)?,
        team_name: row.get(7)?,
        parent_external_issue_id: row.get(8)?,
        title: row.get(9)?,
        description: row.get(10)?,
        state: row.get(11)?,
        priority: row.get(12)?,
        url: row.get(13)?,
        labels_json: row.get(14)?,
        relations_json: row.get(15)?,
        payload_json: row.get(16)?,
        git_branch_name: row.get(17)?,
        due_date: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
        completed_at: row.get(21)?,
        archived_at: row.get(22)?,
        first_seen_at: row.get(23)?,
        last_seen_at: row.get(24)?,
        last_ingest_run_id: row.get(25)?,
    })
}

fn map_planning_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPlanningItem> {
    Ok(StoredPlanningItem {
        id: row.get(0)?,
        canonical_key: row.get(1)?,
        item_type: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        status: row.get(5)?,
        reconciliation_status: row.get(6)?,
        source_system: row.get(7)?,
        source_workspace: row.get(8)?,
        source_project: row.get(9)?,
        source_key: row.get(10)?,
        seeded_from_mirror_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn map_planning_item_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPlanningItemLink> {
    Ok(StoredPlanningItemLink {
        id: row.get(0)?,
        planning_item_id: row.get(1)?,
        link_type: row.get(2)?,
        target_planning_item_id: row.get(3)?,
        target_external_issue_mirror_id: row.get(4)?,
        metadata_json: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn map_promotion_record_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPromotionRecord> {
    Ok(StoredPromotionRecord {
        id: row.get(0)?,
        subject_kind: row.get(1)?,
        subject_id: row.get(2)?,
        promotion_state: row.get(3)?,
        reason: row.get(4)?,
        source_ingest_run_id: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn map_cadence_object_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredCadenceObject> {
    Ok(StoredCadenceObject {
        id: row.get(0)?,
        cadence_kind: row.get(1)?,
        title: row.get(2)?,
        summary: row.get(3)?,
        payload_json: row.get(4)?,
        scope_type: row.get(5)?,
        scope_ref: row.get(6)?,
        source_type: row.get(7)?,
        source_ref: row.get(8)?,
        admission_policy: row.get(9)?,
        projection_policy: row.get(10)?,
        status: row.get(11)?,
        is_current: row.get::<_, i64>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn map_cadence_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredCadenceLink> {
    Ok(StoredCadenceLink {
        id: row.get(0)?,
        src_cadence_object_id: row.get(1)?,
        dst_cadence_object_id: row.get(2)?,
        relation_type: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn map_nota_runtime_transaction_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredNotaRuntimeTransaction> {
    Ok(StoredNotaRuntimeTransaction {
        id: row.get(0)?,
        actor_role: row.get(1)?,
        surface_action: row.get(2)?,
        transaction_kind: row.get(3)?,
        title: row.get(4)?,
        payload_json: row.get(5)?,
        status: row.get(6)?,
        forge_task_id: row.get(7)?,
        cadence_checkpoint_id: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn map_nota_runtime_receipt_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredNotaRuntimeReceipt> {
    Ok(StoredNotaRuntimeReceipt {
        id: row.get(0)?,
        transaction_id: row.get(1)?,
        receipt_kind: row.get(2)?,
        payload_json: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn map_nota_runtime_allocation_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredNotaRuntimeAllocation> {
    Ok(StoredNotaRuntimeAllocation {
        id: row.get(0)?,
        allocator_role: row.get(1)?,
        allocator_surface: row.get(2)?,
        allocation_kind: row.get(3)?,
        source_transaction_id: row.get(4)?,
        lineage_ref: row.get(5)?,
        child_execution_kind: row.get(6)?,
        child_execution_ref: row.get(7)?,
        return_target_kind: row.get(8)?,
        return_target_ref: row.get(9)?,
        escalation_target_kind: row.get(10)?,
        escalation_target_ref: row.get(11)?,
        status: row.get(12)?,
        payload_json: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn map_decision_record_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDecisionRecord> {
    Ok(StoredDecisionRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        statement: row.get(2)?,
        rationale: row.get(3)?,
        decision_type: row.get(4)?,
        decision_status: row.get(5)?,
        scope_type: row.get(6)?,
        scope_ref: row.get(7)?,
        source_ref: row.get(8)?,
        decided_by: row.get(9)?,
        enforcement_level: row.get(10)?,
        actor_scope: row.get(11)?,
        confidence: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn map_decision_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDecisionLink> {
    Ok(StoredDecisionLink {
        id: row.get(0)?,
        src_decision_id: row.get(1)?,
        dst_decision_id: row.get(2)?,
        relation_type: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn map_chat_archive_setting_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredChatArchiveSetting> {
    Ok(StoredChatArchiveSetting {
        id: row.get(0)?,
        scope_type: row.get(1)?,
        scope_ref: row.get(2)?,
        archive_policy: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_chat_capture_record_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredChatCaptureRecord> {
    Ok(StoredChatCaptureRecord {
        id: row.get(0)?,
        session_ref: row.get(1)?,
        role: row.get(2)?,
        capture_mode: row.get(3)?,
        archive_policy: row.get(4)?,
        content: row.get(5)?,
        summary: row.get(6)?,
        scope_type: row.get(7)?,
        scope_ref: row.get(8)?,
        linked_decision_id: row.get(9)?,
        status: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn map_todo_record_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredTodoRecord> {
    Ok(StoredTodoRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        status: row.get(2)?,
        priority: row.get(3)?,
        project: row.get(4)?,
        created_at: row.get(5)?,
        done_at: row.get(6)?,
        temperature: row.get(7)?,
        due_on: row.get(8)?,
        remind_every_days: row.get(9)?,
        remind_next_on: row.get(10)?,
        last_reminded_at: row.get(11)?,
        reminder_status: row.get(12)?,
    })
}

fn map_vision_record_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVisionRecord> {
    Ok(StoredVisionRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        statement: row.get(2)?,
        horizon: row.get(3)?,
        vision_status: row.get(4)?,
        scope_type: row.get(5)?,
        scope_ref: row.get(6)?,
        source_ref: row.get(7)?,
        confidence: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn map_memory_fragment_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredMemoryFragment> {
    Ok(StoredMemoryFragment {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        kind: row.get(3)?,
        source_type: row.get(4)?,
        source_ref: row.get(5)?,
        source_hash: row.get(6)?,
        scope_type: row.get(7)?,
        scope_ref: row.get(8)?,
        target_table: row.get(9)?,
        target_ref: row.get(10)?,
        status: row.get(11)?,
        triage_status: row.get(12)?,
        temperature: row.get(13)?,
        tags: row.get(14)?,
        notes: row.get(15)?,
        confidence: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn map_memory_link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredMemoryLink> {
    Ok(StoredMemoryLink {
        id: row.get(0)?,
        src_kind: row.get(1)?,
        src_id: row.get(2)?,
        dst_kind: row.get(3)?,
        dst_id: row.get(4)?,
        relation_type: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn fetch_vault_mcp_config(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredVaultMcpConfig>> {
    connection
        .query_row(
            r#"
            SELECT id, name, transport, endpoint, enabled, created_at, updated_at
            FROM plugin_vault_mcp_configs
            WHERE id = ?1
            "#,
            [id],
            map_vault_mcp_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_source_ingest_run(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredSourceIngestRun>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                source_system,
                source_workspace,
                source_project,
                artifact_path,
                artifact_sha256,
                status,
                imported_issue_count,
                imported_document_count,
                imported_milestone_count,
                imported_planning_item_count,
                error_message,
                created_at,
                completed_at
            FROM source_ingest_runs
            WHERE id = ?1
            "#,
            [id],
            map_source_ingest_run_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_source_artifact(
    connection: &Connection,
    ingest_run_id: i64,
    artifact_kind: &str,
    artifact_key: &str,
) -> Result<Option<StoredSourceArtifact>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                ingest_run_id,
                artifact_kind,
                artifact_key,
                title,
                url,
                payload_json,
                created_at
            FROM source_artifacts
            WHERE ingest_run_id = ?1
              AND artifact_kind = ?2
              AND artifact_key = ?3
            "#,
            params![ingest_run_id, artifact_kind, artifact_key],
            map_source_artifact_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_external_issue_mirror_by_key(
    connection: &Connection,
    mirror_key: &str,
) -> Result<Option<StoredExternalIssueMirror>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                mirror_key,
                source_system,
                source_workspace,
                source_project,
                external_issue_id,
                project_name,
                team_name,
                parent_external_issue_id,
                title,
                description,
                state,
                priority,
                url,
                labels_json,
                relations_json,
                payload_json,
                git_branch_name,
                due_date,
                created_at,
                updated_at,
                completed_at,
                archived_at,
                first_seen_at,
                last_seen_at,
                last_ingest_run_id
            FROM external_issue_mirrors
            WHERE mirror_key = ?1
            "#,
            [mirror_key],
            map_external_issue_mirror_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item(connection: &Connection, id: i64) -> Result<Option<StoredPlanningItem>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                canonical_key,
                item_type,
                title,
                description,
                status,
                reconciliation_status,
                source_system,
                source_workspace,
                source_project,
                source_key,
                seeded_from_mirror_id,
                created_at,
                updated_at
            FROM planning_items
            WHERE id = ?1
            "#,
            [id],
            map_planning_item_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item_by_canonical_key(
    connection: &Connection,
    canonical_key: &str,
) -> Result<Option<StoredPlanningItem>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                canonical_key,
                item_type,
                title,
                description,
                status,
                reconciliation_status,
                source_system,
                source_workspace,
                source_project,
                source_key,
                seeded_from_mirror_id,
                created_at,
                updated_at
            FROM planning_items
            WHERE canonical_key = ?1
            "#,
            [canonical_key],
            map_planning_item_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item_link(
    connection: &Connection,
    planning_item_id: i64,
    link_type: &str,
    target_planning_item_id: Option<i64>,
    target_external_issue_mirror_id: Option<i64>,
) -> Result<Option<StoredPlanningItemLink>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                planning_item_id,
                link_type,
                target_planning_item_id,
                target_external_issue_mirror_id,
                metadata_json,
                created_at
            FROM planning_item_links
            WHERE planning_item_id = ?1
              AND link_type = ?2
              AND ((target_planning_item_id IS NULL AND ?3 IS NULL) OR target_planning_item_id = ?3)
              AND ((target_external_issue_mirror_id IS NULL AND ?4 IS NULL) OR target_external_issue_mirror_id = ?4)
            LIMIT 1
            "#,
            params![
                planning_item_id,
                link_type,
                target_planning_item_id,
                target_external_issue_mirror_id
            ],
            map_planning_item_link_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_planning_item_link_by_id(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredPlanningItemLink>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                planning_item_id,
                link_type,
                target_planning_item_id,
                target_external_issue_mirror_id,
                metadata_json,
                created_at
            FROM planning_item_links
            WHERE id = ?1
            "#,
            [id],
            map_planning_item_link_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_promotion_record(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredPromotionRecord>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                subject_kind,
                subject_id,
                promotion_state,
                reason,
                source_ingest_run_id,
                created_at
            FROM promotion_records
            WHERE id = ?1
            "#,
            [id],
            map_promotion_record_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_cadence_object_by_id(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredCadenceObject>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                cadence_kind,
                title,
                summary,
                payload_json,
                scope_type,
                scope_ref,
                source_type,
                source_ref,
                admission_policy,
                projection_policy,
                status,
                is_current,
                created_at,
                updated_at
            FROM cadence_objects
            WHERE id = ?1
            "#,
            [id],
            map_cadence_object_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_cadence_link(
    connection: &Connection,
    src_cadence_object_id: i64,
    dst_cadence_object_id: i64,
    relation_type: &str,
) -> Result<Option<StoredCadenceLink>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                src_cadence_object_id,
                dst_cadence_object_id,
                relation_type,
                status,
                created_at
            FROM cadence_links
            WHERE src_cadence_object_id = ?1
              AND dst_cadence_object_id = ?2
              AND relation_type = ?3
            "#,
            params![src_cadence_object_id, dst_cadence_object_id, relation_type],
            map_cadence_link_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_nota_runtime_transaction(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredNotaRuntimeTransaction>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                actor_role,
                surface_action,
                transaction_kind,
                title,
                payload_json,
                status,
                forge_task_id,
                cadence_checkpoint_id,
                created_at,
                updated_at
            FROM nota_runtime_transactions
            WHERE id = ?1
            "#,
            [id],
            map_nota_runtime_transaction_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_nota_runtime_receipt(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredNotaRuntimeReceipt>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                transaction_id,
                receipt_kind,
                payload_json,
                status,
                created_at
            FROM nota_runtime_receipts
            WHERE id = ?1
            "#,
            [id],
            map_nota_runtime_receipt_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_nota_runtime_allocation(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredNotaRuntimeAllocation>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                allocator_role,
                allocator_surface,
                allocation_kind,
                source_transaction_id,
                lineage_ref,
                child_execution_kind,
                child_execution_ref,
                return_target_kind,
                return_target_ref,
                escalation_target_kind,
                escalation_target_ref,
                status,
                payload_json,
                created_at,
                updated_at
            FROM nota_runtime_allocations
            WHERE id = ?1
            "#,
            [id],
            map_nota_runtime_allocation_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_decision_record(connection: &Connection, id: i64) -> Result<Option<StoredDecisionRecord>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                title,
                statement,
                rationale,
                decision_type,
                decision_status,
                scope_type,
                scope_ref,
                source_ref,
                decided_by,
                enforcement_level,
                actor_scope,
                confidence,
                created_at,
                updated_at
            FROM decisions
            WHERE id = ?1
            "#,
            [id],
            map_decision_record_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_decision_link(
    connection: &Connection,
    src_decision_id: i64,
    dst_decision_id: i64,
    relation_type: &str,
) -> Result<Option<StoredDecisionLink>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                src_decision_id,
                dst_decision_id,
                relation_type,
                status,
                created_at
            FROM decision_links
            WHERE src_decision_id = ?1
              AND dst_decision_id = ?2
              AND relation_type = ?3
            "#,
            params![src_decision_id, dst_decision_id, relation_type],
            map_decision_link_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_chat_archive_setting(
    connection: &Connection,
    scope_type: &str,
    scope_ref: &str,
) -> Result<Option<StoredChatArchiveSetting>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                scope_type,
                scope_ref,
                archive_policy,
                updated_at
            FROM chat_archive_settings
            WHERE scope_type = ?1
              AND scope_ref = ?2
            "#,
            params![scope_type, scope_ref],
            map_chat_archive_setting_row,
        )
        .optional()
        .map_err(Into::into)
}

fn fetch_chat_capture_record(
    connection: &Connection,
    id: i64,
) -> Result<Option<StoredChatCaptureRecord>> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                session_ref,
                role,
                capture_mode,
                archive_policy,
                content,
                summary,
                scope_type,
                scope_ref,
                linked_decision_id,
                status,
                created_at
            FROM chat_capture_records
            WHERE id = ?1
            "#,
            [id],
            map_chat_capture_record_row,
        )
        .optional()
        .map_err(Into::into)
}

fn ensure_forge_task_columns(connection: &Connection) -> Result<()> {
    if !table_exists(connection, "plugin_forge_tasks")? {
        return Ok(());
    }

    let mut statement = connection.prepare("PRAGMA table_info(plugin_forge_tasks)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    if !columns.iter().any(|column| column == "required_tokens") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN required_tokens TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "status_message") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN status_message TEXT",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "working_dir") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN working_dir TEXT",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "stdin_text") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN stdin_text TEXT",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "metadata") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN metadata TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }

    if !columns.iter().any(|column| column == "heartbeat_at") {
        connection.execute(
            "ALTER TABLE plugin_forge_tasks ADD COLUMN heartbeat_at TEXT",
            [],
        )?;
    }

    Ok(())
}

fn ensure_curated_memory_tables(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS documents (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            slug        TEXT NOT NULL,
            title       TEXT NOT NULL,
            content     TEXT NOT NULL,
            category    TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS todos (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            title               TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'pending',
            priority            INTEGER NOT NULL DEFAULT 2,
            project             TEXT NOT NULL DEFAULT '',
            created_at          TEXT NOT NULL,
            done_at             TEXT,
            temperature         TEXT NOT NULL DEFAULT 'warm',
            due_on              TEXT NOT NULL DEFAULT '',
            remind_every_days   INTEGER NOT NULL DEFAULT 0,
            remind_next_on      TEXT NOT NULL DEFAULT '',
            last_reminded_at    TEXT NOT NULL DEFAULT '',
            reminder_status     TEXT NOT NULL DEFAULT 'none'
        );

        CREATE TABLE IF NOT EXISTS instincts (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            pattern             TEXT NOT NULL,
            action              TEXT NOT NULL,
            confidence          REAL NOT NULL DEFAULT 0.8,
            source              TEXT NOT NULL DEFAULT '',
            ref                 TEXT NOT NULL DEFAULT '',
            created_at          TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'active',
            surfaced_to         TEXT NOT NULL DEFAULT '',
            review_status       TEXT NOT NULL DEFAULT '',
            origin_type         TEXT NOT NULL DEFAULT 'manual',
            lifecycle_status    TEXT NOT NULL DEFAULT 'active',
            temperature         TEXT NOT NULL DEFAULT 'warm',
            updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS coffee_chats (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            project     TEXT NOT NULL,
            stage       TEXT NOT NULL,
            retro       TEXT NOT NULL,
            forward     TEXT NOT NULL,
            priorities  TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            temperature TEXT NOT NULL DEFAULT 'warm'
        );

        CREATE TABLE IF NOT EXISTS decisions (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            title               TEXT NOT NULL,
            statement           TEXT NOT NULL,
            rationale           TEXT NOT NULL DEFAULT '',
            decision_type       TEXT NOT NULL DEFAULT '',
            decision_status     TEXT NOT NULL DEFAULT 'accepted',
            scope_type          TEXT NOT NULL DEFAULT '',
            scope_ref           TEXT NOT NULL DEFAULT '',
            source_ref          TEXT NOT NULL DEFAULT '',
            decided_by          TEXT NOT NULL DEFAULT '',
            enforcement_level   TEXT NOT NULL DEFAULT '',
            actor_scope         TEXT NOT NULL DEFAULT '',
            confidence          REAL NOT NULL DEFAULT 1.0,
            created_at          TEXT NOT NULL,
            updated_at          TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS visions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            title           TEXT NOT NULL,
            statement       TEXT NOT NULL,
            horizon         TEXT NOT NULL DEFAULT '',
            vision_status   TEXT NOT NULL DEFAULT 'active',
            scope_type      TEXT NOT NULL DEFAULT '',
            scope_ref       TEXT NOT NULL DEFAULT '',
            source_ref      TEXT NOT NULL DEFAULT '',
            confidence      REAL NOT NULL DEFAULT 1.0,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_fragments (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            title           TEXT NOT NULL,
            content         TEXT NOT NULL,
            kind            TEXT NOT NULL DEFAULT '',
            source_type     TEXT NOT NULL DEFAULT '',
            source_ref      TEXT NOT NULL DEFAULT '',
            source_hash     TEXT NOT NULL DEFAULT '',
            scope_type      TEXT NOT NULL DEFAULT '',
            scope_ref       TEXT NOT NULL DEFAULT '',
            target_table    TEXT NOT NULL DEFAULT '',
            target_ref      TEXT NOT NULL DEFAULT '',
            status          TEXT NOT NULL DEFAULT '',
            triage_status   TEXT NOT NULL DEFAULT '',
            temperature     TEXT NOT NULL DEFAULT 'warm',
            tags            TEXT NOT NULL DEFAULT '',
            notes           TEXT NOT NULL DEFAULT '',
            confidence      REAL NOT NULL DEFAULT 0.0,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_links (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            src_kind        TEXT NOT NULL,
            src_id          INTEGER NOT NULL,
            dst_kind        TEXT NOT NULL,
            dst_id          INTEGER NOT NULL,
            relation_type   TEXT NOT NULL,
            status          TEXT NOT NULL DEFAULT 'active',
            created_at      TEXT NOT NULL
        );
        "#,
    )?;

    ensure_table_column(
        connection,
        "todos",
        "temperature",
        "ALTER TABLE todos ADD COLUMN temperature TEXT NOT NULL DEFAULT 'warm'",
    )?;
    ensure_table_column(
        connection,
        "todos",
        "due_on",
        "ALTER TABLE todos ADD COLUMN due_on TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_table_column(
        connection,
        "todos",
        "remind_every_days",
        "ALTER TABLE todos ADD COLUMN remind_every_days INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_table_column(
        connection,
        "todos",
        "remind_next_on",
        "ALTER TABLE todos ADD COLUMN remind_next_on TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_table_column(
        connection,
        "todos",
        "last_reminded_at",
        "ALTER TABLE todos ADD COLUMN last_reminded_at TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_table_column(
        connection,
        "todos",
        "reminder_status",
        "ALTER TABLE todos ADD COLUMN reminder_status TEXT NOT NULL DEFAULT 'none'",
    )?;

    ensure_table_column(
        connection,
        "instincts",
        "lifecycle_status",
        "ALTER TABLE instincts ADD COLUMN lifecycle_status TEXT NOT NULL DEFAULT 'active'",
    )?;
    ensure_table_column(
        connection,
        "instincts",
        "temperature",
        "ALTER TABLE instincts ADD COLUMN temperature TEXT NOT NULL DEFAULT 'warm'",
    )?;
    ensure_table_column(
        connection,
        "instincts",
        "updated_at",
        "ALTER TABLE instincts ADD COLUMN updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP",
    )?;

    ensure_table_column(
        connection,
        "coffee_chats",
        "temperature",
        "ALTER TABLE coffee_chats ADD COLUMN temperature TEXT NOT NULL DEFAULT 'warm'",
    )?;

    Ok(())
}

fn ensure_table_column(
    connection: &Connection,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<()> {
    if !table_exists(connection, table)? {
        return Ok(());
    }

    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|name| name == column) {
        connection.execute(alter_sql, [])?;
    }

    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(exists != 0)
}

#[cfg(test)]
fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|name| name == column))
}

fn fallback_app_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn forge_task_logs_round_trip() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0004_create_plugin_forge_task_logs",
                sql: include_str!("../../migrations/0004_create_plugin_forge_task_logs.sql"),
            },
        ]))?;

        let task_id =
            store.insert_forge_task("Echo", "echo", r#"["hello"]"#, None, None, "[]", "{}")?;
        store.append_forge_task_log(task_id, "stdout", "hello")?;
        store.append_forge_task_log(task_id, "stderr", "warn")?;

        let logs = store.list_forge_task_logs(task_id)?;

        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].stream, "stdout");
        assert_eq!(logs[0].line, "hello");
        assert_eq!(logs[1].stream, "stderr");
        assert_eq!(logs[1].line, "warn");

        Ok(())
    }

    #[test]
    fn forge_task_heartbeat_advances_while_running() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0004_create_plugin_forge_task_logs",
                sql: include_str!("../../migrations/0004_create_plugin_forge_task_logs.sql"),
            },
        ]))?;

        let task_id =
            store.insert_forge_task("Echo", "echo", r#"["hello"]"#, None, None, "[]", "{}")?;
        let pending = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        assert!(pending.heartbeat_at.is_none());

        store.update_forge_task_status(task_id, "Running", None, None)?;
        let running = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        let first_heartbeat = running
            .heartbeat_at
            .clone()
            .expect("running task should record an initial heartbeat");

        thread::sleep(Duration::from_millis(2));
        store.append_forge_task_log(task_id, "stdout", "hello")?;
        let after_log = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        assert_ne!(
            after_log.heartbeat_at.as_deref(),
            Some(first_heartbeat.as_str())
        );

        let last_heartbeat = after_log
            .heartbeat_at
            .clone()
            .expect("running task heartbeat should stay present");
        store.update_forge_task_status(task_id, "Done", Some(0), None)?;
        let done = store
            .get_forge_task(task_id)?
            .expect("task should remain queryable");
        assert_eq!(done.heartbeat_at.as_deref(), Some(last_heartbeat.as_str()));

        Ok(())
    }

    #[test]
    fn forge_dispatch_receipts_round_trip() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[
            MigrationStep {
                name: "0002_create_plugin_forge_tasks",
                sql: include_str!("../../migrations/0002_create_plugin_forge_tasks.sql"),
            },
            MigrationStep {
                name: "0006_create_plugin_forge_dispatch_receipts",
                sql: include_str!(
                    "../../migrations/0006_create_plugin_forge_dispatch_receipts.sql"
                ),
            },
        ]))?;

        let parent_task_id =
            store.insert_forge_task("Parent", "echo", r#"["hello"]"#, None, None, "[]", "{}")?;
        let (child_task_id, receipt) = store.insert_forge_task_with_dispatch_receipt(
            "Child",
            "echo",
            r#"["world"]"#,
            None,
            None,
            "[]",
            "{}",
            &NewForgeDispatchReceipt {
                parent_task_id,
                supervision_scope: "dispatch_pipeline",
                supervision_strategy: "one_for_one",
                child_dispatch_role: "agent",
                child_dispatch_tool_name: "forge_dispatch_agent",
                child_slot: Some("agent-1"),
            },
        )?;

        assert!(child_task_id > parent_task_id);
        assert_eq!(receipt.parent_task_id, parent_task_id);
        assert_eq!(receipt.child_task_id, child_task_id);
        assert_eq!(receipt.supervision_strategy, "one_for_one");

        let parent_receipt = store
            .get_forge_dispatch_parent_receipt(child_task_id)?
            .expect("child task should have a parent receipt");
        assert_eq!(parent_receipt.parent_task_id, parent_task_id);
        assert_eq!(
            parent_receipt.child_dispatch_tool_name,
            "forge_dispatch_agent"
        );

        let child_receipts = store.list_forge_dispatch_child_receipts(parent_task_id)?;
        assert_eq!(child_receipts.len(), 1);
        assert_eq!(child_receipts[0].child_task_id, child_task_id);
        assert_eq!(child_receipts[0].child_slot.as_deref(), Some("agent-1"));

        Ok(())
    }

    #[test]
    fn landing_tables_round_trip() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let run = store.create_source_ingest_run(NewSourceIngestRun {
            source_system: "linear",
            source_workspace: "microt",
            source_project: "Entrance",
            artifact_path: Some("A:/Agent/linear-snapshot.json"),
            artifact_sha256: Some("abc123"),
            status: "running",
        })?;

        let artifact = store.insert_source_artifact(NewSourceArtifact {
            ingest_run_id: run.id,
            artifact_kind: "snapshot",
            artifact_key: "linear:microt:Entrance:snapshot:test",
            title: Some("Entrance snapshot"),
            url: Some("https://linear.app/project/entrance"),
            payload_json: r#"{"issues":[]}"#,
        })?;

        let mirror = store.upsert_external_issue_mirror(UpsertExternalIssueMirror {
            ingest_run_id: run.id,
            mirror_key: "linear:microt:Entrance:issue:MYT-1",
            source_system: "linear",
            source_workspace: "microt",
            source_project: "Entrance",
            external_issue_id: "MYT-1",
            project_name: Some("Entrance"),
            team_name: Some("Pub"),
            parent_external_issue_id: None,
            title: "Bootstrap ownership",
            description: Some("first issue"),
            state: Some("Todo"),
            priority: Some("High"),
            url: Some("https://linear.app/microt/issue/MYT-1"),
            labels_json: r#"["Feature"]"#,
            relations_json: r#"{"blocks":[],"blockedBy":[],"relatedTo":[],"duplicateOf":null}"#,
            payload_json: r#"{"id":"MYT-1"}"#,
            git_branch_name: Some("kc2003/myt-1"),
            due_date: None,
            created_at: Some("2026-03-22T00:00:00.000Z"),
            updated_at: Some("2026-03-22T00:00:00.000Z"),
            completed_at: None,
            archived_at: None,
        })?;

        let planning_item = store.upsert_planning_item(UpsertPlanningItem {
            canonical_key: Some("linear:microt:Entrance:issue:MYT-1"),
            item_type: "issue",
            title: "Bootstrap ownership",
            description: Some("seeded from mirror"),
            status: "seeded",
            reconciliation_status: "unreconciled",
            source_system: Some("linear"),
            source_workspace: Some("microt"),
            source_project: Some("Entrance"),
            source_key: Some("MYT-1"),
            seeded_from_mirror_id: Some(mirror.id),
        })?;

        let link = store.ensure_planning_item_link(NewPlanningItemLink {
            planning_item_id: planning_item.id,
            link_type: "mirrors",
            target_planning_item_id: None,
            target_external_issue_mirror_id: Some(mirror.id),
            metadata_json: r#"{"seed":"external_issue_mirror"}"#,
        })?;

        let promotion = store.append_promotion_record(NewPromotionRecord {
            subject_kind: "planning_item",
            subject_id: planning_item.id,
            promotion_state: "storage_only",
            reason: Some("seeded on import"),
            source_ingest_run_id: Some(run.id),
        })?;

        let run = store.complete_source_ingest_run(
            run.id,
            SourceIngestRunCompletion {
                status: "completed",
                imported_issue_count: 1,
                imported_document_count: 0,
                imported_milestone_count: 0,
                imported_planning_item_count: 1,
                error_message: None,
            },
        )?;

        let runs = store.list_source_ingest_runs()?;
        let artifacts = store.list_source_artifacts(run.id)?;
        let mirrors = store.list_external_issue_mirrors()?;
        let items = store.list_planning_items()?;
        let unreconciled = store.list_unreconciled_planning_items()?;
        let links = store.list_planning_item_links()?;
        let promotions = store.list_promotion_records()?;

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(runs[0].imported_issue_count, 1);
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].id, artifact.id);
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].external_issue_id, "MYT-1");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, planning_item.id);
        assert_eq!(unreconciled.len(), 1);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, link.id);
        assert_eq!(promotions.len(), 1);
        assert_eq!(promotions[0].id, promotion.id);

        Ok(())
    }

    #[test]
    fn curated_memory_tables_materialize_remaining_recovery_families() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        store.upsert_decision_record(UpsertDecisionRecord {
            id: 1,
            title: "Single runtime db",
            statement: "Entrance should converge on one runtime db.",
            rationale: "Avoid split truth between repo root and app data.",
            decision_type: "storage",
            decision_status: "accepted",
            scope_type: "project",
            scope_ref: "Entrance",
            source_ref: "recovery_seed:decision:1",
            decided_by: "Human+NOTA",
            enforcement_level: "hard",
            actor_scope: "system",
            confidence: 0.95,
            created_at: "2026-03-23T00:00:00Z",
            updated_at: "2026-03-23T00:05:00Z",
        })?;
        store.upsert_vision_record(UpsertVisionRecord {
            id: 1,
            title: "NOTA control plane",
            statement: "Human should primarily interact through NOTA.",
            horizon: "long",
            vision_status: "active",
            scope_type: "system",
            scope_ref: "nota-control-plane",
            source_ref: "recovery_seed:vision:1",
            confidence: 0.92,
            created_at: "2026-03-23T00:00:00Z",
            updated_at: "2026-03-23T00:05:00Z",
        })?;
        store.upsert_memory_fragment_record(UpsertMemoryFragmentRecord {
            id: 1,
            title: "Delete directory safety",
            content: "Raw directory deletion is forbidden.",
            kind: "decision",
            source_type: "human-chat",
            source_ref: "chat:2026-03-21/raw-directory-delete-policy",
            source_hash: "hash",
            scope_type: "system",
            scope_ref: "filesystem",
            target_table: "decisions",
            target_ref: "1",
            status: "promoted",
            triage_status: "promoted",
            temperature: "hot",
            tags: "safety",
            notes: "Recovered and clarified.",
            confidence: 1.0,
            created_at: "2026-03-23T00:00:00Z",
            updated_at: "2026-03-23T00:05:00Z",
        })?;
        store.upsert_memory_link_record(UpsertMemoryLinkRecord {
            id: 1,
            src_kind: "decision",
            src_id: 1,
            dst_kind: "memory_fragments",
            dst_id: 1,
            relation_type: "derived_from",
            status: "active",
            created_at: "2026-03-23T00:05:00Z",
        })?;

        store.with_connection(|connection| {
            assert!(table_exists(connection, "decisions")?);
            assert!(table_exists(connection, "visions")?);
            assert!(table_exists(connection, "memory_fragments")?);
            assert!(table_exists(connection, "memory_links")?);

            assert!(table_has_column(
                connection,
                "decisions",
                "decision_status"
            )?);
            assert!(table_has_column(connection, "visions", "vision_status")?);
            assert!(table_has_column(
                connection,
                "memory_fragments",
                "target_table"
            )?);
            assert!(table_has_column(
                connection,
                "memory_links",
                "relation_type"
            )?);

            let decision_count =
                connection.query_row("SELECT COUNT(*) FROM decisions", [], |row| {
                    row.get::<_, i64>(0)
                })?;
            let vision_count = connection.query_row("SELECT COUNT(*) FROM visions", [], |row| {
                row.get::<_, i64>(0)
            })?;
            let fragment_count =
                connection.query_row("SELECT COUNT(*) FROM memory_fragments", [], |row| {
                    row.get::<_, i64>(0)
                })?;
            let link_count =
                connection.query_row("SELECT COUNT(*) FROM memory_links", [], |row| {
                    row.get::<_, i64>(0)
                })?;

            assert_eq!(decision_count, 1);
            assert_eq!(vision_count, 1);
            assert_eq!(fragment_count, 1);
            assert_eq!(link_count, 1);

            Ok(())
        })?;

        Ok(())
    }
}
