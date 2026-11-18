# tensa-tools

Мета-крейт инструментов секретаря (ТЗ tensa-tools): трейт `Tool`, реестр
`ToolRegistry` и сами инструменты.

## Назначение

- `TelegramTool`/`TelegramService` — бот-секретарь: уведомления, решения из чата
  (inline-кнопки), цикл одобрения;
- `EmailTool`/`EmailService` — отправка почты (SMTP/lettre, автодетект
  провайдеров) + IMAP-клиент (`email::imap`, Контракт 6);
- `AppsTool`-модуль `apps` — сканирование приложений ОС, кэш `apps_cache.json`,
  чёрный список, запуск, открытие URL (Контракт 5, MVP);
- `WolTool` — Wake-on-LAN;
- `DocumentsTool` — извлечение текста PDF/DOCX/XLSX/PPTX/TXT/MD.

## Принципы

- Сетевые действия — только через `ApprovalStore` (централизованно);
  инструмент лишь объявляет `requires_approval`.
- Секреты не логируются; аудиты — в `~/.tensa/logs/tools.log`.

## Использование

```rust
use std::sync::Arc;
use tensa_tools::{ToolRegistry, WolTool};

let mut registry = ToolRegistry::new();
registry.register(Arc::new(WolTool::new()));
assert!(registry.get("wol").is_some());
```

## Структура

- `src/email/` — SMTP/IMAP, автодетект, соединения, ошибки;
- `src/apps.rs` — приложения ОС (Контракт 5, MVP);
- `src/telegram/`, `src/wol/`, `src/documents/` — остальные инструменты;
- `src/audit.rs` — аудит вызовов.

## Тесты

```bash
cargo test -p tensa-tools
```

## Связанные ADR и контракты

- ADR-001, ADR-015; Контракты 5, 6; ТЗ tensa-tools.
