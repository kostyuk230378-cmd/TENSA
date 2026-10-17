//! Ошибки крейта (thiserror).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, ToolsError>;

/// Ошибки tensa-tools.
#[derive(Debug, Error)]
pub enum ToolsError {
    /// Ошибка инструмента WoL (разбор параметров, отправка пакета).
    #[error("wol error: {0}")]
    Wol(#[from] crate::wol::WolError),

    /// Ошибка инструмента Telegram (клиент, очередь, обработчики).
    #[error("telegram error: {0}")]
    Telegram(#[from] crate::telegram::error::TelegramError),

    /// Ошибка инструмента Email (аккаунты, SMTP, очередь).
    #[error("email error: {0}")]
    Email(#[from] crate::email::error::EmailError),
}
