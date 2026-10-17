//! Файлы ядра: статус индекса, запуск сканирования, поиск по имени и тексту.
//!
//! Маршруты [derived] (в Контракте 12 их нет в таком виде): нужны, чтобы
//! секретарь сам находил документы на ПК — `GET /files/status`,
//! `POST /files/index`, `GET /files/search?q=`.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::error::{Result, ServerError};
use crate::files::{FileHit, FilesStatus};
use crate::state::AppState;

/// Параметры поиска.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Что искать.
    #[serde(default)]
    pub q: String,
    /// Сколько результатов.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Лимит по умолчанию.
fn default_limit() -> usize {
    10
}

/// Преобразовать ошибку индекса в HTTP-ошибку.
fn map_error(error: crate::files::FilesError) -> ServerError {
    ServerError::Storage(error.to_string())
}

/// `GET /files/status` — состояние индекса.
pub async fn status(State(state): State<AppState>) -> Result<Json<FilesStatus>> {
    let status = state
        .files
        .status(&state.index_dirs)
        .await
        .map_err(map_error)?;
    Ok(Json(status))
}

/// `POST /files/index` — запустить сканирование в фоне.
pub async fn index(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let index = state.files.clone();
    let dirs = state.index_dirs.clone();
    if dirs.is_empty() {
        return Err(ServerError::BadRequest(
            "каталоги индексации не заданы (Documents/Desktop/Downloads)".to_string(),
        ));
    }
    let scan_dirs = dirs.clone();
    tokio::spawn(async move {
        match index.scan(&scan_dirs).await {
            Ok(count) => tracing::info!(indexed = count, "индекс файлов обновлён"),
            Err(error) => tracing::warn!(error = %error, "сканирование файлов"),
        }
    });
    Ok(Json(json!({
        "started": true,
        "dirs": dirs.iter().map(|dir| dir.display().to_string()).collect::<Vec<_>>(),
    })))
}

/// `GET /files/search?q=&limit=` — поиск по имени и содержимому.
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<FileHit>>> {
    let text = query.q.trim();
    if text.is_empty() {
        return Err(ServerError::BadRequest("параметр q обязателен".to_string()));
    }
    let limit = query.limit.clamp(1, 50);
    let hits = state
        .files
        .search(text, limit)
        .await
        .map_err(map_error)?;
    Ok(Json(hits))
}
