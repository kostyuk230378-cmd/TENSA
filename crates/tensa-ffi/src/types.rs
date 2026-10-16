//! Типы UDL (Контракт 9, `teh_doc/contracts/ffi/tensa.udl`).
//!
//! [derived] Важная особенность uniffi UDL-режима: типы объявляются
//! обычными Rust-типами БЕЗ derive-макросов uniffi — конверсию FfiConverter
//! генерирует `udl_derive` из scaffolding (своя реализация дала бы конфликт).
//!
//! Формы повторяют кодоген: unit-варианты enum записываются с пустыми
//! скобками (`Persona {}`), идентификаторы и JSON в UDL — строки `String`.

/// Сводный статус здоровья ядра (`namespace.health`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    /// Ядро здорово в целом.
    pub healthy: bool,
    /// Версия ядра.
    pub version: String,
    /// Аптайм строкой (формат — на отображение).
    pub uptime: String,
    /// Есть деградация хотя бы в одном компоненте.
    pub degraded: bool,
    /// Причина деградации, если есть.
    pub degraded_reason: Option<String>,
}

/// Действие в очереди одобрения (Контракт 1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Action {
    /// UUID действия строкой.
    pub id: String,
    /// Вид действия (`email_send`, `telegram_send`, ...).
    pub kind: String,
    /// Payload JSON-строкой.
    pub payload: String,
    /// Статус (`pending`, `approved`, ...).
    pub status: String,
    /// Создано (unix-время).
    pub created_at: i64,
    /// Истекает (unix-время).
    pub expires_at: i64,
    /// Подтверждено (unix-время строкой), если уже.
    pub approved_at: Option<String>,
    /// Исполнено (unix-время строкой), если уже.
    pub executed_at: Option<String>,
    /// Результат исполнения JSON-строкой, если есть.
    pub result: Option<String>,
}

/// Запрос на новое действие (Контракт 1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewAction {
    /// Вид действия (`email_send`, `telegram_send`, ...).
    pub kind: String,
    /// Payload JSON-строкой.
    pub payload: String,
}

/// Результат поиска по документам.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    /// UUID документа строкой.
    pub document_id: String,
    /// Путь к исходному файлу.
    pub source_path: String,
    /// Найденный фрагмент.
    pub chunk_text: String,
    /// Оценка релевантности.
    pub score: f32,
    /// Когда проиндексировано.
    pub indexed_at: i64,
}

/// Метаданные документа.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentMeta {
    /// UUID документа строкой.
    pub document_id: String,
    /// Путь к исходному файлу.
    pub source_path: String,
    /// Заголовок.
    pub title: String,
    /// MIME-тип.
    pub mime_type: String,
    /// Размер файла.
    pub size_bytes: i64,
    /// Создан файл.
    pub created_at: i64,
    /// Изменён файл.
    pub modified_at: i64,
    /// Проиндексирован.
    pub indexed_at: i64,
    /// Число фрагментов.
    pub chunk_count: i32,
}

/// Блок core memory (ADR-021, Контракт 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Персона секретаря.
    Persona {},
    /// Сведения о владельце.
    Human {},
    /// Текущие задачи.
    Tasks {},
    /// Предпочтения.
    Preferences {},
}

impl BlockType {
    /// Строковое значение (как в ядре).
    pub fn as_str(self) -> &'static str {
        match self {
            BlockType::Persona {} => "persona",
            BlockType::Human {} => "human",
            BlockType::Tasks {} => "tasks",
            BlockType::Preferences {} => "preferences",
        }
    }
}

/// Кто изменил блок памяти.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOrigin {
    /// Изменено LLM.
    Llm {},
    /// Изменено пользователем.
    User {},
    /// Системное изменение.
    System {},
}

impl MemoryOrigin {
    /// Строковое значение (как в ядре).
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryOrigin::Llm {} => "llm",
            MemoryOrigin::User {} => "user",
            MemoryOrigin::System {} => "system",
        }
    }
}

