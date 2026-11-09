//! Интеграционные тесты мастера первого запуска (Контракт 12: `/first-run/*`).
//!
//! Роутер собирается поверх in-memory очереди/заметок; мастер получает
//! инициализатор core memory поверх файловых SQLite-хранилищ во временном
//! каталоге, поэтому после `finish` проверяются config.toml, secrets.toml
//! и блоки core memory в memory.db.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tensa_approval::SqliteApprovalStore;
use tensa_core::{Config, Language, Secrets, SqliteNotesStore};
use tensa_memory::{
    CoreMemoryConfig, CoreMemoryStore as _, DefaultMemoryManager, HashEmbedder,
    MemoryManagerConfig, SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};
use tensa_server::{router, AppState, PwaToken};
use tower::ServiceExt;

/// Фиксированный токен тестов (существует только в памяти теста).
const TOKEN: &str = "test-token-0123456789abcdef";

/// Временный корень данных (config.toml / secrets.toml / memory.db).
fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("tensa-first-run-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("временный корень");
    root
}

/// Приложение с файловыми хранилищами памяти и in-memory очередью/заметками.
/// Корень данных задаётся явно: тест ошибки сохранения указывает config_path
/// в «каталоге»-файле, который нельзя использовать как корень данных.
async fn test_app_at(config_path: PathBuf, data_root: PathBuf) -> Router {
    let approvals = SqliteApprovalStore::new_in_memory()
        .await
        .expect("хранилище одобрений");
    let notes = SqliteNotesStore::new_in_memory()
        .await
        .expect("хранилище заметок");
    std::fs::create_dir_all(data_root.join("memory")).expect("memory/");
    let core = SqliteCoreMemoryStore::new(
        &data_root.join("memory.db"),
        CoreMemoryConfig::default(),
        "node-test".into(),
    )
    .await
    .expect("core memory");
    let recall = SqliteRecallMemoryStore::new(&data_root.join("memory.db"))
        .await
        .expect("recall memory");
    let archival = SqliteArchivalMemoryStore::new(
        &data_root.join("memory").join("archival.db"),
        Arc::new(HashEmbedder::new(384)),
    )
    .await
    .expect("archival memory");
    let manager = DefaultMemoryManager::new(
        Arc::new(core),
        Arc::new(recall),
        Arc::new(archival),
        MemoryManagerConfig::default(),
        "node-test".into(),
    )
    .await
    .expect("менеджер памяти");
    let state = AppState::new(Arc::new(approvals), Arc::new(notes), PwaToken::new(TOKEN))
        .with_first_run_memory(Some(Arc::new(manager)))
        .with_data_root(data_root)
        .with_config_path(config_path)
        .with_static_dir(std::env::temp_dir().join("tensa-server-no-static"));
    router(state)
}

/// Обёртка: корень данных — каталог файла config.toml.
async fn test_app(config_path: PathBuf) -> Router {
    let data_root = config_path.parent().expect("корень").to_path_buf();
    test_app_at(config_path, data_root).await
}

/// Запрос с токеном и JSON-телом.
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

/// Шаг 1: язык (обязательный).
async fn pass_language(app: &Router) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/first-run/step/language",
            Some(json!({ "language": "ru" })),
        ))
        .await
        .expect("ответ шага языка");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// Шаг 2: знакомство (обязательный).
