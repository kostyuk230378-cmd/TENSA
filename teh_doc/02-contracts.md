# Tensa — Контракты

---

## Контракт 1: ApprovalStore (SQLite)

**Назначение:**
ApprovalStore — центральный элемент безопасности Tensa. Хранит действия, требующие одобрения владельца, до их выполнения.

Принципы: авто-ответы запрещены, прямое исполнение сетевых действий из LLM запрещено, все действия проходят через очередь одобрения, действия от trusted_origin могут исполняться без одобрения, но с логированием.

**Расположение:**
- Файл: `~/.tensa/approval.db` (SQLite).
- Схема: `crates/tensa-approval/schema.sql`.
- Rust-структуры: `crates/tensa-approval/src/models.rs`.

**Таблицы:**

`actions` — очередь действий:

| Поле | Тип | Описание |
|---|---|---|
| id | TEXT (UUID) | Уникальный идентификатор |
| kind | TEXT | Тип: email_send, telegram_send, whatsapp_send, calendar_create, app_launch, file_open |
| payload | TEXT (JSON) | Полезная нагрузка |
| status | TEXT | pending, approved, rejected, executed, failed, expired |
| created_at | INTEGER | Unix timestamp |
| created_by | TEXT | llm, user, telegram, email, system |
| created_on_node | TEXT | node_id |
| expires_at | INTEGER | Unix timestamp (TTL 24 часа) |
| approved_at | INTEGER | Unix timestamp |
| approved_by | TEXT | user, telegram, mobile, desktop |
| approved_on_node | TEXT | node_id |
| executed_at | INTEGER | Unix timestamp |
| executed_on_node | TEXT | node_id |
| result | TEXT (JSON) | Результат |
| operation_id | TEXT (UUID) | ID для идемпотентности |
| vector_clock | TEXT (JSON) | Векторные часы |

Индексы: idx_actions_status, idx_actions_expires_at, idx_actions_operation_id.

`action_events` — журнал событий (event sourcing):

| Поле | Тип | Описание |
|---|---|---|
| id | INTEGER | Автоинкремент |
| action_id | TEXT (UUID) | ID действия |
| event_type | TEXT | created, approved, rejected, executed, failed, expired |
| event_data | TEXT (JSON) | Данные |
| timestamp | INTEGER | Unix timestamp |
| node_id | TEXT | node_id |
| vector_clock | TEXT (JSON) | Векторные часы |

`sync_state` — состояние синхронизации:

| Поле | Тип | Описание |
|---|---|---|
| node_id | TEXT | node_id |
| last_event_id | INTEGER | Последний event_id |
| last_sync_at | INTEGER | Unix timestamp |

**Rust-структуры:**

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

pub enum ActionKind {
    EmailSend, TelegramSend, WhatsAppSend,
    CalendarCreate, AppLaunch, FileOpen,
}

pub enum ActionStatus {
    Pending, Approved, Rejected, Executed, Failed, Expired,
}

pub enum ActionOrigin {
    Llm, User, Telegram, Email, System,
}
```

**API (трейт):**

```rust
pub trait ApprovalStore {
    async fn request_action(&self, action: NewAction) -> Result<Action>;
    async fn approve(&self, id: Uuid, approved_by: ActionOrigin) -> Result<Action>;
    async fn reject(&self, id: Uuid, rejected_by: ActionOrigin) -> Result<Action>;
    async fn execute(&self, id: Uuid) -> Result<Action>;
    async fn get_pending(&self) -> Result<Vec<Action>>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Action>>;
    async fn expire_old(&self) -> Result<usize>;
    async fn apply_event(&self, event: ActionEvent) -> Result<()>;
}
```

**Жизненный цикл:**

```
created → pending → approved → executed
                 ↘ rejected
                 ↘ expired (TTL 24 часа)
                 ↘ failed (ошибка)
