

\---



\## Файл 3: Дополнение к ТЗ tensa-memory



```markdown

\# Дополнение к ТЗ: tensa-memory



\*\*Версия:\*\* 0.1.0

\*\*Дата:\*\* 2026-10-06

\*\*Статус:\*\* Дополнение (иерархическая память)

\*\*Связано с:\*\* ADR-021, Контракт 13



\---



\## Что добавляется



К существующему ТЗ tensa-memory добавляются:

\- Core memory (SQLite) — всегда в контексте LLM.

\- Recall memory (SQLite + FTS5) — история разговоров.

\- Archival memory (Lance) — уже есть, дополняется tool calls.

\- Memory Manager — координатор трёх уровней.

\- Tool calls — для управления памятью агентом.



\---



\## Обновлённый публичный API



\### Типы (дополнение)



```rust

/// Тип блока core memory.

pub enum BlockType {

&#x20;   Persona,      // персона секретаря

&#x20;   Human,        // информация о владельце

&#x20;   Tasks,        // текущие задачи

&#x20;   Preferences,  // предпочтения владельца

}



/// Источник изменения памяти.

pub enum MemoryOrigin {

&#x20;   Llm,

&#x20;   User,

&#x20;   System,

}



/// Операция с памятью.

pub enum MemoryOperation {

&#x20;   Replace,

&#x20;   Append,

}



/// Блок core memory.

pub struct CoreMemoryBlock {

&#x20;   pub id: Uuid,

&#x20;   pub block: BlockType,

&#x20;   pub value: String,

&#x20;   pub size\_chars: i32,

&#x20;   pub size\_tokens: i32,

&#x20;   pub created\_at: i64,

&#x20;   pub updated\_at: i64,

&#x20;   pub updated\_by: MemoryOrigin,

&#x20;   pub updated\_on\_node: String,

&#x20;   pub vector\_clock: VectorClock,

}



/// Запись в recall memory.

pub struct RecallMessage {

&#x20;   pub id: Uuid,

&#x20;   pub role: Role,           // user, assistant, system

&#x20;   pub content: String,

&#x20;   pub timestamp: i64,

&#x20;   pub session\_id: Uuid,

&#x20;   pub node\_id: String,

&#x20;   pub metadata: serde\_json::Value,

}



/// Запись в archival memory.

pub struct ArchivalEntry {

&#x20;   pub id: Uuid,

&#x20;   pub content: String,

&#x20;   pub embedding: Vec<f32>,  // 384

&#x20;   pub tags: Vec<String>,

&#x20;   pub source: Option<String>,

&#x20;   pub created\_at: i64,

&#x20;   pub node\_id: String,

&#x20;   pub metadata: serde\_json::Value,

}



/// Результат поиска в памяти.

pub struct MemorySearchResult {

&#x20;   pub id: Uuid,

&#x20;   pub content: String,

&#x20;   pub score: f32,

&#x20;   pub source: MemorySource,

&#x20;   pub timestamp: i64,

}



pub enum MemorySource {

&#x20;   Core,

&#x20;   Recall,

&#x20;   Archival,

}

```



\### Трейты (дополнение)



```rust

/// Core memory — всегда в контексте LLM.

\#\[async\_trait]

pub trait CoreMemoryStore: Send + Sync {

&#x20;   async fn get\_block(\&self, block: BlockType) -> Result<CoreMemoryBlock>;

&#x20;   async fn get\_all\_blocks(\&self) -> Result<Vec<CoreMemoryBlock>>;

&#x20;   async fn replace\_block(

&#x20;       \&self,

&#x20;       block: BlockType,

&#x20;       value: String,

&#x20;       origin: MemoryOrigin,

&#x20;   ) -> Result<CoreMemoryBlock>;

&#x20;   async fn append\_block(

&#x20;       \&self,

&#x20;       block: BlockType,

&#x20;       value: String,

&#x20;       origin: MemoryOrigin,

&#x20;   ) -> Result<CoreMemoryBlock>;

&#x20;   async fn get\_history(

&#x20;       \&self,

&#x20;       block: BlockType,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<CoreMemoryHistory>>;

&#x20;   async fn get\_config(\&self) -> Result<CoreMemoryConfig>;

&#x20;   async fn update\_config(\&self, config: CoreMemoryConfig) -> Result<()>;

&#x20;   async fn validate\_size(\&self, block: BlockType, value: \&str) -> Result<()>;

}



