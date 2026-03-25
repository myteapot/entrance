CREATE TABLE IF NOT EXISTS anti_zeno_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    checkpoint_id       INTEGER,
    acceptance_bundle_id INTEGER,
    event_kind          TEXT NOT NULL,
    boundary_ref        TEXT NOT NULL DEFAULT '',
    budget_axis         TEXT NOT NULL DEFAULT 'semantic',
    event_weight        INTEGER NOT NULL DEFAULT 1,
    summary             TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_anti_zeno_events_checkpoint
    ON anti_zeno_events (checkpoint_id, budget_axis, id DESC);
