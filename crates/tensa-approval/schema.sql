-- Схема ApprovalStore (Контракт 1 из 02-contracts.md, ТЗ tensa-approval).
-- Файл часть крейта tensa-approval; изменения — через ревью Архитектора.

-- Очередь действий
CREATE TABLE IF NOT EXISTS actions (
    id               TEXT PRIMARY KEY,          -- UUID
    kind             TEXT NOT NULL,             -- email_send | telegram_send | whatsapp_send | calendar_create | app_launch | file_open
    payload          TEXT NOT NULL,             -- JSON
    status           TEXT NOT NULL,             -- pending | approved | rejected | executed | failed | expired
    created_at       INTEGER NOT NULL,          -- Unix timestamp
    created_by       TEXT NOT NULL,             -- llm | user | telegram | email | system
    created_on_node  TEXT NOT NULL,             -- node_id
    expires_at       INTEGER NOT NULL,          -- Unix timestamp (TTL 24 часа)
    approved_at      INTEGER,
    approved_by      TEXT,                      -- user | telegram | mobile | desktop
    approved_on_node TEXT,
    executed_at      INTEGER,
    executed_on_node TEXT,
    result           TEXT,                      -- JSON
    operation_id     TEXT NOT NULL,             -- UUID, идемпотентность
    vector_clock     TEXT NOT NULL              -- JSON: {node_id: counter}
);

CREATE INDEX IF NOT EXISTS idx_actions_status       ON actions(status);
CREATE INDEX IF NOT EXISTS idx_actions_expires_at   ON actions(expires_at);
CREATE INDEX IF NOT EXISTS idx_actions_operation_id ON actions(operation_id);

-- Журнал событий (event sourcing)
CREATE TABLE IF NOT EXISTS action_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    action_id    TEXT NOT NULL,                 -- UUID действия
    event_type   TEXT NOT NULL,                 -- created | approved | rejected | executed | failed | expired
    event_data   TEXT NOT NULL,                 -- JSON: снимок действия + метаданные изменения
    timestamp    INTEGER NOT NULL,              -- Unix timestamp
    node_id      TEXT NOT NULL,
    vector_clock TEXT NOT NULL                  -- JSON: {node_id: counter}
);

CREATE INDEX IF NOT EXISTS idx_action_events_action_id ON action_events(action_id);

-- Состояние синхронизации
CREATE TABLE IF NOT EXISTS sync_state (
    node_id       TEXT PRIMARY KEY,
    last_event_id INTEGER NOT NULL,
    last_sync_at  INTEGER NOT NULL
);
