-- Down-миграция для schema.sql (ApprovalStore, Контракт 1).
-- Дополнение по 05-rollback.md: каждая схема должна иметь откат.
-- Код крейта применяет только schema.sql (include_str!); этот файл —
-- ручная процедура отката, в автоматике не используется.
--
-- Применение (только после бэкапа файла БД, см. 05-rollback.md):
--   sqlite3 approval.db.bak < schema_down.sql

DROP TABLE IF EXISTS sync_state;
DROP TABLE IF EXISTS action_events;
DROP TABLE IF EXISTS actions;