async fn pass_introduction(app: &Router) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/first-run/step/introduction",
            Some(json!({
                "owner_name": "Иван",
                "owner_title": "Шеф",
                "secretary_name": "Tensa",
                "timezone": "Europe/Moscow"
            })),
        ))
        .await
        .expect("ответ шага знакомства");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn first_run_status_reflects_config_presence() {
    let root = temp_root();
    let config_path = root.join("config.toml");
    let app = test_app(config_path.clone()).await;

    let response = app
        .clone()
        .oneshot(request("GET", "/api/v1/first-run", None))
        .await
        .expect("ответ статуса");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["is_first_run"], json!(true));

    // config.toml появился — первый запуск уже был.
    Config::default_with_node(&uuid::Uuid::new_v4().to_string())
        .save(&config_path)
        .expect("config");
    let response = app
        .oneshot(request("GET", "/api/v1/first-run", None))
        .await
        .expect("ответ статуса после сохранения конфига");
    assert_eq!(json_body(response).await["is_first_run"], json!(false));
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn first_run_full_cycle_writes_config_secrets_memory() {
    let root = temp_root();
    let config_path = root.join("config.toml");
    let app = test_app(config_path.clone()).await;

    pass_language(&app).await;
    pass_introduction(&app).await;
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/first-run/step/telegram",
            Some(json!({ "bot_token": "bot-secret", "owner_chat_id": "42" })),
        ))
        .await
        .expect("ответ шага telegram");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Опциональные шаги без тела (Контракт 12 не определяет requestBody).
    for uri in [
        "/api/v1/first-run/step/models",
        "/api/v1/first-run/step/email",
        "/api/v1/first-run/step/whatsapp",
        "/api/v1/first-run/step/tailscale",
    ] {
        let response = app
            .clone()
            .oneshot(request("POST", uri, None))
            .await
            .expect("ответ опционального шага");
        assert_eq!(response.status(), StatusCode::NO_CONTENT, "шаг {uri}");
    }

    let response = app
        .oneshot(request("POST", "/api/v1/first-run/step/finish", None))
        .await
        .expect("ответ finish");
    assert_eq!(response.status(), StatusCode::OK);
    let result = json_body(response).await;
    assert_eq!(result["config_saved"], json!(true));
    assert_eq!(result["secrets_saved"], json!(true));
    assert_eq!(result["core_memory_initialized"], json!(true));

    // config.toml: язык, telegram, owner_chat_id.
    let config = Config::load(&config_path).expect("config");
    assert_eq!(config.general.language, Language::Ru);
    assert_eq!(
        config.telegram.get("enabled"),
        Some(&toml::Value::Boolean(true))
    );
    assert_eq!(
        config
            .telegram
            .get("owner_chat_id")
            .and_then(|value| value.as_str()),
        Some("42")
    );
    // secrets.toml: токен бота в секретах, а не в config.
    let secrets = Secrets::load(&root.join("secrets.toml")).expect("secrets");
    assert_eq!(secrets.telegram.credential.as_deref(), Some("bot-secret"));
    // core memory: четыре блока в memory.db (инициализация из мастера).
    let store = SqliteCoreMemoryStore::new(
        &root.join("memory.db"),
        CoreMemoryConfig::default(),
        "check".into(),
    )
    .await
    .expect("хранилище памяти");
    assert_eq!(store.get_all_blocks().await.expect("блоки").len(), 4);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn first_run_finish_without_mandatory_steps_is_400() {
    let root = temp_root();
    let app = test_app(root.join("config.toml")).await;
    let response = app
        .oneshot(request("POST", "/api/v1/first-run/step/finish", None))
        .await
        .expect("ответ finish");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn first_run_unknown_language_is_400() {
    let root = temp_root();
    let app = test_app(root.join("config.toml")).await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/first-run/step/language",
            Some(json!({ "language": "de" })),
        ))
        .await
        .expect("ответ шага языка");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn first_run_telegram_without_token_is_400() {
    let root = temp_root();
    let app = test_app(root.join("config.toml")).await;
    pass_language(&app).await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/first-run/step/telegram",
            Some(json!({ "bot_token": "  ", "owner_chat_id": "42" })),
        ))
        .await
        .expect("ответ шага telegram");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn first_run_email_without_imap_host_is_400() {
    let root = temp_root();
    let app = test_app(root.join("config.toml")).await;
    pass_language(&app).await;
    pass_introduction(&app).await;
    let response = app
        .oneshot(request(
            "POST",
            "/api/v1/first-run/step/email",
            Some(json!({
                "account": {
                    "id": "personal",
                    "label": "Личный",
                    "email": "test@example.com",
                    "imap_host": " ",
                    "imap_port": 993,
                    "smtp_host": "smtp.example.com",
                    "smtp_port": 587
                }
            })),
        ))
        .await
        .expect("ответ шага email");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn first_run_finish_config_save_error_is_500() {
    // Родитель config.toml — обычный файл: create_dir_all в Config::save
    // падает по вводу-выводу (каталог поверх файла создать нельзя).
    let root = temp_root();
    let blocker = root.join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("файл-блокер");
    let app = test_app_at(blocker.join("config.toml"), root.clone()).await;
    pass_language(&app).await;
    pass_introduction(&app).await;
    let response = app
        .oneshot(request("POST", "/api/v1/first-run/step/finish", None))
        .await
        .expect("ответ finish");
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = std::fs::remove_dir_all(root);
}
