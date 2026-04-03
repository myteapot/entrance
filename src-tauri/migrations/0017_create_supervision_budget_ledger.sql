CREATE TABLE IF NOT EXISTS supervision_budget_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    allocation_id INTEGER NOT NULL,
    signal_family TEXT NOT NULL,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    action_taken TEXT NOT NULL,
    budget_max INTEGER NOT NULL,
    budget_remaining INTEGER NOT NULL,
    exhausted INTEGER NOT NULL DEFAULT 0,
    signal_summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (allocation_id) REFERENCES nota_runtime_allocations(id)
);

CREATE INDEX IF NOT EXISTS idx_budget_ledger_allocation
    ON supervision_budget_ledger(allocation_id);
