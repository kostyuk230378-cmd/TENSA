//! Общее состояние сервера (разделяется между обработчиками через `axum::extract::State`)
//! и тип события для SSE-потока `/events`.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tensa_approval::ApprovalStore;
use tensa_core::NotesStore;
use tokio::sync::broadcast;

use crate::auth::PwaToken;
use crate::calendar::CalendarStore;
use crate::files::FilesIndex;
use crate::query::QueryService;
use crate::routes::email::AccountInfo;
use crate::voice::VoiceService;

/// Каталог со собранной статикой PWA по умолчанию (ТЗ apps/pwa: сборка кладёт
/// `static/`; здесь MVP-путь относительно рабочего каталога процесса).
pub const DEFAULT_STATIC_DIR: &str = "pwa/static";

/// Ёмкость канала событий SSE (медленный клиент получает `Lagged` и пропуск,
/// а не блокировку сервера).
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Событие сервера, транслируемое в SSE-поток.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEvent {
    /// Имя события SSE (например, `approval:new`).
    pub event: String,
    /// Полезная нагрузка события (JSON).
    pub data: serde_json::Value,
}

impl ServerEvent {
    /// Имя приветственного события потока (сразу после подключения).
    pub const WELCOME: &'static str = "welcome";
    /// Снимок состояния ядра (имя из ТЗ apps/pwa).
    pub const STATUS_HEALTH: &'static str = "status:health";
    /// Новое действие в очереди одобрения (имя из ТЗ apps/pwa).
    pub const APPROVAL_NEW: &'static str = "approval:new";
    /// Изменение статуса действия (approve/reject).
    pub const APPROVAL_UPDATED: &'static str = "approval:updated";

    /// Событие с произвольным именем и JSON-нагрузкой.
    pub fn new(event: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            event: event.into(),
            data,
        }
    }

    /// Приветственное событие: подтверждает подписку и сообщает версию ядра.
    pub fn welcome(version: &str) -> Self {
        Self::new(
            Self::WELCOME,
            serde_json::json!({ "message": "connected", "version": version }),
        )
    }
}

/// Состояние сервера: хранилища, доступ, канал событий, статика.
#[derive(Clone)]
pub struct AppState {
    /// Хранилище очереди одобрения (`tensa-approval`).
    pub approvals: Arc<dyn ApprovalStore>,
    /// Хранилище заметок (`tensa-core`).
    pub notes: Arc<dyn NotesStore>,
    /// Шлюз запросов к моделям (`/query`).
    pub query: Arc<QueryService>,
    /// Календарь ядра (JSON-файл `<root>/calendar.json`).
    pub calendar: Arc<CalendarStore>,
    /// Почтовые аккаунты из конфига (для APK/PWA).
    pub email_accounts: Vec<AccountInfo>,
    /// Корень данных ядра (модели и пр.).
    pub data_root: PathBuf,
    /// Путь к config.toml (для почтового сервиса и секретов).
    pub config_path: PathBuf,
    /// Индекс локальных файлов (поиск по имени и содержимому).
    pub files: Arc<FilesIndex>,
    /// Каталоги, которые индексируются.
    pub index_dirs: Vec<PathBuf>,
    /// Локальное распознавание речи (whisper).
    pub voice: Arc<VoiceService>,
    /// Bearer-доступ PWA. Значение не логируется и наружу не отдаётся:
    /// обработчикам доступна только проверка [`AppState::token_matches`].
    access: PwaToken,
    /// Канал событий для `/events` (tokio::sync::broadcast).
    events: broadcast::Sender<ServerEvent>,
    /// Каталог со статикой PWA.
    pub static_dir: PathBuf,
    /// Момент старта сервера (unix timestamp) — для аптайма в `/health`.
    pub started_at: i64,
}

impl AppState {
    /// Новое состояние с открытыми хранилищами и доступом.
    ///
    /// Токен передаётся явно: так тесты задают известное значение, а
    /// продуктовый путь берёт его из файла (`<root>/pwa_token`).
    pub fn new(
        approvals: Arc<dyn ApprovalStore>,
        notes: Arc<dyn NotesStore>,
        access: PwaToken,
    ) -> Self {
        let (events, _receiver) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            approvals,
            notes,
            query: Arc::new(QueryService::disabled()),
            calendar: Arc::new(CalendarStore::in_memory()),
            email_accounts: Vec::new(),
            data_root: PathBuf::from("."),
            config_path: PathBuf::from("config.toml"),
            files: Arc::new(FilesIndex::disabled()),
            index_dirs: Vec::new(),
            voice: Arc::new(VoiceService::disabled()),
            access,
            events,
            static_dir: PathBuf::from(DEFAULT_STATIC_DIR),
            started_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Каталог статики PWA (для тестов и нестандартной раскладки сборки).
    pub fn with_static_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.static_dir = dir.into();
        self
    }

    /// Момент старта (для тестов аптайма).
    pub fn with_started_at(mut self, started_at: i64) -> Self {
        self.started_at = started_at;
        self
    }

    /// Шлюз запросов к моделям (собирается в `main` из конфига и секретов).
    pub fn with_query(mut self, query: Arc<QueryService>) -> Self {
        self.query = query;
        self
    }

    /// Календарь (файл в корне данных).
    pub fn with_calendar(mut self, calendar: Arc<CalendarStore>) -> Self {
        self.calendar = calendar;
        self
    }

    /// Почтовые аккаунты из конфига.
    pub fn with_email_accounts(mut self, accounts: Vec<AccountInfo>) -> Self {
        self.email_accounts = accounts;
        self
    }

    /// Корень данных (каталог моделей).
    pub fn with_data_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.data_root = root.into();
        self
    }

    /// Путь к config.toml (почта читает config + secrets).
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = path.into();
        self
    }

    /// Индекс файлов и каталоги сканирования.
    pub fn with_files(mut self, files: Arc<FilesIndex>, dirs: Vec<PathBuf>) -> Self {
        self.files = files;
        self.index_dirs = dirs;
        self
    }

    /// Локальное распознавание речи (модель whisper в корне данных).
    pub fn with_voice(mut self, voice: Arc<VoiceService>) -> Self {
        self.voice = voice;
        self
    }

    /// Проверка Bearer-токена (сравнение без раннего выхода по байтам).
    pub fn token_matches(&self, provided: &str) -> bool {
        self.access.matches(provided)
    }

    /// Подписка на поток событий (для `/events`).
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.events.subscribe()
    }

    /// Опубликовать событие подписчикам; возвращает число получателей.
    ///
    /// Отсутствие подписчиков — не ошибка (никто не слушает SSE).
    pub fn publish(&self, event: ServerEvent) -> usize {
        self.events.send(event).unwrap_or(0)
    }

    /// Число активных подписчиков SSE.
    pub fn subscriber_count(&self) -> usize {
        self.events.receiver_count()
    }
}

impl std::fmt::Debug for AppState {
    /// Значение токена в вывод не попадает.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("static_dir", &self.static_dir)
            .field("started_at", &self.started_at)
            .field("subscribers", &self.subscriber_count())
            .finish_non_exhaustive()
    }
}
