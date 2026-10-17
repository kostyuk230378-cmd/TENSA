//! Ошибки email-инструмента (ТЗ tensa-tools: `email/src/error.rs`).

use thiserror::Error;

/// Ошибки почтового инструмента.
#[derive(Debug, Error)]
pub enum EmailError {
    /// В конфигурации нет учётной записи (или запрошенной).
    #[error("email: не задана учётная запись «{0}» (config.toml, секция [email])")]
    MissingAccount(String),

    /// Нет креденшелов аккаунта.
    #[error(
        "email: нет креденшелов для аккаунта «{0}» (secrets.toml или IMAP_PASSWORD/SMTP_PASSWORD)"
    )]
    MissingCredential(String),

    /// Некорректные параметры действия.
    #[error("email: некорректные параметры: {0}")]
    InvalidParams(String),

    /// Ошибка сборки MIME-письма.
    #[error("email: ошибка сборки письма: {0}")]
    MessageBuild(String),

    /// Ошибка SMTP-транспорта (текст обрезан, секреты не выводятся).
    #[error("email: SMTP: {0}")]
    Smtp(String),

    /// Ошибка ядра (конфиг/секреты).
    #[error("email: ошибка ядра: {0}")]
    Core(#[from] tensa_core::CoreError),

    /// Ошибка очереди одобрения.
    #[error("email: ошибка одобрений: {0}")]
    Approval(#[from] tensa_approval::ApprovalError),

    /// Ошибка ввода-вывода.
    #[error("email: ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
}

/// Укоротить текст ошибки транспорта (без утечки длинных деталей).
pub fn shorten(text: &str, max: usize) -> String {
    let mut short: String = text.chars().take(max).collect();
    if short.chars().count() < text.chars().count() {
        short.push('…');
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_account_message_is_clear() {
        let text = EmailError::MissingAccount("work".to_string()).to_string();
        assert!(text.contains("work"));
        assert!(text.contains("config.toml"));
    }

    #[test]
    fn shorten_keeps_utf8_boundary() {
        let text = "юникод-ошибка-очень-длинная";
        let short = shorten(text, 8);
        assert_eq!(short.chars().count(), 9, "8 символов + многоточие");
        assert!(short.ends_with('…'));
    }
}
