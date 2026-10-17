//! Файлы ядра: статус индекса, запуск сканирования, поиск по имени и тексту.
//!
//! Маршруты [derived] (в Контракте 12 их нет в таком виде): нужны, чтобы
//! секретарь сам находил документы на ПК — `GET /files/status`,
//! `POST /files/index`, `GET /files/search?q=`.

use axum::extract::{Query, State};
use axum::response::IntoResponse;
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

/// Тело запроса «открыть файл».
#[derive(Debug, Deserialize)]
pub struct OpenBody {
    /// Путь, который вернул поиск.
    pub path: String,
}

/// `POST /files/open` — открыть файл на этом ПК программой по умолчанию.
///
/// Открываются только файлы из индекса (произвольный путь не подсунуть).
pub async fn open_file(
    State(state): State<AppState>,
    Json(body): Json<OpenBody>,
) -> Result<Json<serde_json::Value>> {
    let path = body.path.trim().to_string();
    if path.is_empty() {
        return Err(ServerError::BadRequest("path обязателен".to_string()));
    }
    let indexed = state.files.search_exact(&path).await.map_err(map_error)?;
    if !indexed {
        return Err(ServerError::NotFound(
            "файл не найден в индексе".to_string(),
        ));
    }
    open_with_default_app(&path)?;
    Ok(Json(json!({ "opened": true, "path": path })))
}

/// `GET /files/download?path=` — отдать файл (телефон/браузер).
pub async fn download(
    State(state): State<AppState>,
    Query(query): Query<OpenBody>,
) -> Result<axum::response::Response> {
    let path = query.path.trim().to_string();
    if path.is_empty() {
        return Err(ServerError::BadRequest("path обязателен".to_string()));
    }
    let indexed = state.files.search_exact(&path).await.map_err(map_error)?;
    if !indexed {
        return Err(ServerError::NotFound(
            "файл не найден в индексе".to_string(),
        ));
    }
    let data = tokio::fs::read(&path).await.map_err(ServerError::Io)?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let disposition = format!("attachment; filename*=UTF-8''{}", urlencode(&name));
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/octet-stream".to_string(),
            ),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
        ],
        data,
    )
        .into_response())
}

/// Процентное кодирование имени файла для Content-Disposition.
fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Открыть файл штатным приложением (Windows: start, прочие: xdg-open).
fn open_with_default_app(path: &str) -> Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(path)
            .spawn()
            .map_err(|error| ServerError::Storage(format!("открытие файла: {error}")))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| ServerError::Storage(format!("открытие файла: {error}")))?;
        Ok(())
    }
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
    let hits = state.files.search(text, limit).await.map_err(map_error)?;
    Ok(Json(hits))
}
