//! Настройка почты: проверка SMTP, сохранение в config/secrets.
//!
//! Пароль приходит в поле `credential`, никогда не логируется и не отдаётся
//! обратно в API. Поддерживаются Gmail/Outlook/Яндекс/Mail.ru и произвольный
//! SMTP при явно указанных хостах.

use std::path::Path;

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use tensa_core::{Secrets, TensaPaths};
use tensa_tools::email::autodetect::{self, SmtpTls};
use tensa_tools::email::connections::Account;
use tensa_tools::email::{smtp, EmailService};

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Тело настройки почты.
#[derive(Debug, Deserialize)]
pub struct EmailBody {
    /// Адрес.
    pub email: String,
    /// Подпись.
    #[serde(default)]
    pub label: String,
    /// Пароль приложения.
    pub credential: String,
    /// SMTP-хост (пусто — автоопределение по домену).
    #[serde(default)]
    pub smtp_host: Option<String>,
    /// SMTP-порт (0 — авто).
    #[serde(default)]
    pub smtp_port: Option<u16>,
    /// IMAP-хост.
    #[serde(default)]
    pub imap_host: Option<String>,
    /// IMAP-порт.
    #[serde(default)]
    pub imap_port: Option<u16>,
}

/// Учётная запись для UI.
#[derive(Debug, Clone, Serialize)]
pub struct EmailView {
    /// id в config.
    pub id: String,
    /// Адрес.
    pub email: String,
    /// Подпись.
    pub label: String,
    /// SMTP-хост.
    pub smtp_host: String,
    /// SMTP-порт.
    pub smtp_port: u16,
    /// IMAP-хост.
    pub imap_host: String,
    /// IMAP-порт.
    pub imap_port: u16,
    /// Пароль сохранён.
    pub has_credential: bool,
}

/// `GET /email/config` — аккаунты с серверами, без пароля.
pub async fn config(State(state): State<AppState>) -> Result<Json<Value>> {
    let service = EmailService::from_files(&state.config_path)
        .map_err(|error| ServerError::Storage(format!("почта: {error}")))?;
    let secrets = Secrets::load(&TensaPaths::with_root(state.data_root.clone()).secrets_file())
        .unwrap_or_default();
    let accounts: Vec<EmailView> = service
        .accounts()
        .into_iter()
        .map(|account| EmailView {
            has_credential: secrets
                .email
                .accounts
                .get(&account.id)
                .and_then(|secret| secret.credential.as_ref())
                .is_some(),
            id: account.id,
            email: account.email,
            label: account.label,
            smtp_host: account.smtp_host,
            smtp_port: account.smtp_port,
            imap_host: account.imap_host,
            imap_port: account.imap_port,
        })
        .collect();
    Ok(Json(json!({ "accounts": accounts })))
}

/// `POST /email/accounts` — проверить SMTP и сохранить аккаунт.
pub async fn save(
    State(state): State<AppState>,
    Json(body): Json<EmailBody>,
) -> Result<Json<Value>> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(ServerError::BadRequest("некорректный email".to_string()));
    }
    if body.credential.trim().is_empty() {
        return Err(ServerError::BadRequest(
            "нужен пароль приложения".to_string(),
        ));
    }
    let preset = autodetect::detect(&email);
    let smtp_host = body
        .smtp_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| preset.map(|preset| preset.smtp_host.to_string()))
        .ok_or_else(|| {
            ServerError::BadRequest("неизвестный домен: укажите SMTP-сервер".to_string())
        })?;
    let imap_host = body
        .imap_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| preset.map(|preset| preset.imap_host.to_string()))
        .unwrap_or_else(|| smtp_host.clone());
    let smtp_port = body
        .smtp_port
        .filter(|port| *port > 0)
        .or_else(|| preset.map(|preset| preset.smtp_port))
        .unwrap_or(587);
    let imap_port = body
        .imap_port
        .filter(|port| *port > 0)
        .or_else(|| preset.map(|preset| preset.imap_port))
        .unwrap_or(993);
    let smtp_tls = match smtp_port {
        465 => SmtpTls::Implicit,
        _ => SmtpTls::StartTls,
    };

    let test_account = Account {
        id: "test".to_string(),
        email: email.clone(),
        label: body.label.clone(),
        imap_host: imap_host.clone(),
        imap_port,
        smtp_host: smtp_host.clone(),
        smtp_port,
        smtp_tls,
    };
    smtp::test_connection(&test_account, body.credential.trim())
        .await
        .map_err(|error| ServerError::BadRequest(format!("SMTP: {error}")))?;

    let id = account_id(&state.config_path, &email);
    write_account(&state.config_path, &id, &test_account)?;
    let paths = TensaPaths::with_root(state.data_root.clone());
    let mut secrets = Secrets::load(&paths.secrets_file()).unwrap_or_default();
    secrets
        .email
        .accounts
        .entry(id.clone())
        .or_default()
        .credential = Some(body.credential.trim().to_string());
    secrets
        .save(&paths.secrets_file())
        .map_err(|error| ServerError::Storage(format!("secrets.toml: {error}")))?;
    Ok(Json(json!({ "status": "ok", "id": id })))
}

