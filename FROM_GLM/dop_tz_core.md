
---

##  Дополнение к ТЗ tensa-core

```markdown
# Дополнение к ТЗ: tensa-core

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Дополнение (first-run wizard, файл памяти)
**Связано с:** ADR-021, Контракт 13

---

## Что добавляется

К существующему ТЗ tensa-core добавляются:
- First-run wizard — мастер первого запуска.
- Файл памяти — инициализация core memory.
- Настройка персоны — как называть секретаря.
- Настройка владельца — имя, предпочтения, язык.
- Интеграция с MemoryManager — инициализация памяти.

---

## First-run wizard

### Назначение

При первом запуске Tensa проводит пользователя через мастер
настройки. Мастер собирает:
- Язык интерфейса.
- Имя владельца.
- Как называть секретаря.
- Как обращаться к владельцу.
- Часовой пояс.
- Telegram-бот.
- Почта.
- WhatsApp (опционально).
- Tailscale.

**Результат:** заполненный config.toml + secrets.toml +
инициализированная core memory.

### Шаги мастера

#### Шаг 1: Язык интерфейса

**Вопрос:** «Выберите язык интерфейса».

**Варианты:**
- Русский (ru).
- English (en).

**Результат:** general.language.

#### Шаг 2: Знакомство

**Вопрос:** «Как вас зовут?»

**Пример:** Александр.

**Результат:** core_memory.human (имя владельца).

**Вопрос:** «Как к вам обращаться?»

**Пример:** Александр, Шеф, Саша.

**Результат:** core_memory.human (обращение).

**Вопрос:** «Как называть вашего секретаря?»

**Пример:** Tensa, Секретарь, Ассистент.

**Результат:** core_memory.persona (имя секретаря).

**Вопрос:** «Ваш часовой пояс?»

**Варианты:**
- Europe/Moscow (Москва).
- Europe/Kiev (Киев).
- Europe/Minsk (Минск).
- Asia/Almaty (Алматы).
- Asia/Tbilisi (Тбилиси).
- Другой (ввод).

**Результат:** core_memory.human (часовой пояс).

#### Шаг 3: Загрузка моделей

**Вопрос:** «Скачать локальные ИИ-модули?»

**Что скачивается:**
- llama-server бинарник.
- gemma-4-E2B-it-XS-TS-Q3_K_S.gguf (или S).
- multilingual-e5-small.
- ggml-base.bin (Whisper).
- RustO! + Cyrillic OCR.

**Прогресс:** через SSE.

**Результат:** модели в ~/.tensa/models/.

#### Шаг 4: Telegram

**Вопрос:** «Настроить Telegram-бот?»

**Поля:**
- Токен бота.
- Chat ID владельца.

**Проверка:** тестовое сообщение.

**Результат:** telegram.bot_token, telegram.owner_chat_id.

#### Шаг 5: Почта

**Вопрос:** «Настроить почту?»

**Варианты:**
- Импорт из Thunderbird.
- Импорт из Outlook.
- Ручной ввод (IMAP/SMTP).
- Автоопределение.

**Поля:**
- Email.
- Пароль / OAuth2.

**Проверка:** подключение к IMAP.

**Результат:** email.accounts.

#### Шаг 6: WhatsApp (опционально)

**Вопрос:** «Настроить WhatsApp?»

**Варианты:**
- Да (Cloud API Tier 0).
- Нет (пропустить).

**Результат:** whatsapp (если да).

#### Шаг 7: Tailscale

**Вопрос:** «Настроить Tailscale?»

**Инструкция:**
1. Установить Tailscale.
2. Войти в аккаунт.
3. Подключить устройства.

**Проверка:** доступность других узлов.

**Результат:** sync.tailscale_enabled = true.

#### Шаг 8: Завершение

**Что показывается:**
- Сводка настроек.
- Кнопка «Готово».

**Что происходит:**
- Запись config.toml.
- Запись secrets.toml.
- Инициализация core memory.
- Запуск ядра.

### Реализация

```rust
pub struct FirstRunWizard {
    config_path: PathBuf,
    secrets_path: PathBuf,
    memory_manager: Arc<dyn MemoryManager>,
    node_id: String,
}