```

**Синхронизация:** события из action_events синхронизируются через gRPC (ADR-012). Идемпотентность через operation_id. Конфликты — vector_clock. LWW по timestamp.

**Безопасность:** действия из trusted_origin (user, telegram от владельца) могут исполняться без одобрения, но логируются. Действия от llm — всегда через ApprovalStore. TTL 24 часа.

**Связанные ADR:** ADR-001, ADR-012, ADR-013, ADR-015, ADR-017, ADR-018.

## Контракт 2: Память (Lance)

**Версия:** 0.1.1 (обновлено 2026-10-15)
**Статус:** Утверждено

**Назначение:**
Память Tensa хранит индексированные документы и их эмбеддинги для семантического и гибридного поиска.

Принципы: файловая природа, версионирование, гибридный поиск, индекс по умолчанию — Flat (ADR-004).

**Расположение:**
- Каталог: `~/.tensa/memory/` (Lance-файлы).
- Схема: `crates/tensa-memory/schema.rs`.
- Rust-структуры: `crates/tensa-memory/src/models.rs`.

**Таблицы (Lance datasets):**

`documents` — индексированные документы:

| Поле | Тип | Описание |
|---|---|---|
| id | TEXT (UUID) | ID чанка |
| document_id | TEXT (UUID) | ID документа |
| chunk_index | INTEGER | Номер чанка |
| source_path | TEXT | Путь к файлу |
| source_hash | TEXT | Хэш файла |
| chunk_text | TEXT | Текст чанка |
| chunk_hash | TEXT | Хэш чанка |
| embedding | FIXED_SIZE_LIST<FLOAT32, 384> | Эмбеддинг |
| metadata | TEXT (JSON) | Метаданные |
| account_id | TEXT | ID ящика (если из почты) |
| indexed_at | INTEGER | Unix timestamp |
| updated_at | INTEGER | Unix timestamp |

Индексы: векторный по embedding (Flat), скалярные по source_path, document_id, account_id, полнотекстовый по chunk_text.

`documents_meta` — метаданные документов:

| Поле | Тип | Описание |
|---|---|---|
| document_id | TEXT (UUID) | ID документа |
| source_path | TEXT | Путь |
| source_hash | TEXT | Хэш |
| title | TEXT | Название |
| mime_type | TEXT | MIME-тип |
| size_bytes | INTEGER | Размер |
| created_at | INTEGER | Unix timestamp |
| modified_at | INTEGER | Unix timestamp |
| indexed_at | INTEGER | Unix timestamp |
| chunk_count | INTEGER | Количество чанков |
| metadata | TEXT (JSON) | Метаданные |

`search_cache` — кэш поисковых запросов:

| Поле | Тип | Описание |
|---|---|---|
| query_hash | TEXT | Хэш запроса |
| query_text | TEXT | Текст запроса |
| query_embedding | FIXED_SIZE_LIST<FLOAT32, 384> | Эмбеддинг |
| result_ids | TEXT (JSON) | ID результатов |
| created_at | INTEGER | Unix timestamp |
| expires_at | INTEGER | Unix timestamp (TTL 1 час) |

**Чанкинг:**
- Размер чанка: 1000 символов.
- Перекрытие: 200 символов (унифицированно для всех языков в v0.1).
- Хэш чанка: SHA-256.
- Дедупликация: если chunk_hash уже есть — не индексировать.

**Эмбеддинги:**
- Модель: intfloat/multilingual-e5-small.
- Размерность: 384.
- Префиксы: query: для запросов, passage: для документов.
- Кэширование: эмбеддинги запросов кэшируются.

**Поиск:**
- Семантический: префикс query:, эмбеддинг, top-k по косинусному расстоянию.
- Гибридный: семантический + полнотекстовый (RRF).

**Синхронизация:** Lance-файлы синхронизируются через Syncthing (ADR-018). On-demand через gRPC (ADR-013). Конфликты — LWW по updated_at.

**Rust API (lance 13.0, проверено 2026-10-15, апрув Архитектора):**
- Открытие: `Dataset::open(uri)`.
- Создание/запись: `Dataset::write(batches, dest, Some(WriteParams { mode }))`; режимы — `WriteMode::Create` / `Append` / `Overwrite`.
- Векторный поиск: `dataset.scan().nearest("embedding", &query, k).limit(Some(k), None)`; сбор — `try_into_batch()` / `try_into_stream()`.
- Удаление: `dataset.delete(предикат по document_id)`.
- Схема: типы — `lance::deps::arrow_schema` (re-export arrow, версии совпадают); `embedding` — `DataType::FixedSizeList(Arc<Field>, 384)`.
- Версия крейта: `lance = "13.0"` (0.9 не компилируется rustc 1.98); LanceDB-стиль (`Connection::open`, `create_table`) в Rust-крейте lance не существует.
- Эмбеддинги: `fastembed = "7.1"` — `TextInitOptions`, синхронный `TextEmbedding::try_new`, `embed` за `Mutex` + `spawn_blocking`; префиксы `query:`/`passage:` передаёт вызывающий.
- Компиляционная проверка API: `crates/tensa-memory/tests/api_surface.rs` (CI: `cargo test -p tensa-memory --features lance,fastembed`).

**Связанные ADR:** ADR-001, ADR-004, ADR-005, ADR-012, ADR-013, ADR-018.

---
## Контракт 3: Заметки/контакты (SQLite)

**Версия:** 0.1.1 (обновлено 2026-10-15)
**Статус:** Утверждено

**Назначение:**
Tensa хранит заметки и контакты пользователя. Они используются:
- Для контекста при работе секретаря.
- Для поиска.
- Для автодополнения при отправке почты/сообщений.
- Для синхронизации между узлами.

Принципы:
- SQLite + FTS5 для полнотекстового поиска.
- Синхронизация через gRPC (ADR-012).
- On-demand sync (ADR-013).
- LWW по `updated_at` при конфликтах.

**Расположение:**
- Файл: `~/.tensa/notes.db` (SQLite).
- Схема: `crates/tensa-core/schema.sql`.
- Rust-структуры: `crates/tensa-core/src/models.rs`.

**Таблицы:**

`notes` — заметки:

| Поле | Тип | Описание |
|---|---|---|
| `id` | TEXT (UUID) | Уникальный идентификатор |
| `title` | TEXT | Заголовок |
| `body` | TEXT | Текст заметки |
| `tags` | TEXT (JSON) | Теги (массив строк) |
| `created_at` | INTEGER | Unix timestamp создания |
| `updated_at` | INTEGER | Unix timestamp обновления |
| `created_on_node` | TEXT | `node_id` узла создания |
| `updated_on_node` | TEXT | `node_id` узла обновления |
| `deleted` | INTEGER | Флаг удаления (0/1) |
| `vector_clock` | TEXT (JSON) | Векторные часы |

Индексы:
- `idx_notes_updated_at` — по `updated_at`.
- `idx_notes_deleted` — по `deleted`.

`notes_fts` — полнотекстовый индекс заметок (виртуальная таблица FTS5):

| Поле | Тип | Описание |
|---|---|---|
| `title` | TEXT | Заголовок |
| `body` | TEXT | Текст |
| `tags` | TEXT | Теги |

Триггеры: синхронизация с `notes` при INSERT/UPDATE/DELETE.

`contacts` — контакты:

| Поле | Тип | Описание |
|---|---|---|
| `id` | TEXT (UUID) | Уникальный идентификатор |
| `name` | TEXT | Имя |
| `email` | TEXT | Email |
| `phone` | TEXT | Телефон |
| `telegram` | TEXT | Telegram username |
| `whatsapp` | TEXT | WhatsApp номер |
| `notes` | TEXT | Заметки о контакте |
| `tags` | TEXT (JSON) | Теги |
| `created_at` | INTEGER | Unix timestamp создания |
| `updated_at` | INTEGER | Unix timestamp обновления |
| `created_on_node` | TEXT | `node_id` узла создания |
| `updated_on_node` | TEXT | `node_id` узла обновления |
| `deleted` | INTEGER | Флаг удаления (0/1) |
| `vector_clock` | TEXT (JSON) | Векторные часы |

Индексы:
- `idx_contacts_email` — по `email`.
- `idx_contacts_name` — по `name`.
- `idx_contacts_updated_at` — по `updated_at`.
- `idx_contacts_deleted` — по `deleted`.

`contacts_fts` — полнотекстовый индекс контактов (виртуальная таблица FTS5):

| Поле | Тип | Описание |
|---|---|---|
| `name` | TEXT | Имя |
| `email` | TEXT | Email |
| `notes` | TEXT | Заметки |
| `tags` | TEXT | Теги |

Триггеры: синхронизация с `contacts` при INSERT/UPDATE/DELETE.

`sync_state` — состояние синхронизации:

| Поле | Тип | Описание |
|---|---|---|
| `node_id` | TEXT | `node_id` удалённого узла |
| `last_note_updated_at` | INTEGER | Последний `updated_at` заметки |
| `last_contact_updated_at` | INTEGER | Последний `updated_at` контакта |
| `last_sync_at` | INTEGER | Unix timestamp последней синхронизации |

Индексы:
- `idx_sync_state_node_id` — по `node_id`.

**Rust-структуры:**

```rust
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
```

**API (трейт):**

`NotesStore`:

```rust
#[async_trait]
pub trait NotesStore: Send + Sync {
    // CRUD
    async fn create_note(&self, note: NewNote) -> Result<Note>;
    async fn update_note(&self, note: Note) -> Result<Note>;
    async fn delete_note(&self, id: Uuid) -> Result<()>;
    async fn get_note(&self, id: Uuid) -> Result<Option<Note>>;

    // Поиск и список
    async fn search_notes(&self, query: &str, limit: usize) -> Result<Vec<Note>>;
    async fn list_notes(&self, limit: usize, offset: usize) -> Result<Vec<Note>>;

    // Синхронизация (добавлено 2026-10-15)
    async fn get_notes_since(&self, timestamp: i64) -> Result<Vec<Note>>;
    async fn count_notes(&self) -> Result<u64>;
}
```

`ContactsStore`:

```rust
#[async_trait]
pub trait ContactsStore: Send + Sync {
    // CRUD
    async fn create_contact(&self, contact: NewContact) -> Result<Contact>;
    async fn update_contact(&self, contact: Contact) -> Result<Contact>;
    async fn delete_contact(&self, id: Uuid) -> Result<()>;
    async fn get_contact(&self, id: Uuid) -> Result<Option<Contact>>;

    // Поиск и список
    async fn find_by_email(&self, email: &str) -> Result<Option<Contact>>;
    async fn find_by_name(&self, name: &str) -> Result<Vec<Contact>>;
    async fn search_contacts(&self, query: &str, limit: usize) -> Result<Vec<Contact>>;
    async fn list_contacts(&self, limit: usize, offset: usize) -> Result<Vec<Contact>>;

