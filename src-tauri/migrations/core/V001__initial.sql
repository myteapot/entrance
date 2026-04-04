CREATE TABLE core_plugins (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL UNIQUE,
    version         TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    permission      TEXT NOT NULL,
    integration     TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE core_hotkeys (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    action          TEXT NOT NULL UNIQUE,
    shortcut        TEXT NOT NULL,
    scope           TEXT NOT NULL DEFAULT 'global',
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE core_event_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    plugin_name     TEXT,
    event_type      TEXT NOT NULL,
    payload         TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (plugin_name) REFERENCES core_plugins(name)
);
