CREATE TABLE plugin_forge_tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    command     TEXT NOT NULL,
    args        TEXT NOT NULL,          -- JSON string array
    required_tokens TEXT NOT NULL DEFAULT '[]', -- JSON string array of vault providers
    status      TEXT NOT NULL,          -- Pending, Running, Done, Failed, Cancelled, Blocked
    status_message TEXT,
    exit_code   INTEGER,
    created_at  TEXT NOT NULL,
    finished_at TEXT
);