/// Recall memory — история разговоров.

\#\[async\_trait]

pub trait RecallMemoryStore: Send + Sync {

&#x20;   /// Добавить сообщение в историю.

&#x20;   async fn append\_message(\&self, message: RecallMessage) -> Result<()>;



&#x20;   /// Поиск по истории (FTS5).

&#x20;   async fn search(

&#x20;       \&self,

&#x20;       query: \&str,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<MemorySearchResult>>;



&#x20;   /// Получить историю сессии.

&#x20;   async fn get\_session(

&#x20;       \&self,

&#x20;       session\_id: Uuid,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<RecallMessage>>;



&#x20;   /// Получить последние N сообщений.

&#x20;   async fn get\_recent(\&self, limit: usize) -> Result<Vec<RecallMessage>>;



&#x20;   /// Удалить старые сообщения (TTL).

&#x20;   async fn cleanup\_old(\&self, older\_than\_days: u32) -> Result<usize>;

}



/// Archival memory — долговременное хранилище.

\#\[async\_trait]

pub trait ArchivalMemoryStore: Send + Sync {

&#x20;   /// Добавить запись.

&#x20;   async fn insert(\&self, entry: ArchivalEntry) -> Result<Uuid>;



&#x20;   /// Поиск по архиву (семантический).

&#x20;   async fn search(

&#x20;       \&self,

&#x20;       query: \&str,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<MemorySearchResult>>;



&#x20;   /// Поиск по тегам.

&#x20;   async fn search\_by\_tags(

&#x20;       \&self,

&#x20;       tags: \&\[String],

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<MemorySearchResult>>;



&#x20;   /// Удалить запись.

&#x20;   async fn delete(\&self, id: Uuid) -> Result<()>;

}

```



\### Memory Manager



```rust

/// Координатор трёх уровней памяти.

\#\[async\_trait]

pub trait MemoryManager: Send + Sync {

&#x20;   // Core memory

&#x20;   async fn core\_get(\&self, block: BlockType) -> Result<CoreMemoryBlock>;

&#x20;   async fn core\_get\_all(\&self) -> Result<Vec<CoreMemoryBlock>>;

&#x20;   async fn core\_replace(

&#x20;       \&self,

&#x20;       block: BlockType,

&#x20;       value: String,

&#x20;       origin: MemoryOrigin,

&#x20;   ) -> Result<CoreMemoryBlock>;

&#x20;   async fn core\_append(

&#x20;       \&self,

&#x20;       block: BlockType,

&#x20;       value: String,

&#x20;       origin: MemoryOrigin,

&#x20;   ) -> Result<CoreMemoryBlock>;



&#x20;   // Recall memory

&#x20;   async fn recall\_append(\&self, message: RecallMessage) -> Result<()>;

&#x20;   async fn recall\_search(

&#x20;       \&self,

&#x20;       query: \&str,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<MemorySearchResult>>;

&#x20;   async fn recall\_recent(\&self, limit: usize) -> Result<Vec<RecallMessage>>;



&#x20;   // Archival memory

&#x20;   async fn archival\_insert(\&self, entry: ArchivalEntry) -> Result<Uuid>;

&#x20;   async fn archival\_search(

&#x20;       \&self,

&#x20;       query: \&str,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<MemorySearchResult>>;



&#x20;   // Общее

&#x20;   async fn search\_all(

&#x20;       \&self,

&#x20;       query: \&str,

&#x20;       limit: usize,

&#x20;   ) -> Result<Vec<MemorySearchResult>>;



&#x20;   /// Получить контекст для LLM (core + recent recall).

&#x20;   async fn get\_context(\&self) -> Result<MemoryContext>;

}



/// Контекст памяти для LLM.

pub struct MemoryContext {

&#x20;   pub core\_blocks: Vec<CoreMemoryBlock>,

&#x20;   pub recent\_messages: Vec<RecallMessage>,

&#x20;   pub total\_tokens: u32,

}

```



\### Реализация



```rust

pub struct DefaultMemoryManager {

&#x20;   core: Arc<dyn CoreMemoryStore>,

&#x20;   recall: Arc<dyn RecallMemoryStore>,

&#x20;   archival: Arc<dyn ArchivalMemoryStore>,

&#x20;   config: MemoryManagerConfig,

&#x20;   node\_id: String,

}



