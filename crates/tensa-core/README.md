# tensa-core

Центральный крейт Tensa: общие типы, конфигурация, заметки/контакты
(SQLite + FTS5), координатор подсистем, first-run wizard.

## Назначение

Реализует Контракты 3/4/13 и ТЗ `tensa-core` (+ `dop_tz_core.md`):
- **Конфигурация** (`config.rs`): `Config` (секции по Контракту 4),
  TOML load/save (атомарно, бэкап), `validate()`, трейт `ConfigStore`;
- **Заметки и контакты** (`notes.rs`, `contacts.rs`): трейты
  `NotesStore`/`ContactsStore` (Контракт 3) + SQLite-реализации с FTS5
  (unicode61), soft delete, векторные часы, LWW;
- **First-run wizard** (`first_run.rs`): 8 шагов (dop_tz_core, ADR-021);
- **Инициализация памяти** (`init.rs`): 4 блока Контракта 13
  (persona/human/tasks/preferences);
- **Координатор** (`core.rs`): `CoreRuntime` — start/stop/health/reload
  (подсистемы подключаются по мере появления их крейтов);
- **Health** (`health.rs`): `HealthStatus`/`ComponentHealth` из ТЗ.

## Контракт 3: sync-методы (добавлено 2026-10-15, апрув Архитектора)

- `get_notes_since(timestamp)` / `get_contacts_since(timestamp)` — для
  sync-адаптера (`tensa-sync`): изменённые позже timestamp, только
  неудалённые, `ORDER BY updated_at ASC`;
- `count_notes()` / `count_contacts()` — общий count для `DataService.total`.

## Использование

```rust
use tensa_core::{SqliteNotesStore, NotesStore, NewNote};

let store = SqliteNotesStore::new_in_memory().await?;
let note = store.create_note(NewNote {
    title: "план".into(),
    body: "текст".into(),
    tags: vec!["работа".into()],
}).await?;
```

## Тесты

```bash
cargo test -p tensa-core
```

## Ограничения

Не дублирует логику других крейтов: `VectorClock` — из `tensa-approval`;
ядро памяти — в `tensa-memory`. Секреты не хранятся в `config.toml`
(только `secrets.toml`, Контракт 4).
