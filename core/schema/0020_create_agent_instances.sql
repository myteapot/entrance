CREATE TABLE IF NOT EXISTS agent_instances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    role TEXT NOT NULL CHECK(role IN ('nota', 'arch', 'dev', 'agent')),
    parent_instance_id INTEGER REFERENCES agent_instances(id),
    agent_tier TEXT NOT NULL DEFAULT 'ArchNota',
    status TEXT NOT NULL DEFAULT 'Idle' CHECK(status IN ('Idle', 'Busy', 'Stale', 'Stopped')),
    display_name TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    workspace_path TEXT,
    last_heartbeat_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_instances_status ON agent_instances(status);
CREATE INDEX IF NOT EXISTS idx_agent_instances_parent ON agent_instances(parent_instance_id);
CREATE INDEX IF NOT EXISTS idx_agent_instances_role ON agent_instances(role);
