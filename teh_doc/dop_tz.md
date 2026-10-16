# Дополнение к ТЗ: tensa-memory

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Дополнение (иерархическая память)
**Связано с:** ADR-021, Контракт 13

---

## Что добавляется

К существующему ТЗ tensa-memory добавляются:
- Core memory (SQLite) — всегда в контексте LLM.
- Recall memory (SQLite + FTS5) — история разговоров.
- Archival memory (Lance) — уже есть, дополняется tool calls.
- Memory Manager — координатор трёх уровней.
- Tool calls — для управления памятью агентом.

---

## Обновлённый публичный API

### Типы (дополнение)

```rust
/// Тип блока core memory.
pub enum BlockType {
    Persona,      // персона секретаря
    Human,        // информация о владельце
    Tasks,        // текущие задачи
    Preferences,  // предпочтения владельца
}

/// Источник изменения памяти.
pub enum MemoryOrigin {
    Llm,
    User,
    System,
}

/// Операция с памятью.
pub enum MemoryOperation {
    Replace,
    Append,
}

/// Блок core memory.
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

/// Запись в recall memory.
pub struct RecallMessage {
    pub id: Uuid,
    pub role: Role,           // user, assistant, system
    pub content: String,
    pub timestamp: i64,
    pub session_id: Uuid,
    pub node_id: String,
    pub metadata: serde_json::Value,
}

/// Запись в archival memory.
pub struct ArchivalEntry {
    pub id: Uuid,
    pub content: String,
    pub embedding: Vec<f32>,  // 384
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub node_id: String,
    pub metadata: serde_json::Value,
}

/// Результат поиска в памяти.
pub struct MemorySearchResult {
    pub id: Uuid,
    pub content: String,
    pub score: f32,
    pub source: MemorySource,
    pub timestamp: i64,
}

pub enum MemorySource {
    Core,
    Recall,
    Archival,
}
```

### Трейты (дополнение)

```rust
/// Core memory — всегда в контексте LLM.
#[async_trait]
pub trait CoreMemoryStore: Send + Sync {
    async fn get_block(&self, block: BlockType) -> Result<CoreMemoryBlock>;
    async fn get_all_blocks(&self) -> Result<Vec<CoreMemoryBlock>>;
    async fn replace_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;
    async fn append_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;
    async fn get_history(
        &self,
        block: BlockType,
        limit: usize,
    ) -> Result<Vec<CoreMemoryHistory>>;
    async fn get_config(&self) -> Result<CoreMemoryConfig>;
    async fn update_config(&self, config: CoreMemoryConfig) -> Result<()>;
    async fn validate_size(&self, block: BlockType, value: &str) -> Result<()>;
}

/// Recall memory — история разговоров.
#[async_trait]
pub trait RecallMemoryStore: Send + Sync {
    /// Добавить сообщение в историю.
    async fn append_message(&self, message: RecallMessage) -> Result<()>;

    /// Поиск по истории (FTS5).
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;

    /// Получить историю сессии.
    async fn get_session(
        &self,
        session_id: Uuid,
        limit: usize,
    ) -> Result<Vec<RecallMessage>>;

    /// Получить последние N сообщений.
    async fn get_recent(&self, limit: usize) -> Result<Vec<RecallMessage>>;

    /// Удалить старые сообщения (TTL).
    async fn cleanup_old(&self, older_than_days: u32) -> Result<usize>;
}

/// Archival memory — долговременное хранилище.
#[async_trait]
pub trait ArchivalMemoryStore: Send + Sync {
    /// Добавить запись.
    async fn insert(&self, entry: ArchivalEntry) -> Result<Uuid>;

    /// Поиск по архиву (семантический).
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;

    /// Поиск по тегам.
    async fn search_by_tags(
        &self,
        tags: &[String],
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;

    /// Удалить запись.
    async fn delete(&self, id: Uuid) -> Result<()>;
}
```

### Memory Manager