impl DefaultMemoryManager {

&#x20;   pub async fn new(

&#x20;       core: Arc<dyn CoreMemoryStore>,

&#x20;       recall: Arc<dyn RecallMemoryStore>,

&#x20;       archival: Arc<dyn ArchivalMemoryStore>,

&#x20;       config: MemoryManagerConfig,

&#x20;       node\_id: String,

&#x20;   ) -> Result<Self>;



&#x20;   /// Инициализация при первом запуске.

&#x20;   pub async fn init\_default(\&self) -> Result<()>;

}



pub struct MemoryManagerConfig {

&#x20;   /// Максимум токенов в core memory.

&#x20;   pub max\_core\_tokens: u32,        // 8000



&#x20;   /// Максимум токенов на блок core memory.

&#x20;   pub max\_block\_tokens: u32,       // 2000



&#x20;   /// Размер message buffer (последних сообщений).

&#x20;   pub message\_buffer\_size: usize,  // 20



&#x20;   /// Максимум токенов в контексте (core + buffer).

&#x20;   pub max\_context\_tokens: u32,     // 16000



&#x20;   /// TTL recall memory (дни).

&#x20;   pub recall\_ttl\_days: u32,        // 365



&#x20;   /// Размер эмбеддингов archival.

&#x20;   pub archival\_embedding\_dim: usize, // 384

}

```



\---



\## Обновлённый раздел «Структура»



```

crates/tensa-memory/

├── Cargo.toml

├── schema.sql                       # SQLite для core + recall

├── src/

│   ├── lib.rs

│   ├── config.rs

│   ├── models.rs

│   ├── store.rs                     # трейт MemoryStore (Lance)

│   ├── lance\_store.rs               # реализация Lance

│   ├── core\_memory.rs               # НОВОЕ: CoreMemoryStore

│   ├── recall\_memory.rs             # НОВОЕ: RecallMemoryStore

│   ├── archival\_memory.rs           # НОВОЕ: ArchivalMemoryStore

│   ├── manager.rs                   # НОВОЕ: MemoryManager

│   ├── embedder.rs

│   ├── fastembed.rs

│   ├── chunker.rs

│   ├── cache.rs

│   ├── search.rs

│   ├── tools.rs                     # НОВОЕ: tool calls

│   └── error.rs

└── tests/

&#x20;   ├── core\_memory.rs               # НОВОЕ

&#x20;   ├── recall\_memory.rs             # НОВОЕ

&#x20;   ├── archival\_memory.rs           # НОВОЕ

&#x20;   ├── manager.rs                   # НОВОЕ

&#x20;   └── integration.rs

```



\---



\## Tool calls для памяти



\### core\_memory\_replace



\*\*Назначение:\*\* заменить блок core memory.



\*\*Параметры:\*\*

```json

{

&#x20; "block": "persona",

&#x20; "value": "Новое содержимое блока"

}

```



\*\*Реализация:\*\*

```rust

pub async fn core\_memory\_replace(

&#x20;   manager: \&dyn MemoryManager,

&#x20;   block: BlockType,

&#x20;   value: String,

&#x20;   origin: MemoryOrigin,

) -> Result<CoreMemoryBlock> {

&#x20;   manager.core\_replace(block, value, origin).await

}

```



\### core\_memory\_append



\*\*Назначение:\*\* добавить в конец блока.



\*\*Параметры:\*\*

```json

{

&#x20; "block": "human",

&#x20; "value": "\\nДополнительная информация"

}

```



\*\*Реализация:\*\*

```rust

pub async fn core\_memory\_append(

&#x20;   manager: \&dyn MemoryManager,

&#x20;   block: BlockType,

&#x20;   value: String,

&#x20;   origin: MemoryOrigin,

) -> Result<CoreMemoryBlock> {

&#x20;   manager.core\_append(block, value, origin).await

}

```



\### recall\_memory\_search



\*\*Назначение:\*\* поиск по истории разговоров.



\*\*Параметры:\*\*

```json

{

&#x20; "query": "смета за апрель",

&#x20; "limit": 10

}

```



\*\*Реализация:\*\*

```rust

pub async fn recall\_memory\_search(

&#x20;   manager: \&dyn MemoryManager,

&#x20;   query: String,

&#x20;   limit: usize,

) -> Result<Vec<MemorySearchResult>> {

&#x20;   manager.recall\_search(\&query, limit).await

}

