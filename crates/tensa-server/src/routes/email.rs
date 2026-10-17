//! Почта: список учётных записей из конфига (`GET /api/v1/email/accounts`).
//!
//! Отправка/чтение писем — контур tensa-tools (SMTP/IMAP); сервер отдаёт
//! APK/PWA состав аккаунтов, чтобы экран «Почта» показывал реальные записи.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::Result;
use crate::state::AppState;

/// Учётная запись почты для клиентов.
#[derive(Debug, Clone, Serialize)]
pub struct AccountInfo {
    /// Идентификатор в конфиге (`[email.accounts.<id>]`).
    pub id: String,
    /// Адрес почты.
    pub email: String,
    /// Отображаемое имя.
    pub label: String,
}

/// Собрать аккаунты из секции `[email]` конфига.
pub fn accounts_from_config(email: &toml::Table) -> Vec<AccountInfo> {
    let Some(accounts) = email.get("accounts").and_then(|value| value.as_table()) else {
        return Vec::new();
    };
    accounts
        .iter()
        .map(|(id, value)| AccountInfo {
            id: id.clone(),
            email: value
                .get("email")
                .and_then(|field| field.as_str())
                .unwrap_or_default()
                .to_string(),
            label: value
                .get("label")
                .and_then(|field| field.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect()
}

/// `GET /email/accounts` — аккаунты из конфига.
pub async fn accounts(State(state): State<AppState>) -> Result<Json<Vec<AccountInfo>>> {
    Ok(Json(state.email_accounts.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accounts_section() {
        let raw = r#"
[accounts.work]
email = "me@example.com"
label = "Работа"
[accounts.personal]
email = "home@example.com"
label = "Личная"
"#;
        let table: toml::Table = toml::from_str(raw).expect("toml");
        let accounts = accounts_from_config(&table);
        assert_eq!(accounts.len(), 2);
        let work = accounts
            .iter()
            .find(|account| account.id == "work")
            .expect("work");
        assert_eq!(work.email, "me@example.com");
        assert_eq!(work.label, "Работа");
    }
}
