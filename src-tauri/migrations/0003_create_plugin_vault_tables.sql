CREATE TABLE IF NOT EXISTS plugin_vault_tokens (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    provider        TEXT NOT NULL,
    encrypted_value TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_plugin_vault_tokens_provider
    ON plugin_vault_tokens(provider, name);

CREATE TABLE IF NOT EXISTS plugin_vault_mcp_configs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    transport   TEXT NOT NULL,
    endpoint    TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
