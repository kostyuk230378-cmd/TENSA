# Tensa — Технические задания (ТЗ)

---

## ТЗ: tensa-approval

**Назначение:**
Крейт, реализующий ApprovalStore. Центральный элемент безопасности Tensa: хранит действия, требующие одобрения владельца, до их выполнения.

Принципы: авто-ответы запрещены, прямое исполнение сетевых действий из LLM запрещено, все действия через очередь, trusted_origin без одобрения но с логированием, event-sourced для синхронизации.

**Публичный API:**

Типы:

```rust
pub struct Action {
    pub id: Uuid,
    pub kind: ActionKind,
    pub payload: serde_json::Value,
    pub status: ActionStatus,
    pub created_at: i64,
    pub created_by: ActionOrigin,
    pub created_on_node: String,
    pub expires_at: i64,
    pub approved_at: Option<i64>,
    pub approved_by: Option<ActionOrigin>,
    pub approved_on_node: Option<String>,
    pub executed_at: Option<i64>,
    pub executed_on_node: Option<String>,
    pub result: Option<serde_json::Value>,
    pub operation_id: Uuid,
    pub vector_clock: VectorClock,
}

pub enum ActionKind { EmailSend, TelegramSend, WhatsAppSend, CalendarCreate, AppLaunch, FileOpen }
pub enum ActionStatus { Pending, Approved, Rejected, Executed, Failed, Expired }
pub enum ActionOrigin { Llm, User, Telegram, Email, System }

pub struct NewAction {
    pub kind: ActionKind,
    pub payload: serde_json::Value,
    pub created_by: ActionOrigin,
    pub expires_in_hours: Option<u64>,
}

pub struct ActionEvent {
    pub event_id: u64,
    pub action_id: Uuid,
    pub event_type: EventType,
    pub event_data: serde_json::Value,
    pub timestamp: i64,
    pub node_id: String,
    pub vector_clock: VectorClock,
}

pub enum EventType { Created, Approved, Rejected, Executed, Failed, Expired }
```

Трейт:

```rust
#[async_trait]
pub trait ApprovalStore: Send + Sync {
    async fn request_action(&self, action: NewAction) -> Result<Action>;
    async fn approve(&self, id: Uuid, approved_by: ActionOrigin) -> Result<Action>;
    async fn reject(&self, id: Uuid, rejected_by: ActionOrigin) -> Result<Action>;
    async fn execute(&self, id: Uuid) -> Result<Action>;
    async fn get_pending(&self) -> Result<Vec<Action>>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Action>>;
    async fn expire_old(&self) -> Result<usize>;
    async fn apply_event(&self, event: ActionEvent) -> Result<()>;
    async fn get_events_since(&self, last_event_id: u64) -> Result<Vec<ActionEvent>>;
}
```

Реализация: `SqliteApprovalStore`.

**Зависимости:**
Внутренние: `tensa-core`, `tensa-sync` (опционально).
Внешние: tokio, sqlx, serde, serde_json, uuid, chrono, thiserror, tracing, async-trait.

**Структура:**

```
crates/tensa-approval/
├── Cargo.toml
├── schema.sql
├── src/
│   ├── lib.rs
│   ├── models.rs
│   ├── events.rs
│   ├── store.rs
│   ├── sqlite.rs
│   ├── config.rs
│   ├── error.rs
│   └── vector_clock.rs
└── tests/
```

**Конфигурация:**

```rust
pub struct ApprovalConfig {
    pub action_ttl_hours: u64,           // 24
    pub auto_approve_trusted: bool,      // false
    pub allow_email_send: bool,          // true
    pub allow_telegram_send: bool,       // true
    pub allow_whatsapp_send: bool,       // false
    pub confirm_unknown_recipients: bool, // true
    pub max_pending_actions: usize,      // 1000
}
```

**Жизненный цикл:**

```
created → pending → approved → executed
                 ↘ rejected
                 ↘ expired (TTL 24 часа)
                 ↘ failed (ошибка)
```

**Тесты:**
Unit: request_action, approve, reject, execute, expire_old, apply_event, get_events_since, валидация.
Integration: полный цикл, отклонение, истечение, идемпотентность, конфликты, синхронизация, выключатели, незнакомые получатели.
Ошибки: несуществующий ID, уже одобрено, уже выполнено, истекло, превышен max_pending.

**Ограничения:** не выполнять действия, не отправлять сетевые запросы, не принимать решения, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-012, ADR-013, ADR-015, ADR-017, Контракт 1, Контракт 8, Контракт 9.

## ТЗ: tensa-sync

**Назначение:**
Крейт, реализующий синхронизацию между узлами Tensa (ADR-012). Обеспечивает: синхронизацию ApprovalStore (постоянно), заметок (по требованию), контактов (по требованию), heartbeat узлов, управление соединениями (ADR-015), управление Syncthing (ADR-011, ADR-018).

Принципы: gRPC (tonic), Protobuf, идемпотентность (operation_id), векторные часы, стриминг.

**Публичный API:**

Типы:

```rust
pub struct SyncConfig {
    pub grpc_port: u16,
    pub grpc_tls: bool,
    pub tailscale_enabled: bool,
    pub tailscale_hostname: String,
    pub syncthing_enabled: bool,
    pub syncthing_port: u16,
    pub syncthing_key: String,
    pub heartbeat_interval_seconds: u64,  // 30
    pub heartbeat_timeout_seconds: u64,   // 90
    pub sync_interval_seconds: u64,       // 60
}

pub struct SyncStatus {
    pub running: bool,
    pub last_sync_at: Option<i64>,
    pub peers_online: usize,
    pub peers_total: usize,
    pub syncthing_status: Option<SyncthingStatus>,
}

pub struct SyncthingStatus {
    pub running: bool,
    pub connections: usize,
    pub last_error: Option<String>,
}
```

Трейт:

```rust
#[async_trait]
pub trait SyncService: Send + Sync {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn status(&self) -> Result<SyncStatus>;
    async fn sync_approval_events(&self) -> Result<Vec<ActionEvent>>;
    async fn push_approval_event(&self, event: ActionEvent) -> Result<()>;
    async fn sync_notes(&self) -> Result<Vec<NoteEvent>>;
    async fn push_note_event(&self, event: NoteEvent) -> Result<()>;
    async fn sync_contacts(&self) -> Result<Vec<ContactEvent>>;
    async fn push_contact_event(&self, event: ContactEvent) -> Result<()>;
    async fn send_heartbeat(&self) -> Result<()>;
    async fn list_online_nodes(&self) -> Result<Vec<Node>>;
    async fn acquire_connection(&self, resource: &str, account_id: Option<&str>) -> Result<()>;
    async fn release_connection(&self, resource: &str, account_id: Option<&str>) -> Result<()>;
}
```

