-- Схема nodes.db (Контракт 7: реестр узлов).
-- Файл часть крейта tensa-nodes (include_str!); изменения — через ревью Архитектора.

-- Реестр узлов
CREATE TABLE IF NOT EXISTS nodes (
    node_id            TEXT PRIMARY KEY,        -- UUID
    name               TEXT NOT NULL,
    device_type        TEXT NOT NULL,           -- desktop | android
    platform           TEXT NOT NULL,           -- windows | linux | android
    role               TEXT NOT NULL,           -- heavy | thin | auto
    tailscale_ip       TEXT NOT NULL DEFAULT '',
    grpc_port          INTEGER NOT NULL DEFAULT 0,
    last_seen          INTEGER NOT NULL,
    online             INTEGER NOT NULL DEFAULT 1,
    ram_total_mb       INTEGER NOT NULL DEFAULT 0,
    ram_available_mb   INTEGER NOT NULL DEFAULT 0,
    cpu_cores          INTEGER NOT NULL DEFAULT 0,
    cpu_usage_percent  REAL NOT NULL DEFAULT 0,
    has_gpu            INTEGER NOT NULL DEFAULT 0,
    has_npu            INTEGER NOT NULL DEFAULT 0,
    battery_percent    INTEGER,
    battery_charging   INTEGER,
    thermal_status     TEXT NOT NULL DEFAULT 'nominal',
    resources          TEXT NOT NULL,           -- JSON: NodeResources
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nodes_online    ON nodes(online);
CREATE INDEX IF NOT EXISTS idx_nodes_last_seen ON nodes(last_seen);
CREATE INDEX IF NOT EXISTS idx_nodes_role      ON nodes(role);

-- История heartbeat (для деградации/аналитики)
CREATE TABLE IF NOT EXISTS node_heartbeats (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id            TEXT NOT NULL,
    ram_available_mb   INTEGER NOT NULL,
    cpu_usage_percent  REAL NOT NULL,
    battery_percent    INTEGER,
    battery_charging   INTEGER,
    thermal_status     TEXT NOT NULL,
    timestamp          INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_heartbeats_node ON node_heartbeats(node_id, timestamp);

-- Активные соединения (ADR-015: TTL 15 минут)
CREATE TABLE IF NOT EXISTS node_connections (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id      TEXT NOT NULL,
    resource     TEXT NOT NULL,
    account_id   TEXT,
    active       INTEGER NOT NULL DEFAULT 1,
    acquired_at  INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_connections_resource ON node_connections(resource, account_id, active);
CREATE INDEX IF NOT EXISTS idx_connections_node     ON node_connections(node_id, active);
