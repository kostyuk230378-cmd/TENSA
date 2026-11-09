//! Приложения операционной системы (Контракт 5, MVP): список, сканирование,
//! запуск, открытие URL. Инструменты LLM из `system_prompt_tensa.md`:
//! `launch_app` → `POST /apps/launch`, `open_url` → `POST /apps/open-url`.
//!
//! Запуск инициируется владельцем (Bearer-запрос, trusted origin): применяется
//! чёрный список `[security.denylist]` из `apps.toml`; полный белый список
//! Контракта 5 — следующая доводка. Кэш сканирования — `apps_cache.json`.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use tensa_tools::apps;
use tensa_tools::apps::ScannedApp;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Тело запуска приложения.
#[derive(Debug, Deserialize)]
pub struct LaunchBody {
    /// Имя приложения из списка.
    pub name: String,
}

/// Тело открытия URL.
#[derive(Debug, Deserialize)]
pub struct UrlBody {
    /// Адрес (только http/https).
    pub url: String,
}

/// Список приложений: кэш, а без кэша — сканирование с записью кэша.
fn cached_or_scan(state: &AppState) -> Vec<ScannedApp> {
    apps::read_cache(&state.data_root).unwrap_or_else(|| {
        let found = apps::scan_apps();
        if let Err(error) = apps::write_cache(&state.data_root, &found) {
            tracing::warn!(error = %error, "apps: кэш не записан");
        }
        found
    })
}

/// `GET /apps` — список приложений (кэш сканирования).
pub async fn list(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let apps = cached_or_scan(&state);
    Ok(Json(json!({ "apps": apps })))
}

/// `POST /apps/scan` — пересканировать систему и обновить кэш.
pub async fn scan(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    let found = apps::scan_apps();
    let count = found.len();
    apps::write_cache(&state.data_root, &found)
        .map_err(|error| ServerError::Storage(format!("apps_cache.json: {error}")))?;
    Ok(Json(json!({ "count": count })))
}

/// `POST /apps/launch` — запустить приложение по имени из списка.
pub async fn launch(
    State(state): State<AppState>,
    Json(body): Json<LaunchBody>,
) -> Result<Json<serde_json::Value>> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ServerError::BadRequest("нужно имя приложения".to_string()));
    }
    let lowered = name.to_lowercase();
    if apps::denylist(&state.data_root)
        .iter()
        .any(|denied| lowered.contains(denied.as_str()))
    {
        return Err(ServerError::BadRequest(format!(
            "приложение «{name}» в чёрном списке"
        )));
    }
    let found = cached_or_scan(&state);
    let app = found
        .iter()
        .find(|app| app.name.to_lowercase() == lowered)
        .ok_or_else(|| {
            ServerError::NotFound(format!(
                "приложение «{name}» не в списке — выполните POST /apps/scan"
            ))
        })?;
    apps::launch(app).map_err(|error| ServerError::Storage(format!("запуск: {error}")))?;
    Ok(Json(json!({ "status": "ok", "name": app.name })))
}

/// `POST /apps/open-url` — открыть URL штатным браузером.
pub async fn open_url(
    State(_state): State<AppState>,
    Json(body): Json<UrlBody>,
) -> Result<Json<serde_json::Value>> {
    apps::open_url(&body.url).map_err(ServerError::BadRequest)?;
    Ok(Json(json!({ "status": "ok" })))
}
