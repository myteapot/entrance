CREATE TABLE IF NOT EXISTS documents (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    slug        TEXT NOT NULL,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    category    TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_slug_category
    ON documents(slug, category);

CREATE TABLE IF NOT EXISTS todos (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    title               TEXT NOT NULL,
    status              TEXT NOT NULL,
    priority            INTEGER NOT NULL,
    project             TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    done_at             TEXT,
    temperature         TEXT NOT NULL,
    due_on              TEXT NOT NULL,
    remind_every_days   INTEGER NOT NULL,
    remind_next_on      TEXT NOT NULL,
    last_reminded_at    TEXT NOT NULL,
    reminder_status     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS instincts (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern             TEXT NOT NULL,
    action              TEXT NOT NULL,
    confidence          REAL NOT NULL,
    source              TEXT NOT NULL,
    ref                 TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    status              TEXT NOT NULL,
    surfaced_to         TEXT NOT NULL,
    review_status       TEXT NOT NULL,
    origin_type         TEXT NOT NULL,
    lifecycle_status    TEXT NOT NULL,
    temperature         TEXT NOT NULL,
    updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS coffee_chats (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project     TEXT NOT NULL,
    stage       TEXT NOT NULL,
    retro       TEXT NOT NULL,
    forward     TEXT NOT NULL,
    priorities  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    temperature TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS decisions (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    title               TEXT NOT NULL,
    statement           TEXT NOT NULL,
    rationale           TEXT NOT NULL,
    decision_type       TEXT NOT NULL,
    decision_status     TEXT NOT NULL,
    scope_type          TEXT NOT NULL,
    scope_ref           TEXT NOT NULL,
    source_ref          TEXT NOT NULL,
    decided_by          TEXT NOT NULL,
    enforcement_level   TEXT NOT NULL,
    actor_scope         TEXT NOT NULL,
    confidence          REAL NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS visions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    title           TEXT NOT NULL,
    statement       TEXT NOT NULL,
    horizon         TEXT NOT NULL,
    vision_status   TEXT NOT NULL,
    scope_type      TEXT NOT NULL,
    scope_ref       TEXT NOT NULL,
    source_ref      TEXT NOT NULL,
    confidence      REAL NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS memory_fragments (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    title           TEXT NOT NULL,
    content         TEXT NOT NULL,
    kind            TEXT NOT NULL,
    source_type     TEXT NOT NULL,
    source_ref      TEXT NOT NULL,
    source_hash     TEXT NOT NULL,
    scope_type      TEXT NOT NULL,
    scope_ref       TEXT NOT NULL,
    target_table    TEXT NOT NULL,
    target_ref      TEXT NOT NULL,
    status          TEXT NOT NULL,
    triage_status   TEXT NOT NULL,
    temperature     TEXT NOT NULL,
    tags            TEXT NOT NULL,
    notes           TEXT NOT NULL,
    confidence      REAL NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS memory_links (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    src_kind        TEXT NOT NULL,
    src_id          INTEGER NOT NULL,
    dst_kind        TEXT NOT NULL,
    dst_id          INTEGER NOT NULL,
    relation_type   TEXT NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
