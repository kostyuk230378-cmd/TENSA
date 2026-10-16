# tensa-sync

Синхронизация между узлами Tensa (ADR-012, Контракт 8, ТЗ `tensa-sync`).

## Назначение

- ApprovalStore — постоянная синхронизация (интервал 60 с, события event sourcing);
- заметки и контакты — on-demand (ADR-013);
- heartbeat узлов: каждые 30 с, офлайн > 90 с;
- управление соединениями к разделяемым ресурсам: TTL 15 минут, очередь,
  освобождение при офлайн (ADR-015);
- Syncthing для файлов (ADR-011, ADR-018) — фича `syncthing`.

## Ограничения (из ТЗ)

Крейт **не хранит данные** (данные — в `tensa-approval` и др.), не выполняет
действия, не принимает решения, не логирует секреты (API-ключ Syncthing
исключён из сериализации конфига).

## Фичи

| Фича | Что включает |
|---|---|
| `grpc` | gRPC-транспорт (tonic/prost) — **в default** с 2026-10-15; сборка без транспорта: `--no-default-features` |
| `syncthing` | REST API клиент Syncthing (reqwest + rustls) |

Ядро логики (heartbeat, соединения, курсоры, трейт синхронизации) тестируется
без сетевых зависимостей.

## Структура

| Модуль | Содержание |
|---|---|
| `config` | `SyncConfig` (дефолты из ТЗ) |
| `status` | `SyncStatus`, `SyncthingStatus` |
| `heartbeat` | `NodeHeartbeat`, `HeartbeatState` |
| `connections` | `ConnectionManager`, `Lease` (TTL 15 мин) |
| `store_ext` | `ApprovalSync` (адаптер над `tensa-approval`), `SyncCursor` |
| `sync_log` | `SyncLedger` — учёт раундов синхронизации |
| `syncthing` | `SyncthingClient` (фича `syncthing`) |

## Статус реализации

Реализованы: типы, трейт `SyncService`, gRPC-сервер всех пяти сервисов
Контракта 8 (Sync/Node/Data — рабочие; Voice/Query — UNIMPLEMENTED до
`tensa-llm`), клиент `GrpcSyncService`, sync-адаптеры заметок/контактов
поверх Stores (`StoreNotesSync`/`StoreContactsSync`; Контракт 3, одобрено
2026-10-15). `NoopSyncSource` остаётся fallback.

Ограничение: применение события для **отсутствующей локально** заметки/контакта
требует insert-with-id (в трейт не входит) — отказ явный, решение ожидает апрува.

## Тесты

```bash
cargo test -p tensa-sync
cargo test -p tensa-sync --features syncthing
```
