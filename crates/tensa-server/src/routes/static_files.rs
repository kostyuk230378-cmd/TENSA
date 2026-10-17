//! Статика PWA: `GET /` и `GET /app` (ТЗ `apps/pwa`: `start_url: /app`),
//! ассеты — `/static/...`.
//!
//! Статика отдаётся без Bearer-токена: браузер не добавляет заголовок
//! `Authorization` при обычной навигации, поэтому оболочка PWA публична,
//! а токен вводится уже внутри интерфейса и используется только для API.

use std::path::Path;

use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tower_http::services::ServeDir;

use crate::state::AppState;

/// HTML-заглушка, если собранной PWA нет (в репозитории нет каталога `pwa/static`).
const PLACEHOLDER_HTML: &str = "\
<!doctype html>
<html lang=\"ru\">
<head><meta charset=\"utf-8\"><title>Tensa PWA</title></head>
<body>
<h1>Tensa PWA: статика не найдена</h1>
<p>Каталог <code>pwa/static</code> отсутствует. Соберите фронтенд PWA
(ТЗ apps/pwa) или укажите свой каталог через
<code>AppState::with_static_dir</code>.</p>
<p>HTTP API доступно по адресу <code>/api/v1/health</code> (нужен Bearer-токен
из файла <code>pwa_token</code>).</p>
</body>
</html>
";

/// Роутер статики (публичный, без авторизации).
///
/// Каталог фиксируется при сборке роутера; существование `index.html`
/// проверяется лениво — при запросе, поэтому каталог можно создать позже.
pub fn routes(static_dir: &Path) -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/app", get(index))
        // Плавающий виджет: отдельный путь вне scope service worker (/static),
        // иначе WebView2 отдаёт закешированную старую версию.
        .route("/widget", get(widget))
        .nest_service("/static", ServeDir::new(static_dir))
}

/// `GET /widget`: плавающий виджет (тот же widget.html, без SW-кеша).
async fn widget(State(state): State<AppState>) -> Response {
    let path = state.static_dir.join("widget.html");
    match tokio::fs::read(&path).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "widget.html не найден");
            (
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "widget.html не найден — пересоберите PWA",
            )
                .into_response()
        }
    }
}

/// `GET /` и `GET /app`: отдать `index.html` собранной PWA.
async fn index(State(state): State<AppState>) -> Response {
    let path = state.static_dir.join("index.html");
    match tokio::fs::read(&path).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(err) => {
            tracing::debug!(
                path = %path.display(),
                error = %err,
                "index.html не найден — отдаю заглушку"
            );
            (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                PLACEHOLDER_HTML,
            )
                .into_response()
        }
    }
}
