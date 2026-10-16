Принято. `03-review.md` утверждён. Переходим к `04-documentation.md`.

---

## Документация для GLM 5.3: `04-documentation.md`

```markdown
# Руководство для GLM 5.3 — Документирование

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Документирование

---

## Назначение

Этот документ описывает, как GLM 5.3 должен документировать код
Tensa. Цель: чтобы код был понятен и человеку, и AI, и чтобы
документация не устаревала.

---

## Принципы

### 1. Документация — часть кода

Код без документации не принимается. Комментарии, README, примеры —
всё часть обязательств.

### 2. Документация — для человека

Пиши так, чтобы человек понял. Не для «галочки», не для AI, а для
того, кто будет читать через год.

### 3. Документация — актуальная

Если изменил код — обнови документацию. Устаревшая документация
хуже, чем её отсутствие.

### 4. Документация — на русском и английском

- Комментарии в коде — на русском (для команды).
- Публичный API — на английском (стандарт).
- README — на русском и английском.

### 5. Документация — структурированная

- Модули — README.
- Публичный API — `///`.
- Сложные места — `//`.
- Примеры — `examples/`.

---

## Типы документации

### 1. Комментарии `///` (rustdoc)

**Что:** документация публичного API.

**Где:** перед `pub fn`, `pub struct`, `pub enum`, `pub trait`.

**Формат:**
```rust
/// Создаёт новое действие в очереди одобрения.
///
/// # Аргументы
///
/// * `action` — параметры действия (тип, payload, источник).
///
/// # Возвращает
///
/// Созданное действие с заполненными полями `id`, `operation_id`,
/// `created_at`, `expires_at`.
///
/// # Ошибки
///
/// * `ApprovalError::MaxPendingReached` — если очередь заполнена.
/// * `ApprovalError::Database` — если ошибка БД.
///
/// # Пример
///
/// ```
/// use tensa_approval::{SqliteApprovalStore, NewAction, ActionKind, ActionOrigin};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = SqliteApprovalStore::new_in_memory().await?;
/// let action = store.request_action(NewAction {
///     kind: ActionKind::EmailSend,
///     payload: serde_json::json!({"to": "test@example.com"}),
///     created_by: ActionOrigin::Llm,
///     expires_in_hours: Some(24),
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn request_action(&self, action: NewAction) -> Result<Action>;
```

**Правила:**
- Краткое описание — первая строка.
- Секции: `# Аргументы`, `# Возвращает`, `# Ошибки`, `# Пример`.
- Пример должен компилироваться (doc-test).
- Не дублируй сигнатуру — она и так видна.

### 2. Комментарии `//` (внутренние)

**Что:** объяснение сложных мест.

**Где:** внутри функций, рядом с нетривиальной логикой.

**Формат:**
```rust
// Вычисляем expires_at как created_at + TTL.
// TTL задаётся в часах, переводим в секунды.
let expires_at = created_at + (ttl_hours * 3600) as i64;

// Проверяем лимит pending-действий.
// Если превышен — возвращаем ошибку, не создаём действие.
if pending_count >= self.config.max_pending_actions {
    return Err(ApprovalError::MaxPendingReached);
}
```

**Правила:**
- Объясняй **почему**, а не **что** (что видно из кода).
- Не комментируй очевидное.
- Не оставляй закомментированный код.

### 3. README крейта

**Что:** обзор крейта.

**Где:** `crates/tensa-approval/README.md`.

**Формат:**
```markdown
# tensa-approval

ApprovalStore для Tensa — центральный элемент безопасности.

## Назначение

Хранит действия, требующие одобрения владельца, до их выполнения.
Event-sourced для синхронизации между узлами.

## Принципы

- Авто-ответы запрещены.
- Прямое исполнение сетевых действий из LLM запрещено.
- Все действия через очередь одобрения.
- Действия от trusted_origin — без одобрения, но с логированием.

## Использование

```rust
use tensa_approval::{SqliteApprovalStore, NewAction, ActionKind, ActionOrigin};

let store = SqliteApprovalStore::new_in_memory().await?;
let action = store.request_action(NewAction {
    kind: ActionKind::EmailSend,
    payload: serde_json::json!({"to": "test@example.com"}),
    created_by: ActionOrigin::Llm,
    expires_in_hours: Some(24),
}).await?;

