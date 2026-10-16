//! Интеграционные тесты HTTP API PWA (Контракт 12).
//!
//! Роутер собирается поверх in-memory хранилищ; проверяются авторизация,
//! health/version, CRUD заметок, список одобрений, SSE-поток и заглушки 501.
//! Токен в тестах фиксированный — в файловую систему ничего не пишется.

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tensa_approval::SqliteApprovalStore;
use tensa_core::SqliteNotesStore;
use tensa_server::{router, AppState, PwaToken};
use tokio_stream::StreamExt;
use tower::ServiceExt;

/// Фиксированный токен тестов (существует только в памяти теста).
const TOKEN: &str = "test-token-0123456789abcdef";

/// Приложение с in-memory хранилищами и заведомо отсутствующим каталогом статики.
async fn test_app() -> Router {
    let approvals = SqliteApprovalStore::new_in_memory()
        .await
        .expect("in-memory хранилище одобрений");
    let notes = SqliteNotesStore::new_in_memory()
        .await
        .expect("in-memory хранилище заметок");
    let state = AppState::new(Arc::new(approvals), Arc::new(notes), PwaToken::new(TOKEN))
        .with_static_dir(std::env::temp_dir().join("tensa-server-no-static"));
    router(state)
}

/// Запрос с необязательными токеном и JSON-телом.
fn request(method: &str, uri: &str, with_token: bool, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if with_token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    }
    match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(value.to_string()))
            .expect("запрос с телом"),
        None => builder.body(Body::empty()).expect("запрос без тела"),
    }
}

/// JSON-тело ответа (до 1 МиБ).
async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("тело ответа");
    serde_json::from_slice(&bytes).expect("JSON в теле")
}

#[tokio::test]
async fn health_requires_bearer_token() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/health", false, None))
        .await
        .expect("ответ без токена");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let wrong = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/health")
                .header(header::AUTHORIZATION, "Bearer wrong-token")
                .body(Body::empty())
                .expect("запрос с чужим токеном"),
        )
        .await
        .expect("ответ с чужим токеном");
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_and_version_with_token() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/health", true, None))
        .await
        .expect("ответ health");
    assert_eq!(response.status(), StatusCode::OK);
    let health = json_body(response).await;
    assert_eq!(health["healthy"], json!(true));
    assert_eq!(health["version"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(
        health["components"]["approval_store"]["healthy"],
        json!(true)
    );
    assert_eq!(health["components"]["notes_store"]["healthy"], json!(true));

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/version", true, None))
        .await
        .expect("ответ version");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await, json!(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn notes_crud_roundtrip() {
    let app = test_app().await;

    // Создание.
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/notes",
            true,
            Some(json!({"title": "Test note", "body": "Body", "tags": ["test"]})),
        ))
        .await
        .expect("ответ создания");
    assert_eq!(response.status(), StatusCode::CREATED);
    let note = json_body(response).await;
    let id = note["id"].as_str().expect("id заметки").to_string();
    assert_eq!(note["title"], json!("Test note"));

    // Список и чтение одной заметки.
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/notes?limit=10&offset=0",
            true,
            None,
        ))
        .await
        .expect("ответ списка");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await.as_array().expect("массив").len(),
        1
    );

    let response = app
        .clone()
        .oneshot(request("GET", &format!("/api/v1/notes/{id}"), true, None))
        .await
        .expect("ответ чтения");
    assert_eq!(response.status(), StatusCode::OK);

    // Обновление.
    let response = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/v1/notes/{id}"),
            true,
            Some(json!({"title": "Updated note", "body": "Body", "tags": []})),
        ))
        .await
        .expect("ответ обновления");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["title"], json!("Updated note"));

    // Поиск (FTS5).
    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/notes/search?q=Updated", true, None))
        .await
        .expect("ответ поиска");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await.as_array().expect("массив").len(),
        1
    );

    // Мягкое удаление: 204, затем чтение — 404.
    let response = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/notes/{id}"),
            true,
            None,
        ))
        .await
        .expect("ответ удаления");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = app
        .clone()
        .oneshot(request("GET", &format!("/api/v1/notes/{id}"), true, None))
        .await
        .expect("ответ после удаления");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn approvals_list_is_empty_and_stubs_return_501() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/approvals", true, None))
        .await
        .expect("ответ approvals");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await, json!([]));

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/apps", true, None))
        .await
        .expect("ответ заглушки");
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn events_stream_starts_with_welcome() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/events", true, None))
        .await
        .expect("ответ events");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.starts_with("text/event-stream"),
        "неожиданный Content-Type: {content_type}"
    );

    let mut stream = response.into_body().into_data_stream();
    let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("SSE-кадр не пришёл за 5 секунд")
        .expect("поток не завершился неожиданно")
        .expect("кадр без ошибок");
    let text = String::from_utf8_lossy(&frame);
    assert!(text.contains("welcome"), "первый кадр SSE: {text}");
}

#[tokio::test]
async fn static_root_is_public_and_serves_placeholder() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(request("GET", "/", false, None))
        .await
        .expect("ответ статики");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.starts_with("text/html"));
}
