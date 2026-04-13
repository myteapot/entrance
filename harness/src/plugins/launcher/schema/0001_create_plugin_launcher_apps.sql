CREATE TABLE IF NOT EXISTS plugin_launcher_apps (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    path            TEXT NOT NULL UNIQUE,
    arguments       TEXT,
    working_dir     TEXT,
    icon_path       TEXT,
    source          TEXT NOT NULL,
    launch_count    INTEGER NOT NULL DEFAULT 0,
    last_used       TEXT,
    pinned          INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_plugin_launcher_apps_search
    ON plugin_launcher_apps (normalized_name, pinned, launch_count);
