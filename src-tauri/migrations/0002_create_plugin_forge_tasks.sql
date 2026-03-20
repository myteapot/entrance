CREATE TABLE plugin_forge_tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    command     TEXT NOT NULL,
    args        TEXT NOT NULL,          -- JSON string array
    status      TEXT NOT NULL,          -- Pending, Running, Done, Failed, Cancelled
    exit_code   INTEGER,
    created_at  TEXT NOT NULL,
    finished_at TEXT
);
