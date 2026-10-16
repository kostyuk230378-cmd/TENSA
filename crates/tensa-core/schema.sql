-- Схема notes.db: заметки и контакты (Контракт 3, ТЗ tensa-core).
-- Файл часть крейта tensa-core (include_str!); изменения — через ревью Архитектора.

-- Заметки
CREATE TABLE IF NOT EXISTS notes (
    id              TEXT PRIMARY KEY,           -- UUID
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    tags            TEXT NOT NULL,              -- JSON-массив
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    created_on_node TEXT NOT NULL,
    updated_on_node TEXT NOT NULL,
    deleted         INTEGER NOT NULL DEFAULT 0, -- soft delete
    vector_clock    TEXT NOT NULL               -- JSON: {node_id: counter}
);

CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);
CREATE INDEX IF NOT EXISTS idx_notes_deleted    ON notes(deleted);

-- Полнотекстовый индекс заметок (FTS5, unicode61)
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title, body,
    content='notes', content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS notes_fts_insert AFTER INSERT ON notes BEGIN INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body); END;

CREATE TRIGGER IF NOT EXISTS notes_fts_delete AFTER DELETE ON notes BEGIN INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body); END;

CREATE TRIGGER IF NOT EXISTS notes_fts_update AFTER UPDATE ON notes BEGIN INSERT INTO notes_fts(notes_fts, rowid, title, body) VALUES ('delete', old.rowid, old.title, old.body); INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body); END;

-- Контакты
CREATE TABLE IF NOT EXISTS contacts (
    id              TEXT PRIMARY KEY,           -- UUID
    name            TEXT NOT NULL,
    email           TEXT,
    phone           TEXT,
    telegram        TEXT,
    whatsapp        TEXT,
    notes           TEXT,
    tags            TEXT NOT NULL,              -- JSON-массив
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    created_on_node TEXT NOT NULL,
    updated_on_node TEXT NOT NULL,
    deleted         INTEGER NOT NULL DEFAULT 0, -- soft delete
    vector_clock    TEXT NOT NULL               -- JSON: {node_id: counter}
);

CREATE INDEX IF NOT EXISTS idx_contacts_email      ON contacts(email);
CREATE INDEX IF NOT EXISTS idx_contacts_updated_at ON contacts(updated_at);
CREATE INDEX IF NOT EXISTS idx_contacts_deleted    ON contacts(deleted);

-- Полнотекстовый индекс контактов (FTS5, unicode61)
CREATE VIRTUAL TABLE IF NOT EXISTS contacts_fts USING fts5(
    name, notes,
    content='contacts', content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS contacts_fts_insert AFTER INSERT ON contacts BEGIN INSERT INTO contacts_fts(rowid, name, notes) VALUES (new.rowid, new.name, new.notes); END;

CREATE TRIGGER IF NOT EXISTS contacts_fts_delete AFTER DELETE ON contacts BEGIN INSERT INTO contacts_fts(contacts_fts, rowid, name, notes) VALUES ('delete', old.rowid, old.name, old.notes); END;

CREATE TRIGGER IF NOT EXISTS contacts_fts_update AFTER UPDATE ON contacts BEGIN INSERT INTO contacts_fts(contacts_fts, rowid, name, notes) VALUES ('delete', old.rowid, old.name, old.notes); INSERT INTO contacts_fts(rowid, name, notes) VALUES (new.rowid, new.name, new.notes); END;

-- Состояние синхронизации (Контракт 3)
CREATE TABLE IF NOT EXISTS sync_state (
    node_id       TEXT PRIMARY KEY,
    last_event_id INTEGER NOT NULL,
    last_sync_at  INTEGER NOT NULL
);
