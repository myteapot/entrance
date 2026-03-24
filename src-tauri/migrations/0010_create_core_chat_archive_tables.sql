CREATE TABLE IF NOT EXISTS chat_archive_settings (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    scope_type      TEXT NOT NULL,
    scope_ref       TEXT NOT NULL,
    archive_policy  TEXT NOT NULL DEFAULT 'off',
    updated_at      TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_chat_archive_settings_scope
    ON chat_archive_settings(scope_type, scope_ref);

CREATE TABLE IF NOT EXISTS chat_capture_records (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    session_ref         TEXT NOT NULL DEFAULT '',
    role                TEXT NOT NULL,
    capture_mode        TEXT NOT NULL,
    archive_policy      TEXT NOT NULL,
    content             TEXT NOT NULL DEFAULT '',
    summary             TEXT NOT NULL DEFAULT '',
    scope_type          TEXT NOT NULL DEFAULT '',
    scope_ref           TEXT NOT NULL DEFAULT '',
    linked_decision_id  INTEGER,
    status              TEXT NOT NULL DEFAULT 'captured',
    created_at          TEXT NOT NULL,
    FOREIGN KEY (linked_decision_id) REFERENCES decisions(id)
);

CREATE INDEX IF NOT EXISTS idx_chat_capture_records_created
    ON chat_capture_records(created_at DESC, id DESC);
