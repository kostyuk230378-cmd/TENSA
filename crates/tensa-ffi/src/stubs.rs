//! Интерфейсы UDL, ещё не подключённые к ядру (честные заглушки).
//!
//! [derived] Каждый интерфейс объявлен по UDL (Контракт 9) и даёт рабочий
//! конструктор, но методы возвращают `TensaError::InternalError` с записью
//! причины в лог: этапы указаны в доккомментариях. Так мобильные клиенты
//! видят полную поверхность контракта и отличают «ещё нет» от выключенного
//! доступа. Заглушки убираются по мере подключения крейтов (tensa-tools,
//! tensa-router, voice-стек, мастер запуска).

use crate::error::TensaError;
use crate::types::{
    AppInfo, CalendarEvent, Conflict, Email, EmailAccount, NewCalendarEvent, OutgoingEmail,
    QueryRequest, QueryResponse, WhatsAppConfig, WizardResult,
};

/// Конструктор-заглушка: интерфейс ещё не привязан к ядру.
async fn stub_constructor() -> Result<(), TensaError> {
    Ok(())
}

/// `MemoryStore` — документы/поиск (в tensa-memory пока только трейт).
pub struct MemoryStore;

impl MemoryStore {
    /// Этап: подключение document-хранилища tensa-memory.
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// Поиск по документам.
    pub async fn search(
        &self,
        _query: String,
        _limit: u32,
    ) -> Result<Vec<crate::types::SearchResult>, TensaError> {
        Err(TensaError::not_implemented("memory.search"))
    }

    /// Гибридный поиск.
    pub async fn search_hybrid(
        &self,
        _query: String,
        _limit: u32,
    ) -> Result<Vec<crate::types::SearchResult>, TensaError> {
        Err(TensaError::not_implemented("memory.search_hybrid"))
    }

    /// Метаданные документа.
    pub async fn get_document(
        &self,
        _document_id: String,
    ) -> Result<Option<crate::types::DocumentMeta>, TensaError> {
        Err(TensaError::not_implemented("memory.get_document"))
    }

    /// Список документов.
    pub async fn list_documents(
        &self,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<crate::types::DocumentMeta>, TensaError> {
        Err(TensaError::not_implemented("memory.list_documents"))
    }
}

/// `EmailClient` — почта (этап tensa-tools: email, Контракт 6).
pub struct EmailClient;

impl EmailClient {
    /// Этап: tensa-tools/email.
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// Подключить аккаунт.
    pub async fn connect(&self, _account_id: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("email.connect"))
    }

    /// Отключить аккаунт.
    pub async fn disconnect(&self, _account_id: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("email.disconnect"))
    }

    /// Папки аккаунта.
    pub async fn list_folders(&self, _account_id: String) -> Result<Vec<String>, TensaError> {
        Err(TensaError::not_implemented("email.list_folders"))
    }

    /// Письма из папки.
    pub async fn fetch_emails(
        &self,
        _account_id: String,
        _folder: String,
        _limit: u32,
    ) -> Result<Vec<Email>, TensaError> {
        Err(TensaError::not_implemented("email.fetch_emails"))
    }

    /// Поиск писем.
    pub async fn search(
        &self,
        _account_id: String,
        _query: String,
    ) -> Result<Vec<Email>, TensaError> {
        Err(TensaError::not_implemented("email.search"))
    }

    /// Отправка (через ApprovalStore ядра — вне FFI-обёртки).
    pub async fn send(&self, _email: OutgoingEmail) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("email.send"))
    }

    /// Черновик письма.
    pub async fn create_draft(&self, _email: OutgoingEmail) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("email.create_draft"))
    }
}

/// `CalendarClient` — календарь (этап tensa-tools: calendar).
pub struct CalendarClient;

impl CalendarClient {
    /// Этап: tensa-tools/calendar (CalDAV).
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// События за период.
    pub async fn list_events(
        &self,
        _start: String,
        _end: String,
    ) -> Result<Vec<CalendarEvent>, TensaError> {
        Err(TensaError::not_implemented("calendar.list_events"))
    }

    /// Конфликты за период.
    pub async fn check_conflicts(
        &self,
        _start: String,
        _end: String,
    ) -> Result<Vec<Conflict>, TensaError> {
        Err(TensaError::not_implemented("calendar.check_conflicts"))
    }

    /// Создать событие.
    pub async fn create_event(
        &self,
        _event: NewCalendarEvent,
    ) -> Result<CalendarEvent, TensaError> {
        Err(TensaError::not_implemented("calendar.create_event"))
    }

