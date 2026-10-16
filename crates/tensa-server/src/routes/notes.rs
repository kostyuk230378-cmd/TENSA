//! `/notes` (Контракт 12): CRUD заметок поверх `tensa_core::NotesStore`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tensa_core::{NewNote, Note};
use uuid::Uuid;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Лимит списка заметок по умолчанию.
const DEFAULT_LIMIT: usize = 50;

/// Лимит поиска по умолчанию (Контракт 12: `GET /notes/search?q=`).
const DEFAULT_SEARCH_LIMIT: usize = 10;

/// Тело запроса создания/обновления заметки (Контракт 12: `NewNote`).
///
/// Поля, которыми управляет ядро (`id`, `created_at`, `updated_at`, векторные
/// часы, узел), в запросе не принимаются — их назначает хранилище.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NoteBody {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Query-параметры `GET /notes`.
#[derive(Debug, Clone, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

/// Query-параметры `GET /notes/search`.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

fn default_search_limit() -> usize {
    DEFAULT_SEARCH_LIMIT
}

/// `GET /notes` — список заметок, свежие первыми.
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Note>>> {
    let notes = state.notes.list_notes(query.limit, query.offset).await?;
    Ok(Json(notes))
}

/// `POST /notes` — создание заметки (201 + созданная заметка).
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<NoteBody>,
) -> Result<(StatusCode, Json<Note>)> {
    let draft = NewNote {
        title: body.title,
        body: body.body,
        tags: body.tags,
    };
    let note = state.notes.create_note(draft).await?;
    Ok((StatusCode::CREATED, Json(note)))
}

/// `GET /notes/{id}` — одна заметка.
pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Note>> {
    Ok(Json(load(&state, &id).await?))
}

/// `PUT /notes/{id}` — обновление заметки целиком (`title`, `body`, `tags`).
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<NoteBody>,
) -> Result<Json<Note>> {
    let mut note = load(&state, &id).await?;
    note.title = body.title;
    note.body = body.body;
    note.tags = body.tags;
    let updated = state.notes.update_note(note).await?;
    Ok(Json(updated))
}

/// `DELETE /notes/{id}` — удаление заметки (Контракт 3: мягкое удаление).
pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    let id = parse_id(&id)?;
    state.notes.delete_note(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /notes/search?q=&limit=` — полнотекстовый поиск (FTS5).
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Note>>> {
    let raw = query.q.unwrap_or_default();
    let query_text = raw.trim();
    if query_text.is_empty() {
        return Err(ServerError::BadRequest("параметр q обязателен".to_string()));
    }
    let notes = state.notes.search_notes(query_text, query.limit).await?;
    Ok(Json(notes))
}

/// Загрузить заметку по строковому id или вернуть 404.
async fn load(state: &AppState, raw_id: &str) -> Result<Note> {
    let id = parse_id(raw_id)?;
    match state.notes.get_note(id).await? {
        Some(note) => Ok(note),
        None => Err(ServerError::NotFound(format!("заметка {id} не найдена"))),
    }
}

/// Разбор UUID из пути (400 при неверном формате).
fn parse_id(raw: &str) -> Result<Uuid> {
    Uuid::parse_str(raw)
        .map_err(|err| ServerError::BadRequest(format!("некорректный id {raw}: {err}")))
}
