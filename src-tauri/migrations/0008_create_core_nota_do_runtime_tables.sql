CREATE TABLE IF NOT EXISTS nota_runtime_transactions (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_role              TEXT NOT NULL DEFAULT 'nota',
    surface_action          TEXT NOT NULL DEFAULT 'do',
    transaction_kind        TEXT NOT NULL,
    title                   TEXT NOT NULL,
    payload_json            TEXT NOT NULL DEFAULT '{}',
    status                  TEXT NOT NULL DEFAULT 'accepted',
    forge_task_id           INTEGER,
    cadence_checkpoint_id   INTEGER,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    FOREIGN KEY (forge_task_id) REFERENCES plugin_forge_tasks(id),
    FOREIGN KEY (cadence_checkpoint_id) REFERENCES cadence_objects(id)
);

CREATE INDEX IF NOT EXISTS idx_nota_runtime_transactions_surface
    ON nota_runtime_transactions(surface_action, id DESC);

CREATE TABLE IF NOT EXISTS nota_runtime_receipts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id  INTEGER NOT NULL,
    receipt_kind    TEXT NOT NULL,
    payload_json    TEXT NOT NULL DEFAULT '{}',
    status          TEXT NOT NULL DEFAULT 'recorded',
    created_at      TEXT NOT NULL,
    FOREIGN KEY (transaction_id) REFERENCES nota_runtime_transactions(id)
);

CREATE INDEX IF NOT EXISTS idx_nota_runtime_receipts_tx
    ON nota_runtime_receipts(transaction_id, id ASC);
