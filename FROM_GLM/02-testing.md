Принято. `01-task-format.md` утверждён. Переходим к `02-testing.md`.

---

## Документация для GLM 5.3: `02-testing.md`

```markdown
# Руководство для GLM 5.3 — Тестирование

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Тестирование

---

## Назначение

Этот документ описывает, как GLM 5.3 должен писать тесты для Tensa.
Цель: покрыть код тестами, поймать ошибки до релиза, обеспечить
регрессионную защиту.

---

## Принципы

### 1. Тесты — обязательны

Без тестов код не принимается. Даже если код «работает» — без тестов
он не считается готовым.

### 2. Тесты пишутся вместе с кодом

Не «сначала код, потом тесты». А «код + тесты вместе».

### 3. Тесты — часть контракта

Если контракт говорит: «метод должен возвращать ошибку при X» — тест
должен это проверять.

### 4. Тесты должны быть быстрыми

Unit-тесты — миллисекунды. Integration — секунды. E2E — минуты.
Если тест медленный — он должен быть помечен.

### 5. Тесты должны быть детерминированными

Никаких «иногда проходит». Если тест флакует — исправить или удалить.

---

## Типы тестов

### 1. Unit-тесты

**Что:** отдельные функции, методы, модули.

**Где:** `#[cfg(test)] mod tests` в том же файле, или `tests/` в крейте.

**Скорость:** миллисекунды.

**Пример:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_status_serialization() {
        let status = ActionStatus::Pending;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"pending\"");
    }

    #[test]
    fn test_vector_clock_merge() {
        let mut vc1 = VectorClock::new();
        vc1.increment("node1");
        let mut vc2 = VectorClock::new();
        vc2.increment("node2");
        vc1.merge(&vc2);
        assert_eq!(vc1.get("node1"), 1);
        assert_eq!(vc1.get("node2"), 1);
    }
}
```

### 2. Integration-тесты

**Что:** взаимодействие модулей, работа с БД, сеть.

**Где:** `tests/` в крейте.

**Скорость:** секунды.

**Пример:**
```rust
// tests/integration.rs
use tensa_approval::{SqliteApprovalStore, NewAction, ActionKind, ActionOrigin};

#[tokio::test]
async fn test_create_and_approve() {
    let store = SqliteApprovalStore::new_in_memory().await.unwrap();
    let action = store.request_action(NewAction {
        kind: ActionKind::EmailSend,
        payload: serde_json::json!({"to": "test@example.com"}),
        created_by: ActionOrigin::Llm,
        expires_in_hours: Some(24),
    }).await.unwrap();

    assert_eq!(action.status, ActionStatus::Pending);

    let approved = store.approve(action.id, ActionOrigin::User).await.unwrap();
    assert_eq!(approved.status, ActionStatus::Approved);
}
```

### 3. Error-тесты

**Что:** обработка ошибок, граничные случаи.

**Где:** рядом с unit или integration.

**Пример:**
```rust
#[tokio::test]
async fn test_approve_nonexistent_action() {
    let store = SqliteApprovalStore::new_in_memory().await.unwrap();
    let result = store.approve(Uuid::new_v4(), ActionOrigin::User).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ApprovalError::NotFound(_)));
}

#[tokio::test]
async fn test_max_pending_actions() {
    let store = SqliteApprovalStore::new_with_limit(2).await.unwrap();
    store.request_action(...).await.unwrap();
    store.request_action(...).await.unwrap();
    let result = store.request_action(...).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ApprovalError::MaxPendingReached));
}
```

### 4. E2E-тесты

**Что:** полные сценарии (UI → ядро → БД → UI).

**Где:** `tests/e2e/` в приложениях.

**Скорость:** минуты.

**Пример (Playwright для десктопа):**
```typescript
import { test, expect } from '@playwright/test';