```



\### archival\_memory\_search



\*\*Назначение:\*\* поиск по архиву.



\*\*Параметры:\*\*

```json

{

&#x20; "query": "договор с Иваном",

&#x20; "limit": 10

}

```



\*\*Реализация:\*\*

```rust

pub async fn archival\_memory\_search(

&#x20;   manager: \&dyn MemoryManager,

&#x20;   query: String,

&#x20;   limit: usize,

) -> Result<Vec<MemorySearchResult>> {

&#x20;   manager.archival\_search(\&query, limit).await

}

```



\### archival\_memory\_insert



\*\*Назначение:\*\* добавить в архив.



\*\*Параметры:\*\*

```json

{

&#x20; "content": "Иван предпочитает получать сметы в PDF",

&#x20; "tags": \["иван", "предпочтения", "почта"]

}

```



\*\*Реализация:\*\*

```rust

pub async fn archival\_memory\_insert(

&#x20;   manager: \&dyn MemoryManager,

&#x20;   content: String,

&#x20;   tags: Vec<String>,

) -> Result<Uuid> {

&#x20;   let embedding = manager.embed(\&content).await?;

&#x20;   let entry = ArchivalEntry {

&#x20;       id: Uuid::new\_v4(),

&#x20;       content,

&#x20;       embedding,

&#x20;       tags,

&#x20;       source: None,

&#x20;       created\_at: Utc::now().timestamp(),

&#x20;       node\_id: manager.node\_id().to\_string(),

&#x20;       metadata: serde\_json::json!({}),

&#x20;   };

&#x20;   manager.archival\_insert(entry).await

}

```



\---



\## Контекст для LLM



Функция get\_context формирует контекст:



```rust

pub struct MemoryContext {

&#x20;   pub core\_blocks: Vec<CoreMemoryBlock>,

&#x20;   pub recent\_messages: Vec<RecallMessage>,

&#x20;   pub total\_tokens: u32,

}

```



\*\*Что входит:\*\*

1\. Core memory — все блоки (persona, human, tasks, preferences).

2\. Recent recall — последние N сообщений (message buffer).

3\. Retrieved memories — если есть запрос, добавляются результаты поиска.



\*\*Что НЕ входит:\*\*

\- Вся история recall (только recent + retrieved).

\- Весь архив archival (только retrieved).



\*\*Ограничение:\*\* max\_context\_tokens (16000 по умолчанию).



\---



\## Инициализация при первом запуске



DefaultMemoryManager::init\_default создаёт:



\*\*Core memory blocks:\*\*

\- persona — с описанием секретаря.

\- human — с информацией о владельце.

\- tasks — пустой.

\- preferences — пустой.



\*\*Recall memory:\*\*

\- Пустая.



\*\*Archival memory:\*\*

\- Пустая.



\*\*Значения по умолчанию:\*\*

\- owner\_name: "Владелец".

\- owner\_title: "Шеф".

\- language: "ru".

\- timezone: "Europe/Moscow".



\---



\## Тесты (дополнение)



\### Unit-тесты



\- CoreMemoryStore — get, replace, append, history.

\- RecallMemoryStore — append, search, session, cleanup.

\- ArchivalMemoryStore — insert, search, delete.

\- MemoryManager — get\_context, search\_all.

\- Валидация размеров (max\_tokens\_per\_block).



\### Integration-тесты



\- Инициализация при первом запуске.

\- Замена блока core memory.

\- Добавление в блок core memory.

\- Поиск по recall.

\- Поиск по archival.

\- Общий поиск.

\- Синхронизация между узлами.

\- Конфликты (vector\_clock).



\### Error-тесты



\- Превышение размера блока.

\- Несуществующий блок.

\- Ошибка БД.

\- Ошибка эмбеддинга.

\- Конфликт версий.



\---



\## Связанные ADR и контракты



\- ADR-001: Rust как язык ядра.

\- ADR-004: Lance для векторного поиска.

\- ADR-005: fastembed-rs для эмбеддингов.

\- ADR-012: gRPC (tonic) для синхронизации.

\- ADR-013: On-demand синхронизация.

\- ADR-020: Документы.

\- ADR-021: Иерархическая память (Letta-style).

\- Контракт 2: Память (Lance).

\- Контракт 8: gRPC-контракты.

\- Контракт 9: uniffi UDL.

\- Контракт 13: Core Memory (SQLite).

```



\---