    // Синхронизация (добавлено 2026-10-15)
    async fn get_contacts_since(&self, timestamp: i64) -> Result<Vec<Contact>>;
    async fn count_contacts(&self) -> Result<u64>;
}
```

**Семантика новых методов:**

`get_notes_since(timestamp)` — получить заметки, изменённые после указанного времени.

SQL:
```sql
SELECT * FROM notes
WHERE updated_at > ? AND deleted = 0
ORDER BY updated_at ASC;
```

Использование: sync-адаптер `tensa-sync` — для передачи изменённых заметок другому узлу.
Параметры: `timestamp` — Unix timestamp (последний `last_note_updated_at` из `sync_state`).
Возвращает: список заметок, упорядоченный по `updated_at`.

`count_notes()` — получить общее количество заметок (не удалённых).

SQL:
```sql
SELECT COUNT(*) FROM notes WHERE deleted = 0;
```

Использование: `DataService.total` — для возврата общего количества, не размера страницы.
Возвращает: `u64`.

`get_contacts_since(timestamp)` — получить контакты, изменённые после указанного времени.

SQL:
```sql
SELECT * FROM contacts
WHERE updated_at > ? AND deleted = 0
ORDER BY updated_at ASC;
```

Использование: sync-адаптер `tensa-sync` — для передачи изменённых контактов.
Параметры: `timestamp` — Unix timestamp (последний `last_contact_updated_at` из `sync_state`).
Возвращает: список контактов, упорядоченный по `updated_at`.

`count_contacts()` — получить общее количество контактов (не удалённых).

SQL:
```sql
SELECT COUNT(*) FROM contacts WHERE deleted = 0;
```

Использование: `DataService.total`.
Возвращает: `u64`.

**Поиск:**

Полнотекстовый поиск (FTS5):
- Заметки: по `title`, `body`, `tags`.
- Контакты: по `name`, `email`, `notes`, `tags`.
- Поддержка русского языка: FTS5 с `unicode61` tokenizer.
- Внимание: FTS5 не матчит словоформы (`«Иван» ≠ «Иваном»`). Для этого используется AND-префикс (`слово*`) или расширение индекса.

Семантический поиск:
- Заметки и контакты индексируются в Lance (ADR-004).
- Эмбеддинги: `intfloat/multilingual-e5-small`.
- Префиксы: `query:` / `passage:`.

Гибридный поиск:
- FTS5 + семантический поиск.
- RRF (Reciprocal Rank Fusion).

**Синхронизация:**
- Заметки и контакты синхронизируются через gRPC (ADR-012).
- On-demand sync (ADR-013).
- Конфликты: LWW по `updated_at` + `vector_clock`.
- Удаление: soft delete (`deleted = 1`) для синхронизации.
- Физическое удаление: после синхронизации на всех узлах.
- Дельта-синхронизация: через `get_notes_since` / `get_contacts_since` (добавлено 2026-10-15).

**Связанные ADR и контракты:**
- ADR-001: Rust как язык ядра.
- ADR-004: Lance для векторного поиска.
- ADR-005: fastembed-rs для эмбеддингов.
- ADR-012: gRPC (tonic) для синхронизации.
- ADR-013: On-demand синхронизация.
- ADR-015: Почта напрямую.
- Контракт 8: gRPC-контракты.
- Контракт 9: uniffi UDL.

**История изменений:**

| Дата | Версия | Изменения |
|---|---|---|
| 2026-10-06 | 0.1.0 | Первоначальная версия |
| 2026-10-15 | 0.1.1 | Добавлены `get_notes_since`, `count_notes`, `get_contacts_since`, `count_contacts` для дельта-синхронизации и подсчёта |

## Контракт 4: Конфигурация (TOML)

**Назначение:**
Конфигурация Tensa хранит настройки, учётные данные и параметры работы. Читается при старте ядра, может обновляться через UI.

Принципы: один файл `~/.tensa/config.toml`, секреты — в `secrets.toml`, не синхронизируется, валидация при загрузке.

**Расположение:**
- Файл: `~/.tensa/config.toml`.
- Секреты: `~/.tensa/secrets.toml`.
- Схема: `crates/tensa-core/config.rs`.

**Структура (фрагмент):**

```toml
[general]
node_id = "550e8400-e29b-41d4-a716-446655440000"
language = "ru"
device_type = "desktop"
role = "auto"

[brain]
mode = "auto"
local_model = "TheStageAI/gemma-4-E2B-it-GGUF"
local_model_tier = "XS"
local_model_file = "gemma-4-E2B-it-XS-TS-Q3_K_S.gguf"
local_model_path = "~/.tensa/models/gemma-4-E2B-it-XS-TS-Q3_K_S.gguf"
cloud_models = [
    "google/gemma-4-26b-a4b-it:free",
    "google/gemma-4-26b-a4b-it",
]
cloud_auto_fallback = true
cloud_free_timeout_seconds = 30
cloud_free_max_retries = 2
cloud_fallback_to_local = true
openrouter_daily_limit_tokens = 100000
openrouter_monthly_limit_usd = 10.0
openrouter_warn_at_percent = 80

[memory]
chunk_size = 1000
chunk_overlap = 200
embedding_model = "intfloat/multilingual-e5-small"
embedding_dim = 384
index_type = "flat"

[voice]
vad_enabled = true
vad_model = "snakers4/silero-vad"
vad_min_speech_ms = 100
asr_model = "whisper.cpp"
asr_model_file = "ggml-base.bin"
asr_model_path = "~/.tensa/models/ggml-base.bin"
asr_language = "ru"

[sync]
enabled = true
grpc_port = 50051
grpc_tls = true
tailscale_enabled = true
tailscale_hostname = "tensa-desktop"
syncthing_enabled = true
syncthing_port = 8384

[approval]
action_ttl_hours = 24
auto_approve_trusted = false
allow_email_send = true
allow_telegram_send = true
allow_whatsapp_send = false
confirm_unknown_recipients = true

[telegram]
enabled = true
owner_chat_id = "123456789"
mode = "auto"

[email]
default_account = "personal"

[[email.accounts]]
id = "personal"
label = "Личный"
imap_host = "imap.example.com"
imap_port = 993
imap_use_tls = true
imap_auth = "password"
smtp_host = "smtp.example.com"
smtp_port = 587
smtp_use_tls = true
smtp_auth = "password"
auto_detect = true
auto_detect_timeout_seconds = 10

[calendar]
enabled = true
caldav_url = "https://caldav.example.com"
caldav_user = "user@example.com"

[apps]
enabled = true
require_approval = true

[security]
encryption_enabled = false
remote_wipe_enabled = false

[updates]
llama_cpp_auto_update = false
llama_cpp_check_interval_hours = 24
tensa_auto_update = false

[documents]
formats = ["pdf", "docx", "xlsx", "pptx", "txt", "md"]
ocr_engine = "rusto"
ocr_model = "cyrillic_ocr"
ocr_languages = ["rus", "eng"]
pdf_renderer = "pdfplumber-rs"
docx_library = "docx-rs"
xlsx_library = "calamine"
pptx_library = "custom"
chunk_size = 1000
chunk_overlap = 200
max_file_size_mb = 100
skip_macros = true
skip_embedded_objects = true
```

**Секреты (`secrets.toml`):**

```toml
[openrouter]
# api_key — читается из окружения (OPENROUTER_API_KEY)

[telegram]
# bot_token — читается из окружения (TELEGRAM_BOT_TOKEN)

[email.accounts.personal]
# imap_password — читается из окружения (IMAP_PASSWORD)
# smtp_password — читается из окружения (SMTP_PASSWORD)

[calendar]
# caldav_password — читается из окружения (CALDAV_PASSWORD)

[syncthing]
# api_key — читается из окружения (SYNCTHING_API_KEY)
```

**Загрузка и валидация:** при старте, валидация полей, создание с дефолтами при отсутствии, атомарная запись, бэкап.

**Миграция:** версия конфига, миграции при обновлении.

**Связанные ADR:** ADR-001, ADR-003, ADR-004, ADR-005, ADR-012, ADR-014, ADR-015, ADR-016, ADR-017, ADR-018.

## Контракт 5: Белый список приложений (TOML)

**Назначение:**
Белый список приложений определяет, какие приложения Tensa может запускать. Элемент безопасности: LLM не должна запускать произвольные команды.

Принципы: Tensa сканирует систему и показывает список найденных приложений. Пользователь выбирает галочками. `apps.toml` заполняется автоматически. Запуск из ненадёжного источника — через ApprovalStore. Логирование всех запусков.

**Расположение:**
- Файл: `~/.tensa/apps.toml`.
- Кэш сканирования: `~/.tensa/apps_cache.json`.
- Иконки: `~/.tensa/icons/`.
- Схема: `crates/tensa-tools/apps/schema.rs`.

**Структура:**

```toml
[general]
require_approval = true
log_launches = true
max_runtime_seconds = 0

[windows]
apps = {}
associations = {}

[linux]
apps = {}
associations = {}

[android]
apps = {}
associations = {}

[security]
[security.denylist]
windows = ["cmd.exe", "powershell.exe", "regedit.exe", "mmc.exe"]
linux = ["rm", "dd", "mkfs", "shutdown", "reboot"]
android = ["com.android.settings", "com.android.systemui"]

[security.forbidden_args]
windows = ["/c", "-Command", "-EncodedCommand"]
linux = ["-c", "--command", "-e"]
android = []
```

**Сканирование системы:**

Windows: реестр (`App Paths`), Пуск, `%LOCALAPPDATA%\Programs`, `%ProgramFiles%`.

Linux: `.desktop`-файлы, Snap, Flatpak, AppImage.

Android: `PackageManager.getInstalledApplications()` — только запускаемые (`ACTION_MAIN` + `CATEGORY_LAUNCHER`).

**Кэш сканирования:** `~/.tensa/apps_cache.json` — результат сканирования для UI. Иконки — в `~/.tensa/icons/`.

**Периодичность сканирования:** раз в день + по запросу.

**API (трейт):**

```rust
pub trait AppScanner {
    async fn scan(&self) -> Result<Vec<ScannedApp>>;
    async fn scan_cached(&self) -> Result<Vec<ScannedApp>>;
}

