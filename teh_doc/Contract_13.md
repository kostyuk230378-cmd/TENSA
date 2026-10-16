# Контракт: Core Memory (SQLite)

**Версия:** 0.1.0
**Статус:** Утверждено

---

## Назначение

Core Memory — первый уровень иерархической памяти Tensa
(ADR-021). Хранит структурированные блоки, которые **всегда
находятся в контексте LLM**:
- persona — персона секретаря.
- human — информация о владельце.
- tasks — текущие задачи.
- preferences — предпочтения владельца.

Core Memory **редактируется агентом** через tool calls
(core_memory_replace, core_memory_append).

Принципы:
- SQLite для хранения.
- Блоки фиксированного размера.
- Всегда в контексте LLM.
- Редактируется агентом.
- Синхронизируется между узлами.

---

## Расположение

- **Файл:** `~/.tensa/memory.db` (SQLite).
- **Схема:** `crates/tensa-memory/schema.sql`.
- **Rust-структуры:** `crates/tensa-memory/src/core_memory.rs`.

---

## Таблицы

### core_memory_blocks — блоки core memory

| Поле | Тип | Описание |
|---|---|---|
| id | TEXT (UUID) | Уникальный идентификатор блока |
| block | TEXT | Имя блока: persona, human, tasks, preferences |
| value | TEXT | Содержимое блока |
| size_chars | INTEGER | Размер в символах |
| size_tokens | INTEGER | Размер в токенах (оценка) |
| created_at | INTEGER | Unix timestamp создания |
| updated_at | INTEGER | Unix timestamp обновления |
| updated_by | TEXT | llm, user, system |
| updated_on_node | TEXT | node_id узла обновления |
| vector_clock | TEXT (JSON) | Векторные часы |

**Индексы:**
- idx_core_memory_block — по block (уникальный).

### core_memory_history — история изменений

| Поле | Тип | Описание |
|---|---|---|
| id | INTEGER | Автоинкремент |
| block | TEXT | Имя блока |
| old_value | TEXT | Старое значение |
| new_value | TEXT | Новое значение |
| changed_at | INTEGER | Unix timestamp |
| changed_by | TEXT | llm, user, system |
| changed_on_node | TEXT | node_id |
| operation | TEXT | replace, append |

**Индексы:**
- idx_core_memory_history_block — по block.
- idx_core_memory_history_changed_at — по changed_at.

### core_memory_config — конфигурация

| Поле | Тип | Описание |
|---|---|---|
| key | TEXT | Ключ |
| value | TEXT (JSON) | Значение |

**Пример:**

```json
{
  "max_tokens_per_block": 2000,
  "max_total_tokens": 8000,
  "blocks": ["persona", "human", "tasks", "preferences"]
}
```

---

## Rust-структуры

```rust
pub struct CoreMemoryBlock {
    pub id: Uuid,
    pub block: BlockType,
    pub value: String,
    pub size_chars: i32,
    pub size_tokens: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub updated_by: MemoryOrigin,
    pub updated_on_node: String,
    pub vector_clock: VectorClock,
}

pub enum BlockType {
    Persona,
    Human,
    Tasks,
    Preferences,
}

pub enum MemoryOrigin {
    Llm,
    User,
    System,
}

pub struct CoreMemoryHistory {
    pub id: i64,
    pub block: BlockType,
    pub old_value: Option<String>,
    pub new_value: String,
    pub changed_at: i64,
    pub changed_by: MemoryOrigin,
    pub changed_on_node: String,
    pub operation: MemoryOperation,
}

pub enum MemoryOperation {
    Replace,
    Append,
}

pub struct CoreMemoryConfig {
    pub max_tokens_per_block: u32,
    pub max_total_tokens: u32,
    pub blocks: Vec<BlockType>,
}
```

---

## API (трейт)

