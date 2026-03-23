CREATE TABLE IF NOT EXISTS cadence_objects (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    cadence_kind        TEXT NOT NULL,
    title               TEXT NOT NULL,
    summary             TEXT NOT NULL DEFAULT '',
    payload_json        TEXT NOT NULL DEFAULT '{}',
    scope_type          TEXT NOT NULL DEFAULT '',
    scope_ref           TEXT NOT NULL DEFAULT '',
    source_type         TEXT NOT NULL DEFAULT '',
    source_ref          TEXT NOT NULL DEFAULT '',
    admission_policy    TEXT NOT NULL DEFAULT 'AP_STORAGE_AND_COLD_ALWAYS',
    projection_policy   TEXT NOT NULL DEFAULT 'PP_HOT_ACTIVE_ONLY',
    status              TEXT NOT NULL DEFAULT 'active',
    is_current          INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cadence_objects_kind_current
    ON cadence_objects(cadence_kind, is_current, id DESC);

CREATE TABLE IF NOT EXISTS cadence_links (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    src_cadence_object_id   INTEGER NOT NULL,
    dst_cadence_object_id   INTEGER NOT NULL,
    relation_type           TEXT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'active',
    created_at              TEXT NOT NULL,
    FOREIGN KEY (src_cadence_object_id) REFERENCES cadence_objects(id),
    FOREIGN KEY (dst_cadence_object_id) REFERENCES cadence_objects(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_cadence_links_unique_relation
    ON cadence_links(src_cadence_object_id, dst_cadence_object_id, relation_type);
