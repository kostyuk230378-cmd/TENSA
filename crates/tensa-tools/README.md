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
| `email` | Отправка почты через одобрение: SMTP (lettre), автоопределение провайдеров; IMAP-чтение — следующая итерация |
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

## Инструмент `email`

Отправка почты через цикл одобрения (ТЗ/Контракт 6, ADR-015): `propose_send`
ставит письмо в очередь (`email_send`, origin `llm`), владелец одобряет в PWA
или боте, `execute_approved` отправляет через SMTP и фиксирует `executed`.

- Аккаунты: `config.toml` `[email.accounts.<id>]`; порт 0 и пустые хосты —
  автоопределение по домену (`autodetect`: gmail/outlook/yandex/mail.ru).
- Секреты: `secrets.toml` → `[email.accounts.<id>].credential` либо env
  `SMTP_PASSWORD`/`IMAP_PASSWORD` (окружение приоритетнее).
- Транспорт: `lettre` (rustls); 587 — STARTTLS, 465 — неявный TLS.
- Демо: `cargo run -p tensa-tools --example email_flow -- <config.toml> propose <to> <subject> <body>`.

[derived] `imap.rs` (чтение/поиск через async-imap) и `oauth2.rs` — следующая
итерация; сейчас контур отправки полностью рабочий и проверенный без сети.

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

0. новые действия бот присылает сам: сторож очереди опрашивает её раз в 5 минут
   (и сразу при старте — накопленное не теряется);

1. `TelegramService::propose_send(text, store)` — создаёт действие
   `telegram_send` (origin `llm`, статус `pending`); владелец видит его в PWA
   («Одобрения») и в боте (`/approvals` с кнопками);
2. решение: кнопка в боте или approve в PWA (origin `telegram` — trusted,
   с аудитом);
3. `execute_approved(store, id)` — отправка через Bot API и статус `executed`.

Демо: `cargo run -p tensa-tools --example telegram_flow -- <config.toml> propose "текст"`
(далее `execute <uuid>` после одобрения или `bot` для long polling).

Все действия пишутся в `<root>/logs/tools.log` (ТЗ: аудит).

## Инструмент `documents`

Извлечение текста для поиска и индексации памяти (ТЗ `documents/`):
PDF (`pdf-extract`; сканы без OCR-слоя — этап `ocr`), DOCX/PPTX (`zip` +
`quick-xml`, текст из `w:t`/`a:t`), XLSX (`calamine`, листы секциями),
TXT/MD (как текст; [derived] markdown без рендера — для индексации хватает).
Тип определяется по расширению и «магии» (`%PDF`, ZIP-части word/xl/ppt).

Инструмент `DocumentsTool` (имя `documents`): параметры `{"path": "..."}`,
ответ — `kind`, `chars`, `preview`; `requires_approval() = false` (чтение
локального файла не сетевое, ТЗ), каждое извлечение — в `tools.log`.
[derived] PDF — на `pdf-extract`: ADR-020 называет pdfplumber (Alpha),
план разведки прямо предусматривает этот fallback.

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