test('approve action from UI', async ({ page }) => {
    await page.goto('http://localhost:5173');
    await page.click('[data-testid="approvals-tab"]');
    await expect(page.locator('[data-testid="pending-count"]')).toHaveText('1');
    await page.click('[data-testid="approve-button"]');
    await expect(page.locator('[data-testid="pending-count"]')).toHaveText('0');
});
```

### 5. Property-тесты

**Что:** проверка инвариантов на случайных данных.

**Где:** `tests/property.rs`.

**Инструмент:** `proptest`.

**Пример:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_chunking_preserves_text(text in "\\PC{100,5000}") {
        let chunks = chunk_text(&text, 1000, 200);
        let reconstructed: String = chunks.join("");
        // Проверяем, что текст не потерян
        prop_assert!(reconstructed.contains(&text[..100]));
    }
}
```

### 6. Benchmark-тесты

**Что:** производительность.

**Где:** `benches/` в крейте.

**Инструмент:** `criterion`.

**Пример:**
```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_search(c: &mut Criterion) {
    let store = setup_store();
    c.bench_function("search_10k", |b| {
        b.iter(|| store.search("смета за апрель", 10))
    });
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
```

---

## Что тестировать

### 1. Публичный API

Каждый публичный метод — минимум один тест.

### 2. Граничные случаи

- Пустые входные данные.
- Максимальные значения.
- Минимальные значения.
- Нулевые значения.

### 3. Обработка ошибок

- Невалидные входные данные.
- Отсутствующие ресурсы.
- Сетевые ошибки.
- Конфликты.

### 4. Инварианты

- Данные не теряются.
- Порядок сохраняется.
- Идемпотентность.

### 5. Синхронизация

- Данные синхронизируются.
- Конфликты разрешаются.
- Идемпотентность.

### 6. Безопасность

- Секреты не логируются.
- ApprovalStore работает.
- Выключатели работают.

---

## Что НЕ тестировать

- Сторонние библиотеки (у них свои тесты).
- Приватные методы (тестируй через публичный API).
- Конфигурацию (тестируй через интеграцию).

---

## Структура тестов

```
crates/tensa-approval/
├── src/
│   ├── lib.rs
│   ├── sqlite.rs
│   │   └── #[cfg(test)] mod tests { ... }  # unit
│   └── ...
├── tests/
│   ├── integration.rs                       # integration
│   ├── errors.rs                            # error
│   ├── property.rs                          # property
│   └── fixtures/
│       └── mod.rs                           # общие фикстуры
└── benches/
    └── approval.rs                          # benchmark
```

---

## Фикстуры

**Что:** общие данные для тестов.

**Где:** `tests/fixtures/`.

**Пример:**
```rust
// tests/fixtures/mod.rs
pub async fn setup_store() -> SqliteApprovalStore {
    SqliteApprovalStore::new_in_memory().await.unwrap()
}

pub fn sample_action() -> NewAction {
    NewAction {
        kind: ActionKind::EmailSend,
        payload: serde_json::json!({
            "to": "test@example.com",
            "subject": "Test",
            "body": "Test body",
        }),
        created_by: ActionOrigin::Llm,
        expires_in_hours: Some(24),
    }
}
```

---

## Mocking

**Что:** замена внешних зависимостей.

**Инструмент:** `mockall`.

**Пример:**
```rust
use mockall::automock;

#[automock]
#[async_trait]
pub trait LlmEngine {
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse>;
}

#[tokio::test]
async fn test_router_fallback() {
    let mut mock = MockLlmEngine::new();
    mock.expect_generate()
        .times(1)
        .returning(|_| Err(LlmError::ModelNotLoaded));

    let router = BrainRouter::new(mock);
    let result = router.query(...).await;
    assert!(result.is_err());
}
```

**Правило:** mock только внешние зависимости (сеть, БД, LLM). Не mock свой код.

---

## Покрытие

**Цель:** >80% для критичных крейтов, >60% для остальных.

**Инструмент:** `cargo-llvm-cov`.

```bash
cargo llvm-cov --html
```

**Что важно:**
- Публичный API — 100%.
- Обработка ошибок — 100%.
- Граничные случаи — 100%.
- Внутренняя логика — 80%+.

---

## CI и тесты