Реализация: `GrpcSyncService`.

**Зависимости:**
Внутренние: `tensa-core`, `tensa-approval`, `tensa-nodes`.
Внешние: tokio, tonic, prost, tonic-build (build), serde, serde_json, uuid, chrono, thiserror, tracing, async-trait, reqwest, rustls.

**Структура:**

```
crates/tensa-sync/
├── Cargo.toml
├── build.rs
├── proto/
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── status.rs
│   ├── service.rs
│   ├── grpc.rs
│   ├── server.rs
│   ├── client.rs
│   ├── heartbeat.rs
│   ├── connections.rs
│   ├── syncthing.rs
│   ├── vector_clock.rs
│   └── error.rs
└── tests/
```

**gRPC-сервисы:** SyncService, NodeService, DataService, VoiceService, QueryService.

**Heartbeat:** каждые 30 секунд, офлайн > 90 секунд.

**Управление соединениями:** node_connections, TTL 15 минут, очередь при конфликте, освобождение при офлайн.

**Syncthing:** REST API, конфигурация при первом запуске, мониторинг, обработка конфликтов.

**Тесты:**
Unit: SyncConfig, VectorClock, heartbeat, connections, syncthing.
Integration: синхронизация ApprovalStore/заметок/контактов, heartbeat, соединения, Syncthing.
Ошибки: узел недоступен, сеть разорвана, конфликт версий, превышен TTL, ошибка Syncthing.

**Ограничения:** не хранить данные, не выполнять действия, не принимать решения, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-011, ADR-012, ADR-013, ADR-014, ADR-015, ADR-018, Контракт 7, Контракт 8.

## ТЗ: tensa-router

**Назначение:**
Крейт, реализующий маршрутизацию запросов к LLM (локальная vs облачная) и выбор узла для инференса.

Принципы: локальная по умолчанию (Gemma 4 E2B), облачная для сложных задач (Gemma 4 26B A4B), автоматический fallback, динамический выбор узла, учёт бюджета, degraded mode.

**Публичный API:**

Типы:

```rust
pub struct RouterConfig {
    pub mode: BrainMode,
    pub local_model: LocalModelConfig,
    pub cloud_models: Vec<CloudModelConfig>,
    pub auto_fallback: bool,
    pub cloud_free_timeout_seconds: u64,    // 30
    pub cloud_free_max_retries: u32,        // 2
    pub cloud_fallback_to_local: bool,      // true
    pub openrouter_daily_limit_tokens: u64,
    pub openrouter_monthly_limit_usd: f64,
    pub openrouter_warn_at_percent: u8,     // 80
}

pub struct LocalModelConfig {
    pub model: String,
    pub tier: ModelTier,
    pub file: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub quantization: String,
    pub capabilities: Vec<String>,
}

pub struct CloudModelConfig {
    pub model: String,
    pub priority: u8,
    pub free: bool,
}

pub enum BrainMode { Local, Cloud, Auto }
pub enum ModelTier { XS, S, M, L }

pub struct QueryRequest {
    pub text: String,
    pub language: Language,
    pub context: HashMap<String, String>,
    pub use_cloud: bool,
    pub priority: QueryPriority,
}

pub enum QueryPriority { Low, Normal, High }

pub struct QueryResponse {
    pub text: String,
    pub action: Option<String>,
    pub action_payload: Option<serde_json::Value>,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Option<String>,
    pub degraded: bool,
    pub model_used: String,
    pub node_used: String,
    pub tokens_used: Option<u64>,
}

pub struct ToolCall {
    pub tool: String,
    pub params: serde_json::Value,
}
```

Трейт:

```rust
#[async_trait]
pub trait BrainRouter: Send + Sync {
    async fn query(&self, request: QueryRequest) -> Result<QueryResponse>;
    async fn query_stream(&self, request: QueryRequest) -> Result<impl Stream<Item = Result<QueryChunk>>>;
    async fn health(&self) -> Result<RouterHealth>;
    async fn get_budget_status(&self) -> Result<BudgetStatus>;
}

pub struct RouterHealth {
    pub local_available: bool,
    pub cloud_available: bool,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
    pub current_node: String,
}

pub struct BudgetStatus {
    pub daily_tokens_used: u64,
    pub daily_tokens_limit: u64,
    pub monthly_usd_used: f64,
    pub monthly_usd_limit: f64,
    pub warn: bool,
}
```

Реализация: `DefaultBrainRouter`.

**Зависимости:**
Внутренние: `tensa-core`, `tensa-llm`, `tensa-nodes`, `tensa-perf`, `tensa-sync`.
Внешние: tokio, reqwest, serde, serde_json, thiserror, tracing, async-trait, futures, rustls.

**Структура:**

```
crates/tensa-router/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── models.rs
│   ├── router.rs
│   ├── default.rs
│   ├── local.rs
│   ├── cloud.rs
│   ├── fallback.rs
│   ├── budget.rs
│   ├── node_selector.rs
│   ├── degraded.rs
│   └── error.rs
└── tests/
```

**Логика маршрутизации:**
1. Определить приоритет (Low → локальная, Normal → локальная, High → облако).
2. Определить доступность (локальный heavy → локальная, удалённый heavy → gRPC, только облако → облако).
3. Облачный fallback (бесплатная → платная → локальная; таймаут 30 сек, 2 попытки).
4. Degraded mode (если всё недоступно — только поиск).

**Бюджет OpenRouter:** дневной лимит токенов (100 000), месячный лимит USD (10.0), предупреждение при 80%, блокировка при 100%.

**Degraded mode:** только поиск (Lance), без генерации.

**Тесты:**
Unit: RouterConfig, BudgetTracker, node_selector, fallback, degraded.
Integration: локальный инференс, облачный, fallback, удалённый gRPC, бюджет, degraded.
Ошибки: модель не загружена, облако недоступно, API key невалиден, превышен лимит, узел офлайн.

**Ограничения:** не хранить историю, не выполнять действия, не логировать секреты, не превышать бюджет.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-003, ADR-012, ADR-013, Контракт 4, Контракт 8.

## ТЗ: tensa-memory

**Назначение:**
Крейт, реализующий память Tensa: векторный поиск (Lance), эмбеддинги (fastembed-rs), индексация документов, гибридный поиск, кэш поисковых запросов, **иерархическая память (core / recall / archival)**.

Принципы: файловая природа, версионирование, индекс Flat (ADR-004), модель multilingual-e5-small (ADR-005), префиксы query:/passage:, чанкинг 1000/200, **иерархическая память (ADR-021)**.

**Публичный API:**

Типы:

