CREATE TABLE IF NOT EXISTS plugin_forge_dispatch_receipts (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_task_id          INTEGER NOT NULL,
    child_task_id           INTEGER NOT NULL UNIQUE,
    supervision_scope       TEXT NOT NULL,
    supervision_strategy    TEXT NOT NULL,
    child_dispatch_role     TEXT NOT NULL,
    child_dispatch_tool_name TEXT NOT NULL,
    child_slot              TEXT,
    created_at              TEXT NOT NULL,
    FOREIGN KEY (parent_task_id) REFERENCES plugin_forge_tasks (id) ON DELETE CASCADE,
    FOREIGN KEY (child_task_id) REFERENCES plugin_forge_tasks (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forge_dispatch_receipts_parent
    ON plugin_forge_dispatch_receipts (parent_task_id, id);

CREATE INDEX IF NOT EXISTS idx_forge_dispatch_receipts_child
    ON plugin_forge_dispatch_receipts (child_task_id);
