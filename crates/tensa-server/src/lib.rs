//! HTTP-бэкенд PWA (Контракт 12): axum-роутер поверх крейтов Tensa.
//!
//! Состав MVP:
//! - `GET /api/v1/health`, `GET /api/v1/version` — состояние и версия ядра;
//! - `GET /api/v1/approvals` (+ approve/reject) — очередь одобрения;
//! - `GET /api/v1/notes` (+ CRUD и поиск) — заметки;
//! - `GET /api/v1/events` — SSE-поток событий;
//! - прочие маршруты контракта — 501 (см. [`routes::stubs`]);
//! - `GET /` и `GET /app` — статика PWA (публична), ассеты — `/static/...`;
//! - доступ к API — по Bearer-токену из `<root>/pwa_token`
//!   (см. [`load_or_create_token`]).
//!
//! Точка сборки приложения — [`router`]; запуск — [`serve`].

pub mod auth;
pub mod calendar;
pub mod error;
pub mod models_upload;
pub mod query;
pub mod routes;
pub mod state;

pub use auth::{load_or_create_token, token_path, PwaToken, TOKEN_FILE};
pub use error::{ErrorBody, Result, ServerError};
pub use query::{QueryMode, QueryService};
pub use state::{AppState, ServerEvent};

use std::net::SocketAddr;

use axum::middleware;
use axum::Router;

/// Полное приложение: публичная статика и API `/api/v1` за Bearer-авторизацией.
///
/// Разделение доступа по ТЗ `apps/pwa`: оболочка PWA и ассеты отдаются без
/// токена (иначе навигация браузера невозможна), а все API-маршруты требуют
/// заголовок авторизации с Bearer-схемой.
pub fn router(state: AppState) -> Router {
    let api = routes::api().route_layer(middleware::from_fn_with_state(
        state.clone(),
        auth::require_bearer,
    ));
    Router::new()
        .merge(routes::static_files::routes(&state.static_dir))
        .nest("/api/v1", api)
        .with_state(state)
}

/// Запустить HTTP-сервер на адресе (ТЗ `apps/pwa`: `127.0.0.1:8765`).
pub async fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        addr = %addr,
        static_dir = %state.static_dir.display(),
        "PWA-бэкенд запущен"
    );
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