pub trait AppLauncher {
    async fn launch(&self, name: &str) -> Result<()>;
    async fn open_file(&self, path: &Path) -> Result<()>;
    async fn open_file_with(&self, name: &str, path: &Path) -> Result<()>;
    async fn open_url(&self, url: &str) -> Result<()>;
    fn list_apps(&self) -> Vec<String>;
    fn supports(&self, name: &str) -> bool;
}
```

**UI:** вкладка «Приложения» в настройках. Список найденных, чекбоксы, поиск, фильтр. Кнопки: «Обновить список», «Добавить вручную», «Импорт/Экспорт».

**Безопасность:** белый список (пустой по умолчанию), чёрный список (опасные утилиты), запрещённые аргументы (инъекции), ApprovalStore, логирование.

**Связанные ADR:** ADR-001, ADR-002, ADR-012, ADR-017.

## Контракт 6: Учётные записи почты (TOML)

**Назначение:**
Tensa работает с почтой (IMAP/SMTP) напрямую, без шлюза (ADR-015). В MVP — один ящик, в v0.2+ — несколько.

Принципы: прямая работа с IMAP/SMTP, несколько ящиков (архитектура готова), один активный ящик на узел, секреты отдельно, работа без ПК.

**Расположение:**
- Файл: `~/.tensa/config.toml` (секция `[email]`).
- Секреты: `~/.tensa/secrets.toml`.
- Схема: `crates/tensa-tools/email/schema.rs`.

**Структура (фрагмент):**

```toml
[email]
default_account = "personal"

[[email.accounts]]
id = "personal"
label = "Личный"
imap_host = "imap.example.com"
imap_port = 993              # настраиваемый, 0 = автоопределение
imap_use_tls = true
imap_auth = "password"       # password, oauth2, plain, login
smtp_host = "smtp.example.com"
smtp_port = 587              # настраиваемый, 0 = автоопределение
smtp_use_tls = true
smtp_auth = "password"
auto_detect = true
auto_detect_timeout_seconds = 10

[email.sync]
active_account = "personal"
connection_mode = "active_standby"  # active_standby, both
connection_ttl_seconds = 900
use_idle = true
send_queue_enabled = true
send_queue_max_retries = 3
send_id_header = "X-Tensa-Send-Id"

[email.folders]
inbox = "INBOX"
sent = "Sent"
drafts = "Drafts"
trash = "Trash"
archive = "Archive"
skip = ["Junk", "Spam"]

[email.indexing]
index_attachments = true
index_body = true
max_size_mb = 25
max_batch = 100

[email.security]
require_approval = true
confirm_unknown_recipients = true
allowed_domains = []
denied_domains = []
max_attachment_mb = 25
```

**Автоопределение (все методы):**
1. SRV-записи DNS (`_imaps._tcp`, `_imap._tcp`, `_submission._tcp`, `_smtps._tcp`).
2. Autodiscover (Outlook/Exchange).
3. Mozilla Autoconfig.
4. PortScan (IMAP: 993, 143; SMTP: 587, 465, 25).
5. Ручной ввод.

**OAuth2:** поддерживается (Gmail, Outlook, Яндекс, custom). Токены — в `secrets.toml`.

**Проверка соединения (Вариант D):**
- Debounce 500ms при вводе.
- Асинхронный UI-индикатор (spinner).
- Визуализация статуса (зелёный/красный).
- Финальная валидация при сохранении.

**API (трейт):**

```rust
pub trait EmailClient {
    async fn connect(&self, account_id: &str) -> Result<()>;
    async fn disconnect(&self, account_id: &str) -> Result<()>;
    async fn list_folders(&self, account_id: &str) -> Result<Vec<Folder>>;
    async fn fetch_emails(&self, account_id: &str, folder: &str, limit: usize) -> Result<Vec<Email>>;
    async fn search(&self, account_id: &str, query: &str) -> Result<Vec<Email>>;
    async fn send(&self, email: OutgoingEmail) -> Result<()>;
    async fn create_draft(&self, email: OutgoingEmail) -> Result<()>;
    async fn mark_read(&self, account_id: &str, uid: u32) -> Result<()>;
}