let approved = store.approve(action.id, ActionOrigin::User).await?;
```

## Структура

- `src/models.rs` — типы (`Action`, `ActionKind`, `ActionStatus`).
- `src/events.rs` — события (`ActionEvent`, `EventType`).
- `src/store.rs` — трейт `ApprovalStore`.
- `src/sqlite.rs` — реализация `SqliteApprovalStore`.
- `src/config.rs` — конфигурация.
- `src/error.rs` — ошибки.

## Тесты

```bash
cargo test -p tensa-approval
```

## Связанные ADR

- ADR-001, ADR-012, ADR-013, ADR-015, ADR-017.

## Связанные контракты

- Контракт 1: ApprovalStore (SQLite).
- Контракт 8: gRPC-контракты.
- Контракт 9: uniffi UDL.
```

**Правила:**
- Кратко: назначение, принципы, использование.
- Примеры кода.
- Ссылки на ADR и контракты.
- Не дублируй ТЗ — оно отдельно.

### 4. Примеры (`examples/`)

**Что:** рабочие примеры использования.

**Где:** `crates/tensa-approval/examples/`.

**Формат:**
```rust
// crates/tensa-approval/examples/basic.rs
use tensa_approval::{SqliteApprovalStore, NewAction, ActionKind, ActionOrigin};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteApprovalStore::new_in_memory().await?;

    let action = store.request_action(NewAction {
        kind: ActionKind::EmailSend,
        payload: serde_json::json!({"to": "test@example.com"}),
        created_by: ActionOrigin::Llm,
        expires_in_hours: Some(24),
    }).await?;

    println!("Created action: {}", action.id);

    let approved = store.approve(action.id, ActionOrigin::User).await?;
    println!("Approved: {}", approved.status);

    Ok(())
}
```

**Правила:**
- Один пример — один сценарий.
- Пример должен компилироваться и запускаться.
- Комментарии по ходу.

### 5. Документация в `docs/`

**Что:** общая документация проекта.

**Где:** `docs/`.

**Что уже есть:**
- `00-index.md` — оглавление.
- `01-adr.md` — ADR.
- `02-contracts.md` — контракты.
- `03-specs.md` — ТЗ.
- `04-examples.md` — примеры.
- `glm/` — это руководство.
- `contracts/` — `.proto`, UDL, OpenAPI.
- `guides/` — руководства.

**Правила:**
- Не дублируй содержимое.
- Ссылайся на ADR, контракты, ТЗ.
- Обновляй при изменениях.

---

## Что документировать

### 1. Публичный API

Каждый `pub fn`, `pub struct`, `pub enum`, `pub trait` — с `///`.

### 2. Сложные места

Любое место, где логика неочевидна — с `//`.

### 3. Модули

Каждый модуль — с `//!` в начале файла.

```rust
//! Модуль работы с SQLite для ApprovalStore.
//!
//! Реализует трейт `ApprovalStore` через `sqlx`.
//! Поддерживает синхронизацию через event sourcing.
```

### 4. Ошибки

Каждый вариант `Error` — с описанием.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    /// Действие не найдено.
    #[error("Action not found: {0}")]
    NotFound(Uuid),

    /// Превышен лимит pending-действий.
    #[error("Max pending actions reached: {0}")]
    MaxPendingReached(usize),

    /// Ошибка БД.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

### 5. Конфигурация

Каждое поле конфига — с описанием.

```rust
pub struct ApprovalConfig {
    /// TTL действия в часах.
    pub action_ttl_hours: u64,  // 24

    /// Авто-одобрение из trusted_origin.
    pub auto_approve_trusted: bool,  // false

    /// Разрешить отправку почты.
    pub allow_email_send: bool,  // true

    /// Разрешить отправку в Telegram.
    pub allow_telegram_send: bool,  // true

    /// Разрешить отправку в WhatsApp.
    pub allow_whatsapp_send: bool,  // false

    /// Подтверждение незнакомых получателей.
    pub confirm_unknown_recipients: bool,  // true

    /// Максимум pending-действий.
    pub max_pending_actions: usize,  // 1000
}
```

### 6. Примеры

Каждый публичный API — с примером в doc-test.

---

## Что НЕ документировать

- Очевидное (`let x = 1; // присваиваем 1`).
- Закомментированный код (удалить).
- TODO без срока (лучше issue).
- Секреты (никогда).
- Внутренние детали, которые могут измениться.

---

## Формат doc-test

**Правило:** пример должен компилироваться и запускаться.