```rust
pub struct MemoryConfig {
    pub chunk_size: usize,           // 1000
    pub chunk_overlap: usize,        // 200
    pub embedding_model: String,
    pub embedding_dim: usize,        // 384
    pub index_type: IndexType,       // Flat
    pub max_file_size_mb: u64,       // 100
    pub search_cache_ttl_seconds: u64, // 3600
}

pub enum IndexType { Flat, IvfPq, Hnsw }

pub struct Document {
    pub id: Uuid,
    pub source_path: String,
    pub source_hash: String,
    pub title: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub created_at: i64,
    pub modified_at: i64,
    pub chunks: Vec<DocumentChunk>,
    pub metadata: serde_json::Value,
}

pub struct DocumentChunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub chunk_index: i32,
    pub source_path: String,
    pub source_hash: String,
    pub chunk_text: String,
    pub chunk_hash: String,
    pub embedding: Vec<f32>,
    pub metadata: serde_json::Value,
    pub account_id: Option<String>,
    pub indexed_at: i64,
    pub updated_at: i64,
}

pub struct DocumentMeta {
    pub document_id: Uuid,
    pub source_path: String,
    pub source_hash: String,
    pub title: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub created_at: i64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub chunk_count: i32,
    pub metadata: serde_json::Value,
}

pub struct SearchResult {
    pub document_id: Uuid,
    pub source_path: String,
    pub chunk_text: String,
    pub score: f32,
    pub indexed_at: i64,
}

pub struct SearchCacheEntry {
    pub query_hash: String,
    pub query_text: String,
    pub query_embedding: Vec<f32>,
    pub result_ids: Vec<Uuid>,
    pub created_at: i64,
    pub expires_at: i64,
}
```

Трейт:

```rust
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn index_document(&self, doc: Document) -> Result<Vec<DocumentChunk>>;
    async fn reindex_document(&self, document_id: Uuid) -> Result<Vec<DocumentChunk>>;
    async fn delete_document(&self, document_id: Uuid) -> Result<()>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    async fn get_document(&self, document_id: Uuid) -> Result<Option<DocumentMeta>>;
    async fn list_documents(&self, limit: usize, offset: usize) -> Result<Vec<DocumentMeta>>;
    async fn clear_cache(&self) -> Result<()>;
}
```

Реализация: `LanceMemoryStore`.

**Rust API (проверено 2026-10-15, апрув Архитектора):**

- lance 13.0 — `Dataset::open(uri)`; `Dataset::write(batches, dest, Some(WriteParams { mode }))` (`WriteMode::Create` / `Append` / `Overwrite`); поиск — `dataset.scan().nearest("embedding", &query, k).limit(Some(k), None)`; удаление — `dataset.delete(предикат по document_id)`; схема — `lance::deps::arrow_schema`; `embedding` — `DataType::FixedSizeList(Arc<Field>, 384)`.
- fastembed 7.1 — `TextInitOptions::new(EmbeddingModel::MultilingualE5Small)`; `TextEmbedding::try_new` синхронный (инициализация вне async-контекста); `embed(texts, Some(batch))` требует `&mut self` → `Arc<Mutex<TextEmbedding>>` внутри `spawn_blocking`; префиксы `query:`/`passage:` передаёт вызывающий.
- LanceDB-стиль (`Connection::open`, `create_table`) в Rust-крейте lance не существует.
- Компиляционная фиксация: `crates/tensa-memory/tests/api_surface.rs` (CI: `cargo test -p tensa-memory --features lance,fastembed`).

**Дополнение (ADR-021, Контракт 13):**

Добавляются:
- `CoreMemoryStore` — core memory (SQLite).
- `RecallMemoryStore` — recall memory (SQLite + FTS5).
- `ArchivalMemoryStore` — archival memory (Lance).
- `MemoryManager` — координатор трёх уровней.
- Tool calls: `core_memory_replace`, `core_memory_append`, `recall_memory_search`, `archival_memory_search`, `archival_memory_insert`.

**Зависимости:**
Внутренние: `tensa-core`, `tensa-tools/documents`.
Внешние: tokio, lance, fastembed, serde, serde_json, uuid, chrono, thiserror, tracing, async-trait, sha2, lru.

**Структура:**

```
crates/tensa-memory/
├── Cargo.toml
├── schema.sql                       # SQLite для core + recall
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── models.rs
│   ├── store.rs
│   ├── lance_store.rs
│   ├── core_memory.rs
│   ├── recall_memory.rs
│   ├── archival_memory.rs
│   ├── manager.rs
│   ├── embedder.rs
│   ├── fastembed.rs
│   ├── chunker.rs
│   ├── cache.rs
│   ├── search.rs
│   ├── tools.rs
│   └── error.rs
└── tests/
```

**Чанкинг:** 1000 символов, перекрытие 200, SHA-256, дедупликация.

**Эмбеддинги:** multilingual-e5-small (384), префиксы query:/passage:, кэширование.

**Поиск:** семантический (top-k по косинусному расстоянию), гибридный (RRF).

**Кэш:** TTL 1 час, LRU.

**Синхронизация:** Lance-файлы через Syncthing, on-demand через gRPC, LWW по updated_at.

**Тесты:**
Unit: MemoryConfig, chunker, embedder, cache, search, core_memory, recall_memory, archival_memory.
Integration: индексация, поиск, гибридный, переиндексация, удаление, кэш, синхронизация, конфликты, инициализация при первом запуске.
Ошибки: документ слишком большой, ошибка эмбеддинга, ошибка Lance, ошибка кэша, конфликт версий, превышение размера блока.

**Ограничения:** не извлекать текст, не выполнять OCR, не синхронизировать, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-004, ADR-005, ADR-012, ADR-013, ADR-018, ADR-020, ADR-021, Контракт 2, Контракт 8, Контракт 13.

## ТЗ: tensa-nodes

**Назначение:**
Крейт, реализующий реестр узлов (NodeRegistry). Хранит информацию о всех узлах Tensa в сети, их роли, ресурсы, heartbeat, активные соединения.

Принципы: SQLite, heartbeat, роли heavy/thin/auto, ресурсы, управление соединениями (ADR-015).

**Публичный API:**

Типы:

