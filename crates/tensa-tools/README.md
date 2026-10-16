# tensa-tools

Мета-крейт инструментов секретаря (ТЗ tensa-tools): общий трейт `Tool`,
реестр `ToolRegistry` и сами инструменты.

## Состав

| Компонент | Назначение |
| --- | --- |
| `Tool` | Трейт инструмента: `name`, `description`, `execute(params)`, `requires_approval` |
| `ToolRegistry` | Реестр `имя → Arc<dyn Tool>`: `register`, `get`, `list` |
| `wol` | Wake-on-LAN: magic packet (6 × `0xFF` + MAC × 16) по UDP, `WolTool` |

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

## Проверки

```powershell
cargo test -p tensa-tools
cargo clippy -p tensa-tools --all-targets -- -D warnings
```

Тесты: разбор MAC (валидные/невалидные формы), побайтовая сверка magic packet,
реальная отправка на локальный UDP-сокет, работа через `ToolRegistry`.

## Дальше по плану

Подкрейты (порядок — по отчёту о зависимостях): apps → documents → ocr →
telegram → whatsapp → calendar → email.
