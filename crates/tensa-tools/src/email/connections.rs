//! Учётные записи почты: `config.toml` `[email]` + правила Контракта 6.
//!
//! Хосты/порты берутся из конфига; если порт 0 или хост пуст — автоопределение
//! по домену адреса (см. [`super::autodetect`]). Секреты здесь не читаются:
//! креденшелы подтягивает сервис (secrets.toml / окружение).
//!
//! [derived] MVP: один активный ящик (Контракт 6), но структура — набор
//! аккаунтов: `find_account(id=None)` возвращает первый.

use super::autodetect::{self, SmtpTls};
use super::error::EmailError;

/// Готовая учётная запись для SMTP/IMAP-клиентов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Идентификатор в конфиге (`[email.accounts.<id>]`).
    pub id: String,
    /// Адрес почты (он же SMTP-логин).
    pub email: String,
    /// Отображаемое имя.
    pub label: String,
    /// IMAP-хост.
    pub imap_host: String,
    /// IMAP-порт.
    pub imap_port: u16,
    /// SMTP-хост.
    pub smtp_host: String,
    /// SMTP-порт.
    pub smtp_port: u16,
    /// Режим TLS для SMTP.
    pub smtp_tls: SmtpTls,
}

/// Собрать учётные записи из конфига ядра (секция `[email.accounts]`).
pub fn resolve_accounts(config: &tensa_core::Config) -> Vec<Account> {
    let Some(accounts) = config
        .email
        .get("accounts")
        .and_then(|value| value.as_table())
    else {
        return Vec::new();
    };
    accounts
        .iter()
        .filter_map(|(id, value)| value.as_table().and_then(|table| build(id, table)))
        .collect()
}

/// Одна запись: явные значения конфига приоритетнее автоопределения.
fn build(id: &str, table: &toml::Table) -> Option<Account> {
    let email = table
        .get("email")
        .and_then(|value| value.as_str())?
        .to_string();
    let label = table
        .get("label")
        .and_then(|value| value.as_str())
        .unwrap_or(id)
        .to_string();
    let preset = autodetect::detect(&email);

    let imap_host = table
        .get("imap_host")
        .and_then(|value| value.as_str())
        .filter(|host| !host.is_empty())
        .map(str::to_string)
        .or_else(|| preset.map(|preset| preset.imap_host.to_string()))?;
    let smtp_host = table
        .get("smtp_host")
        .and_then(|value| value.as_str())
        .filter(|host| !host.is_empty())
        .map(str::to_string)
        .or_else(|| preset.map(|preset| preset.smtp_host.to_string()))?;

    let imap_port = port(table.get("imap_port"))
        .or_else(|| preset.map(|preset| preset.imap_port))
        .unwrap_or(993);
    let smtp_port = port(table.get("smtp_port"))
        .or_else(|| preset.map(|preset| preset.smtp_port))
        .unwrap_or(587);

    // Режим TLS: явный порт важнее предустановки (465 — неявный, 587/25 — STARTTLS).
    let smtp_tls = match smtp_port {
        465 => SmtpTls::Implicit,
        587 | 25 => SmtpTls::StartTls,
        _ => preset
            .map(|preset| preset.smtp_tls)
            .unwrap_or(SmtpTls::StartTls),
    };

    Some(Account {
        id: id.to_string(),
        email,
        label,
        imap_host,
        imap_port,
        smtp_host,
        smtp_port,
        smtp_tls,
    })
}

/// Порт из TOML: положительные целые; 0/мусор → `None` (автоопределение).
fn port(value: Option<&toml::Value>) -> Option<u16> {
    value
        .and_then(|value| value.as_integer())
        .filter(|port| *port > 0)
        .and_then(|port| u16::try_from(port).ok())
}

/// Найти аккаунт по id (или первый — MVP: один активный ящик).
pub fn find_account<'a>(
    accounts: &'a [Account],
    id: Option<&str>,
) -> Result<&'a Account, EmailError> {
    match id {
        Some(id) => accounts
            .iter()
            .find(|account| account.id == id)
            .ok_or_else(|| EmailError::MissingAccount(id.to_string())),
        None => accounts
            .first()
            .ok_or_else(|| EmailError::MissingAccount("<первый активный>".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(entries: &[(&str, &[(&str, toml::Value)])]) -> tensa_core::Config {
        let mut config = tensa_core::Config::default();
        let mut accounts = toml::Table::new();
        for (id, fields) in entries {
            let mut table = toml::Table::new();
            for (key, value) in fields.iter() {
                table.insert((*key).to_string(), value.clone());
            }
            accounts.insert((*id).to_string(), toml::Value::Table(table));
        }
        config
            .email
            .insert("accounts".to_string(), toml::Value::Table(accounts));
        config
    }

    fn s(value: &str) -> toml::Value {
        toml::Value::String(value.to_string())
    }

    #[test]
    fn autodetects_known_provider() {
        let config = config_with(&[("work", &[("email", s("owner@gmail.com"))])]);
        let accounts = resolve_accounts(&config);
        assert_eq!(accounts.len(), 1);
        let account = &accounts[0];
        assert_eq!(account.imap_host, "imap.gmail.com");
        assert_eq!(account.smtp_host, "smtp.gmail.com");
        assert_eq!(account.smtp_port, 587);
        assert_eq!(account.smtp_tls, SmtpTls::StartTls);
    }

    #[test]
    fn explicit_hosts_override_preset() {
        let config = config_with(&[(
            "corp",
            &[
                ("email", s("me@corp.example")),
                ("imap_host", s("mail.corp.example")),
                ("smtp_host", s("mail.corp.example")),
                ("smtp_port", toml::Value::Integer(465)),
            ],
        )]);
        let accounts = resolve_accounts(&config);
        let account = &accounts[0];
        assert_eq!(account.imap_host, "mail.corp.example");
        assert_eq!(account.smtp_host, "mail.corp.example");
        assert_eq!(account.smtp_tls, SmtpTls::Implicit);
    }

    #[test]
    fn zero_port_means_autodetect() {
        let config = config_with(&[(
            "yandex",
            &[
                ("email", s("me@yandex.ru")),
                ("imap_host", s("")),
                ("imap_port", toml::Value::Integer(0)),
                ("smtp_host", s("")),
                ("smtp_port", toml::Value::Integer(0)),
            ],
        )]);
        let accounts = resolve_accounts(&config);
        let account = &accounts[0];
        assert_eq!(account.imap_host, "imap.yandex.ru");
        assert_eq!(account.smtp_host, "smtp.yandex.ru");
        assert_eq!(account.smtp_port, 465);
    }

    #[test]
    fn unknown_domain_without_hosts_is_skipped() {
        let config = config_with(&[("odd", &[("email", s("me@unknown.example"))])]);
        assert!(resolve_accounts(&config).is_empty());
    }

    #[test]
    fn find_account_by_id_or_first() {
        let config = config_with(&[
            ("a", &[("email", s("a@gmail.com"))]),
            ("b", &[("email", s("b@yandex.ru"))]),
        ]);
        let accounts = resolve_accounts(&config);
        assert_eq!(
            find_account(&accounts, Some("b")).expect("b").email,
            "b@yandex.ru"
        );
        assert_eq!(find_account(&accounts, None).expect("first").id, "a");
        assert!(find_account(&accounts, Some("zzz")).is_err());
    }
}
