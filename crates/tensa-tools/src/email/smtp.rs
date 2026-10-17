//! SMTP-отправка (ТЗ tensa-tools: `email/src/smtp.rs`; Контракт 6, ADR-015).
//!
//! Транспорт — `lettre` (асинхронный rustls-транспорт): STARTTLS для 587,
//! неявный TLS для 465 (режим выбирает [`super::autodetect`] / конфиг).
//! Сборка письма отделена от сети — тестируется без сокетов.

use lettre::message::header::ContentType;
use lettre::message::{Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

use super::autodetect::SmtpTls;
use super::connections::Account;
use super::error::{shorten, EmailError};

/// Письмо к отправке (поля — как у UDL `OutgoingEmail`; вложения — позже).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingEmail {
    /// Аккаунт-отправитель; `None` — первый активный (Контракт 6).
    pub account_id: Option<String>,
    /// Получатель (один адрес; списки — через запятую).
    pub to: String,
    /// Копия (адреса через запятую).
    pub cc: Option<String>,
    /// Скрытая копия (адреса через запятую).
    pub bcc: Option<String>,
    /// Тема.
    pub subject: String,
    /// Текст.
    pub body: String,
}

/// Собрать MIME-письмо (без сети).
pub fn build_message(account: &Account, email: &OutgoingEmail) -> Result<Message, EmailError> {
    let from: Mailbox = account
        .email
        .parse()
        .map_err(|error| EmailError::MessageBuild(format!("from «{}»: {error}", account.email)))?;
    let mut builder = Message::builder().from(from).subject(&email.subject);

    let mut recipients = 0;
    for address in split_addresses(&email.to) {
        builder = builder.to(address
            .parse()
            .map_err(|error| EmailError::MessageBuild(format!("to «{address}»: {error}")))?);
        recipients += 1;
    }
    if recipients == 0 {
        return Err(EmailError::InvalidParams("поле to пустое".to_string()));
    }
    for address in split_addresses(email.cc.as_deref().unwrap_or_default()) {
        builder = builder.cc(address
            .parse()
            .map_err(|error| EmailError::MessageBuild(format!("cc «{address}»: {error}")))?);
    }
    for address in split_addresses(email.bcc.as_deref().unwrap_or_default()) {
        builder = builder.bcc(
            address
                .parse()
                .map_err(|error| EmailError::MessageBuild(format!("bcc «{address}»: {error}")))?,
        );
    }

    builder
        .header(ContentType::TEXT_PLAIN)
        .body(email.body.clone())
        .map_err(|error| EmailError::MessageBuild(error.to_string()))
}

/// Отправить письмо через SMTP-транспорт lettre.
///
/// [derived] Текст ошибки обрезается ([`shorten`]): транспорту в ошибку
/// попадают детали сессии, длинные простыни в лог и в чат не нужны.
pub async fn send(
    account: &Account,
    credential: &str,
    email: &OutgoingEmail,
) -> Result<(), EmailError> {
    let message = build_message(account, email)?;

    let builder = match account.smtp_tls {
        SmtpTls::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
        }
        SmtpTls::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&account.smtp_host),
    };
    let mailer = builder
        .map_err(|error| EmailError::Smtp(shorten(&error.to_string(), 200)))?
        .port(account.smtp_port)
        .credentials(Credentials::new(
            account.email.clone(),
            credential.to_string(),
        ))
        .build();

    mailer
        .send(message)
        .await
        .map_err(|error| EmailError::Smtp(shorten(&error.to_string(), 200)))?;
    Ok(())
}

/// Разбить строку адресов «a@x, b@y» на части.
fn split_addresses(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::email::autodetect::SmtpTls;

    fn account() -> Account {
        Account {
            id: "work".to_string(),
            email: "owner@gmail.com".to_string(),
            label: "Работа".to_string(),
            imap_host: "imap.gmail.com".to_string(),
            imap_port: 993,
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            smtp_tls: SmtpTls::StartTls,
        }
    }

    fn email() -> OutgoingEmail {
        OutgoingEmail {
            account_id: None,
            to: "friend@example.com".to_string(),
            cc: Some("boss@example.com, team@example.com".to_string()),
            bcc: None,
            subject: "Проверка почты".to_string(),
            body: "Plain body text.".to_string(),
        }
    }

    #[test]
    fn message_contains_recipients_and_subject() {
        let message = build_message(&account(), &email()).expect("сборка");
        let rendered = String::from_utf8_lossy(&message.formatted()).to_string();
        assert!(
            rendered.contains("friend@example.com"),
            "rendered: {rendered}"
        );
        assert!(rendered.contains("boss@example.com"));
        assert!(rendered.contains("team@example.com"));
        assert!(rendered.contains("Subject:"));
        // Тема в UTF-8 кодируется RFC 2047, тело — MIME-кодировкой:
        // проверяем кодированную форму, а не сырые байты кириллицы.
        assert!(rendered.contains("=?utf-8?"));
        assert!(rendered.contains("Plain body text."));
        assert!(rendered.contains("Content-Transfer-Encoding"));
    }

    #[test]
    fn utf8_body_builds_without_raw_assertions() {
        let mut draft = email();
        draft.subject = "Тема с юникодом".to_string();
        draft.body = "Тело письма — юникод и emoji 🚀".to_string();
        assert!(build_message(&account(), &draft).is_ok());
    }

    #[test]
    fn empty_to_is_rejected() {
        let mut draft = email();
        draft.to = "  ,  ".to_string();
        assert!(build_message(&account(), &draft).is_err());
    }

    #[test]
    fn bad_address_is_rejected() {
        let mut draft = email();
        draft.to = "не-адрес".to_string();
        assert!(build_message(&account(), &draft).is_err());
    }
}
