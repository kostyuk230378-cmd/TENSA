//! Ошибки крейта (thiserror).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, NodesError>;

/// Ошибки tensa-nodes.
#[derive(Debug, Error)]
pub enum NodesError {
    /// Узел уже зарегистрирован (дубликат node_id при register).
    #[error("duplicate node: {0}")]
    DuplicateNode(String),

    /// Узел не найден в реестре.
    #[error("node not found: {0}")]
    NodeNotFound(String),

    /// Ресурс уже занят другим узлом (конфликт соединений).
    #[error("resource '{resource}' busy: held by {holder}")]
    ResourceBusy { resource: String, holder: String },

    /// Аренда соединения просрочена.
    #[error("connection lease expired for resource '{0}'")]
    LeaseExpired(String),

    /// Узел офлайн — операция недоступна.
    #[error("node offline: {0}")]
    NodeOffline(String),

    /// Некорректная конфигурация.
    #[error("config error: {0}")]
    Config(String),

    /// Ошибка базы данных.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Ошибка сериализации.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Некорректный UUID.
    #[error("invalid uuid: {0}")]
    InvalidUuid(#[from] uuid::Error),

    /// Ошибка ввода-вывода.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
