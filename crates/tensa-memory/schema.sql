-- Схема memory.db: core memory + recall memory (Контракт 13, dop_tz).
-- Archival memory — Lance (фича lance), не здесь.
-- Файл часть крейта tensa-memory (include_str!); изменения — через ревью Архитектора.

-- === Core Memory (Контракт 13) ===

-- Блоки core memory (всегда в контексте LLM)
CREATE TABLE IF NOT EXISTS core_memory_blocks (
    id              TEXT PRIMARY KEY,           -- UUID блока
    block           TEXT NOT NULL UNIQUE,       -- persona | human | tasks | preferences
    value           TEXT NOT NULL,
    size_chars      INTEGER NOT NULL,
    size_tokens     INTEGER NOT NULL,           -- оценка
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    updated_by      TEXT NOT NULL,              -- llm | user | system
    updated_on_node TEXT NOT NULL,
    vector_clock    TEXT NOT NULL               -- JSON: {node_id: counter}
);

CREATE INDEX IF NOT EXISTS idx_core_memory_block ON core_memory_blocks(block);

-- История изменений блоков
CREATE TABLE IF NOT EXISTS core_memory_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    block           TEXT NOT NULL,
    old_value       TEXT,
    new_value       TEXT NOT NULL,
    changed_at      INTEGER NOT NULL,
    changed_by      TEXT NOT NULL,
    changed_on_node TEXT NOT NULL,
    operation       TEXT NOT NULL               -- replace | append
);

CREATE INDEX IF NOT EXISTS idx_core_memory_history_block      ON core_memory_history(block);
CREATE INDEX IF NOT EXISTS idx_core_memory_history_changed_at ON core_memory_history(changed_at);

-- Конфигурация core memory
CREATE TABLE IF NOT EXISTS core_memory_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL                        -- JSON
);

-- === Recall Memory (dop_tz: SQLite + FTS5) ===

-- История разговоров
CREATE TABLE IF NOT EXISTS recall_messages (
    id        TEXT PRIMARY KEY,                 -- UUID сообщения
    role      TEXT NOT NULL,                    -- user | assistant | system
    content   TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    session_id TEXT NOT NULL,                   -- UUID сессии
    node_id   TEXT NOT NULL,
    metadata  TEXT NOT NULL                     -- JSON
);

CREATE INDEX IF NOT EXISTS idx_recall_session  ON recall_messages(session_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_recall_time     ON recall_messages(timestamp);

-- Полнотекстовый индекс истории
CREATE VIRTUAL TABLE IF NOT EXISTS recall_fts USING fts5(
    content, content='recall_messages', content_rowid='rowid', tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS recall_fts_insert AFTER INSERT ON recall_messages BEGIN INSERT INTO recall_fts(rowid, content) VALUES (new.rowid, new.content); END;

CREATE TRIGGER IF NOT EXISTS recall_fts_delete AFTER DELETE ON recall_messages BEGIN INSERT INTO recall_fts(recall_fts, rowid, content) VALUES ('delete', old.rowid, old.content); END;

CREATE TRIGGER IF NOT EXISTS recall_fts_update AFTER UPDATE ON recall_messages BEGIN INSERT INTO recall_fts(recall_fts, rowid, content) VALUES ('delete', old.rowid, old.content); INSERT INTO recall_fts(rowid, content) VALUES (new.rowid, new.content); END;
