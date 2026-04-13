CREATE TABLE IF NOT EXISTS projection_targets (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    projection_class    TEXT NOT NULL,
    target_key          TEXT NOT NULL,
    title               TEXT NOT NULL,
    target_path         TEXT NOT NULL,
    source_scope        TEXT NOT NULL DEFAULT 'runtime:Entrance',
    repair_action       TEXT NOT NULL DEFAULT '',
    projection_policy   TEXT NOT NULL DEFAULT 'required',
    is_required         INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (projection_class, target_key)
);

CREATE TABLE IF NOT EXISTS projection_runs (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    target_id                   INTEGER NOT NULL,
    truth_checkpoint_id         INTEGER,
    truth_human_round_id        INTEGER,
    truth_acceptance_bundle_id  INTEGER,
    run_state                   TEXT NOT NULL,
    freshness_state             TEXT NOT NULL,
    trigger_kind                TEXT NOT NULL,
    summary                     TEXT NOT NULL,
    error_message               TEXT,
    repair_hint                 TEXT,
    started_at                  TEXT NOT NULL,
    completed_at                TEXT NOT NULL,
    FOREIGN KEY (target_id) REFERENCES projection_targets(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_projection_targets_class_key
    ON projection_targets (projection_class, target_key);

CREATE INDEX IF NOT EXISTS idx_projection_runs_target_id
    ON projection_runs (target_id, id DESC);
