//! Ошибки крейта (thiserror).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Ошибки tensa-core.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Конфигурация: неверный путь, формат или валидация.
    #[error("config error: {0}")]
    Config(String),

    /// Объект не найден.
    #[error("not found: {0}")]
    NotFound(String),

    /// Ошибка базы данных.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Ошибка сериализации/десериализации.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Ошибка разбора/записи TOML.
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Ошибка записи TOML.
    #[error("toml serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    /// Ошибка ввода-вывода.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Некорректный UUID.
    #[error("invalid uuid: {0}")]
    InvalidUuid(#[from] uuid::Error),

    /// Ошибка подсистемы одобрения.
    #[error("approval error: {0}")]
    Approval(#[from] tensa_approval::ApprovalError),

    /// Wizard ещё не завершён (первый запуск).
    #[error("first-run wizard not completed")]
    FirstRunNotCompleted,
}
