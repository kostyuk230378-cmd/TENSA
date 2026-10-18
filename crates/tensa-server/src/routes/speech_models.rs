//! HTTP-маршруты каталога моделей распознавания речи.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// `GET /models/speech` — каталог и активная модель.
pub async fn list(State(state): State<AppState>) -> Result<Json<Value>> {
    let (models, active) = state.speech_models.list().await;
    Ok(Json(json!({ "models": models, "active": active })))
}

/// `POST /models/speech/:id/download` — начать загрузку.
pub async fn download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    state
        .speech_models
        .download(&id)
        .await
        .map_err(ServerError::BadRequest)?;
    Ok(Json(json!({ "status": "ok" })))
}

/// `POST /models/speech/:id/activate` — выбрать модель.
pub async fn activate(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    state
        .speech_models
        .activate(&id)
        .await
        .map_err(ServerError::BadRequest)?;
    Ok(Json(json!({ "status": "ok", "active": id })))
}