```rust
pub struct Node {
    pub node_id: Uuid,
    pub name: String,
    pub device_type: DeviceType,
    pub platform: Platform,
    pub role: NodeRole,
    pub tailscale_ip: String,
    pub grpc_port: u16,
    pub last_seen: i64,
    pub online: bool,
    pub ram_total_mb: u64,
    pub ram_available_mb: u64,
    pub cpu_cores: u32,
    pub cpu_usage_percent: f32,
    pub has_gpu: bool,
    pub has_npu: bool,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,
    pub thermal_status: ThermalStatus,
    pub resources: NodeResources,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct NodeResources {
    pub telegram: bool,
    pub email: bool,
    pub whatsapp: bool,
    pub llm_local: bool,
    pub llm_cloud: bool,
    pub memory_store: bool,
    pub approval_store: bool,
}

pub enum DeviceType { Desktop, Android }
pub enum Platform { Windows, Linux, Android }
pub enum NodeRole { Heavy, Thin, Auto }
pub enum ThermalStatus { Nominal, Fair, Serious, Critical }

pub struct Heartbeat {
    pub node_id: Uuid,
    pub ram_available_mb: u64,
    pub cpu_usage_percent: f32,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,
    pub thermal_status: ThermalStatus,
    pub timestamp: i64,
}

pub struct NodeConnection {
    pub id: i64,
    pub node_id: Uuid,
    pub resource: String,
    pub account_id: Option<String>,
    pub active: bool,
    pub acquired_at: i64,
    pub expires_at: i64,
}
```

Трейт:

```rust
#[async_trait]
pub trait NodeRegistry: Send + Sync {
    async fn register(&self, node: Node) -> Result<()>;
    async fn unregister(&self, node_id: Uuid) -> Result<()>;
    async fn update_heartbeat(&self, heartbeat: Heartbeat) -> Result<()>;
    async fn get(&self, node_id: Uuid) -> Result<Option<Node>>;
    async fn list_online(&self) -> Result<Vec<Node>>;
    async fn list_all(&self) -> Result<Vec<Node>>;
    async fn find_heavy(&self) -> Result<Option<Node>>;
    async fn find_with_resource(&self, resource: &str) -> Result<Vec<Node>>;
    async fn acquire_connection(&self, node_id: Uuid, resource: &str, account_id: Option<&str>) -> Result<()>;
    async fn release_connection(&self, node_id: Uuid, resource: &str, account_id: Option<&str>) -> Result<()>;
    async fn get_active_connection(&self, resource: &str, account_id: Option<&str>) -> Result<Option<Uuid>>;
    async fn cleanup_stale(&self, timeout_seconds: u64) -> Result<usize>;
}
```

Реализация: `SqliteNodeRegistry`.

**Зависимости:**
Внутренние: `tensa-core`.
Внешние: tokio, sqlx, serde, serde_json, uuid, chrono, thiserror, tracing, async-trait, sysinfo.

**Структура:**

```
crates/tensa-nodes/
├── Cargo.toml
├── schema.sql
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── models.rs
│   ├── registry.rs
│   ├── sqlite.rs
│   ├── heartbeat.rs
│   ├── connections.rs
│   ├── profile.rs
│   └── error.rs
└── tests/
```

**Heartbeat:** каждые 30 секунд, офлайн > 90 секунд.

**Роли:** heavy, thin, auto.

**Ресурсы:** telegram, email, whatsapp, llm_local, llm_cloud, memory_store, approval_store.

**Управление соединениями:** TTL 15 минут, очередь при конфликте, освобождение при офлайн.

**Профилирование:** RAM, CPU, GPU, NPU, батарея, thermal.

**Тесты:**
Unit: NodeConfig, Node, Heartbeat, connections, profile.
Integration: регистрация, heartbeat, онлайн→офлайн, поиск heavy, поиск с ресурсом, acquire/release, очередь, освобождение.
Ошибки: дубликат node_id, несуществующий node_id, превышен TTL, конфликт соединений.

**Ограничения:** не синхронизировать, не выполнять действия, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-012, ADR-013, ADR-014, ADR-015, ADR-016, Контракт 7, Контракт 8.

## ТЗ: tensa-tools

**Назначение:**
Мета-крейт, объединяющий инструменты секретаря: Telegram, Email, Calendar, Apps, Documents, OCR, WhatsApp (точка входа), WoL (точка входа).

Принципы: каждый инструмент — отдельный подкрейт, сетевые действия только через ApprovalStore, trusted origin без ApprovalStore но с логированием, аудит.

**Структура:**

```
crates/tensa-tools/
├── Cargo.toml
├── src/lib.rs
├── telegram/
│   ├── src/bot.rs, handlers.rs, callbacks.rs, error.rs
├── email/
│   ├── src/imap.rs, smtp.rs, autodetect.rs, oauth2.rs, connections.rs, error.rs
├── calendar/
│   ├── src/caldav.rs, guard.rs, error.rs
├── apps/
│   ├── src/scanner.rs, launcher.rs, windows.rs, linux.rs, android.rs, error.rs
├── documents/
│   ├── src/pdf.rs, docx.rs, xlsx.rs, pptx.rs, txt.rs, detect.rs, markdown.rs, error.rs
├── ocr/
│   ├── src/rusto.rs, oar.rs, cyrillic.rs, error.rs
├── whatsapp/
│   ├── src/client.rs, error.rs
└── wol/
    ├── src/magic.rs, error.rs
```

**Общий трейт:**

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value>;
    fn requires_approval(&self) -> bool;
}
```

**Реестр инструментов:**

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Arc<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn list(&self) -> Vec<String>;
}
```

**Аудит:** все действия в `~/.tensa/logs/tools.log`.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-011, ADR-014, ADR-015, ADR-017, ADR-018, ADR-019, ADR-020, Контракт 5, Контракт 6.

## ТЗ: tensa-llm

**Назначение:**
Крейт, реализующий локальный инференс LLM: FFI к llama.cpp, загрузка моделей GGUF, генерация текста, tool use, reasoning, управление версиями, автообновление с подтверждением.

Принципы: чистый Rust (ADR-001), безопасный C-FFI (ADR-003), CPU-инференс (ADR-004), int8/int4, SIMD, версия llama.cpp — git-зависимость, автообновление с подтверждением.

**Структура:**

```
crates/tensa-llm/
├── Cargo.toml
├── build.rs
├── llama-sys/
│   └── src/lib.rs
├── llama/
│   └── src/engine.rs, model.rs, context.rs, session.rs, sampling.rs, update.rs, error.rs
└── litert/
    └── src/lib.rs
```

**Публичный API:**

Типы:

```rust
pub struct LlmConfig {
    pub model_path: PathBuf,
    pub model_tier: ModelTier,
    pub context_size: u32,          // 4096
    pub threads: u32,               // 0 = auto
    pub batch_size: u32,            // 512
    pub gpu_layers: u32,            // 0 = CPU only
    pub temperature: f32,           // 0.7
    pub top_p: f32,                 // 0.95
    pub top_k: u32,                 // 40
    pub max_tokens: u32,            // 2048
    pub repeat_penalty: f32,        // 1.1
}

pub enum ModelTier { XS, S, M, L }

pub struct GenerateRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop: Vec<String>,
}

pub struct GenerateResponse {
    pub text: String,
    pub tokens_generated: u32,
    pub tokens_prompt: u32,
    pub time_ms: u64,
    pub finish_reason: FinishReason,
}

pub enum FinishReason { Stop, Length, Error }

pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

pub enum Role { System, User, Assistant }

pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub struct ChatResponse {
    pub message: ChatMessage,
    pub tool_calls: Vec<ToolCall>,
    pub tokens_generated: u32,
    pub time_ms: u64,
}
```

