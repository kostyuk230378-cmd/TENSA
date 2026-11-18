# tensa-server

HTTP-бэкенд PWA (Контракт 12): axum-роутер поверх крейтов Tensa, Bearer-доступ,
SSE-события, статика PWA.

## Назначение

- `/api/v1` — API Контракта 12: health/version, одобрения, заметки, контакты,
  календарь, файлы, модели, master первого запуска, узлы сети;
- `/query` — шлюз моделей (облако/локальная GGUF) с парсером протокола
  секретаря (`tensa-router::protocol`); memory tool calls применяются сразу;
- `/events` — SSE-поток (broadcast);
- `/` и `/app` — статика PWA (публична), ассеты — `/static/...`;
- gRPC-сервер синхронизации (ADR-012) стартует в том же процессе при
  `[sync] enabled = true`.

## Принципы

- Ошибки — `ServerError` → JSON `{"error": "..."}` с корректным HTTP-статусом;
  секреты не логируются.
- Маршруты, не реализованные в текущей фазе, отдают 501 (явные заглушки
  `routes/stubs.rs` — не 404).
- Паник в обработчиках нет: любые ошибки — значениями.

## Использование

```bash
# Запуск (порт 8765, статика pwa/static, токен из <root>/pwa_token):
tensa-server [путь\к\config.toml]
# Слушать на всех интерфейсах (для телефона/Tailscale):
set TENSA_BIND=0.0.0.0:8765 && tensa-server
```

## Структура

- `src/routes/` — обработчики: approvals, notes, contacts, calendar, files,
  email(+imap-контур), telegram_setup, brain, speech_models, first_run, nodes;
- `src/state.rs` — `AppState` (хранилища, события, сессия мастера);
- `src/error.rs` — `ServerError` (маппинг на HTTP-статусы);
- `src/brain.rs`, `src/query.rs` — шлюз моделей и `/query`.

## Тесты

```bash
cargo test -p tensa-server
```

## Связанные ADR и контракты

- ADR-010, ADR-012, ADR-021; Контракты 1, 3, 6, 7, 12; ТЗ apps/pwa.