    /// Удалить событие.
    pub async fn delete_event(&self, _id: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("calendar.delete_event"))
    }
}

/// `AppLauncher` — приложения ОС (этап tensa-tools: apps).
pub struct AppLauncher;

impl AppLauncher {
    /// Этап: tensa-tools/apps.
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// Запустить приложение по имени.
    pub async fn launch(&self, _name: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("apps.launch"))
    }

    /// Открыть файл.
    pub async fn open_file(&self, _path: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("apps.open_file"))
    }

    /// Открыть файл в приложении.
    pub async fn open_file_with(&self, _name: String, _path: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("apps.open_file_with"))
    }

    /// Открыть URL.
    pub async fn open_url(&self, _url: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("apps.open_url"))
    }

    /// Список известных приложений.
    pub async fn list_apps(&self) -> Result<Vec<AppInfo>, TensaError> {
        Err(TensaError::not_implemented("apps.list_apps"))
    }

    /// Сканировать приложения.
    pub async fn scan_apps(&self) -> Result<Vec<AppInfo>, TensaError> {
        Err(TensaError::not_implemented("apps.scan_apps"))
    }

    /// Умеет ли платформа открывать такое имя (без ошибок — UDL).
    pub fn supports(&self, _name: String) -> bool {
        false
    }
}

/// `QueryEngine` — диалог с моделями (этап проводки tensa-router + LLM).
pub struct QueryEngine;

impl QueryEngine {
    /// Этап: проводка роутера (инференс уже реализован в tensa-router/llm).
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// Запрос к моделям.
    pub async fn query(&self, _request: QueryRequest) -> Result<QueryResponse, TensaError> {
        Err(TensaError::not_implemented("query.query"))
    }
}

/// `VoiceEngine` — распознавание речи (этап voice-стека: ggml-модель).
pub struct VoiceEngine;

impl VoiceEngine {
    /// Этап: голосовой стек (whisper/ggml).
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// Распознать аудио (WAV/PCM и т. п.).
    pub async fn transcribe(
        &self,
        _audio_data: Vec<u8>,
        _format: String,
    ) -> Result<String, TensaError> {
        Err(TensaError::not_implemented("voice.transcribe"))
    }
}

/// `FirstRunWizard` — мастер первого запуска (этап UI-мастеров приложений).
///
/// В tensa-core мастер реализован (`FirstRunWizard`), но UDL-словарь
/// `EmailAccount` неполон для шага почты (нет `email`/`credential`),
/// а UDL-варианта `NotImplemented` нет — расхождения зафиксированы в отчёте;
/// до их согласования мастер на FFI-поверхности отвечает заглушкой.
pub struct FirstRunWizard;

impl FirstRunWizard {
    /// Этап: согласование неполноты UDL-словарей (email/whatsapp).
    pub async fn new(_config_path: String) -> Result<Self, TensaError> {
        stub_constructor().await?;
        Ok(Self)
    }

    /// Первый ли запуск.
    pub async fn is_first_run(&self) -> Result<bool, TensaError> {
        Err(TensaError::not_implemented("wizard.is_first_run"))
    }

    /// Шаг 1: язык.
    pub async fn step_language(&self, _language: String) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_language"))
    }

    /// Шаг 2: знакомство.
    pub async fn step_introduction(
        &self,
        _owner_name: String,
        _owner_title: String,
        _secretary_name: String,
        _timezone: String,
    ) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_introduction"))
    }

    /// Шаг 3: модели.
    pub async fn step_models(&self) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_models"))
    }

    /// Шаг 4: Telegram.
    pub async fn step_telegram(
        &self,
        _bot_credential: String,
        _owner_chat_id: String,
    ) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_telegram"))
    }

    /// Шаг 5: почта.
    pub async fn step_email(&self, _account: EmailAccount) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_email"))
    }

    /// Шаг 6: WhatsApp.
    pub async fn step_whatsapp(&self, _config: Option<WhatsAppConfig>) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_whatsapp"))
    }

    /// Шаг 7: Tailscale.
    pub async fn step_tailscale(&self) -> Result<(), TensaError> {
        Err(TensaError::not_implemented("wizard.step_tailscale"))
    }

    /// Шаг 8: завершение.
    pub async fn step_finish(&self) -> Result<WizardResult, TensaError> {
        Err(TensaError::not_implemented("wizard.step_finish"))
    }
}
