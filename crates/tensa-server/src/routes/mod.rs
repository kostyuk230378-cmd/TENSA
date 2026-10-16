//! Маршруты HTTP API (базовый путь `/api/v1` — из Контракта 12, `servers.url`).
//!
//! Весь API-роутер закрыт Bearer-авторизацией: middleware навешивается в
//! [`crate::router`] на результат [`api`]. Статика PWA (см. [`static_files`])
//! публична — браузер не прикладывает заголовок при навигации.

pub mod approvals;
pub mod events;
pub mod health;
pub mod notes;
pub mod static_files;
pub mod stubs;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// API-роутер MVP: реализованные маршруты + заглушки 501 остальных.
///
/// Статические сегменты (`/notes/search`) в axum имеют приоритет над
/// параметрическими (`/notes/:id`) независимо от порядка объявления.
pub fn api() -> Router<AppState> {
    Router::new()
        // Состояние и версия ядра.
        .route("/health", get(health::health))
        .route("/version", get(health::version))
        // Очередь одобрения (Контракт 1).
        .route("/approvals", get(approvals::list))
        .route("/approvals/:id", get(approvals::get))
        .route("/approvals/:id/approve", post(approvals::approve))
        .route("/approvals/:id/reject", post(approvals::reject))
        // Заметки (Контракт 3).
        .route("/notes", get(notes::list).post(notes::create))
        .route("/notes/search", get(notes::search))
        .route(
            "/notes/:id",
            get(notes::get).put(notes::update).delete(notes::delete),
        )
        // Поток событий (SSE).
        .route("/events", get(events::events))
        // Остальные маршруты Контракта 12 — 501 Not Implemented.
        .merge(stubs::routes())
}
