CREATE TABLE IF NOT EXISTS plugin_forge_task_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id     INTEGER NOT NULL,
    stream      TEXT NOT NULL,
    line        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES plugin_forge_tasks (id) ON DELETE CASCADE
);
