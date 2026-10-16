//! Ошибки крейта (thiserror).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, SyncError>;

/// Ошибки синхронизации (ТЗ: узел недоступен, сеть разорвана,
/// конфликт версий, превышен TTL, ошибка Syncthing).
#[derive(Debug, Error)]
pub enum SyncError {
    /// Узел недоступен (heartbeat не приходил дольше timeout).
    #[error("node unavailable: {0}")]
    NodeUnavailable(String),

    /// Сетевая ошибка транспорта.
    #[error("network error: {0}")]
    Network(String),

    /// Конфликт версий (векторные часы не разрешимы без LWW-решения).
    #[error("version conflict on {resource}")]
    VersionConflict { resource: String },

    /// Превышен TTL аренды соединения.
    #[error("connection lease expired: {resource}")]
    LeaseExpired { resource: String },

    /// Ошибка Syncthing REST API.
    #[error("syncthing error: {0}")]
    Syncthing(String),

    /// Ошибка хранилища (tensa-core Stores) в sync-адаптерах.
    #[error("store error: {0}")]
    Store(String),

    /// Ошибка конфигурации.
    #[error("config error: {0}")]
    Config(String),

    /// Ошибка сериализации/десериализации.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Ошибка ввода-вывода.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка из крейта ApprovalStore (события и их применение).
    #[error("approval store error: {0}")]
    Approval(#[from] tensa_approval::ApprovalError),
}
