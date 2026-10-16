# tensa-approval

ApprovalStore — центральный элемент безопасности Tensa: хранит действия,
требующие одобрения владельца, до их выполнения.

## Назначение

Крейт реализует Контракт 1 (`02-contracts.md`) и ТЗ `tensa-approval`
(`03-specs.md`). Только хранит и ведёт жизненный цикл действий —
**не исполняет их** и не делает сетевых запросов:

```
created → pending → approved → executed
                 ↘ rejected
                 ↘ expired (TTL 24 часа)
                 ↘ failed (ошибка)
```

## Принципы

- авто-ответы запрещены;
- прямое исполнение сетевых действий из LLM запрещено — всегда через очередь;
- действия от trusted origin (user/telegram) — без одобрения, но с логированием
  (`auto_approve_trusted`);
- event sourcing: каждое изменение — событие в `action_events` для gRPC-синхронизации;
- идемпотентность — `operation_id`; конфликты — векторные часы + LWW.

## Использование

```rust
use tensa_approval::{SqliteApprovalStore, ApprovalConfig, NewAction, ActionKind, ActionOrigin};

let store = SqliteApprovalStore::new("~/.tensa/approval.db", ApprovalConfig::default(), "node-1".into()).await?;
let action = store.request_action(NewAction {
    kind: ActionKind::EmailSend,
    payload: serde_json::json!({ "to": "user@example.com" }),
    created_by: ActionOrigin::Llm,
    expires_in_hours: None,
}).await?;

let approved = store.approve(action.id, ActionOrigin::User).await?;
let executed = store.execute(approved.id).await?;
```

## Структура

| Модуль | Содержание |
|---|---|
| `models` | `Action`, `NewAction`, `ActionKind`, `ActionStatus`, `ActionOrigin` |
| `events` | `ActionEvent`, `EventType` (event sourcing) |
| `store` | трейт `ApprovalStore` (9 методов) |
| `sqlite` | реализация `SqliteApprovalStore` (sqlx) |
| `config` | `ApprovalConfig` (дефолты из ТЗ) |
| `error` | `ApprovalError` (thiserror) |
| `vector_clock` | `VectorClock` (Контракт 8) |
| `schema.sql` | схема БД (`actions`, `action_events`, `sync_state`) |

## Тесты

```bash
cargo test -p tensa-approval
```
