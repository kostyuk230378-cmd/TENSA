# tensa-nodes

Реестр узлов Tensa (NodeRegistry): heartbeat, роли, ресурсы, соединения
(ADR-015, Контракт 7).

## Назначение

- регистрация/удаление узлов сети (`nodes.db`, SQLite); повторная
  регистрация — идемпотентный upsert;
- heartbeat: интервал 30 с, офлайн > 90 с (`cleanup_stale`); история
  телеметрии в `node_heartbeats`;
- роли heavy/thin/auto; `find_heavy` — онлайн heavy (или auto+GPU) с
  максимальной доступной RAM;
- ресурсы узла (telegram/email/whatsapp/llm_local/llm_cloud/memory_store/
  approval_store) и поиск `find_with_resource`;
- соединения к разделяемым ресурсам: TTL 15 минут, конфликт —
  `ResourceBusy`, повторный захват — продление, освобождение при офлайн.

## Профилирование

`NodeProfile` + трейт `NodeProfiler` + `StubProfiler` (desktop/android,
детерминированные значения для тестов). Реальный сбор телеметрии —
`tensa-perf` (`DefaultPerfProfiler`); кадр узлу поставляет оркестратор
(tensa-core).

## Тесты

```bash
cargo test -p tensa-nodes
```

## Ограничения

Не синхронизирует, не выполняет действия, не логирует секреты (ТЗ).