pub trait EmailAutoDetect {
    async fn detect(&self, email: &str) -> Result<DetectedSettings>;
    async fn detect_srv(&self, domain: &str) -> Result<Option<DetectedSettings>>;
    async fn detect_autodiscover(&self, domain: &str) -> Result<Option<DetectedSettings>>;
    async fn detect_autoconfig(&self, domain: &str) -> Result<Option<DetectedSettings>>;
    async fn try_ports(&self, host: &str, ports: &[u16]) -> Result<Option<u16>>;
}
```

**Управление соединениями:** active + standby, TTL 15 минут, явный сигнал, очередь при конфликте, fallback если сервер не ограничивает.

**Идемпотентность отправки:** `X-Tensa-Send-Id` (UUID).

**Связанные ADR:** ADR-001, ADR-002, ADR-004, ADR-011, ADR-012, ADR-015.

## Контракт 7: Реестр узлов (SQLite)

**Назначение:**
Реестр узлов (NodeRegistry) хранит информацию о всех узлах Tensa в сети. Используется для синхронизации, on-demand запросов, динамического распределения ролей, управления соединениями, WoL (отложено).

**Расположение:**
- Файл: `~/.tensa/nodes.db` (SQLite).
- Схема: `crates/tensa-nodes/schema.sql`.

**Таблицы:**

`nodes` — реестр узлов:

| Поле | Тип | Описание |
|---|---|---|
| node_id | TEXT (UUID) | ID |
| name | TEXT | Имя |
| device_type | TEXT | desktop, android |
| platform | TEXT | windows, linux, android |
| role | TEXT | heavy, thin, auto |
| tailscale_ip | TEXT | IP |
| grpc_port | INTEGER | Порт |
| last_seen | INTEGER | Unix timestamp |
| online | INTEGER | 0/1 |
| ram_total_mb | INTEGER | RAM |
| ram_available_mb | INTEGER | Доступная RAM |
| cpu_cores | INTEGER | Ядра |
| cpu_usage_percent | REAL | Загрузка |
| has_gpu | INTEGER | 0/1 |
| has_npu | INTEGER | 0/1 |
| battery_percent | INTEGER | Батарея |
| battery_charging | INTEGER | 0/1 |
| thermal_status | TEXT | nominal, fair, serious, critical |
| resources | TEXT (JSON) | Ресурсы |
| created_at | INTEGER | Unix timestamp |
| updated_at | INTEGER | Unix timestamp |

`node_heartbeats` — история heartbeat.

`node_connections` — активные соединения (resource, account_id, active, acquired_at, expires_at).

**Ресурсы (JSON):**

```json
{
  "telegram": true,
  "email": true,
  "whatsapp": false,
  "llm_local": true,
  "llm_cloud": true,
  "memory_store": true,
  "approval_store": true
}
```

**API (трейт):**

```rust
pub trait NodeRegistry {
    async fn register(&self, node: Node) -> Result<()>;
    async fn unregister(&self, node_id: Uuid) -> Result<()>;
    async fn update_heartbeat(&self, node_id: Uuid, heartbeat: Heartbeat) -> Result<()>;
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

**Heartbeat:** каждые 30 секунд, офлайн > 90 секунд, RAM/CPU/батарея/thermal, синхронизация через gRPC.

**Роли:** heavy, thin, auto.

**Связанные ADR:** ADR-001, ADR-012, ADR-013, ADR-014, ADR-015, ADR-016.

## Контракт 8: gRPC-контракты (Protobuf)

**Назначение:**
gRPC-контракты определяют, как узлы Tensa общаются между собой (ADR-012). Все сервисы описаны в `.proto`-файлах.

Принципы: типизация (Protobuf), идемпотентность (operation_id), векторные часы, стриминг, mTLS.

**Расположение:**
- Файлы: `docs/contracts/proto/`.
- Кодогенерация: `tonic-build` в `build.rs`.
- Rust-крейт: `crates/tensa-sync/`.

**Файлы:**

| Файл | Назначение |
|---|---|
| common.proto | Общие типы (UUID, VectorClock, Empty, Error) |
| sync.proto | Синхронизация ApprovalStore, памяти, заметок |
| node.proto | Реестр узлов, heartbeat |
| data.proto | On-demand запросы данных |
| voice.proto | Передача аудио, распознавание |
| query.proto | Текстовые запросы к ядру |

**common.proto:**

```protobuf
syntax = "proto3";
package tensa.common;

message Uuid { string value = 1; }

message VectorClock {
  map<string, uint64> clocks = 1;
}

message Empty {}

message Error {
  string code = 1;
  string message = 2;
  string details = 3;
}

message Result {
  bool success = 1;
  Error error = 2;
  string operation_id = 3;
}

enum Language {
  LANGUAGE_UNSPECIFIED = 0;
  LANGUAGE_RU = 1;
  LANGUAGE_EN = 2;
}

enum Platform {
  PLATFORM_UNSPECIFIED = 0;
  PLATFORM_WINDOWS = 1;
  PLATFORM_LINUX = 2;
  PLATFORM_ANDROID = 3;
  PLATFORM_MACOS = 4;
  PLATFORM_IOS = 5;
}

enum NodeRole {
  NODE_ROLE_UNSPECIFIED = 0;
  NODE_ROLE_HEAVY = 1;
  NODE_ROLE_THIN = 2;
  NODE_ROLE_AUTO = 3;
}

enum ActionStatus {
  ACTION_STATUS_UNSPECIFIED = 0;
  ACTION_STATUS_PENDING = 1;
  ACTION_STATUS_APPROVED = 2;
  ACTION_STATUS_REJECTED = 3;
  ACTION_STATUS_EXECUTED = 4;
  ACTION_STATUS_FAILED = 5;
  ACTION_STATUS_EXPIRED = 6;
}

enum ActionKind {
  ACTION_KIND_UNSPECIFIED = 0;
  ACTION_KIND_EMAIL_SEND = 1;
  ACTION_KIND_TELEGRAM_SEND = 2;
  ACTION_KIND_WHATSAPP_SEND = 3;
  ACTION_KIND_CALENDAR_CREATE = 4;
  ACTION_KIND_APP_LAUNCH = 5;
  ACTION_KIND_FILE_OPEN = 6;
}
```

**sync.proto:**

```protobuf
syntax = "proto3";
package tensa.sync;

import "common.proto";

service SyncService {
  rpc SyncApprovalEvents(SyncApprovalRequest) returns (stream ApprovalEvent);
  rpc PushApprovalEvent(ApprovalEvent) returns (tensa.common.Result);

  rpc SyncNotes(SyncNotesRequest) returns (stream NoteEvent);
  rpc PushNoteEvent(NoteEvent) returns (tensa.common.Result);

  rpc SyncContacts(SyncContactsRequest) returns (stream ContactEvent);
  rpc PushContactEvent(ContactEvent) returns (tensa.common.Result);

  rpc SubscribeEvents(SubscribeRequest) returns (stream Event);
}

message SyncApprovalRequest {
  string node_id = 1;
  uint64 last_event_id = 2;
  tensa.common.VectorClock vector_clock = 3;
}

message ApprovalEvent {
  uint64 event_id = 1;
  string action_id = 2;
  string event_type = 3;
  string event_data = 4;
  int64 timestamp = 5;
  string node_id = 6;
  tensa.common.VectorClock vector_clock = 7;
  string operation_id = 8;
}

message SyncNotesRequest {
  string node_id = 1;
  int64 last_updated_at = 2;
  tensa.common.VectorClock vector_clock = 3;
}

message NoteEvent {
  string note_id = 1;
  string event_type = 2;
  string note_data = 3;
  int64 timestamp = 4;
  string node_id = 5;
  tensa.common.VectorClock vector_clock = 6;
}

message SyncContactsRequest {
  string node_id = 1;
  int64 last_updated_at = 2;
  tensa.common.VectorClock vector_clock = 3;
}

message ContactEvent {
  string contact_id = 1;
  string event_type = 2;
  string contact_data = 3;
  int64 timestamp = 4;
  string node_id = 5;
  tensa.common.VectorClock vector_clock = 6;
}

message SubscribeRequest {
  string node_id = 1;
  repeated string event_types = 2;
}

message Event {
  string event_type = 1;
  string data = 2;
  int64 timestamp = 3;
  string node_id = 4;
}
```

**node.proto:**

```protobuf
syntax = "proto3";
package tensa.node;

import "common.proto";

service NodeService {
  rpc Register(RegisterRequest) returns (tensa.common.Result);
  rpc Unregister(UnregisterRequest) returns (tensa.common.Result);

  rpc Heartbeat(HeartbeatRequest) returns (tensa.common.Result);
  rpc StreamHeartbeats(StreamHeartbeatsRequest) returns (stream HeartbeatRequest);

  rpc ListNodes(ListNodesRequest) returns (ListNodesResponse);
  rpc GetNode(GetNodeRequest) returns (Node);

  rpc AcquireConnection(AcquireConnectionRequest) returns (tensa.common.Result);
  rpc ReleaseConnection(ReleaseConnectionRequest) returns (tensa.common.Result);
  rpc GetActiveConnection(GetActiveConnectionRequest) returns (GetActiveConnectionResponse);
}

message RegisterRequest {
  Node node = 1;
}

message UnregisterRequest {
  string node_id = 1;
}

message Node {
  string node_id = 1;
  string name = 2;
  string device_type = 3;
  tensa.common.Platform platform = 4;
  tensa.common.NodeRole role = 5;
  string tailscale_ip = 6;
  uint32 grpc_port = 7;
  int64 last_seen = 8;
  bool online = 9;
  uint64 ram_total_mb = 10;
  uint64 ram_available_mb = 11;
  uint32 cpu_cores = 12;
  float cpu_usage_percent = 13;
  bool has_gpu = 14;
  bool has_npu = 15;
  optional uint32 battery_percent = 16;
  optional bool battery_charging = 17;
  string thermal_status = 18;
  NodeResources resources = 19;
  int64 created_at = 20;
  int64 updated_at = 21;
}

message NodeResources {
  bool telegram = 1;
  bool email = 2;
  bool whatsapp = 3;
  bool llm_local = 4;
  bool llm_cloud = 5;
  bool memory_store = 6;
  bool approval_store = 7;
}

message HeartbeatRequest {
  string node_id = 1;
  uint64 ram_available_mb = 2;
  float cpu_usage_percent = 3;
  optional uint32 battery_percent = 4;
  optional bool battery_charging = 5;
  string thermal_status = 6;
  int64 timestamp = 7;
}

message StreamHeartbeatsRequest {
  string node_id = 1;
}

message ListNodesRequest {
  bool online_only = 1;
}

message ListNodesResponse {
  repeated Node nodes = 1;
}

message GetNodeRequest {
  string node_id = 1;
}

message AcquireConnectionRequest {
  string node_id = 1;
  string resource = 2;
  optional string account_id = 3;
}

message ReleaseConnectionRequest {
  string node_id = 1;
  string resource = 2;
  optional string account_id = 3;
}

message GetActiveConnectionRequest {
  string resource = 1;
  optional string account_id = 2;
}

message GetActiveConnectionResponse {
  optional string node_id = 1;
}
```

**data.proto:**

```protobuf
syntax = "proto3";
package tensa.data;

import "common.proto";

service DataService {
  rpc GetNotes(GetNotesRequest) returns (GetNotesResponse);
  rpc GetNote(GetNoteRequest) returns (Note);

  rpc GetContacts(GetContactsRequest) returns (GetContactsResponse);
  rpc GetContact(GetContactRequest) returns (Contact);

  rpc SearchFiles(SearchFilesRequest) returns (stream SearchResult);

  rpc GetApprovalQueue(GetApprovalQueueRequest) returns (GetApprovalQueueResponse);
}

message GetNotesRequest {
  string node_id = 1;
  uint32 limit = 2;
  uint32 offset = 3;
}

message GetNotesResponse {
  repeated Note notes = 1;
  uint32 total = 2;
}

message GetNoteRequest {
  string note_id = 1;
}

message Note {
  string note_id = 1;
  string title = 2;
  string body = 3;
  repeated string tags = 4;
  int64 created_at = 5;
  int64 updated_at = 6;
}

message GetContactsRequest {
  string node_id = 1;
  uint32 limit = 2;
  uint32 offset = 3;
}

message GetContactsResponse {
  repeated Contact contacts = 1;
  uint32 total = 2;
}

message GetContactRequest {
  string contact_id = 1;
}

message Contact {
  string contact_id = 1;
  string name = 2;
  optional string email = 3;
  optional string phone = 4;
  optional string telegram = 5;
  optional string whatsapp = 6;
}

message SearchFilesRequest {
  string node_id = 1;
  string query = 2;
  uint32 limit = 3;
}

message SearchResult {
  string document_id = 1;
  string source_path = 2;
  string chunk_text = 3;
  float score = 4;
  int64 indexed_at = 5;
}

message GetApprovalQueueRequest {
  string node_id = 1;
}

message GetApprovalQueueResponse {
  repeated ApprovalAction actions = 1;
}

message ApprovalAction {
  string action_id = 1;
  tensa.common.ActionKind kind = 2;
  string payload = 3;
  tensa.common.ActionStatus status = 4;
  int64 created_at = 5;
  int64 expires_at = 6;
}
```

**voice.proto:**

```protobuf
syntax = "proto3";
package tensa.voice;

import "common.proto";

service VoiceService {
  rpc Transcribe(stream AudioChunk) returns (TranscribeResponse);
  rpc TranscribeFile(TranscribeFileRequest) returns (TranscribeResponse);

  rpc DetectVoiceActivity(stream AudioChunk) returns (stream VadResult);
}

message AudioChunk {
  bytes data = 1;
  uint32 sample_rate = 2;
  uint32 channels = 3;
  string format = 4;
}

message TranscribeResponse {
  string text = 1;
  string language = 2;
  float confidence = 3;
  repeated Segment segments = 4;
}

message Segment {
  string text = 1;
  float start = 2;
  float end = 3;
  float confidence = 4;
}

message TranscribeFileRequest {
  string node_id = 1;
  bytes audio_data = 2;
  string format = 3;
  string language = 4;
}

message VadResult {
  bool speech_detected = 1;
  float start = 2;
  float end = 3;
}
```

**query.proto:**

```protobuf
syntax = "proto3";
package tensa.query;

import "common.proto";
import "voice.proto";

service QueryService {
  rpc Query(QueryRequest) returns (QueryResponse);
  rpc StreamQuery(QueryRequest) returns (stream QueryChunk);

  rpc VoiceQuery(stream tensa.voice.AudioChunk) returns (QueryResponse);
}

message QueryRequest {
  string node_id = 1;
  string text = 2;
  tensa.common.Language language = 3;
  map<string, string> context = 4;
  bool use_cloud = 5;
}

message QueryResponse {
  string text = 1;
  string action = 2;
  string action_payload = 3;
  repeated ToolCall tool_calls = 4;
  string reasoning = 5;
  bool degraded = 6;
  string model_used = 7;
}

message ToolCall {
  string tool = 1;
  string params = 2;
}

message QueryChunk {
  string text = 1;
  bool done = 2;
}
```

**Кодогенерация:** `tonic-build` в `build.rs` в `crates/tensa-sync`.

**Стриминг:** SyncApprovalEvents, SyncNotes, SyncContacts, SubscribeEvents, SearchFiles, StreamHeartbeats, Transcribe, StreamQuery.

**Идемпотентность:** operation_id (UUID).

**Векторные часы:** vector_clock в каждом событии.

**Аутентификация:** mTLS или Tailscale-шифрование + токены.

**Связанные ADR:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-012, ADR-013, ADR-014, ADR-015.

## Контракт 9: uniffi UDL (FFI)

**Назначение:**
uniffi UDL определяет, как Rust-ядро Tensa общается с UI (Kotlin/Flutter на Android, Swift на iOS — после MVP). UDL — UniFFI Definition Language.

Принципы: автоматическая генерация биндингов, строгая типизация, async, кросс-платформенность, минимум ручного кода.

**Расположение:**
- Файл: `docs/contracts/ffi/tensa.udl`.
- Кодогенерация: `uniffi-bindgen` в `build.rs`.
- Rust-крейт: `crates/tensa-ffi/`.

**Что покрывает:**

| Компонент | Покрытие |
|---|---|
| ApprovalStore | request_action, approve, reject, get_pending |
| Память | index_document, search, get_document |
| Заметки | create_note, update_note, search_notes |
| Контакты | create_contact, find_by_email, search_contacts |
| Почта | connect, send, fetch_emails |
| Календарь | create_event, check_conflicts |
| Приложения | launch, open_file, list_apps |
| Узлы | list_nodes, get_node, acquire_connection |
| Конфигурация | get_config, update_config |
| Статус | health, status |

**tensa.udl (полный):**

```udl
namespace tensa {
    // Инициализация ядра
    [Throws=TensaError]
    void init(string config_path);

    // Проверка состояния
    [Throws=TensaError]
    HealthStatus health();

    // Версия
    string version();
};

// === Общие типы ===

[Error]
enum TensaError {
    "ConfigError",
    "NetworkError",
    "ApprovalError",
    "MemoryError",
    "EmailError",
    "CalendarError",
    "AppError",
    "NodeError",
    "InternalError",
};

dictionary HealthStatus {
    boolean healthy;
    string version;
    string uptime;
    boolean degraded;
    string? degraded_reason;
};

// === ApprovalStore ===

dictionary Action {
    string id;
    string kind;
    string payload;
    string status;
    i64 created_at;
    i64 expires_at;
    string? approved_at;
    string? executed_at;
    string? result;
};

dictionary NewAction {
    string kind;
    string payload;
};

interface ApprovalStore {
    [Throws=TensaError]
    Action request_action(NewAction action);

    [Throws=TensaError]
    Action approve(string id, string approved_by);

    [Throws=TensaError]
    Action reject(string id, string rejected_by);

    [Throws=TensaError]
    sequence<Action> get_pending();

    [Throws=TensaError]
    Action? get_by_id(string id);
};

// === Память ===

dictionary SearchResult {
    string document_id;
    string source_path;
    string chunk_text;
    f32 score;
    i64 indexed_at;
};

dictionary DocumentMeta {
    string document_id;
    string source_path;
    string title;
    string mime_type;
    i64 size_bytes;
    i64 created_at;
    i64 modified_at;
    i64 indexed_at;
    i32 chunk_count;
};

interface MemoryStore {
    [Throws=TensaError]
    sequence<SearchResult> search(string query, u32 limit);

    [Throws=TensaError]
    sequence<SearchResult> search_hybrid(string query, u32 limit);

    [Throws=TensaError]
    DocumentMeta? get_document(string document_id);

    [Throws=TensaError]
    sequence<DocumentMeta> list_documents(u32 limit, u32 offset);
};

// === Core Memory (ADR-021, Контракт 13) ===

enum BlockType {
    "Persona",
    "Human",
    "Tasks",
    "Preferences",
};

enum MemoryOrigin {
    "Llm",
    "User",
    "System",
};

dictionary CoreMemoryBlock {
    string id;
    BlockType block;
    string value;
    i32 size_chars;
    i32 size_tokens;
    i64 created_at;
    i64 updated_at;
    MemoryOrigin updated_by;
    string updated_on_node;
};

interface CoreMemoryStore {
    [Throws=TensaError]
    CoreMemoryBlock get_block(BlockType block);

    [Throws=TensaError]
    sequence<CoreMemoryBlock> get_all_blocks();

    [Throws=TensaError]
    CoreMemoryBlock replace_block(BlockType block, string value, MemoryOrigin origin);

    [Throws=TensaError]
    CoreMemoryBlock append_block(BlockType block, string value, MemoryOrigin origin);
};

// === Заметки ===

dictionary Note {
    string id;
    string title;
    string body;
    sequence<string> tags;
    i64 created_at;
    i64 updated_at;
};

dictionary NewNote {
    string title;
    string body;
    sequence<string> tags;
};

interface NotesStore {
    [Throws=TensaError]
    Note create_note(NewNote note);

    [Throws=TensaError]
    Note update_note(Note note);

    [Throws=TensaError]
    void delete_note(string id);

    [Throws=TensaError]
    Note? get_note(string id);

    [Throws=TensaError]
    sequence<Note> search_notes(string query, u32 limit);

    [Throws=TensaError]
    sequence<Note> list_notes(u32 limit, u32 offset);
};

// === Контакты ===

dictionary Contact {
    string id;
    string name;
    string? email;
    string? phone;
    string? telegram;
    string? whatsapp;
    string? notes;
    sequence<string> tags;
    i64 created_at;
    i64 updated_at;
};

dictionary NewContact {
    string name;
    string? email;
    string? phone;
    string? telegram;
    string? whatsapp;
    string? notes;
    sequence<string> tags;
};

interface ContactsStore {
    [Throws=TensaError]
    Contact create_contact(NewContact contact);

    [Throws=TensaError]
    Contact update_contact(Contact contact);

    [Throws=TensaError]
    void delete_contact(string id);

    [Throws=TensaError]
    Contact? get_contact(string id);

    [Throws=TensaError]
    Contact? find_by_email(string email);

    [Throws=TensaError]
    sequence<Contact> find_by_name(string name);

    [Throws=TensaError]
    sequence<Contact> search_contacts(string query, u32 limit);
};

// === Почта ===

dictionary EmailAccount {
    string id;
    string label;
    string imap_host;
    u16 imap_port;
    string smtp_host;
    u16 smtp_port;
};

dictionary Email {
    string id;
    string account_id;
    string folder;
    string from;
    string to;
    string subject;
    string body;
    i64 date;
    boolean read;
    boolean has_attachments;
};

dictionary OutgoingEmail {
    string account_id;
    string to;
    string? cc;
    string? bcc;
    string subject;
    string body;
    sequence<string> attachments;
};

interface EmailClient {
    [Throws=TensaError]
    void connect(string account_id);

    [Throws=TensaError]
    void disconnect(string account_id);

    [Throws=TensaError]
    sequence<string> list_folders(string account_id);

    [Throws=TensaError]
    sequence<Email> fetch_emails(string account_id, string folder, u32 limit);

    [Throws=TensaError]
    sequence<Email> search(string account_id, string query);

    [Throws=TensaError]
    void send(OutgoingEmail email);

    [Throws=TensaError]
    void create_draft(OutgoingEmail email);
};

// === Календарь ===

dictionary CalendarEvent {
    string id;
    string title;
    string start;
    string end;
    string? description;
    string? location;
    sequence<string> attendees;
};

dictionary NewCalendarEvent {
    string title;
    string start;
    string end;
    string? description;
    string? location;
    sequence<string> attendees;
};

dictionary Conflict {
    string event_id;
    string title;
    string start;
    string end;
};

interface CalendarClient {
    [Throws=TensaError]
    sequence<CalendarEvent> list_events(string start, string end);

    [Throws=TensaError]
    sequence<Conflict> check_conflicts(string start, string end);

    [Throws=TensaError]
    CalendarEvent create_event(NewCalendarEvent event);

    [Throws=TensaError]
    void delete_event(string id);
};

// === Приложения ===

dictionary AppInfo {
    string name;
    string path;
    string? icon_path;
    sequence<string> associations;
};

interface AppLauncher {
    [Throws=TensaError]
    void launch(string name);

    [Throws=TensaError]
    void open_file(string path);

    [Throws=TensaError]
    void open_file_with(string name, string path);

    [Throws=TensaError]
    void open_url(string url);

    [Throws=TensaError]
    sequence<AppInfo> list_apps();

    [Throws=TensaError]
    sequence<AppInfo> scan_apps();

    boolean supports(string name);
};

// === Узлы ===

dictionary Node {
    string node_id;
    string name;
    string device_type;
    string platform;
    string role;
    string tailscale_ip;
    u16 grpc_port;
    i64 last_seen;
    boolean online;
    u64 ram_total_mb;
    u64 ram_available_mb;
    u32 cpu_cores;
    f32 cpu_usage_percent;
    boolean has_gpu;
    boolean has_npu;
    u8? battery_percent;
    boolean? battery_charging;
    string thermal_status;
};

interface NodeRegistry {
    [Throws=TensaError]
    sequence<Node> list_nodes(boolean online_only);

    [Throws=TensaError]
    Node? get_node(string node_id);

    [Throws=TensaError]
    Node? find_heavy();

    [Throws=TensaError]
    void acquire_connection(string node_id, string resource, string? account_id);

    [Throws=TensaError]
    void release_connection(string node_id, string resource, string? account_id);

    [Throws=TensaError]
    string? get_active_connection(string resource, string? account_id);
};

// === Конфигурация ===

dictionary Config {
    string node_id;
    string language;
    string device_type;
    string role;
    string brain_mode;
    string local_model;
    string cloud_model;
};

interface ConfigStore {
    [Throws=TensaError]
    Config get_config();

    [Throws=TensaError]
    void update_config(Config config);

    [Throws=TensaError]
    void reload_config();
};

// === Запросы ===

dictionary QueryRequest {
    string text;
    string language;
    boolean use_cloud;
};

dictionary QueryResponse {
    string text;
    string? action;
    string? action_payload;
    sequence<ToolCall> tool_calls;
    string? reasoning;
    boolean degraded;
    string model_used;
};

dictionary ToolCall {
    string tool;
    string params;
};

interface QueryEngine {
    [Throws=TensaError]
    QueryResponse query(QueryRequest request);
};

// === Голос ===

interface VoiceEngine {
    [Throws=TensaError]
    string transcribe(sequence<u8> audio_data, string format);
};

// === First-Run Wizard (ADR-021, дополнение к tensa-core) ===

dictionary WizardResult {
    boolean config_saved;
    boolean secrets_saved;
    boolean core_memory_initialized;
};

dictionary WhatsAppConfig {
    string access_token;
    string phone_number_id;
    string waba_id;
};

interface FirstRunWizard {
    [Throws=TensaError]
    boolean is_first_run();

    [Throws=TensaError]
    void step_language(string language);

    [Throws=TensaError]
    void step_introduction(
        string owner_name,
        string owner_title,
        string secretary_name,
        string timezone
    );

    [Throws=TensaError]
    void step_models();

    [Throws=TensaError]
    void step_telegram(string bot_token, string owner_chat_id);

    [Throws=TensaError]
    void step_email(EmailAccount account);

    [Throws=TensaError]
    void step_whatsapp(WhatsAppConfig? config);

    [Throws=TensaError]
    void step_tailscale();

    [Throws=TensaError]
    WizardResult step_finish();
};
```

**Кодогенерация:**

```bash
# Kotlin (Android)
uniffi-bindgen generate docs/contracts/ffi/tensa.udl \
    --language kotlin --out-dir apps/android/lib/generated/

# Swift (iOS, после MVP)
uniffi-bindgen generate docs/contracts/ffi/tensa.udl \
    --language swift --out-dir apps/ios/generated/

# Python (CLI)
uniffi-bindgen generate docs/contracts/ffi/tensa.udl \
    --language python --out-dir crates/tensa-cli/generated/
```

**Async:** через `[Async]` в UDL.

**Обработка ошибок:** uniffi генерирует исключения (Kotlin: TensaException.ConfigError, Swift: TensaError.configError, Python: TensaError.ConfigError).

**Что не покрывает:** Tauri IPC (десктоп), PWA HTTP API, gRPC (между узлами).

**Связанные ADR:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-012, ADR-017.

## Контракт 10: Tauri IPC (десктоп)

**Назначение:**
Tauri IPC — механизм, через который Svelte-фронтенд десктопного приложения Tensa общается с Rust-ядром.

Принципы: типизация (Rust-структуры + serde), асинхронность (tokio), события для real-time, минимум кода.

**Расположение:**
- Команды: `apps/desktop/src-tauri/src/commands.rs`.
- События: `apps/desktop/src-tauri/src/events.rs`.
- Типы: `apps/desktop/src-tauri/src/types.rs`.
- JS-биндинги: `apps/desktop/src/lib/api/`.

**Команды (фрагмент):**

```rust
#[tauri::command]
async fn init(config_path: String) -> Result<(), String>;

#[tauri::command]
async fn health() -> Result<HealthStatus, String>;

#[tauri::command]
async fn request_action(action: NewAction) -> Result<Action, String>;

#[tauri::command]
async fn approve(id: String, approved_by: String) -> Result<Action, String>;

#[tauri::command]
async fn get_pending() -> Result<Vec<Action>, String>;

#[tauri::command]
async fn search_files(query: String, limit: u32) -> Result<Vec<SearchResult>, String>;

#[tauri::command]
async fn email_connect(account_id: String) -> Result<(), String>;

#[tauri::command]
async fn email_send(email: OutgoingEmail) -> Result<(), String>;

#[tauri::command]
async fn calendar_create_event(event: NewCalendarEvent) -> Result<CalendarEvent, String>;

#[tauri::command]
async fn app_launch(name: String) -> Result<(), String>;

#[tauri::command]
async fn node_list(online_only: bool) -> Result<Vec<Node>, String>;

#[tauri::command]
async fn config_get() -> Result<Config, String>;

#[tauri::command]
async fn query(request: QueryRequest) -> Result<QueryResponse, String>;

#[tauri::command]
async fn voice_transcribe(audio_data: Vec<u8>, format: String) -> Result<String, String>;
```

**События:**
- `approval:new`, `approval:approved`, `approval:rejected`, `approval:executed`, `approval:failed`, `approval:expired`.
- `status:health`, `status:degraded`, `status:node_online`, `status:node_offline`.
- `sync:started`, `sync:progress`, `sync:completed`, `sync:error`.
- `bootstrap:progress`, `bootstrap:completed`, `bootstrap:error`.
- `update:available`, `update:progress`, `update:completed`, `update:error`.
- `search:started`, `search:result`, `search:completed`.

**JS-биндинги (фрагмент):**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export async function init(configPath: string): Promise<void> {
    return invoke('init', { configPath });
}

export async function health(): Promise<HealthStatus> {
    return invoke('health');
}

export async function getPending(): Promise<Action[]> {
    return invoke('get_pending');
}

export function onApprovalNew(callback: (action: Action) => void) {
    return listen<Action>('approval:new', (event) => callback(event.payload));
}
```

**Безопасность:** Tauri IPC только локально, белый список команд, валидация, без аутентификации.

**Связанные ADR:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-012, ADR-017.

## Контракт 11: Flutter MethodChannel (Android)

**Назначение:**
Flutter MethodChannel — механизм, через который Dart-код Flutter-приложения Tensa общается с нативным Kotlin-кодом, который вызывает Rust-ядро через uniffi.

Схема: `Flutter (Dart) → MethodChannel → Kotlin → uniffi → Rust Core`.

Принципы: типизация, асинхронность, события через EventChannel, минимум кода.

**Расположение:**
- Dart-биндинги: `apps/android/lib/api/tensa.dart`.
- Kotlin-мост: `apps/android/android/app/src/main/kotlin/.../TensaPlugin.kt`.
- uniffi-биндинги: `apps/android/lib/generated/uniffi/tensa/`.

**MethodChannel (фрагмент):**

```dart
class Tensa {
  static const _channel = MethodChannel('com.tensorlogix.tensa');
  static const _events = EventChannel('com.tensorlogix.tensa/events');