Трейт:

```rust
#[async_trait]
pub trait LlmEngine: Send + Sync {
    async fn load(&self, config: LlmConfig) -> Result<()>;
    async fn unload(&self) -> Result<()>;
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse>;
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> Result<impl Stream<Item = Result<String>>>;
    async fn health(&self) -> Result<LlmHealth>;
    fn model_info(&self) -> Option<ModelInfo>;
}
```

Реализация: `LlamaEngine`.

**Зависимости:**
Внутренние: `tensa-core`.
Внешние: tokio, bindgen (build), cc (build), serde, serde_json, thiserror, tracing, async-trait, futures, reqwest, semver.

**llama.cpp:** git-зависимость в Cargo.toml (тег или коммит), сборка через cc, флаги AVX2/AVX-512/NEON, C API через bindgen.

**Безопасный C-FFI:** уровень 1 (`llama-sys`, unsafe), уровень 2 (`llama`, safe wrapper), изоляция, тесты (miri, sanitizers).

**Автообновление:** проверка при запуске, сравнение версий, уведомление, обновление с подтверждением, smoke-тесты, откат.

**Tool use:** JSON-схема, парсинг из ответа LLM, валидация, интеграция с `tensa-router`.

**Reasoning:** пошаговые рассуждения, парсинг, интеграция с `tensa-router`.

**Тесты:**
Unit: LlamaEngine, generate, chat, chat_stream, sampling, update.
Integration: загрузка XS/S, генерация на русском, tool use, reasoning, стриминг.
Ошибки: модель не найдена, повреждена, недостаточно RAM, ошибка llama.cpp, ошибка обновления.

**Ограничения:** не маршрутизировать, не хранить историю, не выполнять действия, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-003, ADR-006, ADR-007, Контракт 4.

## ТЗ: tensa-perf

**Назначение:**
Крейт, реализующий профилирование устройства и динамическое распределение ролей (heavy/thin).

Принципы: профилирование RAM/CPU/GPU/NPU/батареи/thermal, определение роли, динамическое переключение, интеграция с router и nodes.

**Публичный API:**

Типы:

```rust
pub struct PerfConfig {
    pub profile_interval_seconds: u64,      // 60
    pub heavy_ram_threshold_mb: u64,        // 8192
    pub heavy_cpu_cores_threshold: u32,     // 4
    pub heavy_gpu_required: bool,           // false
    pub thin_battery_threshold_percent: u8, // 20
    pub thermal_throttle_threshold: ThermalStatus,
    pub benchmark_on_startup: bool,         // true
}

pub struct DeviceProfile {
    pub ram_total_mb: u64,
    pub ram_available_mb: u64,
    pub cpu_cores: u32,
    pub cpu_frequency_mhz: u32,
    pub cpu_usage_percent: f32,
    pub gpu: Option<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub disk_total_gb: u64,
    pub disk_available_gb: u64,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,
    pub thermal_status: ThermalStatus,
    pub os: String,
    pub arch: String,
}

pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u64,
    pub backend: GpuBackend,
}

pub struct NpuInfo {
    pub name: String,
    pub backend: NpuBackend,
}

pub struct BenchmarkResult {
    pub memory_bandwidth_mbps: f64,
    pub disk_read_mbps: f64,
    pub disk_write_mbps: f64,
    pub cpu_score: u32,
    pub timestamp: i64,
}

pub struct NodeRoleDecision {
    pub role: NodeRole,
    pub reason: String,
    pub can_run_llm: bool,
    pub recommended_model_tier: ModelTier,
    pub can_run_memory: bool,
    pub can_run_telegram: bool,
    pub can_run_email: bool,
}
```

Трейт:

```rust
#[async_trait]
pub trait PerfProfiler: Send + Sync {
    async fn profile(&self) -> Result<DeviceProfile>;
    async fn benchmark(&self) -> Result<BenchmarkResult>;
    async fn decide_role(&self, profile: &DeviceProfile) -> Result<NodeRoleDecision>;
    async fn start_monitoring(&self) -> Result<()>;
    async fn stop_monitoring(&self) -> Result<()>;
    async fn current_profile(&self) -> Result<DeviceProfile>;
}
```

Реализация: `DefaultPerfProfiler`.

**Зависимости:**
Внутренние: `tensa-core`, `tensa-nodes`.
Внешние: tokio, sysinfo, nvml-wrapper (опционально), serde, thiserror, tracing, async-trait.

**Структура:**

```
crates/tensa-perf/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── models.rs
│   ├── profiler.rs
│   ├── default.rs
│   ├── cpu.rs
│   ├── memory.rs
│   ├── gpu.rs
│   ├── npu.rs
│   ├── battery.rs
│   ├── thermal.rs
│   ├── benchmark.rs
│   ├── role.rs
│   └── error.rs
└── tests/
```

**Профилирование:** RAM (sysinfo), CPU (sysinfo), GPU (nvml-wrapper), NPU (NNAPI/CoreML/DirectML), батарея (battery/windows), thermal (PowerManager/sysfs/WMI).

**Бенчмарк:** memory bandwidth, disk read/write, CPU score, кэш в `~/.tensa/benchmark.json`.

**Определение роли:** heavy (RAM ≥ 8 ГБ, CPU ≥ 4 ядра, GPU опционально, батарея > 20%, thermal ≤ Serious), thin (иначе), auto (динамически).

**Мониторинг:** каждые 60 секунд, обновление current_profile, уведомление nodes/router.

**Тесты:**
Unit: PerfConfig, DeviceProfile, role, benchmark.
Integration: профилирование, бенчмарк, определение heavy/thin, динамическое переключение, мониторинг.
Ошибки: недостаточно прав, GPU недоступен, NPU недоступен, батарея недоступна, thermal недоступен.

**Ограничения:** не маршрутизировать, не синхронизировать, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-004, ADR-012, ADR-013, ADR-014, Контракт 7, Контракт 8.

## ТЗ: tensa-ffi

**Назначение:**
Крейт, реализующий FFI-биндинги для UI через uniffi (ADR-006). Генерирует биндинги для Kotlin (Android), Swift (iOS, после MVP), Python (CLI).

Принципы: uniffi, строгая типизация, async, минимум ручного кода, кросс-платформенность.