```rust
/// Координатор трёх уровней памяти.
#[async_trait]
pub trait MemoryManager: Send + Sync {
    // Core memory
    async fn core_get(&self, block: BlockType) -> Result<CoreMemoryBlock>;
    async fn core_get_all(&self) -> Result<Vec<CoreMemoryBlock>>;
    async fn core_replace(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;
    async fn core_append(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;

    // Recall memory
    async fn recall_append(&self, message: RecallMessage) -> Result<()>;
    async fn recall_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;
    async fn recall_recent(&self, limit: usize) -> Result<Vec<RecallMessage>>;

    // Archival memory
    async fn archival_insert(&self, entry: ArchivalEntry) -> Result<Uuid>;
    async fn archival_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;

    // Общее
    async fn search_all(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;

    /// Получить контекст для LLM (core + recent recall).
    async fn get_context(&self) -> Result<MemoryContext>;
}

/// Контекст памяти для LLM.
pub struct MemoryContext {
    pub core_blocks: Vec<CoreMemoryBlock>,
    pub recent_messages: Vec<RecallMessage>,
    pub total_tokens: u32,
}
```

### Реализация

```rust
pub struct DefaultMemoryManager {
    core: Arc<dyn CoreMemoryStore>,
    recall: Arc<dyn RecallMemoryStore>,
    archival: Arc<dyn ArchivalMemoryStore>,
    config: MemoryManagerConfig,
    node_id: String,
}

impl DefaultMemoryManager {
    pub async fn new(
        core: Arc<dyn CoreMemoryStore>,
        recall: Arc<dyn RecallMemoryStore>,
        archival: Arc<dyn ArchivalMemoryStore>,
        config: MemoryManagerConfig,
        node_id: String,
    ) -> Result<Self>;

    /// Инициализация при первом запуске.
    pub async fn init_default(&self) -> Result<()>;
}

pub struct MemoryManagerConfig {
    /// Максимум токенов в core memory.
    pub max_core_tokens: u32,        // 8000

    /// Максимум токенов на блок core memory.
    pub max_block_tokens: u32,       // 2000

    /// Размер message buffer (последних сообщений).
    pub message_buffer_size: usize,  // 20

    /// Максимум токенов в контексте (core + buffer).
    pub max_context_tokens: u32,     // 16000

    /// TTL recall memory (дни).
    pub recall_ttl_days: u32,        // 365

    /// Размер эмбеддингов archival.
    pub archival_embedding_dim: usize, // 384
}
```

---

## Обновлённый раздел «Структура»

```
crates/tensa-memory/
├── Cargo.toml
├── schema.sql                       # SQLite для core + recall
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── models.rs
│   ├── store.rs                     # трейт MemoryStore (Lance)
│   ├── lance_store.rs               # реализация Lance
│   ├── core_memory.rs               # НОВОЕ: CoreMemoryStore
│   ├── recall_memory.rs             # НОВОЕ: RecallMemoryStore
│   ├── archival_memory.rs           # НОВОЕ: ArchivalMemoryStore
│   ├── manager.rs                   # НОВОЕ: MemoryManager
│   ├── embedder.rs
│   ├── fastembed.rs
│   ├── chunker.rs
│   ├── cache.rs
│   ├── search.rs
│   ├── tools.rs                     # НОВОЕ: tool calls
│   └── error.rs
└── tests/
    ├── core_memory.rs               # НОВОЕ
    ├── recall_memory.rs             # НОВОЕ
    ├── archival_memory.rs           # НОВОЕ
    ├── manager.rs                   # НОВОЕ
    └── integration.rs
```

---

## Tool calls для памяти

### core_memory_replace

**Назначение:** заменить блок core memory.

**Параметры:**

```json
{
  "block": "persona",
  "value": "Новое содержимое блока"
}
```

**Реализация:**

```rust
pub async fn core_memory_replace(
    manager: &dyn MemoryManager,
    block: BlockType,
    value: String,
    origin: MemoryOrigin,
) -> Result<CoreMemoryBlock> {
    manager.core_replace(block, value, origin).await
}
```

### core_memory_append

**Назначение:** добавить в конец блока.

**Параметры:**

```json
{
  "block": "human",
  "value": "\nДополнительная информация"
}
```

**Реализация:**

```rust
pub async fn core_memory_append(
    manager: &dyn MemoryManager,
    block: BlockType,
    value: String,
    origin: MemoryOrigin,
) -> Result<CoreMemoryBlock> {
    manager.core_append(block, value, origin).await
}
```