  static Future<void> init(String configPath) async {
    await _channel.invokeMethod('init', {'configPath': configPath});
  }

  static Future<HealthStatus> health() async {
    final result = await _channel.invokeMethod('health');
    return HealthStatus.fromMap(result);
  }

  static Future<List<Action>> getPending() async {
    final result = await _channel.invokeMethod('getPending');
    return (result as List).map((e) => Action.fromMap(e)).toList();
  }

  static Future<Action> approve(String id, String approvedBy) async {
    final result = await _channel.invokeMethod('approve', {
      'id': id, 'approvedBy': approvedBy,
    });
    return Action.fromMap(result);
  }

  static Stream<Action> onApprovalNew() {
    return _events
        .receiveBroadcastStream('approval:new')
        .map((e) => Action.fromMap(e));
  }
}
```

**Kotlin-мост (фрагмент):**

```kotlin
class TensaPlugin : FlutterPlugin, MethodChannel.MethodCallHandler, EventChannel.StreamHandler {
    private lateinit var methodChannel: MethodChannel
    private lateinit var eventChannel: EventChannel
    private var eventSink: EventChannel.EventSink? = null
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        scope.launch {
            try {
                when (call.method) {
                    "init" -> {
                        init(call.argument<String>("configPath")!!)
                        result.success(null)
                    }
                    "health" -> result.success(health().toMap())
                    "getPending" -> result.success(ApprovalStore().getPending().map { it.toMap() })
                    // ...
                    else -> result.notImplemented()
                }
            } catch (e: Exception) {
                result.error("TENSA_ERROR", e.message, null)
            }
        }
    }
}
```

**События (EventChannel):** `approval:new`, `approval:approved`, `status:health`, `sync:progress`, `bootstrap:progress`, `search:result`, `update:available`.

**Безопасность:** MethodChannel только внутри приложения, валидация, uniffi — строгая типизация.

**Связанные ADR:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-012, ADR-014, ADR-017.

## Контракт 12: PWA HTTP API (OpenAPI)

**Назначение:**
PWA HTTP API — механизм, через который Svelte-фронтенд PWA-версии Tensa общается с Rust-ядром по HTTP/gRPC (ADR-010).

Принципы: HTTP/JSON, gRPC-Web для стриминга, аутентификация токеном, CORS, OpenAPI.

**Расположение:**
- Файл: `docs/contracts/pwa/openapi.yaml`.
- Rust-реализация: `crates/tensa-core/api/pwa.rs` (axum).
- Svelte-клиент: `apps/pwa/src/lib/api/tensa.ts`.

**Базовый URL:** `http://<tailscale-ip>:8765/api/v1`.

