CREATE TABLE IF NOT EXISTS runtime_hosts (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    host_key            TEXT NOT NULL UNIQUE,
    os_family           TEXT NOT NULL,
    host_label          TEXT NOT NULL,
    kernel_label        TEXT NOT NULL DEFAULT '',
    user_home           TEXT NOT NULL,
    owner_root          TEXT NOT NULL,
    config_path         TEXT NOT NULL,
    runtime_db_path     TEXT NOT NULL,
    exports_path        TEXT NOT NULL,
    worktrees_root      TEXT NOT NULL,
    wsl_distro_name     TEXT,
    path_style          TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'active',
    created_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS owned_worktrees (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    host_key        TEXT NOT NULL,
    project_name    TEXT NOT NULL DEFAULT '',
    issue_id        TEXT,
    branch_name     TEXT NOT NULL DEFAULT '',
    worktree_kind   TEXT NOT NULL,
    worktree_path   TEXT NOT NULL UNIQUE,
    repo_root       TEXT,
    slot_name       TEXT,
    status          TEXT NOT NULL DEFAULT 'observed',
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (host_key) REFERENCES runtime_hosts(host_key)
);

CREATE INDEX IF NOT EXISTS idx_runtime_hosts_host_key
    ON runtime_hosts (host_key);

CREATE INDEX IF NOT EXISTS idx_owned_worktrees_host_key
    ON owned_worktrees (host_key, status, project_name, branch_name);
