//! HTTP-маршруты модели ассистента (локальная Gemma / OpenRouter).

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{Result, ServerError};
use crate::query::QueryService;
use crate::state::AppState;

/// Тело подключения OpenRouter.
#[derive(Debug, Deserialize)]
pub struct CloudBody {
    /// API-ключ OpenRouter.
    pub credential: String,
}

/// `GET /brain/status` — состояние локальной и облачной модели.
pub async fn status(State(state): State<AppState>) -> Result<Json<Value>> {
    Ok(Json(state.brain.status().await))
}

/// `POST /brain/local/download` — скачать локальную Gemma.
pub async fn download_local(State(state): State<AppState>) -> Result<Json<Value>> {
    state
        .brain
        .download_local()
        .await
        .map_err(ServerError::BadRequest)?;
    Ok(Json(json!({ "status": "downloading" })))
}

/// `POST /brain/local/activate` — использовать локальную Gemma.
pub async fn activate_local(State(state): State<AppState>) -> Result<Json<Value>> {
    state
        .brain
        .activate_local()
        .map_err(ServerError::BadRequest)?;
    reload_query(&state).await?;
    Ok(Json(state.brain.status().await))
}

/// `POST /brain/cloud/activate` — включить сохранённый OpenRouter.
pub async fn activate_cloud(State(state): State<AppState>) -> Result<Json<Value>> {
    state
        .brain
        .activate_cloud()
        .map_err(ServerError::BadRequest)?;
    reload_query(&state).await?;
    Ok(Json(state.brain.status().await))
}

/// `POST /brain/cloud` — сохранить ключ и включить Gemma 26B через OpenRouter.
pub async fn connect_cloud(
    State(state): State<AppState>,
    Json(body): Json<CloudBody>,
) -> Result<Json<Value>> {
    state
        .brain
        .connect_cloud(&body.credential)
        .map_err(ServerError::BadRequest)?;
    reload_query(&state).await?;
    Ok(Json(state.brain.status().await))
}

/// Пересобрать QueryService после смены модели/ключа.
async fn reload_query(state: &AppState) -> Result<()> {
    let config = tensa_core::Config::load(&state.config_path)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))?;
    let paths = tensa_core::TensaPaths::with_root(state.data_root.clone());
    let secrets = tensa_core::Secrets::load(&paths.secrets_file())
        .map_err(|error| ServerError::Storage(format!("secrets.toml: {error}")))?;
    let fresh = QueryService::from_config(&config.brain, &secrets, &paths);
    *state.query.write().await = Arc::new(fresh);
    Ok(())
}