/// Блок core memory в представлении UDL.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CoreMemoryBlock {
    /// UUID блока строкой.
    pub id: String,
    /// Тип блока.
    pub block: BlockType,
    /// Значение блока.
    pub value: String,
    /// Размер в символах.
    pub size_chars: i32,
    /// Размер в токенах (оценка).
    pub size_tokens: i32,
    /// Создан.
    pub created_at: i64,
    /// Обновлён.
    pub updated_at: i64,
    /// Кто обновил.
    pub updated_by: MemoryOrigin,
    /// Узел, на котором обновлён.
    pub updated_on_node: String,
}

/// Заметка (Контракт 3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Note {
    /// UUID заметки строкой.
    pub id: String,
    /// Заголовок.
    pub title: String,
    /// Текст.
    pub body: String,
    /// Теги.
    pub tags: Vec<String>,
    /// Создана.
    pub created_at: i64,
    /// Обновлена.
    pub updated_at: i64,
}

/// Новая заметка (Контракт 3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewNote {
    /// Заголовок.
    pub title: String,
    /// Текст.
    pub body: String,
    /// Теги.
    pub tags: Vec<String>,
}

/// Контакт (Контракт 3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Contact {
    /// UUID контакта строкой.
    pub id: String,
    /// Имя.
    pub name: String,
    /// Почта.
    pub email: Option<String>,
    /// Телефон.
    pub phone: Option<String>,
    /// Telegram.
    pub telegram: Option<String>,
    /// WhatsApp.
    pub whatsapp: Option<String>,
    /// Заметки о контакте.
    pub notes: Option<String>,
    /// Теги.
    pub tags: Vec<String>,
    /// Создан.
    pub created_at: i64,
    /// Обновлён.
    pub updated_at: i64,
}

/// Новый контакт (Контракт 3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewContact {
    /// Имя.
    pub name: String,
    /// Почта.
    pub email: Option<String>,
    /// Телефон.
    pub phone: Option<String>,
    /// Telegram.
    pub telegram: Option<String>,
    /// WhatsApp.
    pub whatsapp: Option<String>,
    /// Заметки о контакте.
    pub notes: Option<String>,
    /// Теги.
    pub tags: Vec<String>,
}

/// Учётная запись почты (Контракт 6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EmailAccount {
    /// Идентификатор аккаунта.
    pub id: String,
    /// Отображаемое имя.
    pub label: String,
    /// IMAP-хост.
    pub imap_host: String,
    /// IMAP-порт.
    pub imap_port: u16,
    /// SMTP-хост.
    pub smtp_host: String,
    /// SMTP-порт.
    pub smtp_port: u16,
}

/// Письмо (Контракт 6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Email {
    /// Идентификатор письма.
    pub id: String,
    /// Аккаунт.
    pub account_id: String,
    /// Папка.
    pub folder: String,
    /// Отправитель.
    pub from: String,
    /// Получатели.
    pub to: String,
    /// Тема.
    pub subject: String,
    /// Текст.
    pub body: String,
    /// Дата (unix-время).
    pub date: i64,
    /// Прочитано.
    pub read: bool,
    /// Есть вложения.
    pub has_attachments: bool,
}

/// Исходящее письмо (Контракт 6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutgoingEmail {
    /// Аккаунт.
    pub account_id: String,
    /// Получатель.
    pub to: String,
    /// Копия.
    pub cc: Option<String>,
    /// Скрытая копия.
    pub bcc: Option<String>,
    /// Тема.
    pub subject: String,
    /// Текст.
    pub body: String,
    /// Вложения (пути).
    pub attachments: Vec<String>,
}

/// Событие календаря.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CalendarEvent {
    /// Идентификатор события.
    pub id: String,
    /// Название.
    pub title: String,
    /// Начало (ISO 8601).
    pub start: String,
    /// Конец (ISO 8601).
    pub end: String,
    /// Описание.
    pub description: Option<String>,
    /// Место.
    pub location: Option<String>,
    /// Участники.
    pub attendees: Vec<String>,
}

