//! `/approvals` (Контракт 12): очередь одобрения поверх `tensa-approval`.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use tensa_approval::models::{Action, ActionOrigin};
use uuid::Uuid;

use crate::error::{Result, ServerError};
use crate::state::{AppState, ServerEvent};

/// Тело `POST /approvals/{id}/approve` (все поля необязательны).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApproveRequest {
    /// Кто подтвердил действие (ТЗ `apps/pwa`: `approved_by`, по умолчанию `user`).
    #[serde(default)]
    pub approved_by: Option<String>,
}

/// Тело `POST /approvals/{id}/reject` (все поля необязательны).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RejectRequest {
    /// Кто отклонил действие (по умолчанию `user`).
    #[serde(default)]
    pub rejected_by: Option<String>,
}

/// `GET /approvals` — ожидающие одобрения действия (Контракт 12: `GET /approvals`).
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Action>>> {
    Ok(Json(state.approvals.get_pending().await?))
}

/// `GET /approvals/{id}` — одно действие, включая уже решённые.
pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Action>> {
    let id = parse_id(&id)?;
    match state.approvals.get_by_id(id).await? {
        Some(action) => Ok(Json(action)),
        None => Err(ServerError::NotFound(format!("действие {id} не найдено"))),
    }
}

/// `POST /approvals/{id}/approve` — подтвердить действие.
pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Action>> {
    let id = parse_id(&id)?;
    let request: ApproveRequest = parse_body(&body)?;
    let origin = origin_from(request.approved_by.as_deref());
    let action = state.approvals.approve(id, origin).await?;
    publish_decision(&state, &action);
    Ok(Json(action))
}

/// `POST /approvals/{id}/reject` — отклонить действие.
pub async fn reject(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Action>> {
    let id = parse_id(&id)?;
    let request: RejectRequest = parse_body(&body)?;
    let origin = origin_from(request.rejected_by.as_deref());
    let action = state.approvals.reject(id, origin).await?;
    publish_decision(&state, &action);
    Ok(Json(action))
}

/// Оповестить подписчиков SSE об изменении действия (ТЗ `apps/pwa`: `approval:updated`).
fn publish_decision(state: &AppState, action: &Action) {
    let data = match serde_json::to_value(action) {
        Ok(data) => data,
        Err(err) => {
            tracing::warn!(action = %action.id, error = %err, "не удалось сериализовать действие для SSE");
            return;
        }
    };
    state.publish(ServerEvent::new(ServerEvent::APPROVAL_UPDATED, data));
}

/// Разбор UUID из пути (400 при неверном формате).
fn parse_id(raw: &str) -> Result<Uuid> {
    Uuid::parse_str(raw)
        .map_err(|err| ServerError::BadRequest(format!("некорректный id {raw}: {err}")))
}

/// Толерантный разбор тела: пустое тело — значения по умолчанию.
///
/// В axum 0.7 `Option<Json<T>>` не срабатывает на пустой строке запроса,
/// поэтому тело читается как [`Bytes`] и разбирается вручную.
fn parse_body<T>(body: &Bytes) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if body.is_empty() {
        return Ok(T::default());
    }
    serde_json::from_slice(body)
        .map_err(|err| ServerError::BadRequest(format!("некорректное тело запроса: {err}")))
}

/// Происхождение действия по имени человека, инициировавшего решение.
///
/// Неизвестное имя тоже считается действием пользователя (канал `pwa`),
/// потому что решение пришло из интерфейса, а не от автоматики.
fn origin_from(actor: Option<&str>) -> ActionOrigin {
    match actor.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.parse().unwrap_or(ActionOrigin::User),
        None => ActionOrigin::User,
    }
}