**Публичный API:**
Все типы и интерфейсы — из контракта 9 (uniffi UDL). tensa-ffi — тонкая обёртка.

Интерфейсы: ApprovalStore, MemoryStore, CoreMemoryStore, EmailClient, CalendarClient, AppLauncher, NodeRegistry, ConfigStore, QueryEngine, VoiceEngine, FirstRunWizard.

Ошибки: TensaError (ConfigError, NetworkError, ApprovalError, MemoryError, EmailError, CalendarError, AppError, NodeError, InternalError).

Реализация: TensaRuntime.

**Зависимости:**
Внутренние: `tensa-core`, `tensa-approval`, `tensa-memory`, `tensa-nodes`, `tensa-router`, `tensa-tools`.
Внешние: uniffi, tokio, serde, thiserror, tracing.

**Структура:**

```
crates/tensa-ffi/
├── Cargo.toml
├── build.rs
├── tensa.udl
├── src/
│   ├── lib.rs
│   ├── runtime.rs
│   ├── approval.rs
│   ├── memory.rs
│   ├── email.rs
│   ├── calendar.rs
│   ├── apps.rs
│   ├── nodes.rs
│   ├── config.rs
│   ├── query.rs
│   ├── voice.rs
│   └── error.rs
└── tests/
```

**Кодогенерация:** uniffi-bindgen generate для Kotlin, Swift, Python.

**Async:** через `[Async]` в UDL.

**Обработка ошибок:** uniffi генерирует исключения.

**Тесты:**
Unit: TensaRuntime, TensaError, каждый FFI-интерфейс.
Integration: инициализация, вызов каждого интерфейса, обработка ошибок, async, кросс-платформенность.
Ошибки: неверный config_path, недоступный ресурс, ошибка сети, ошибка FFI.

**Ограничения:** не реализовывать логику, не хранить состояние, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-012, Контракт 9.

## ТЗ: tensa-core

**Назначение:**
Центральный крейт, который координирует все остальные крейты, хранит общие типы (Config, Note, Contact, ошибки), реализует заметки и контакты (SQLite), запускает и останавливает подсистемы, предоставляет единый API для UI, **реализует first-run wizard и инициализацию памяти**.

Принципы: координация (не дублирование), общие типы здесь, заметки/контакты здесь, конфигурация здесь, запуск/остановка здесь, **first-run wizard здесь**.

**Публичный API:**

Типы:

```rust
pub struct Config {
    pub general: GeneralConfig,
    pub brain: BrainConfig,
    pub memory: MemoryConfig,
    pub voice: VoiceConfig,
    pub sync: SyncConfig,
    pub approval: ApprovalConfig,
    pub telegram: TelegramConfig,
    pub email: EmailConfig,
    pub calendar: CalendarConfig,
    pub apps: AppsConfig,
    pub security: SecurityConfig,
    pub updates: UpdatesConfig,
    pub documents: DocumentsConfig,
}

pub struct Note {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub created_on_node: String,
    pub updated_on_node: String,
    pub deleted: bool,
    pub vector_clock: VectorClock,
}

pub struct NewNote { pub title: String, pub body: String, pub tags: Vec<String> }

pub struct Contact {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub telegram: Option<String>,
    pub whatsapp: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub created_on_node: String,
    pub updated_on_node: String,
    pub deleted: bool,
    pub vector_clock: VectorClock,
}

pub struct NewContact {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub telegram: Option<String>,
    pub whatsapp: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

pub struct VectorClock { pub clocks: HashMap<String, u64> }

pub enum Language { Ru, En }
pub enum DeviceType { Desktop, Android }
pub enum NodeRole { Heavy, Thin, Auto }
```

Трейты:

```rust
#[async_trait]
pub trait NotesStore: Send + Sync {
    async fn create_note(&self, note: NewNote) -> Result<Note>;
    async fn update_note(&self, note: Note) -> Result<Note>;
    async fn delete_note(&self, id: Uuid) -> Result<()>;
    async fn get_note(&self, id: Uuid) -> Result<Option<Note>>;
    async fn search_notes(&self, query: &str, limit: usize) -> Result<Vec<Note>>;
    async fn list_notes(&self, limit: usize, offset: usize) -> Result<Vec<Note>>;
    async fn get_notes_since(&self, timestamp: i64) -> Result<Vec<Note>>;
    async fn count_notes(&self) -> Result<u64>;
}

#[async_trait]
pub trait ContactsStore: Send + Sync {
    async fn create_contact(&self, contact: NewContact) -> Result<Contact>;
    async fn update_contact(&self, contact: Contact) -> Result<Contact>;
    async fn delete_contact(&self, id: Uuid) -> Result<()>;
    async fn get_contact(&self, id: Uuid) -> Result<Option<Contact>>;
    async fn find_by_email(&self, email: &str) -> Result<Option<Contact>>;
    async fn find_by_name(&self, name: &str) -> Result<Vec<Contact>>;
    async fn search_contacts(&self, query: &str, limit: usize) -> Result<Vec<Contact>>;
    async fn list_contacts(&self, limit: usize, offset: usize) -> Result<Vec<Contact>>;
    async fn get_contacts_since(&self, timestamp: i64) -> Result<Vec<Contact>>;
    async fn count_contacts(&self) -> Result<u64>;
}

#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn load(&self, path: &Path) -> Result<Config>;
    async fn save(&self, config: &Config) -> Result<()>;
    async fn reload(&self) -> Result<Config>;
    async fn get(&self) -> Result<Config>;
    async fn update(&self, config: Config) -> Result<()>;
}
```

Координатор:

```rust
pub struct TensaCore {
    config: Arc<RwLock<Config>>,
    approval_store: Arc<dyn ApprovalStore>,
    memory_manager: Arc<dyn MemoryManager>,
    notes_store: Arc<dyn NotesStore>,
    contacts_store: Arc<dyn ContactsStore>,
    node_registry: Arc<dyn NodeRegistry>,
    sync_service: Arc<dyn SyncService>,
    brain_router: Arc<dyn BrainRouter>,
    perf_profiler: Arc<dyn PerfProfiler>,
    tool_registry: Arc<ToolRegistry>,
    running: Arc<AtomicBool>,
}

impl TensaCore {
    pub async fn new(config_path: &Path) -> Result<Self>;
    pub async fn is_first_run(&self) -> Result<bool>;
    pub async fn run_first_run_wizard(&self) -> Result<WizardResult>;
    pub async fn init_memory(&self, params: InitParams) -> Result<()>;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;
    pub async fn health(&self) -> Result<HealthStatus>;
    pub async fn reload_config(&self) -> Result<()>;
}
```