/// Новое событие календаря.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewCalendarEvent {
    /// Название.
    pub title: String,
    /// Начало (ISO 8601).
    pub start: String,
    /// Конец (ISO 8601).
    pub end: String,
    /// Описание.
    pub description: Option<String>,
    /// Место.
    pub location: Option<String>,
    /// Участники.
    pub attendees: Vec<String>,
}

/// Конфликт событий.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Conflict {
    /// Идентификатор события.
    pub event_id: String,
    /// Название.
    pub title: String,
    /// Начало.
    pub start: String,
    /// Конец.
    pub end: String,
}

/// Приложение операционной системы.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppInfo {
    /// Имя.
    pub name: String,
    /// Путь.
    pub path: String,
    /// Путь к иконке.
    pub icon_path: Option<String>,
    /// Связанные расширения.
    pub associations: Vec<String>,
}

/// Узел сети (Контракт 8/9).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Node {
    /// UUID узла строкой.
    pub node_id: String,
    /// Имя.
    pub name: String,
    /// Тип устройства.
    pub device_type: String,
    /// Платформа.
    pub platform: String,
    /// Роль (`heavy`/`thin`/`auto`).
    pub role: String,
    /// Адрес Tailscale.
    pub tailscale_ip: String,
    /// gRPC-порт.
    pub grpc_port: u16,
    /// Последняя связь.
    pub last_seen: i64,
    /// Онлайн.
    pub online: bool,
    /// RAM всего, МБ.
    pub ram_total_mb: u64,
    /// RAM свободно, МБ.
    pub ram_available_mb: u64,
    /// Ядер CPU.
    pub cpu_cores: u32,
    /// Загрузка CPU, %.
    pub cpu_usage_percent: f32,
    /// GPU есть.
    pub has_gpu: bool,
    /// NPU есть.
    pub has_npu: bool,
    /// Заряд батареи, %.
    pub battery_percent: Option<u8>,
    /// Батарея заряжается.
    pub battery_charging: Option<bool>,
    /// Тепловой статус.
    pub thermal_status: String,
}

/// Плоская конфигурация (Контракт 4).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// UUID узла.
    pub node_id: String,
    /// Язык интерфейса.
    pub language: String,
    /// Тип устройства.
    pub device_type: String,
    /// Роль узла.
    pub role: String,
    /// Режим brain (`local`/`cloud`/`auto`).
    pub brain_mode: String,
    /// Локальная модель.
    pub local_model: String,
    /// Облачная модель.
    pub cloud_model: String,
}

/// Запрос к моделям.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueryRequest {
    /// Текст запроса.
    pub text: String,
    /// Язык.
    pub language: String,
    /// Использовать облако.
    pub use_cloud: bool,
}

/// Ответ моделей.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueryResponse {
    /// Текст ответа.
    pub text: String,
    /// Выбранное действие.
    pub action: Option<String>,
    /// Payload действия.
    pub action_payload: Option<String>,
    /// Вызовы инструментов.
    pub tool_calls: Vec<ToolCall>,
    /// Рассуждение.
    pub reasoning: Option<String>,
    /// Ответ деградировал.
    pub degraded: bool,
    /// Использованная модель.
    pub model_used: String,
}

/// Вызов инструмента.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// Имя инструмента.
    pub tool: String,
    /// Параметры JSON-строкой.
    pub params: String,
}

/// Результат мастера первого запуска.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WizardResult {
    /// Конфиг записан.
    pub config_saved: bool,
    /// Секреты записаны.
    pub secrets_saved: bool,
    /// Core memory инициализирована.
    pub core_memory_initialized: bool,
}

/// WhatsApp-конфигурация.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WhatsAppConfig {
    /// Токен доступа (значение не логируется).
    pub access_token: String,
    /// Идентификатор номера.
    pub phone_number_id: String,
    /// Идентификатор WABA.
    pub waba_id: String,
}
