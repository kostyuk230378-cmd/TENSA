-- Down-миграция для schema.sql (nodes.db, Контракт 7).
-- Дополнение по 05-rollback.md: каждая схема должна иметь откат.
-- Код крейта применяет только schema.sql (include_str!); этот файл —
-- ручная процедура отката, в автоматике не используется.
--
-- Применение (только после бэкапа файла БД, см. 05-rollback.md):
--   sqlite3 nodes.db.bak < schema_down.sql

DROP TABLE IF EXISTS node_connections;
DROP TABLE IF EXISTS node_heartbeats;
DROP TABLE IF EXISTS nodes;
