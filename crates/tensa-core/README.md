# tensa-core

Ядро Tensa: координатор `TensaCore`, конфигурация, заметки/контакты (SQLite + FTS5),
master первого запуска и инициализация core memory.

## Назначение

- `TensaCore` — координатор подсистем (Контракт 4: один `config.toml`,
  корень данных — каталог конфига; подсистемы подключаются снаружи трейтами);
- `NotesStore` / `ContactsStore` (Контракт 3: SQLite + FTS5, soft delete,
  LWW по `updated_at` + vector clock) — реализация `SqliteNotesStore` /
  `SqliteContactsStore` в общей БД `notes.db`;
- `FirstRunWizard` + `TensaCore::{is_first_run, run_first_run_wizard, init_memory}`
  (доп. ТЗ tensa-core, ADR-021) — 8 шагов мастера, запись `config.toml` +
  `secrets.toml` + инициализация core memory;
- `InitParams` + тексты блоков core memory по умолчанию (Контракт 13).

## Принципы

- Без `unwrap()` в прод-коде: ошибки — `CoreError` через `Result`.
- Секреты (пароли почты, токены) не логируются и не попадают в `config.toml`.
- Хранилища не синхронизируют и не принимают решений (ограничение ТЗ).

## Использование

```rust
use tensa_core::{SqliteContactsStore, NewContact};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteContactsStore::new_in_memory().await?;
    let contact = store
        .create_contact(NewContact {
            name: "Иван".into(),
            email: Some("ivan@example.com".into()),
            phone: None,
            telegram: Some("@ivan".into()),
            whatsapp: None,
            notes: None,
            tags: vec!["работа".into()],
        })
        .await?;
    let found = store.search_contacts("Иван", 10).await?;
    println!("найдено: {}", found.len());
    let _ = contact;
    Ok(())
}
```

## Структура

- `src/core.rs` — `TensaCore` (координатор);
- `src/config.rs` — `Config` (Контракт 4);
- `src/models.rs` — общие типы (Note, Contact, EmailAccount, …);
- `src/notes.rs`, `src/contacts.rs` — SQLite-хранилища (Контракт 3);
- `src/first_run.rs`, `src/init.rs` — мастер первого запуска (ADR-021);
- `src/secrets.rs`, `src/paths.rs` — `secrets.toml` и пути `~/.tensa/`;
- `schema.sql` — схема notes.db (встраивается через `include_str!`).

## Тесты

```bash
cargo test -p tensa-core
```

## Связанные ADR и контракты

- ADR-001, ADR-021.
- Контракты 3, 4, 7, 13; доп. ТЗ tensa-core (`FROM_GLM/dop_tz_core.md`).
