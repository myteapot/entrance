CREATE TABLE IF NOT EXISTS source_ingest_runs (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_system               TEXT NOT NULL,
    source_workspace            TEXT NOT NULL DEFAULT '',
    source_project              TEXT NOT NULL DEFAULT '',
    artifact_path               TEXT,
    artifact_sha256             TEXT,
    status                      TEXT NOT NULL,
    imported_issue_count        INTEGER NOT NULL DEFAULT 0,
    imported_document_count     INTEGER NOT NULL DEFAULT 0,
    imported_milestone_count    INTEGER NOT NULL DEFAULT 0,
    imported_planning_item_count INTEGER NOT NULL DEFAULT 0,
    error_message               TEXT,
    created_at                  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at                TEXT
);

CREATE INDEX IF NOT EXISTS idx_source_ingest_runs_source
    ON source_ingest_runs(source_system, source_workspace, source_project, created_at DESC);

CREATE TABLE IF NOT EXISTS source_artifacts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    ingest_run_id   INTEGER NOT NULL,
    artifact_kind   TEXT NOT NULL,
    artifact_key    TEXT NOT NULL,
    title           TEXT,
    url             TEXT,
    payload_json    TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (ingest_run_id) REFERENCES source_ingest_runs(id) ON DELETE CASCADE,
    UNIQUE(ingest_run_id, artifact_kind, artifact_key)
);

CREATE INDEX IF NOT EXISTS idx_source_artifacts_run_kind
    ON source_artifacts(ingest_run_id, artifact_kind);

CREATE TABLE IF NOT EXISTS external_issue_mirrors (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    mirror_key                  TEXT NOT NULL UNIQUE,
    source_system               TEXT NOT NULL,
    source_workspace            TEXT NOT NULL DEFAULT '',
    source_project              TEXT NOT NULL DEFAULT '',
    external_issue_id           TEXT NOT NULL,
    project_name                TEXT,
    team_name                   TEXT,
    parent_external_issue_id    TEXT,
    title                       TEXT NOT NULL,
    description                 TEXT,
    state                       TEXT,
    priority                    TEXT,
    url                         TEXT,
    labels_json                 TEXT NOT NULL DEFAULT '[]',
    relations_json              TEXT NOT NULL DEFAULT '{}',
    payload_json                TEXT NOT NULL,
    git_branch_name             TEXT,
    due_date                    TEXT,
    created_at                  TEXT,
    updated_at                  TEXT,
    completed_at                TEXT,
    archived_at                 TEXT,
    first_seen_at               TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at                TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_ingest_run_id          INTEGER NOT NULL,
    FOREIGN KEY (last_ingest_run_id) REFERENCES source_ingest_runs(id)
);

CREATE INDEX IF NOT EXISTS idx_external_issue_mirrors_source_issue
    ON external_issue_mirrors(source_system, source_workspace, source_project, external_issue_id);

CREATE INDEX IF NOT EXISTS idx_external_issue_mirrors_run
    ON external_issue_mirrors(last_ingest_run_id, last_seen_at DESC);

CREATE TABLE IF NOT EXISTS planning_items (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_key           TEXT UNIQUE,
    item_type               TEXT NOT NULL,
    title                   TEXT NOT NULL,
    description             TEXT,
    status                  TEXT NOT NULL DEFAULT 'seeded',
    reconciliation_status   TEXT NOT NULL DEFAULT 'unreconciled',
    source_system           TEXT,
    source_workspace        TEXT,
    source_project          TEXT,
    source_key              TEXT,
    seeded_from_mirror_id   INTEGER,
    created_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (seeded_from_mirror_id) REFERENCES external_issue_mirrors(id)
);

CREATE INDEX IF NOT EXISTS idx_planning_items_kind_reconciliation
    ON planning_items(item_type, reconciliation_status, status);

CREATE INDEX IF NOT EXISTS idx_planning_items_seeded_from_mirror
    ON planning_items(seeded_from_mirror_id);

CREATE TABLE IF NOT EXISTS planning_item_links (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    planning_item_id            INTEGER NOT NULL,
    link_type                   TEXT NOT NULL,
    target_planning_item_id     INTEGER,
    target_external_issue_mirror_id INTEGER,
    metadata_json               TEXT NOT NULL DEFAULT '{}',
    created_at                  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (planning_item_id) REFERENCES planning_items(id) ON DELETE CASCADE,
    FOREIGN KEY (target_planning_item_id) REFERENCES planning_items(id) ON DELETE CASCADE,
    FOREIGN KEY (target_external_issue_mirror_id) REFERENCES external_issue_mirrors(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_planning_item_links_source
    ON planning_item_links(planning_item_id, link_type);

CREATE TABLE IF NOT EXISTS promotion_records (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_kind        TEXT NOT NULL,
    subject_id          INTEGER NOT NULL,
    promotion_state     TEXT NOT NULL,
    reason              TEXT,
    source_ingest_run_id INTEGER,
    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_ingest_run_id) REFERENCES source_ingest_runs(id)
);

CREATE INDEX IF NOT EXISTS idx_promotion_records_subject
    ON promotion_records(subject_kind, subject_id, id DESC);