```rust
#[async_trait]
pub trait CoreMemoryStore: Send + Sync {
    /// Получить блок по имени.
    async fn get_block(&self, block: BlockType) -> Result<CoreMemoryBlock>;

    /// Получить все блоки.
    async fn get_all_blocks(&self) -> Result<Vec<CoreMemoryBlock>>;

    /// Заменить блок (tool call: core_memory_replace).
    async fn replace_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;

    /// Добавить в блок (tool call: core_memory_append).
    async fn append_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;

    /// Получить историю изменений блока.
    async fn get_history(
        &self,
        block: BlockType,
        limit: usize,
    ) -> Result<Vec<CoreMemoryHistory>>;

    /// Получить конфигурацию.
    async fn get_config(&self) -> Result<CoreMemoryConfig>;

    /// Обновить конфигурацию.
    async fn update_config(&self, config: CoreMemoryConfig) -> Result<()>;

    /// Проверить, что блок не превышает лимит.
    async fn validate_size(&self, block: BlockType, value: &str) -> Result<()>;
}
```

---

## Реализация

```rust
pub struct SqliteCoreMemoryStore {
    pool: SqlitePool,
    config: CoreMemoryConfig,
    node_id: String,
}

impl SqliteCoreMemoryStore {
    pub async fn new(
        db_path: &Path,
        config: CoreMemoryConfig,
        node_id: String,
    ) -> Result<Self>;

    /// Инициализация при первом запуске.
    pub async fn init_default_blocks(&self) -> Result<()>;
}
```

---

## Файл памяти при первом запуске

При первом запуске init_default_blocks создаёт четыре блока:

**persona:**
```
Я — Tensa, персональный ИИ-секретарь. Моя задача — помогать
владельцу управлять документами, почтой, календарём, Telegram и
приложениями. Я работаю на русском и английском языках. Я не
выполняю действия без одобрения владельца. Я уважаю приватность и
не передаю данные третьим лицам.
```

**human:**
```
Владельца зовут {owner_name}. Он предпочитает обращение
«{owner_title}». Его язык: {language}. Часовой пояс: {timezone}.
```

**tasks:**
```
Активные задачи: {active_tasks}
```

**preferences:**
```
Владелец предпочитает: {preferences}
```

**Значения по умолчанию:**
- owner_name: "Владелец".
- owner_title: "Шеф".
- language: "ru".
- timezone: "Europe/Moscow".
- active_tasks: "Нет".
- preferences: "Нет".

---

## Tool calls

### core_memory_replace

**Назначение:** заменить содержимое блока.

**Параметры:**

```json
{
  "block": "persona",
  "value": "Новое содержимое блока"
}
```

**Поведение:**
1. Проверить, что блок существует.
2. Проверить размер (max_tokens_per_block).
3. Записать старое значение в core_memory_history.
4. Заменить значение в core_memory_blocks.
5. Обновить updated_at, updated_by, updated_on_node.
6. Вернуть обновлённый блок.

### core_memory_append

**Назначение:** добавить в конец блока.

**Параметры:**

```json
{
  "block": "human",
  "value": "\nДополнительная информация"
}
```

**Поведение:**
1. Проверить, что блок существует.
2. Проверить размер (max_tokens_per_block).
3. Записать старое значение в core_memory_history.
4. Добавить значение в конец блока.
5. Обновить updated_at, updated_by, updated_on_node.
6. Вернуть обновлённый блок.

---

## Ограничения

- **Максимум токенов на блок:** 2000 (настраивается).
- **Максимум токенов всего:** 8000 (настраивается).
- **Блоки фиксированные:** persona, human, tasks, preferences.
- **Не удаляются:** блоки можно только заменять/дополнять.

---

## Синхронизация

- Core memory синхронизируется через gRPC (ADR-012).
- События из core_memory_history синхронизируются.
- Конфликты разрешаются через vector_clock.
- LWW по updated_at при равных часах.
- On-demand sync (ADR-013).

---

## Связанные ADR и контракты

- ADR-001: Rust как язык ядра.
- ADR-004: Lance для векторного поиска.
- ADR-005: fastembed-rs для эмбеддингов.
- ADR-012: gRPC (tonic) для синхронизации.
- ADR-013: On-demand синхронизация.
- ADR-021: Иерархическая память (Letta-style).
- Контракт 2: Память (Lance).
- Контракт 8: gRPC-контракты.
- Контракт 9: uniffi UDL.