**Аутентификация:** Bearer token в заголовке `Authorization`.

**Эндпоинты (фрагмент):**

```
GET /health
GET /version
GET /app

GET /approvals
POST /approvals/{id}/approve
POST /approvals/{id}/reject
GET /approvals/{id}

GET /search?q={query}&limit={limit}
GET /search/hybrid?q={query}&limit={limit}
GET /documents/{id}
GET /documents?limit={limit}&offset={offset}

GET /notes
POST /notes
PUT /notes/{id}
DELETE /notes/{id}
GET /notes/{id}
GET /notes/search?q={query}&limit={limit}

GET /contacts
POST /contacts
PUT /contacts/{id}
DELETE /contacts/{id}
GET /contacts/{id}
GET /contacts/search?q={query}&limit={limit}

GET /email/accounts
POST /email/accounts/{id}/connect
POST /email/accounts/{id}/disconnect
GET /email/accounts/{id}/folders
GET /email/accounts/{id}/emails?folder={folder}&limit={limit}
GET /email/accounts/{id}/search?q={query}
POST /email/send

GET /calendar/events?start={start}&end={end}
GET /calendar/conflicts?start={start}&end={end}
POST /calendar/events
DELETE /calendar/events/{id}

GET /apps
POST /apps/scan
POST /apps/launch
POST /apps/open-file
POST /apps/open-url

GET /nodes?online_only={bool}
GET /nodes/{id}
GET /nodes/heavy

GET /config
PATCH /config
POST /config/reload

POST /query
POST /query/stream
POST /voice/transcribe

GET /events (SSE)

GET /first-run
POST /first-run/step/language
POST /first-run/step/introduction
POST /first-run/step/models
POST /first-run/step/telegram
POST /first-run/step/email
POST /first-run/step/whatsapp
POST /first-run/step/tailscale
POST /first-run/step/finish
```