impl FirstRunWizard {
    pub async fn new(
        config_path: PathBuf,
        secrets_path: PathBuf,
        memory_manager: Arc<dyn MemoryManager>,
        node_id: String,
    ) -> Result<Self>;

    /// Запустить мастер.
    pub async fn run(&self) -> Result<WizardResult>;

    /// Шаг 1: Язык.
    pub async fn step_language(&self, language: Language) -> Result<()>;

    /// Шаг 2: Знакомство.
    pub async fn step_introduction(
        &self,
        owner_name: String,
        owner_title: String,
        secretary_name: String,
        timezone: String,
    ) -> Result<()>;

    /// Шаг 3: Загрузка моделей.
    pub async fn step_models(&self) -> Result<()>;

    /// Шаг 4: Telegram.
    pub async fn step_telegram(&self, bot_token: String, owner_chat_id: String) -> Result<()>;

    /// Шаг 5: Почта.
    pub async fn step_email(&self, account: EmailAccount) -> Result<()>;

    /// Шаг 6: WhatsApp.
    pub async fn step_whatsapp(&self, config: Option<WhatsAppConfig>) -> Result<()>;

    /// Шаг 7: Tailscale.
    pub async fn step_tailscale(&self) -> Result<()>;

    /// Шаг 8: Завершение.
    pub async fn step_finish(&self) -> Result<WizardResult>;
}

pub struct WizardResult {
    pub config: Config,
    pub secrets: Secrets,
    pub core_memory_initialized: bool,
}
```

---

## Инициализация core memory

### Функция init_default

```rust
impl DefaultMemoryManager {
    /// Инициализация core memory при первом запуске.
    pub async fn init_default(&self, params: InitParams) -> Result<()> {
        // 1. Персона секретаря
        let persona = format!(
            "Я — {}, персональный ИИ-секретарь. Моя задача — помогать \
             владельцу управлять документами, почтой, календарём, \
             Telegram и приложениями. Я работаю на русском и английском \
             языках. Я не выполняю действия без одобрения владельца. \
             Я уважаю приватность и не передаю данные третьим лицам.",
            params.secretary_name
        );
        self.core_replace(BlockType::Persona, persona, MemoryOrigin::System).await?;

        // 2. Информация о владельце
        let human = format!(
            "Владельца зовут {}. Он предпочитает обращение «{}». \
             Его язык: {}. Часовой пояс: {}.",
            params.owner_name,
            params.owner_title,
            params.language,
            params.timezone
        );
        self.core_replace(BlockType::Human, human, MemoryOrigin::System).await?;

        // 3. Задачи (пусто)
        self.core_replace(BlockType::Tasks, "Активные задачи: нет".to_string(), MemoryOrigin::System).await?;

        // 4. Предпочтения (пусто)
        self.core_replace(BlockType::Preferences, "Предпочтения: нет".to_string(), MemoryOrigin::System).await?;

        Ok(())
    }
}

pub struct InitParams {
    pub owner_name: String,
    pub owner_title: String,
    pub secretary_name: String,
    pub language: Language,
    pub timezone: String,
}
```

### Пример содержимого блоков

**После инициализации:**

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
Владельца зовут Александр. Он предпочитает обращение «Шеф».
Его язык: ru. Часовой пояс: Europe/Moscow.
```

**tasks:**
```
Активные задачи: нет
```

**preferences:**
```
Предпочтения: нет
```

---

## Интеграция с TensaCore

### Обновлённая структура TensaCore

```rust
pub struct TensaCore {
    config: Arc<RwLock<Config>>,
    approval_store: Arc<dyn ApprovalStore>,
    memory_manager: Arc<dyn MemoryManager>,   // НОВОЕ
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

    /// Проверить, первый ли это запуск.
    pub async fn is_first_run(&self) -> Result<bool>;

    /// Запустить first-run wizard.
    pub async fn run_first_run_wizard(&self) -> Result<WizardResult>;

    /// Инициализировать память.
    pub async fn init_memory(&self, params: InitParams) -> Result<()>;

    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;
    pub async fn health(&self) -> Result<HealthStatus>;
    pub async fn reload_config(&self) -> Result<()>;
}
```