```rust
/// # Пример
///
/// ```
/// use tensa_approval::{SqliteApprovalStore, NewAction, ActionKind, ActionOrigin};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = SqliteApprovalStore::new_in_memory().await?;
/// let action = store.request_action(NewAction {
///     kind: ActionKind::EmailSend,
///     payload: serde_json::json!({"to": "test@example.com"}),
///     created_by: ActionOrigin::Llm,
///     expires_in_hours: Some(24),
/// }).await?;
/// # Ok(())
/// # }
/// ```
```

**Правила:**
- `# ` — скрытые строки (setup).
- `?` — для обработки ошибок.
- `async fn example()` — обёртка для async.
- Пример должен компилироваться.

---

## Язык документации

### 1. Комментарии в коде

**Русский** — для команды.

```rust
// Проверяем, что действие не истекло.
if action.expires_at < now {
    return Err(ApprovalError::Expired);
}
```

### 2. Публичный API

**Английский** — стандарт.

```rust
/// Creates a new action in the approval queue.
///
/// # Arguments
///
/// * `action` — action parameters.
///
/// # Returns
///
/// The created action.
///
/// # Errors
///
/// * `ApprovalError::MaxPendingReached` — if queue is full.
pub async fn request_action(&self, action: NewAction) -> Result<Action>;
```

### 3. README

**Русский + английский** — для open-source.

```markdown
# tensa-approval

ApprovalStore for Tensa — the central security element.

[Русская версия ниже](#русская-версия)

...

## Русская версия

ApprovalStore для Tensa — центральный элемент безопасности.
...
```

---

## Обновление документации

### 1. При изменении API

- Обнови `///`.
- Обнови README.
- Обнови примеры.
- Обнови doc-test.

### 2. При изменении поведения

- Обнови описание в `///`.
- Обнови примеры.
- Обнови README.

### 3. При изменении конфигурации

- Обнови описание полей.
- Обнови примеры конфига.
- Обнови README.

### 4. При добавлении модуля

- Добавь `//!` в начало.
- Добавь в README.
- Добавь пример.

### 5. При удалении модуля

- Удали из README.
- Удали примеры.
- Удали ссылки.

---

## Проверка документации

### 1. rustdoc

```bash
cargo doc --no-deps --open
```

Проверь:
- Все публичные API документированы.
- Примеры компилируются.
- Ссылки работают.

### 2. Doc-tests

```bash
cargo test --doc
```

Все doc-tests должны проходить.

### 3. Clippy

```bash
cargo clippy -- -D warnings
```

Clippy проверяет в том числе:
- Отсутствие документации на публичном API (`missing_docs`).
- Устаревшие ссылки.

### 4. Markdown

Проверь, что Markdown-файлы:
- Валидные.
- Без битых ссылок.
- Без устаревшего содержимого.

---

## Чек-лист документации

- [ ] Публичный API документирован (`///`).
- [ ] Модули документированы (`//!`).
- [ ] Ошибки документированы.
- [ ] Конфигурация документирована.
- [ ] Примеры в doc-test.
- [ ] README крейта.
- [ ] Примеры в `examples/`.
- [ ] Ссылки на ADR и контракты.
- [ ] `cargo doc` проходит.
- [ ] `cargo test --doc` проходит.
- [ ] Markdown валидный.

---

## Частые ошибки

### 1. Устаревшая документация

**Плохо:** изменил API, не обновил `///`.

**Хорошо:** обновил API — обновил `///`.

### 2. Дублирование

**Плохо:** копировать описание из ТЗ в README.

**Хорошо:** ссылаться на ТЗ.

### 3. Очевидные комментарии

**Плохо:** `// Присваиваем x значение 1`.

**Хорошо:** `// TTL в часах, переводим в секунды`.

### 4. Закомментированный код

**Плохо:** оставлять `// let old = ...`.

**Хорошо:** удалить (git помнит).

### 5. Секреты в документации

**Плохо:** пример с реальным API-ключом.

**Хорошо:** пример с `sk-or-...` или `<your-key>`.

### 6. Примеры без компиляции

**Плохо:** пример, который не компилируется.

**Хорошо:** doc-test, который проходит.

### 7. Отсутствие ссылок

**Плохо:** описание без ссылок на ADR/контракты.

**Хорошо:** ссылки на ADR-001, Контракт 1.

---

## Следующие файлы

- `05-rollback.md` — откат.
- `06-cargo-workspace.md` — работа с workspace.
- `07-ci.md` — CI.
```

---

