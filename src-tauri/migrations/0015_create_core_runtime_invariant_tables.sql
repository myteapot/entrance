CREATE TABLE IF NOT EXISTS runtime_invariants (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    invariant_key        TEXT NOT NULL UNIQUE,
    title                TEXT NOT NULL,
    status               TEXT NOT NULL,
    severity             TEXT NOT NULL DEFAULT 'info',
    checkpoint_id        INTEGER,
    acceptance_bundle_id INTEGER,
    human_round_id       INTEGER,
    summary              TEXT NOT NULL,
    evidence_json        TEXT NOT NULL DEFAULT '{}',
    repair_action        TEXT NOT NULL DEFAULT '',
    created_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_runtime_invariants_status
    ON runtime_invariants (status, severity, invariant_key);

CREATE TABLE IF NOT EXISTS repair_lane_items (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    repair_key           TEXT NOT NULL UNIQUE,
    source_invariant_key TEXT,
    checkpoint_id        INTEGER,
    acceptance_bundle_id INTEGER,
    item_kind            TEXT NOT NULL,
    urgency              TEXT NOT NULL DEFAULT 'repairable',
    status               TEXT NOT NULL DEFAULT 'open',
    summary              TEXT NOT NULL,
    repair_action        TEXT NOT NULL DEFAULT '',
    evidence_json        TEXT NOT NULL DEFAULT '{}',
    created_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at          TEXT
);

CREATE INDEX IF NOT EXISTS idx_repair_lane_items_status
    ON repair_lane_items (status, urgency, repair_key);
