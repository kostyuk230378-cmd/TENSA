//! Заглушки для остальных маршрутов Контракта 12 (MVP): 501 Not Implemented.
//!
//! Пути объявлены явно по `teh_doc/contracts/pwa/openapi.yaml`, чтобы клиент,
//! сгенерированный из контракта, получал осмысленное «ещё не реализовано»,
//! а не 404 «такого маршрута нет».

use axum::http::{Method, Uri};
use axum::routing::{get, post};
use axum::Router;

use crate::error::ServerError;
use crate::state::AppState;

/// Роутер заглушек: пути и методы взяты из контракта PWA.
pub fn routes() -> Router<AppState> {
    Router::new()
        // Поиск по документам (Контракты 5/6) — вне объёма MVP бэкенда PWA.
        .route("/search", get(not_implemented))
        .route("/search/hybrid", get(not_implemented))
        // Документы.
        .route("/documents", get(not_implemented))
        .route("/documents/:id", get(not_implemented))
        // Контакты.
        .route("/contacts", get(not_implemented).post(not_implemented))
        .route("/contacts/search", get(not_implemented))
        .route(
            "/contacts/:id",
            get(not_implemented)
                .put(not_implemented)
                .delete(not_implemented),
        )
        // Почта: подключение/папки/письма/отправка (Контракт 6).
        .route("/email/accounts/:id/connect", post(not_implemented))
        .route("/email/accounts/:id/disconnect", post(not_implemented))
        .route("/email/accounts/:id/folders", get(not_implemented))
        .route("/email/accounts/:id/emails", get(not_implemented))
        .route("/email/accounts/:id/search", get(not_implemented))
        .route("/email/send", post(not_implemented))
        // Календарь реализован в `routes::calendar`.
        // Приложения операционной системы.
        .route("/apps", get(not_implemented))
        .route("/apps/scan", post(not_implemented))
        .route("/apps/launch", post(not_implemented))
        .route("/apps/open-file", post(not_implemented))
        .route("/apps/open-url", post(not_implemented))
        // Узлы сети (Контракт 9).
        .route("/nodes", get(not_implemented))
        .route("/nodes/heavy", get(not_implemented))
        .route("/nodes/:id", get(not_implemented))
        // Конфигурация (чтение/изменение — Контракт 2).
        .route("/config", get(not_implemented).patch(not_implemented))
        .route("/config/reload", post(not_implemented))
        // Потоковый ответ модели — вне объёма MVP (/query реализован отдельно).
        .route("/query/stream", post(not_implemented))
        // Голос.
        // Первый запуск: шаги мастера (Контракт 11).
        .route("/first-run", get(not_implemented))
        .route("/first-run/step/language", post(not_implemented))
        .route("/first-run/step/introduction", post(not_implemented))
        .route("/first-run/step/models", post(not_implemented))
        .route("/first-run/step/telegram", post(not_implemented))
        .route("/first-run/step/email", post(not_implemented))
        .route("/first-run/step/whatsapp", post(not_implemented))
        .route("/first-run/step/tailscale", post(not_implemented))
        .route("/first-run/step/finish", post(not_implemented))
}

/// Общий обработчик заглушек: 501 с указанием метода и пути.
async fn not_implemented(method: Method, uri: Uri) -> ServerError {
    ServerError::NotImplemented(format!("{method} {}", uri.path()))
}
