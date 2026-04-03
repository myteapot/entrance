CREATE TABLE IF NOT EXISTS compiler_registry_snapshot (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    primitive TEXT NOT NULL,
    object_kind TEXT NOT NULL,
    flow_phase TEXT NOT NULL,
    attention_state TEXT NOT NULL,
    integrity_overlay TEXT,
    control_policy TEXT NOT NULL,
    writer_policy TEXT NOT NULL,
    route_policy TEXT NOT NULL,
    gate_policy TEXT NOT NULL,
    sandbox_policy TEXT NOT NULL,
    effect_kind TEXT NOT NULL,
    supervision_scope TEXT,
    allowed_roles TEXT NOT NULL,
    allowed_rooms TEXT NOT NULL,
    snapshot_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(primitive)
);
