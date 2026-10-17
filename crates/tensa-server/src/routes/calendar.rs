//! HTTP-обработчики календаря (`/api/v1/calendar/*`).

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::calendar::{CalendarError, Event, NewEvent};
use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Окно списка событий (unix seconds).
#[derive(Debug, Deserialize)]
pub struct Window {
    /// Начало окна (включительно).
    #[serde(default)]
    pub from: Option<i64>,
    /// Конец окна (исключительно).
    #[serde(default)]
    pub to: Option<i64>,
}

/// Интервал для проверки конфликтов.
#[derive(Debug, Deserialize)]
pub struct ConflictWindow {
    /// Начало интервала.
    pub starts_at: i64,
    /// Конец интервала.
    pub ends_at: i64,
}

/// Преобразовать ошибку календаря в HTTP-ошибку сервера.
fn map_error(error: CalendarError) -> ServerError {
    match error {
        CalendarError::BadInput(message) => ServerError::BadRequest(message),
        CalendarError::Conflict(events) => {
            let titles: Vec<String> = events.into_iter().map(|event| event.title).collect();
            ServerError::Conflict(format!("пересечение с: {}", titles.join("; ")))
        }
        CalendarError::Storage(message) => ServerError::Storage(message),
    }
}

/// `GET /calendar/events?from=&to=` — события (свежие по началу).
pub async fn list(
    State(state): State<AppState>,
    Query(window): Query<Window>,
) -> Result<Json<Vec<Event>>> {
    let events = state
        .calendar
        .list(window.from, window.to)
        .await
        .map_err(map_error)?;
    Ok(Json(events))
}

/// `POST /calendar/events` — создать событие; 409 при пересечении.
pub async fn create(
    State(state): State<AppState>,
    Json(new_event): Json<NewEvent>,
) -> Result<(StatusCode, Json<Event>)> {
    let event = state.calendar.create(new_event).await.map_err(map_error)?;
    Ok((StatusCode::CREATED, Json(event)))
}

/// `GET /calendar/conflicts?starts_at=&ends_at=` — пересечения интервала.
pub async fn conflicts(
    State(state): State<AppState>,
    Query(window): Query<ConflictWindow>,
) -> Result<Json<Vec<Event>>> {
    let events = state
        .calendar
        .conflicts(window.starts_at, window.ends_at)
        .await
        .map_err(map_error)?;
    Ok(Json(events))
}

/// `DELETE /calendar/events/:id` — удалить событие (204/404).
pub async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|error| ServerError::BadRequest(format!("id не UUID: {error}")))?;
    let removed = state.calendar.delete(uuid).await.map_err(map_error)?;
    if !removed {
        return Err(ServerError::NotFound(format!("событие {uuid}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /calendar/ics` — экспорт в text/calendar.
pub async fn ics(State(state): State<AppState>) -> Result<Response> {
    let body = state.calendar.ics().await.map_err(map_error)?;
    Ok((
        [(header::CONTENT_TYPE, "text/calendar; charset=utf-8")],
        body,
    )
        .into_response())
}