/// `DELETE /email/accounts/:id` — удалить аккаунт.
pub async fn remove(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Value>> {
    let mut table = config_table(&state.config_path)?;
    let removed = table
        .get_mut("email")
        .and_then(|email| email.get_mut("accounts"))
        .and_then(|accounts| accounts.as_table_mut())
        .and_then(|accounts| accounts.remove(&id))
        .is_some();
    if !removed {
        return Err(ServerError::NotFound(format!("аккаунт {id}")));
    }
    save_table(&state.config_path, &table)?;
    let paths = TensaPaths::with_root(state.data_root.clone());
    let mut secrets = Secrets::load(&paths.secrets_file()).unwrap_or_default();
    secrets.email.accounts.remove(&id);
    secrets
        .save(&paths.secrets_file())
        .map_err(|error| ServerError::Storage(format!("secrets.toml: {error}")))?;
    Ok(Json(json!({ "status": "ok" })))
}

fn account_id(config_path: &Path, email: &str) -> String {
    let table = config_table(config_path).unwrap_or_default();
    if let Some(accounts) = table
        .get("email")
        .and_then(|email| email.get("accounts"))
        .and_then(|accounts| accounts.as_table())
    {
        for (id, value) in accounts {
            if value.get("email").and_then(|field| field.as_str()) == Some(email) {
                return id.clone();
            }
        }
    }
    let base: String = email
        .split('@')
        .next()
        .unwrap_or("account")
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(20)
        .collect();
    let base = if base.is_empty() {
        "account".to_string()
    } else {
        base
    };
    let mut candidate = base.clone();
    let mut index = 2;
    while accounts_contains(&table, &candidate) {
        candidate = format!("{base}{index}");
        index += 1;
    }
    candidate
}

fn accounts_contains(table: &toml::Table, id: &str) -> bool {
    table
        .get("email")
        .and_then(|email| email.get("accounts"))
        .and_then(|accounts| accounts.get(id))
        .is_some()
}

fn write_account(config_path: &Path, id: &str, account: &Account) -> Result<()> {
    let mut table = config_table(config_path)?;
    let email_table = table
        .entry("email")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| ServerError::Storage("config.toml: [email] не таблица".to_string()))?;
    let accounts = email_table
        .entry("accounts")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            ServerError::Storage("config.toml: [email.accounts] не таблица".to_string())
        })?;
    let mut entry = toml::Table::new();
    entry.insert(
        "email".to_string(),
        toml::Value::String(account.email.clone()),
    );
    entry.insert(
        "label".to_string(),
        toml::Value::String(account.label.clone()),
    );
    entry.insert(
        "smtp_host".to_string(),
        toml::Value::String(account.smtp_host.clone()),
    );
    entry.insert(
        "smtp_port".to_string(),
        toml::Value::Integer(i64::from(account.smtp_port)),
    );
    entry.insert(
        "imap_host".to_string(),
        toml::Value::String(account.imap_host.clone()),
    );
    entry.insert(
        "imap_port".to_string(),
        toml::Value::Integer(i64::from(account.imap_port)),
    );
    accounts.insert(id.to_string(), toml::Value::Table(entry));
    save_table(config_path, &table)
}

fn config_table(path: &Path) -> Result<toml::Table> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))?;
    toml::from_str(&raw).map_err(|error| ServerError::Storage(format!("config.toml: {error}")))
}

fn save_table(path: &Path, table: &toml::Table) -> Result<()> {
    let body = toml::to_string_pretty(table)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))?;
    std::fs::write(path, body)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))
}
