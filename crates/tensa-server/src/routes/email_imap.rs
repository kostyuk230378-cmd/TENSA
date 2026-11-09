//! IMAP-контур почты (Контракт 6): подключение, папки, письма, поиск,
//! отправка через существующий SMTP-сервис. Инструменты LLM из
//! `system_prompt_tensa.md`: `fetch_emails` → `GET .../emails`,
//! `search_emails` → `GET .../search?q=`, `send_email` → `POST /email/send`.
//!
//! Соединения stateless: каждое обращение открывает IMAP-сессию и закрывает
//! её; `/connect` — проверка логина, `/disconnect` — подтверждение аккаунта
//! без сети (постоянные сессии — следующая доводка).

use axum::extract::{Path as AxumPath, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use tensa_core::{Secrets, TensaPaths};
use tensa_tools::email::connections::Account;
use tensa_tools::email::{imap, EmailService};

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Параметры выборки писем.
#[derive(Debug, Deserialize)]
pub struct EmailsQuery {
    /// Папка (по умолчанию INBOX).
    pub folder: Option<String>,
    /// Сколько последних писем (1..=100, по умолчанию 20).
    pub limit: Option<usize>,
}

/// Параметры поиска писем.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Запрос (IMAP `SEARCH TEXT`).
    pub q: String,
}

/// Тело отправки письма.
#[derive(Debug, Deserialize)]
pub struct SendBody {
    /// Аккаунт-отправитель (None — активный по конфигу).
    pub account_id: Option<String>,
    /// Получатель.
    pub to: String,
    /// Тема.
    pub subject: String,
    /// Текст.
    pub body: String,
}

/// Ошибка почтового инструмента → HTTP-ошибка (аккаунт — 404, остальное 400).
fn email_error(error: tensa_tools::EmailError) -> ServerError {
    match error {
        tensa_tools::EmailError::MissingAccount(id) => ServerError::NotFound(id),
        other => ServerError::BadRequest(other.to_string()),
    }
}

/// Аккаунт конфига + креденшел из secrets.toml (не логируется).
fn imap_account(state: &AppState, id: &str) -> Result<(Account, String)> {
    let service = EmailService::from_files(&state.config_path).map_err(|error| {
        ServerError::Storage(format!(
            "почта: {}",
            tensa_tools::email::error::shorten(&error.to_string(), 300)
        ))
    })?;
    let account = service
        .accounts()
        .into_iter()
        .find(|account| account.id == id)
        .ok_or_else(|| ServerError::NotFound(format!("почтовый аккаунт {id}")))?;
    let secrets = Secrets::load(&TensaPaths::with_root(state.data_root.clone()).secrets_file())
        .unwrap_or_default();
    let credential = secrets
        .email
        .accounts
        .get(id)
        .and_then(|secret| secret.credential.clone())
        .ok_or_else(|| ServerError::NotFound(format!("креденшелы аккаунта {id}")))?;
    Ok((account, credential))
}

/// `POST /email/accounts/:id/connect` — проверить IMAP-логин.
pub async fn connect(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>> {
    let (account, credential) = imap_account(&state, &id)?;
    imap::list_folders(&account, &credential)
        .await
        .map_err(|error| {
            ServerError::Unavailable(tensa_tools::email::error::shorten(&error.to_string(), 300))
        })?;
    Ok(Json(json!({ "status": "ok", "id": id })))
}

/// `POST /email/accounts/:id/disconnect` — подтвердить аккаунт (сессии
/// stateless — соединение закрывается после каждого запроса).
pub async fn disconnect(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>> {
    imap_account(&state, &id)?;
    Ok(Json(json!({ "status": "ok", "id": id })))
}

/// `GET /email/accounts/:id/folders` — папки IMAP.
pub async fn folders(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>> {
    let (account, credential) = imap_account(&state, &id)?;
    let folders = imap::list_folders(&account, &credential)
        .await
        .map_err(|error| {
            ServerError::Unavailable(tensa_tools::email::error::shorten(&error.to_string(), 300))
        })?;
    Ok(Json(json!({ "folders": folders })))
}

/// `GET /email/accounts/:id/emails?folder=&limit=` — последние письма.
pub async fn emails(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<EmailsQuery>,
) -> Result<Json<serde_json::Value>> {
    let (account, credential) = imap_account(&state, &id)?;
    let folder = query
        .folder
        .as_deref()
        .unwrap_or("INBOX")
        .trim()
        .to_string();
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let emails = imap::fetch_emails(&account, &credential, &folder, limit)
        .await
        .map_err(|error| {
            ServerError::Unavailable(tensa_tools::email::error::shorten(&error.to_string(), 300))
        })?;
    Ok(Json(json!({ "emails": emails })))
}

/// `GET /email/accounts/:id/search?q=` — поиск по письмам (INBOX).
pub async fn search(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>> {
    if query.q.trim().is_empty() {
        return Err(ServerError::BadRequest("параметр q обязателен".to_string()));
    }
    let (account, credential) = imap_account(&state, &id)?;
    let emails = imap::search_emails(&account, &credential, &query.q)
        .await
        .map_err(|error| {
            ServerError::Unavailable(tensa_tools::email::error::shorten(&error.to_string(), 300))
        })?;
    Ok(Json(json!({ "emails": emails })))
}

/// `POST /email/send` — отправить письмо (SMTP; одобрение не требуется —
/// команда владельца, как в `EmailService::send_now`).
pub async fn send(
    State(state): State<AppState>,
    Json(body): Json<SendBody>,
) -> Result<Json<serde_json::Value>> {
    let to = body.to.trim().to_string();
    if !to.contains('@') || to.starts_with('@') || to.ends_with('@') {
        return Err(ServerError::BadRequest(
            "некорректный адрес получателя".to_string(),
        ));
    }
    let service = EmailService::from_files(&state.config_path)
        .map_err(|error| ServerError::Storage(format!("почта: {error}")))?;
    service
        .send_now(tensa_tools::email::smtp::OutgoingEmail {
            account_id: body.account_id,
            to,
            cc: None,
            bcc: None,
            subject: body.subject,
            body: body.body,
            attachment: None,
        })
        .await
        .map_err(email_error)?;
    Ok(Json(json!({ "status": "ok" })))
}
