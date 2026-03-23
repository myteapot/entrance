CREATE TABLE IF NOT EXISTS plugin_forge_tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    command     TEXT NOT NULL,
    args        TEXT NOT NULL,          -- JSON string array
    working_dir TEXT,
    stdin_text  TEXT,
    required_tokens TEXT NOT NULL DEFAULT '[]', -- JSON string array of vault providers
    metadata    TEXT NOT NULL DEFAULT '{}', -- JSON metadata for structured workflows
    status      TEXT NOT NULL,          -- Pending, Running, Done, Failed, Cancelled, Blocked
    status_message TEXT,
    exit_code   INTEGER,
    created_at  TEXT NOT NULL,
    heartbeat_at TEXT,
    finished_at TEXT
);
