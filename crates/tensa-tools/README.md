# tensa-tools

Мета-крейт инструментов секретаря (ТЗ tensa-tools): общий трейт `Tool`,
реестр `ToolRegistry` и сами инструменты.

## Состав

| Компонент | Назначение |
| --- | --- |
| `Tool` | Трейт инструмента: `name`, `description`, `execute(params)`, `requires_approval` |
| `ToolRegistry` | Реестр `имя → Arc<dyn Tool>`: `register`, `get`, `list` |
| `wol` | Wake-on-LAN: magic packet (6 × `0xFF` + MAC × 16) по UDP, `WolTool` |
| `telegram` | Бот-секретарь: уведомления владельцу, команды и inline-кнопки решений, цикл одобрения |
| `audit` | Журнал действий `~/.tensa/logs/tools.log` (JSON-строки, без секретов) |

## Инструмент `wol`

Параметры `execute`: `{"mac": "AA:BB:CC:DD:EE:FF" | "AABBCCDDEEFF", "broadcast": "255.255.255.255", "port": 9}` —
обязателен только `mac`. Порт по умолчанию 9, адрес — broadcast `255.255.255.255`.
Отправка — на `std::net::UdpSocket` с явным `set_broadcast(true)`; сторонние
WoL-крейты не используются.

`requires_approval() == true`: сетевые действия проходят через `ApprovalStore`
централизованно (ТЗ), сам инструмент хранилище не трогает.

## Структура и отклонения

`wol` оформлен модулем внутри крейта (`src/wol/`). ТЗ предполагает отдельный
подкрейт `crates/tensa-tools/wol` — его выделение требует правки корневого
`Cargo.toml` воркспейса и делается отдельной задачей ([derived]).

## Инструмент `telegram`

Тонкий клиент Bot API поверх `reqwest` (без `teloxide` — [derived]: принцип
ТЗ «минимум зависимостей»; клиент — `telegram/bot.rs`, обработчики —
`telegram/handlers.rs`, схема кнопок — `telegram/callbacks.rs`).

Настройки: токен бота — `TELEGRAM_BOT_TOKEN` → `secrets.toml`;
чат владельца — `secrets.telegram.owner_chat_id` → `config.telegram.owner_chat_id`.

**Прокси (важно):** `reqwest` читает только переменные окружения
`HTTPS_PROXY`/`HTTP_PROXY`; системный прокси Windows (WinINET) не подхватывается.
Если сеть ходит через прокси, задайте переменную перед запуском, например:
`$env:HTTPS_PROXY='http://127.0.0.1:10809'`.

Цикл одобрения (LLM-инициированные отправки):

1. `TelegramService::propose_send(text, store)` — создаёт действие
   `telegram_send` (origin `llm`, статус `pending`); владелец видит его в PWA
   («Одобрения») и в боте (`/approvals` с кнопками);
2. решение: кнопка в боте или approve в PWA (origin `telegram` — trusted,
   с аудитом);
3. `execute_approved(store, id)` — отправка через Bot API и статус `executed`.

Демо: `cargo run -p tensa-tools --example telegram_flow -- <config.toml> propose "текст"`
(далее `execute <uuid>` после одобрения или `bot` для long polling).

Все действия пишутся в `<root>/logs/tools.log` (ТЗ: аудит).

## Проверки

```powershell
cargo test -p tensa-tools
cargo clippy -p tensa-tools --all-targets -- -D warnings
```

Тесты: разбор MAC (валидные/невалидные формы), побайтовая сверка magic packet,
реальная отправка на локальный UDP-сокет, работа через `ToolRegistry`.

## Дальше по плану

Подкрейты (порядок — по приоритету под дедлайн): email → documents →
calendar → ocr → whatsapp (отложен). Выделение подкрейтов
`crates/tensa-tools/<tool>` из модулей — отдельная механическая задача
([derived], как у `wol`/`telegram`).
