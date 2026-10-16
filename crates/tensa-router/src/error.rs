//! Ошибки крейта (ТЗ tensa-router: «Ошибки: модель не загружена, облако
//! недоступно, API key невалиден, превышен лимит, узел офлайн»).

use thiserror::Error;

/// Результат операций tensa-router.
pub type Result<T> = std::result::Result<T, RouterError>;

/// Ошибки маршрутизации и инференса.
#[derive(Debug, Error)]
pub enum RouterError {
    /// Некорректная конфигурация.
    #[error("Ошибка конфигурации: {0}")]
    Config(String),

    /// Ошибка локальной модели (включая «модель не загружена»).
    #[error("Ошибка локальной модели: {0}")]
    Local(#[from] tensa_llm::LlmError),

    /// Ошибка облачного инференса (сеть, HTTP, ответ).
    #[error("Ошибка облачной модели: {0}")]
    Cloud(String),

    /// Недействительный API-ключ облака.
    #[error("Недействительный API-ключ облака: {0}")]
    ApiKey(String),

    /// Превышен бюджет облака (дневные токены / месячные USD).
    #[error("Бюджет облака исчерпан: {0}")]
    BudgetExceeded(String),

    /// Узел недоступен (офлайн, gRPC-ошибка).
    #[error("Узел недоступен: {0}")]
    NodeUnavailable(String),

    /// Ошибка реестра узлов.
    #[error("Ошибка реестра узлов: {0}")]
    Nodes(#[from] tensa_nodes::NodesError),

    /// Ошибка синхронизации.
    #[error("Ошибка синхронизации: {0}")]
    Sync(#[from] tensa_sync::SyncError),

    /// Ошибка ввода-вывода (файл бюджета).
    #[error("Ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка JSON (файл бюджета).
    #[error("Ошибка JSON: {0}")]
    Json(#[from] serde_json::Error),
}
