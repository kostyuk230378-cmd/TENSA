//! Автоопределение почтовых серверов (ТЗ tensa-tools: `email/src/autodetect.rs`).
//!
//! Контракт 6: `imap_port`/`smtp_port` — «настраиваемый, 0 = автоопределение»;
//! пустые хосты в `config.toml` тоже ведут к автоопределению по домену адреса.
//!
//! [derived] MVP: собственная офлайн-таблица провайдеров (gmail/outlook/
//! yandex/mail.ru). io-pim-discovery из плана разведки — кандидат на замену
//! после дедлайна (нужен только для экзотических доменов).

/// Режим TLS для SMTP-соединения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTls {
    /// Явный TLS (STARTTLS): порт 587.
    StartTls,
    /// Неявный TLS (SMTPS): порт 465.
    Implicit,
}

/// Предустановка провайдера по домену адреса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderPreset {
    /// IMAP-хост.
    pub imap_host: &'static str,
    /// IMAP-порт (TLS).
    pub imap_port: u16,
    /// SMTP-хост.
    pub smtp_host: &'static str,
    /// SMTP-порт.
    pub smtp_port: u16,
    /// Режим TLS для SMTP.
    pub smtp_tls: SmtpTls,
}

/// Определить настройки по адресу почты; `None` — домен неизвестен
/// (тогда хосты обязаны быть явно указаны в `config.toml`).
pub fn detect(address: &str) -> Option<ProviderPreset> {
    let domain = address.rsplit_once('@')?.1.trim().to_ascii_lowercase();
    let preset = match domain.as_str() {
        "gmail.com" | "googlemail.com" => ProviderPreset {
            imap_host: "imap.gmail.com",
            imap_port: 993,
            smtp_host: "smtp.gmail.com",
            smtp_port: 587,
            smtp_tls: SmtpTls::StartTls,
        },
        "outlook.com"
        | "hotmail.com"
        | "live.com"
        | "msn.com"
        | "outlook.office365.com"
        | "office365.com" => ProviderPreset {
            imap_host: "outlook.office365.com",
            imap_port: 993,
            smtp_host: "smtp.office365.com",
            smtp_port: 587,
            smtp_tls: SmtpTls::StartTls,
        },
        "yandex.ru" | "ya.ru" | "yandex.com" | "yandex.by" | "yandex.kz" => ProviderPreset {
            imap_host: "imap.yandex.ru",
            imap_port: 993,
            smtp_host: "smtp.yandex.ru",
            smtp_port: 465,
            smtp_tls: SmtpTls::Implicit,
        },
        "mail.ru" | "inbox.ru" | "list.ru" | "bk.ru" | "internet.ru" => ProviderPreset {
            imap_host: "imap.mail.ru",
            imap_port: 993,
            smtp_host: "smtp.mail.ru",
            smtp_port: 465,
            smtp_tls: SmtpTls::Implicit,
        },
        _ => return None,
    };
    Some(preset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_uses_starttls() {
        let preset = detect("owner@gmail.com").expect("gmail");
        assert_eq!(preset.imap_host, "imap.gmail.com");
        assert_eq!(preset.imap_port, 993);
        assert_eq!(preset.smtp_host, "smtp.gmail.com");
        assert_eq!(preset.smtp_port, 587);
        assert_eq!(preset.smtp_tls, SmtpTls::StartTls);
    }

    #[test]
    fn yandex_uses_implicit_tls() {
        let preset = detect("owner@yandex.ru").expect("yandex");
        assert_eq!(preset.smtp_port, 465);
        assert_eq!(preset.smtp_tls, SmtpTls::Implicit);
    }

    #[test]
    fn domain_is_normalized() {
        assert!(detect("Owner@Hotmail.COM").is_some());
        assert!(detect("owner@MAIL.RU").is_some());
    }

    #[test]
    fn unknown_and_malformed_are_none() {
        assert!(detect("owner@example.com").is_none());
        assert!(detect("без-собаки").is_none());
        assert!(detect("").is_none());
    }
}