**First-Run Wizard (ADR-021, дополнение):**
- Шаг 1: Язык.
- Шаг 2: Знакомство (имя, обращение, имя секретаря, часовой пояс).
- Шаг 3: Загрузка моделей.
- Шаг 4: Telegram.
- Шаг 5: Почта.
- Шаг 6: WhatsApp (опционально).
- Шаг 7: Tailscale.
- Шаг 8: Завершение.

**Инициализация памяти:** `init_default` создаёт core memory blocks (persona, human, tasks, preferences).

**Зависимости:**
Внутренние: все крейты.
Внешние: tokio, sqlx, toml, serde, serde_json, uuid, chrono, thiserror, tracing, async-trait, directories, notify.

**Структура:**

```
crates/tensa-core/
├── Cargo.toml
├── schema.sql
├── src/
│   ├── lib.rs
│   ├── core.rs
│   ├── config.rs
│   ├── models.rs
│   ├── notes.rs
│   ├── contacts.rs
│   ├── health.rs
│   ├── error.rs
│   ├── paths.rs
│   ├── first_run.rs
│   └── init.rs
└── tests/
```

**Конфигурация:** файл `~/.tensa/config.toml`, секреты `~/.tensa/secrets.toml`, загрузка при старте, валидация, обновление через UI, hot-reload через notify.

**Заметки и контакты:** SQLite `~/.tensa/notes.db`, FTS5, синхронизация через `tensa-sync`, soft delete, векторные часы.

**Запуск/остановка:** 11 шагов (конфиг → БД → nodes → perf → memory → approval → sync → router → tools → llm → running=true). Остановка — в обратном порядке.

**Health:**

```rust
pub struct HealthStatus {
    pub healthy: bool,
    pub version: String,
    pub uptime: String,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
    pub components: HashMap<String, ComponentHealth>,
}
```

**Тесты:**
Unit: Config, NotesStore, ContactsStore, VectorClock, paths, first_run, init.
Integration: запуск/остановка, загрузка конфига, создание заметки/контакта, поиск, health, hot-reload, полный цикл wizard, инициализация core memory.
Ошибки: неверный config_path, ошибка БД, ошибка синхронизации, ошибка LLM, ошибка инструментов, неверный токен Telegram, неверный IMAP.

**Ограничения:** не дублировать логику, не хранить секреты, не логировать секреты.

**Связанные ADR и контракты:** ADR-001…ADR-021, Контракт 3, Контракт 4, Контракт 9, Контракт 10, Контракт 11, Контракт 12, Контракт 13.

## ТЗ: tensa-cli

**Назначение:**
Крейт, реализующий CLI для отладки и управления Tensa. Позволяет: запускать ядро без UI, отлаживать компоненты, управлять конфигурацией, синхронизировать узлы, индексировать документы, тестировать LLM.

Принципы: чистый Rust, использует tensa-core и подкрейты, минимум зависимостей, кроссплатформенность.

**Команды:**

```
tensa [OPTIONS] <COMMAND>

OPTIONS:
    -c, --config <PATH>    Путь к config.toml
    -v, --verbose          Подробный вывод
    -h, --help             Справка
    -V, --version          Версия

COMMANDS:
    init, start, stop, status, health
    config get/set/list/reload
    approval list/approve/reject/show
    memory index/reindex/delete/search/list
    notes create/list/search/delete
    contacts create/list/search/delete
    email connect/disconnect/folders/fetch/search/send
    calendar list/conflicts/create/delete
    apps list/scan/launch/open
    nodes list/get/heavy
    sync start/stop/status/push/pull
    query <TEXT>
    transcribe <FILE>
    benchmark
    version
```

**Зависимости:**
Внутренние: все крейты.
Внешние: tokio, clap, serde, serde_json, toml, anyhow, tracing, tracing-subscriber, colored, indicatif, comfy-table.

**Структура:**

```
crates/tensa-cli/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── init.rs, start.rs, stop.rs, status.rs, health.rs
│   │   ├── config.rs, approval.rs, memory.rs, notes.rs, contacts.rs
│   │   ├── email.rs, calendar.rs, apps.rs, nodes.rs, sync.rs
│   │   ├── query.rs, transcribe.rs, benchmark.rs, version.rs
│   ├── output.rs
│   └── error.rs
└── tests/
```

**Вывод:** цветной (colored), таблицы (comfy-table), прогресс (indicatif), JSON (--json), тихий режим (--quiet).

**Интеграция:** прямая через tensa-core (in-process) или через gRPC (если ядро запущено).

**Тесты:**
Unit: cli, output, каждая команда.
Integration: все команды.
Ошибки: неверный config_path, ядро не запущено, неверный ID, ошибка сети, ошибка LLM.

**Ограничения:** не реализовывать логику, не хранить состояние, не логировать секреты.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-012, Контракт 9.

## ТЗ: apps/desktop

**Назначение:**
Десктопное приложение Tensa на Tauri v2 + Svelte 5. Предоставляет: панель управления, Spotlight HUD-панель, Approval Center, мастер первого запуска, настройки, плавающий виджет.

Принципы: Tauri v2, Svelte 5, Rust-ядро через Tauri IPC, кроссплатформенность (Windows, Linux), Spotlight HUD с глобальным хоткеем, плавающий виджет.

**Структура:**

```
apps/desktop/
├── src/
│   ├── main.js, App.svelte
│   ├── lib/api/tensa.ts, events.ts
│   ├── lib/stores/
│   ├── lib/components/
│   │   ├── Spotlight.svelte
│   │   ├── ApprovalCenter.svelte
│   │   ├── Settings.svelte
│   │   ├── FirstRunWizard.svelte
│   │   ├── SearchResults.svelte
│   │   ├── NodesList.svelte
│   │   ├── StatusBar.svelte
│   │   ├── FloatingWidget.svelte
│   │   ├── WidgetZones.svelte
│   │   ├── VoiceWave.svelte
│   │   └── TextInputPopup.svelte
│   ├── lib/i18n/ru.json, en.json
│   └── routes/
├── src-tauri/
│   ├── Cargo.toml, build.rs, tauri.conf.json
│   ├── src/main.rs, lib.rs, commands.rs, events.rs, types.rs, sidecar.rs, widget.rs
└── tests/e2e/
```

**Tauri-конфигурация:** окно main (1000x700), окно spotlight (680x80, безрамочное, плавающее), окно widget (120x48, безрамочное, плавающее, transparent, alwaysOnTop, skipTaskbar).

**Spotlight HUD:** глобальный хоткей Win+Alt+A, поле ввода, переключатель Local E2B ↔ Cloud 26B, кнопка микрофона, смарт-карточки, закрытие по Esc.

**Плавающий виджет:** две зоны (Т + ...), адаптивный (голос ↔ текст), при голосовом вводе — окно с текстом и редактированием с клавиатуры, драг, сворачивание в край. Настройки: позиция, прозрачность, размер, режим по умолчанию, хоткей.

