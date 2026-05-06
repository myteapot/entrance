# Entrance — Data Schema

> 同 v1, 新增 connector 表。详见 oracle.md SSOT 原则。

## 核心原则

entrance.db 是 **Single Source of Truth**。外部服务 (OpenClaw 等) 的数据也存在这里。

## 表结构

同 v1: core_plugins, core_hotkeys, core_event_log, plugin_launcher_apps, plugin_forge_tasks, plugin_vault_tokens, plugin_vault_mcp_servers, plugin_vault_agent_skills

### 新增 (S3): Connector

```sql
-- 已注册的外部服务
CREATE TABLE plugin_connector_services (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,   -- "openclaw", "obsidian"
    adapter     TEXT NOT NULL,          -- adapter 类型
    config      TEXT NOT NULL,          -- JSON: 连接配置
    status      TEXT NOT NULL DEFAULT 'disconnected',
    enabled     INTEGER NOT NULL DEFAULT 1,
    last_sync   TEXT,
    created_at  TEXT NOT NULL
);

-- 消息/数据镜像 (从外部服务回流)
CREATE TABLE plugin_connector_data (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id  INTEGER NOT NULL REFERENCES plugin_connector_services(id),
    data_type   TEXT NOT NULL,          -- "message", "note", "event"
    content     TEXT NOT NULL,          -- JSON
    source_id   TEXT,                   -- 外部系统的原始 ID
    synced_at   TEXT NOT NULL
);
```

## 加密 & Migration

同 v1 (AES-256-GCM, refinery crate)。
