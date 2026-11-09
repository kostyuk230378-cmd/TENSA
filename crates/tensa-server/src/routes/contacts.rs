//! Контакты (Контракт 12 / Контракт 3): CRUD + FTS5-поиск поверх
//! `tensa_core::SqliteContactsStore`. Инструменты LLM из
//! `system_prompt_tensa.md` маппятся так: `create_contact` → `POST /contacts`,
//! `find_contact` → `GET /contacts/search?q=`.

use std::str::FromStr;

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use tensa_core::NewContact;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Параметры списка.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Сколько вернуть (1..=200, по умолчанию 50).
    pub limit: Option<usize>,
    /// Смещение.
    pub offset: Option<usize>,
}

/// Параметры поиска.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Запрос (FTS5 + имя/почта).
    pub q: String,
    /// Сколько вернуть (1..=200, по умолчанию 20).
    pub limit: Option<usize>,
}

/// Валидация полей контакта: имя обязательно, email — форматом с `@`.
fn validate(contact: &NewContact) -> Result<()> {
    if contact.name.trim().is_empty() {
        return Err(ServerError::BadRequest(
            "имя контакта обязательно".to_string(),
        ));
    }
    if let Some(email) = contact.email.as_deref() {
        let email = email.trim();
        if !email.is_empty()
            && (!email.contains('@') || email.starts_with('@') || email.ends_with('@'))
        {
            return Err(ServerError::BadRequest(format!(
                "некорректный email «{email}»"
            )));
        }
    }
    Ok(())
}

/// `GET /contacts` — список (свежие первыми, порядок хранилища).
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<tensa_core::Contact>>> {
    let limit = query.limit.unwrap_or(50).min(200);
    let contacts = state
        .contacts
        .list_contacts(limit, query.offset.unwrap_or(0))
        .await?;
    Ok(Json(contacts))
}

/// `POST /contacts` — создание контакта (201 + созданный контакт).
pub async fn create(
    State(state): State<AppState>,
    Json(contact): Json<NewContact>,
) -> Result<(StatusCode, Json<tensa_core::Contact>)> {
    validate(&contact)?;
    let created = state.contacts.create_contact(contact).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /contacts/search?q=` — поиск (инструмент LLM `find_contact`).
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<tensa_core::Contact>>> {
    if query.q.trim().is_empty() {
        return Err(ServerError::BadRequest("параметр q обязателен".to_string()));
    }
    let limit = query.limit.unwrap_or(20).min(200);
    let contacts = state.contacts.search_contacts(&query.q, limit).await?;
    Ok(Json(contacts))
}

/// Загрузить контакт по строковому id или вернуть 404.
async fn load(state: &AppState, id: &str) -> Result<tensa_core::Contact> {
    let uuid = Uuid::from_str(id)
        .map_err(|error| ServerError::BadRequest(format!("id контакта: {error}")))?;
    state
        .contacts
        .get_contact(uuid)
        .await?
        .filter(|contact| !contact.deleted)
        .ok_or_else(|| ServerError::NotFound(format!("контакт {id}")))
}

/// `GET /contacts/:id` — один контакт.
pub async fn get(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<tensa_core::Contact>> {
    Ok(Json(load(&state, &id).await?))
}

/// `PUT /contacts/:id` — обновление полей контакта (name/email/phone/
/// telegram/whatsapp/notes/tags).
pub async fn update(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<NewContact>,
) -> Result<Json<tensa_core::Contact>> {
    validate(&body)?;
    let mut contact = load(&state, &id).await?;
    contact.name = body.name;
    contact.email = body.email;
    contact.phone = body.phone;
    contact.telegram = body.telegram;
    contact.whatsapp = body.whatsapp;
    contact.notes = body.notes;
    contact.tags = body.tags;
    let updated = state.contacts.update_contact(contact).await?;
    Ok(Json(updated))
}

/// `DELETE /contacts/:id` — мягкое удаление (Контракт 3).
pub async fn delete(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>> {
    let contact = load(&state, &id).await?;
    state.contacts.delete_contact(contact.id).await?;
    Ok(Json(json!({ "status": "ok" })))
}
