//! Ошибки Telegram-инструмента (ТЗ tensa-tools: `telegram/src/error.rs`).

use thiserror::Error;

/// Ошибки клиента и сервиса Telegram.
#[derive(Debug, Error)]
pub enum TelegramError {
    /// Не задан токен бота (ни в secrets.toml, ни в `TELEGRAM_BOT_TOKEN`).
    #[error("telegram: не задан токен бота (secrets.toml / TELEGRAM_BOT_TOKEN)")]
    MissingCredential,

    /// Не задан чат владельца (`owner_chat_id`).
    #[error("telegram: не задан чат владельца (owner_chat_id в secrets.toml или config.toml)")]
    MissingOwnerChat,

    /// Некорректные параметры действия.
    #[error("telegram: некорректные параметры: {0}")]
    InvalidParams(String),

    /// Ошибка ядра (конфиг/секреты).
    #[error("telegram: ошибка ядра: {0}")]
    Core(#[from] tensa_core::CoreError),

    /// Ошибка очереди одобрения.
    #[error("telegram: ошибка одобрений: {0}")]
    Approval(#[from] tensa_approval::ApprovalError),

    /// Транспортная ошибка (сеть, TLS, таймаут).
    #[error("telegram: HTTP-ошибка: {0}")]
    Http(#[from] reqwest::Error),

    /// Bot API вернул `ok: false`.
    #[error("telegram: API вернул {status}: {description}")]
    Api {
        /// HTTP-статус ответа.
        status: u16,
        /// Описание ошибки от Telegram.
        description: String,
    },

    /// Ответ не разобран (неожиданная схема).
    #[error("telegram: неожиданный ответ ({status}): {detail}")]
    Protocol {
        /// HTTP-статус ответа.
        status: u16,
        /// Детали.
        detail: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credential_message_is_clear() {
        let text = TelegramError::MissingCredential.to_string();
        assert!(text.contains("TELEGRAM_BOT_TOKEN"));
    }

    #[test]
    fn api_error_keeps_status_and_description() {
        let error = TelegramError::Api {
            status: 401,
            description: "Unauthorized".to_string(),
        };
        assert!(error.to_string().contains("401"));
        assert!(error.to_string().contains("Unauthorized"));
    }
}
