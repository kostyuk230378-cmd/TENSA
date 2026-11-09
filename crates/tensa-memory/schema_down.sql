-- Down-миграция для schema.sql (memory.db: core + recall memory, Контракт 13).
-- Archival memory живёт в Lance и здесь не откатывается (см. 05-rollback.md:
-- откат Lance — через версию данных).
-- Код крейта применяет только schema.sql (include_str!); этот файл —
-- ручная процедура отката, в автоматике не используется.
--
-- Применение (только после бэкапа файла БД, см. 05-rollback.md):
--   sqlite3 memory.db.bak < schema_down.sql
--
-- Порядок: сначала FTS-таблица (внешний контент), затем обычные.

DROP TABLE IF EXISTS recall_fts;
DROP TABLE IF EXISTS recall_messages;
DROP TABLE IF EXISTS core_memory_config;
DROP TABLE IF EXISTS core_memory_history;
DROP TABLE IF EXISTS core_memory_blocks;