### Логика первого запуска

```rust
impl TensaCore {
    pub async fn start(&self) -> Result<()> {
        // 1. Проверить, первый ли запуск
        if self.is_first_run().await? {
            // 2. Запустить wizard
            let result = self.run_first_run_wizard().await?;

            // 3. Инициализировать память
            self.init_memory(InitParams {
                owner_name: result.owner_name,
                owner_title: result.owner_title,
                secretary_name: result.secretary_name,
                language: result.language,
                timezone: result.timezone,
            }).await?;
        }

        // 4. Запустить подсистемы
        // ...

        Ok(())
    }
}
```

---

## Обновлённая структура

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
│   ├── first_run.rs             # НОВОЕ: FirstRunWizard
│   └── init.rs                  # НОВОЕ: инициализация памяти
└── tests/
    ├── integration.rs
    ├── first_run.rs             # НОВОЕ
    └── fixtures/
```

---

## API для UI (дополнение)

### Tauri IPC

```rust
#[tauri::command]
async fn is_first_run() -> Result<bool, String>;

#[tauri::command]
async fn run_first_run_wizard() -> Result<WizardResult, String>;

#[tauri::command]
async fn wizard_step_language(language: String) -> Result<(), String>;

#[tauri::command]
async fn wizard_step_introduction(
    owner_name: String,
    owner_title: String,
    secretary_name: String,
    timezone: String,
) -> Result<(), String>;

#[tauri::command]
async fn wizard_step_models() -> Result<(), String>;

#[tauri::command]
async fn wizard_step_telegram(bot_token: String, owner_chat_id: String) -> Result<(), String>;

#[tauri::command]
async fn wizard_step_email(account: EmailAccount) -> Result<(), String>;

#[tauri::command]
async fn wizard_step_whatsapp(config: Option<WhatsAppConfig>) -> Result<(), String>;

#[tauri::command]
async fn wizard_step_tailscale() -> Result<(), String>;

#[tauri::command]
async fn wizard_step_finish() -> Result<WizardResult, String>;
```

### uniffi UDL

```udl
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

dictionary WizardResult {
    boolean config_saved;
    boolean secrets_saved;
    boolean core_memory_initialized;
};
```

### PWA HTTP API

```
GET /first-run
  response: { "is_first_run": boolean }

POST /first-run/step/language
  body: { "language": string }

POST /first-run/step/introduction
  body: { "owner_name": string, "owner_title": string, "secretary_name": string, "timezone": string }

POST /first-run/step/models
POST /first-run/step/telegram
POST /first-run/step/email
POST /first-run/step/whatsapp
POST /first-run/step/tailscale
POST /first-run/step/finish
```

---

## Тесты (дополнение)

### Unit-тесты

- FirstRunWizard — каждый шаг.
- init_default — инициализация блоков.
- is_first_run — определение первого запуска.

### Integration-тесты

- Полный цикл wizard.
- Инициализация core memory.
- Сохранение config.toml.
- Сохранение secrets.toml.
- Запуск ядра после wizard.

### Error-тесты

- Неверный токен Telegram.
- Неверный IMAP.
- Ошибка загрузки моделей.
- Ошибка сохранения конфига.

---

## Связанные ADR и контракты

- ADR-001: Rust как язык ядра.
- ADR-004: Lance для векторного поиска.
- ADR-005: fastembed-rs для эмбеддингов.
- ADR-012: gRPC (tonic) для синхронизации.
- ADR-021: Иерархическая память (Letta-style).
- Контракт 4: Конфигурация.
- Контракт 9: uniffi UDL.
- Контракт 10: Tauri IPC.
- Контракт 11: Flutter MethodChannel.
- Контракт 12: PWA HTTP API.
- Контракт 13: Core Memory (SQLite).
```

---

**Файл 4 готов.**

Скажи: **получилось ли скопировать?** Если да — переходим к **Файлу 5: Системный промпт (полная версия)**.