### recall_memory_search

**Назначение:** поиск по истории разговоров.

**Параметры:**

```json
{
  "query": "смета за апрель",
  "limit": 10
}
```

**Реализация:**

```rust
pub async fn recall_memory_search(
    manager: &dyn MemoryManager,
    query: String,
    limit: usize,
) -> Result<Vec<MemorySearchResult>> {
    manager.recall_search(&query, limit).await
}
```

### archival_memory_search

**Назначение:** поиск по архиву.

**Параметры:**

```json
{
  "query": "договор с Иваном",
  "limit": 10
}
```

**Реализация:**

```rust
pub async fn archival_memory_search(
    manager: &dyn MemoryManager,
    query: String,
    limit: usize,
) -> Result<Vec<MemorySearchResult>> {
    manager.archival_search(&query, limit).await
}
```

### archival_memory_insert

**Назначение:** добавить в архив.

**Параметры:**

```json
{
  "content": "Иван предпочитает получать сметы в PDF",
  "tags": ["иван", "предпочтения", "почта"]
}
```

**Реализация:**

```rust
pub async fn archival_memory_insert(
    manager: &dyn MemoryManager,
    content: String,
    tags: Vec<String>,
) -> Result<Uuid> {
    let embedding = manager.embed(&content).await?;
    let entry = ArchivalEntry {
        id: Uuid::new_v4(),
        content,
        embedding,
        tags,
        source: None,
        created_at: Utc::now().timestamp(),
        node_id: manager.node_id().to_string(),
        metadata: serde_json::json!({}),
    };
    manager.archival_insert(entry).await
}
```

---

## Контекст для LLM

Функция get_context формирует контекст:

```rust
pub struct MemoryContext {
    pub core_blocks: Vec<CoreMemoryBlock>,
    pub recent_messages: Vec<RecallMessage>,
    pub total_tokens: u32,
}
```

**Что входит:**
1. Core memory — все блоки (persona, human, tasks, preferences).
2. Recent recall — последние N сообщений (message buffer).
3. Retrieved memories — если есть запрос, добавляются результаты поиска.

**Что НЕ входит:**
- Вся история recall (только recent + retrieved).
- Весь архив archival (только retrieved).

**Ограничение:** max_context_tokens (16000 по умолчанию).

---

## Инициализация при первом запуске

DefaultMemoryManager::init_default создаёт:

**Core memory blocks:**
- persona — с описанием секретаря.
- human — с информацией о владельце.
- tasks — пустой.
- preferences — пустой.

**Recall memory:**
- Пустая.

**Archival memory:**
- Пустая.

**Значения по умолчанию:**
- owner_name: "Владелец".
- owner_title: "Шеф".
- language: "ru".
- timezone: "Europe/Moscow".

---

## Тесты (дополнение)

### Unit-тесты

- CoreMemoryStore — get, replace, append, history.
- RecallMemoryStore — append, search, session, cleanup.
- ArchivalMemoryStore — insert, search, delete.
- MemoryManager — get_context, search_all.
- Валидация размеров (max_tokens_per_block).

### Integration-тесты

- Инициализация при первом запуске.
- Замена блока core memory.
- Добавление в блок core memory.
- Поиск по recall.
- Поиск по archival.
- Общий поиск.
- Синхронизация между узлами.
- Конфликты (vector_clock).

### Error-тесты

- Превышение размера блока.
- Несуществующий блок.
- Ошибка БД.
- Ошибка эмбеддинга.
- Конфликт версий.

---

## Связанные ADR и контракты

- ADR-001: Rust как язык ядра.
- ADR-004: Lance для векторного поиска.
- ADR-005: fastembed-rs для эмбеддингов.
- ADR-012: gRPC (tonic) для синхронизации.
- ADR-013: On-demand синхронизация.
- ADR-020: Документы.
- ADR-021: Иерархическая память (Letta-style).
- Контракт 2: Память (Lance).
- Контракт 8: gRPC-контракты.
- Контракт 9: uniffi UDL.
- Контракт 13: Core Memory (SQLite).
