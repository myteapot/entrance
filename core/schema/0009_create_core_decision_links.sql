CREATE TABLE IF NOT EXISTS decision_links (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    src_decision_id     INTEGER NOT NULL,
    dst_decision_id     INTEGER NOT NULL,
    relation_type       TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'active',
    created_at          TEXT NOT NULL,
    FOREIGN KEY (src_decision_id) REFERENCES decisions(id),
    FOREIGN KEY (dst_decision_id) REFERENCES decisions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_decision_links_unique_relation
    ON decision_links(src_decision_id, dst_decision_id, relation_type);
