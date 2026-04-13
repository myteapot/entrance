CREATE TABLE IF NOT EXISTS nota_runtime_allocations (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    allocator_role          TEXT NOT NULL DEFAULT 'nota',
    allocator_surface       TEXT NOT NULL DEFAULT 'nota_do',
    allocation_kind         TEXT NOT NULL DEFAULT 'forge_agent_dispatch',
    source_transaction_id   INTEGER NOT NULL,
    lineage_ref             TEXT NOT NULL,
    child_execution_kind    TEXT NOT NULL DEFAULT 'forge_task',
    child_execution_ref     TEXT NOT NULL,
    return_target_kind      TEXT NOT NULL,
    return_target_ref       TEXT NOT NULL,
    escalation_target_kind  TEXT NOT NULL,
    escalation_target_ref   TEXT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'task_created',
    payload_json            TEXT NOT NULL DEFAULT '{}',
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL,
    FOREIGN KEY (source_transaction_id) REFERENCES nota_runtime_transactions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_nota_runtime_allocations_source_tx
    ON nota_runtime_allocations(source_transaction_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_nota_runtime_allocations_lineage
    ON nota_runtime_allocations(lineage_ref);

CREATE INDEX IF NOT EXISTS idx_nota_runtime_allocations_status
    ON nota_runtime_allocations(status, id DESC);
