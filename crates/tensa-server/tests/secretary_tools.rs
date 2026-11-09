//! Интеграционные тесты инструментов секретаря (Фаза 2): контакты (CRUD +
//! поиск), приложения (валидация запуска/URL) и ошибочные пути почтового
//! контура. IMAP-протокол тестируется mock-сервером в
//! `tensa-tools/src/email/imap.rs` (без сети и TLS).

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tensa_approval::SqliteApprovalStore;
use tensa_core::{SqliteContactsStore, SqliteNotesStore};
use tensa_server::{router, AppState, PwaToken};
use tower::ServiceExt;

/// Фиксированный токен тестов (существует только в памяти теста).
const TOKEN: &str = "test-token-0123456789abcdef";

/// Приложение с in-memory хранилищами и временным корнем данных.
async fn test_app() -> Router {
    let approvals = SqliteApprovalStore::new_in_memory()
        .await
        .expect("хранилище одобрений");
    let notes = SqliteNotesStore::new_in_memory()
        .await
        .expect("хранилище заметок");
    let contacts = SqliteContactsStore::new_in_memory()
        .await
        .expect("хранилище контактов");
    let data_root = std::env::temp_dir().join(format!("tensa-phase2-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data_root).expect("корень данных");
    let state = AppState::new(
        Arc::new(approvals),
        Arc::new(notes),
        Arc::new(contacts),
        PwaToken::new(TOKEN),
    )
    .with_data_root(data_root)
    .with_static_dir(std::env::temp_dir().join("tensa-server-no-static"));
    router(state)
}

/// Запрос с Bearer-доступом и JSON-телом.
fn request(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
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
async fn contacts_crud_and_search_roundtrip() {
    let app = test_app().await;

    // Создание (201 + контакт).
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/contacts",
            Some(json!({
                "name": "Иван Петров",
                "email": "ivan@example.com",
                "phone": "+7 900 000-00-00",
                "telegram": "@ivan",
                "tags": ["работа"]
            })),
        ))
        .await
        .expect("ответ создания");
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await;
    let id = created["id"].as_str().expect("id контакта").to_string();

    // Список содержит контакт.
    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/contacts", None))
        .await
        .expect("ответ списка");
    assert_eq!(response.status(), StatusCode::OK);
    let list = json_body(response).await;
    assert_eq!(list.as_array().expect("массив контактов").len(), 1);

    // Поиск (find_contact).
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/contacts/search?q=%D0%98%D0%B2%D0%B0%D0%BD",
            None,
        ))
        .await
        .expect("ответ поиска");
    assert_eq!(response.status(), StatusCode::OK);
    let found = json_body(response).await;
    assert_eq!(found.as_array().expect("результаты").len(), 1);

    // Обновление.
    let response = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/v1/contacts/{id}"),
            Some(json!({
                "name": "Иван И.",
                "email": "ivan@example.com",
                "tags": ["работа"]
            })),
        ))
        .await
        .expect("ответ обновления");
    assert_eq!(response.status(), StatusCode::OK);
    let updated = json_body(response).await;
    assert_eq!(updated["name"], json!("Иван И."));

    // Чтение одного.
    let response = app
        .clone()
        .oneshot(request("GET", &format!("/api/v1/contacts/{id}"), None))
        .await
        .expect("ответ чтения");
    assert_eq!(response.status(), StatusCode::OK);

    // Удаление и 404 после.
    let response = app
        .clone()
        .oneshot(request("DELETE", &format!("/api/v1/contacts/{id}"), None))
        .await
        .expect("ответ удаления");
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(request("GET", &format!("/api/v1/contacts/{id}"), None))
        .await
        .expect("ответ после удаления");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn contacts_invalid_email_is_400() {
    let app = test_app().await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/contacts",
            Some(json!({ "name": "Без почты", "email": "no-at-sign", "tags": [] })),
        ))
        .await
        .expect("ответ с некорректным email");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn contacts_empty_name_is_400() {
    let app = test_app().await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/contacts",
            Some(json!({ "name": "   ", "tags": [] })),
        ))
        .await
        .expect("ответ без имени");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn contacts_unknown_id_is_404() {
    let app = test_app().await;
    let response = app
        .oneshot(request(
            "GET",
            &format!("/api/v1/contacts/{}", uuid::Uuid::new_v4()),
            None,
        ))
        .await
        .expect("ответ по несуществующему контакту");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn apps_launch_unknown_name_is_404() {
    let app = test_app().await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/apps/launch",
            Some(json!({ "name": "Нет такого приложения 42" })),
        ))
        .await
        .expect("ответ запуска");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn apps_open_url_invalid_scheme_is_400() {
    let app = test_app().await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/apps/open-url",
            Some(json!({ "url": "file:///C:/Windows/System32/calc.exe" })),
        ))
        .await
        .expect("ответ открытия");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn email_send_invalid_recipient_is_400() {
    let app = test_app().await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/email/send",
            Some(json!({ "to": "no-at-sign", "subject": "Тест", "body": "Текст" })),
        ))
        .await
        .expect("ответ отправки");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn email_unknown_account_is_404() {
    let app = test_app().await;
    // Креденшелы аккаунта ищутся до сети — несуществующий аккаунт даёт 404.
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/email/accounts/unknown/connect",
            None,
        ))
        .await
        .expect("ответ подключения");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