**Approval Center:** список pending, предупреждение для незнакомых, кнопки «Одобрить»/«Отклонить», real-time через события.

**Настройки:** тумблеры по каналам, автозапуск, язык, вкладки «Приложения», «Почта», «Узлы», «Обновления».

**Мастер первого запуска:** язык, загрузка моделей, Telegram, почта, WhatsApp (опционально), Tailscale, знакомство (имя, обращение, имя секретаря, часовой пояс).

**API:** Tauri IPC (контракт 10). Все команды и события.

**Sidecar:** бинарник tensa-core, запуск при старте Tauri, остановка SIGTERM.

**Сборка:** Windows (.msi, .exe), Linux (.deb, .AppImage). macOS — после MVP.

**Тесты:** E2E (Playwright/WebDriver), UI (Svelte), IPC (Tauri commands), сборка (CI).

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-007, ADR-010, ADR-017, Контракт 10.

## ТЗ: apps/android

**Назначение:**
Мобильное приложение Tensa на Flutter + Kotlin. Предоставляет: экран быстрого голосового ввода, очередь одобрений, поиск, настройки, плавающий виджет (overlay + App Widget).

Принципы: Flutter (UI), Kotlin (мост через uniffi), фоновые задачи WorkManager/Foreground Service, Syncthing как библиотека, плавающий виджет с двумя зонами.

**Структура:**

```
apps/android/
├── lib/
│   ├── main.dart, app.dart
│   ├── api/tensa.dart, events.dart
│   ├── models/
│   ├── screens/
│   │   ├── voice_input_screen.dart
│   │   ├── approvals_screen.dart
│   │   ├── search_screen.dart
│   │   ├── documents_screen.dart
│   │   ├── notes_screen.dart, contacts_screen.dart
│   │   ├── email_screen.dart, calendar_screen.dart
│   │   ├── apps_screen.dart, nodes_screen.dart
│   │   ├── settings_screen.dart, first_run_screen.dart
│   ├── widgets/
│   │   ├── approval_card.dart, search_result_card.dart
│   │   ├── voice_button.dart, status_indicator.dart
│   │   ├── floating_widget.dart, widget_zones.dart
│   │   ├── voice_wave.dart, text_input_popup.dart
│   ├── services/
│   │   ├── background_service.dart, syncthing_service.dart, notification_service.dart
│   └── i18n/ru.json, en.json
├── android/app/src/main/
│   ├── AndroidManifest.xml
│   ├── kotlin/com/tensorlogix/tensa/
│   │   ├── MainActivity.kt, TensaPlugin.kt
│   │   ├── BackgroundService.kt, SyncthingService.kt
│   │   ├── FloatingWidgetService.kt, OverlayManager.kt, WidgetLayout.kt
│   │   └── NotificationHelper.kt
│   └── jniLibs/arm64-v8a, armeabi-v7a, x86, x86_64
└── test/
```

**Flutter-зависимости:** http, flutter_secure_storage, record, audioplayers, device_info_plus, workmanager, flutter_local_notifications, permission_handler, path_provider, shared_preferences, intl, provider, go_router.

**Экраны:** 12 экранов (голос, одобрения, поиск, документы, заметки, контакты, почта, календарь, приложения, узлы, настройки, первый запуск).

**Плавающий виджет:** две зоны (Т + ...), overlay + App Widget (оба, настройка в UI), при голосовом вводе — окно с текстом и редактированием с клавиатуры, драг, сворачивание в край. Анимации — Flutter (встроенные).

**Фоновые задачи:** Foreground Service (зарядка / мгновенные ответы), WorkManager (от батареи / экономия), настройка в UI, объединённое уведомление.

**Syncthing:** syncthing-android как библиотека, Foreground Service, управление через REST API.

**Виджет быстрого вызова:** Android App Widget, 1 клик — голосовой ввод.

**Разрешения:** INTERNET, RECORD_AUDIO, POST_NOTIFICATIONS, FOREGROUND_SERVICE, FOREGROUND_SERVICE_DATA_SYNC, FOREGROUND_SERVICE_MICROPHONE, WAKE_LOCK, RECEIVE_BOOT_COMPLETED, REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, SYSTEM_ALERT_WINDOW.

**API:** uniffi + MethodChannel (контракт 11). Все методы и события.

**Сборка:** APK, AAB, подпись keystore.

**Тесты:** Widget, Integration, Unit (Dart), E2E на реальных устройствах.

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-011, ADR-012, ADR-014, ADR-017, ADR-018, Контракт 11.

## ТЗ: apps/pwa

**Назначение:**
PWA-версия Tensa на Svelte. Временный интерфейс для iOS (ADR-010), а также fallback для других устройств.

Принципы: Svelte (UI), HTTP/gRPC к ядру (контракт 12), Tailscale, Safari (iOS), без overlay.

**Структура:**

```
apps/pwa/
├── static/
│   ├── manifest.json, sw.js
│   └── icons/icon-192.png, icon-512.png
├── src/
│   ├── main.js, App.svelte
│   ├── lib/api/tensa.ts, events.ts
│   ├── lib/stores/
│   ├── lib/components/
│   │   ├── ApprovalsList.svelte, SearchBar.svelte
│   │   ├── VoiceButton.svelte, Settings.svelte, StatusBar.svelte
│   ├── lib/i18n/ru.json, en.json
│   └── routes/
└── tests/e2e/
```

**PWA-манифест:** name, short_name, start_url /app, display standalone, background_color, theme_color, orientation, icons 192/512.

**Service Worker:** кэширование статики, offline-режим (ограниченный), без фоновых задач, без push.

**Экраны:** одобрения, поиск, голосовой ввод, документы, заметки, контакты, почта, календарь, приложения, узлы, настройки, первый запуск.

**API:** HTTP (контракт 12). Все эндпоинты.

**События:** SSE (approval:new, status:health, sync:progress, search:result).

**Аутентификация:** токен генерируется при первом запуске ядра, отображается в UI десктопа (QR или текст), пользователь вводит в PWA, хранится в localStorage, проверяется на каждом запросе (Bearer).

**Ограничения PWA:** нет overlay, нет фоновых задач, нет push, нет доступа к файловой системе, нет запуска приложений.

**Установка на iOS:** Safari → `http://<tailscale-ip>:8765/app` → ввести токен → «На экран Домой».

**Сборка:** npm run build, размещение в core/static/pwa/, Service Worker регистрируется автоматически.

**Тесты:** Unit (Svelte), E2E (Playwright, Chrome + Safari), PWA (Lighthouse).

**Связанные ADR и контракты:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-010, ADR-012, Контракт 12.