**OpenAPI (фрагмент):**

```yaml
openapi: 3.0.0
info:
  title: Tensa PWA API
  version: 1.0.0

servers:
  - url: http://localhost:8765/api/v1
  - url: http://{tailscale_ip}:8765/api/v1

security:
  - BearerAuth: []

paths:
  /health:
    get:
      summary: Проверка состояния
      responses:
        '200':
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HealthStatus'

  /approvals:
    get:
      summary: Очередь одобрений
      responses:
        '200':
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Action'

components:
  securitySchemes:
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT

  schemas:
    HealthStatus:
      type: object
      properties:
        healthy: { type: boolean }
        version: { type: string }
        uptime: { type: string }
        degraded: { type: boolean }
        degraded_reason: { type: string, nullable: true }

    Action:
      type: object
      properties:
        id: { type: string }
        kind: { type: string }
        payload: { type: string }
        status: { type: string }
        created_at: { type: integer, format: int64 }
        expires_at: { type: integer, format: int64 }
```

**Rust-реализация (axum, фрагмент):**

```rust
pub fn pwa_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/approvals", get(get_approvals))
        .route("/approvals/:id/approve", post(approve))
        .route("/approvals/:id/reject", post(reject))
        .route("/search", get(search))
        // ...
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}
```

**Svelte-клиент (фрагмент):**

```typescript
const API_BASE = import.meta.env.VITE_API_BASE || 'http://localhost:8765/api/v1';
let authToken = getToken();

export async function health(): Promise<HealthStatus> {
    return request('/health');
}

export async function getPending(): Promise<Action[]> {
    return request('/approvals');
}

export async function approve(id: string, approvedBy: string): Promise<Action> {
    return request(`/approvals/${id}/approve`, {
        method: 'POST',
        body: JSON.stringify({ approved_by: approvedBy }),
    });
}
```

**Аутентификация:** токен генерируется при первом запуске, отображается в UI десктопа (QR-код или текст), пользователь вводит в PWA, хранится в localStorage.

**CORS:** ограничен Tailscale-адресами.

**Связанные ADR:** ADR-001, ADR-002, ADR-006, ADR-007, ADR-010, ADR-012.

