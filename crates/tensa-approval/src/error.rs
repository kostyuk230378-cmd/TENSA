//! Ошибки крейта (thiserror). Коды ошибок согласованы с Контрактом 1
//! и Контрактом 9 (`TensaError.ApprovalError`).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, ApprovalError>;

/// Ошибки ApprovalStore.
#[derive(Debug, Error)]
pub enum ApprovalError {
    /// Действие с указанным id не найдено.
    #[error("action not found: {0}")]
    NotFound(String),

    /// Переход статуса запрещён (например, approve по уже executed).
    #[error("invalid status transition: action {action_id} is {from:?}, expected {expected}")]
    InvalidTransition {
        action_id: String,
        from: crate::models::ActionStatus,
        expected: String,
    },

    /// Действие истекло (TTL).
    #[error("action {0} expired")]
    Expired(String),

    /// Превышен лимит ожидающих действий (max_pending_actions).
    #[error("max pending actions reached: {limit}")]
    MaxPendingReached { limit: usize },

    /// Выключено конфигурацией (allow_email_send / allow_telegram_send / ...).
    #[error("action kind {kind} is disabled by config")]
    KindDisabled { kind: String },

    /// Ошибка конфигурации.
    #[error("config error: {0}")]
    Config(String),

    /// Ошибка базы данных.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Ошибка сериализации/десериализации JSON.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Некорректный UUID (из БД или ввода).
    #[error("invalid uuid: {0}")]
    InvalidUuid(#[from] uuid::Error),
}
