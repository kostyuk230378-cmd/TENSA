-- Down-миграция для schema.sql (notes.db: заметки и контакты, Контракт 3).
-- Дополнение по 05-rollback.md: каждая схема должна иметь откат.
-- Код крейта применяет только schema.sql (include_str!); этот файл —
-- ручная процедура отката, в автоматике не используется.
--
-- Применение (только после бэкапа файла БД, см. 05-rollback.md):
--   sqlite3 notes.db.bak < schema_down.sql
--
-- Порядок: сначала FTS-таблицы (внешний контент), затем обычные.

DROP TABLE IF EXISTS sync_state;
DROP TABLE IF EXISTS contacts_fts;
DROP TABLE IF EXISTS contacts;
DROP TABLE IF EXISTS notes_fts;
DROP TABLE IF EXISTS notes;