**Что запускается в CI:**
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test` (все тесты)
- `cargo test --release` (для проверки оптимизаций)
- `cargo llvm-cov` (покрытие)
- `cargo bench` (опционально)

**Что блокирует merge:**
- Провал любого теста.
- Покрытие ниже порога.
- Clippy warnings.

---

## Именование тестов

**Правило:** `test_<что>_<условие>_<ожидание>`.

**Примеры:**
- `test_request_action_with_valid_data_succeeds`
- `test_request_action_with_invalid_kind_fails`
- `test_approve_nonexistent_action_returns_error`
- `test_max_pending_actions_rejects_new`

---

## Пример полного набора тестов

Для метода `request_action`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_action_with_valid_data_succeeds() {
        let store = setup_store().await;
        let action = store.request_action(sample_action()).await.unwrap();
        assert_eq!(action.status, ActionStatus::Pending);
        assert!(action.expires_at > action.created_at);
    }

    #[tokio::test]
    async fn test_request_action_generates_unique_id() {
        let store = setup_store().await;
        let a1 = store.request_action(sample_action()).await.unwrap();
        let a2 = store.request_action(sample_action()).await.unwrap();
        assert_ne!(a1.id, a2.id);
        assert_ne!(a1.operation_id, a2.operation_id);
    }

    #[tokio::test]
    async fn test_request_action_respects_ttl() {
        let store = setup_store().await;
        let mut action = sample_action();
        action.expires_in_hours = Some(1);
        let result = store.request_action(action).await.unwrap();
        let expected = result.created_at + 3600;
        assert_eq!(result.expires_at, expected);
    }

    #[tokio::test]
    async fn test_request_action_rejects_when_max_pending() {
        let store = SqliteApprovalStore::new_with_limit(2).await.unwrap();
        store.request_action(sample_action()).await.unwrap();
        store.request_action(sample_action()).await.unwrap();
        let result = store.request_action(sample_action()).await;
        assert!(matches!(result.unwrap_err(), ApprovalError::MaxPendingReached));
    }

    #[tokio::test]
    async fn test_request_action_creates_event() {
        let store = setup_store().await;
        let action = store.request_action(sample_action()).await.unwrap();
        let events = store.get_events_since(0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::Created);
        assert_eq!(events[0].action_id, action.id);
    }
}
```

---

## Чек-лист тестов

- [ ] Публичный API покрыт.
- [ ] Граничные случаи покрыты.
- [ ] Обработка ошибок покрыта.
- [ ] Инварианты проверены.
- [ ] Синхронизация проверена.
- [ ] Безопасность проверена.
- [ ] Тесты детерминированные.
- [ ] Тесты быстрые.
- [ ] Покрытие >80% (критичные) / >60% (остальные).
- [ ] CI проходит.

---

## Частые ошибки

### 1. Тесты без assert

**Плохо:** тест вызывает метод, но ничего не проверяет.

**Хорошо:** тест проверяет результат.

### 2. Тесты с unwrap

**Плохо:** `let x = foo().unwrap();` в тесте — если упадёт, непонятно почему.

**Хорошо:** `let x = foo().expect("foo should succeed");`

### 3. Тесты с sleep

**Плохо:** `tokio::time::sleep(Duration::from_secs(5)).await;`

**Хорошо:** использовать `tokio::time::pause()` и `advance()`.

### 4. Тесты с реальной сетью

**Плохо:** тест обращается к реальному API.

**Хорошо:** mock или локальный сервер.

### 5. Тесты с реальной БД

**Плохо:** тест использует production БД.

**Хорошо:** in-memory БД или временный файл.

### 6. Флакующие тесты

**Плохо:** тест иногда проходит, иногда нет.

**Хорошо:** детерминированный тест.

### 7. Тесты, которые ничего не тестируют

**Плохо:** `assert!(true);`

**Хорошо:** конкретная проверка.

---

## Следующие файлы

- `03-review.md` — ревью.
- `04-documentation.md` — документирование.
- `05-rollback.md` — откат.
- `06-cargo-workspace.md` — работа с workspace.
- `07-ci.md` — CI.
```

---

