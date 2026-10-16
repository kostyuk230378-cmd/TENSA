# tensa-ffi

uniffi-обёртка Tensa (Контракт 9): UDL-биндинги для мобильных и настольных
клиентов. Источник истины поверхности — `teh_doc/contracts/ffi/tensa.udl`
(синхронизируется с `teh_doc/02-contracts.md`, Контракт 9).

## Как устроено

- `build.rs` генерирует scaffolding из UDL через
  `uniffi::generate_scaffolding_for_crate(udl, "tensa_ffi")` (UDL лежит вне
  крейта, авто-режим его не находит).
- `src/lib.rs` подключает `uniffi::include_scaffolding!("tensa")` и объявляет
  namespace-функции `init` / `health` / `version`.
- Типы UDL (`src/types.rs`) — обычные Rust-типы без derive-макросов uniffi:
  конверсию FfiConverter генерирует `udl_derive` из scaffolding; unit-варианты
  enum записываются с пустыми скобками (`Persona {}`), как требует кодоген.

## Состояние интерфейсов

| Интерфейс | Статус |
| --- | --- |
| `ApprovalStore` | Реально: tensa-approval (`approval.db`) |
| `NotesStore` | Реально: tensa-core (`notes.db`) |
| `ContactsStore` | Реально: tensa-core (`contacts.db`) |
| `ConfigStore` | Реально: config.toml (плоское UDL-представление) |
| `NodeRegistry` | Реально: tensa-nodes (`nodes.db`) |
| `CoreMemoryStore` | Реально: tensa-memory (`memory.db`) |
| `MemoryStore` (документы) | Заглушка: в tensa-memory пока только трейт |
| `EmailClient`, `CalendarClient`, `AppLauncher` | Заглушка: этап tensa-tools |
| `QueryEngine` | Заглушка: проводка роутера |
| `VoiceEngine` | Заглушка: голосовой стек |
| `FirstRunWizard` | Заглушка: см. расхождения UDL ниже |

Заглушка = рабочий конструктор + метод возвращает `TensaError::InternalError`
(варианта `NotImplemented` в UDL нет), причина пишется в лог `tracing`.

## Расхождения UDL (зафиксированы, правка контракта — за Архитектором)

- варианта `NotImplemented` в `TensaError` нет;
- `TensaError` без payload — текст ошибки теряется (класс уходит наружу,
  причина — в лог);
- `EmailAccount` не содержит `email`/`credential` (ядру нужны);
- `WhatsAppConfig.access_token` vs `credential` в ядре.

## Проверки

```powershell
cargo test -p tensa-ffi
cargo clippy -p tensa-ffi --all-targets -- -D warnings
```

Тесты — smoke на временном корне (конструкторы, CRUD заметок через FFI,
честные отказы заглушек). Генерация bindings для Kotlin/Swift/Python —
отдельный шаг (uniffi-bindgen) при сборке приложений.
