//! `POST /api/v1/query` — запрос к модели через ядро (Контракт 12).
//!
//! Провайдер выбирает [`crate::query::QueryService`]: облако OpenRouter по
//! ключу из секретов или локальная GGUF. Тело: `{ "text": "...", "use_cloud"?: bool,
//! "model"?: "..." }`; ответ — `QueryResponse` (text/model_used/tokens_used/...).

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use tensa_router::models::{QueryRequest, QueryResponse};

use crate::error::{Result, ServerError};
use crate::files::{build_context, format_hits, looks_like_file_search};
use crate::state::AppState;

/// Тело запроса `/query`.
#[derive(Debug, Deserialize)]
pub struct QueryBody {
    /// Текст запроса (обязателен).
    pub text: String,
    /// Принудительно облако (иначе — режим из `[brain] mode`).
    #[serde(default)]
    pub use_cloud: Option<bool>,
    /// Конкретная облачная модель (идентификатор OpenRouter).
    #[serde(default)]
    pub model: Option<String>,
}

/// `POST /query` — ответ модели через шлюз ядра.
pub async fn query(
    State(state): State<AppState>,
    Json(body): Json<QueryBody>,
) -> Result<Json<QueryResponse>> {
    let text = body.text.trim().to_string();
    if text.is_empty() {
        return Err(ServerError::BadRequest(
            "поле text обязательно и не должно быть пустым".to_string(),
        ));
    }

    let mut request = QueryRequest::text(text.clone());
    if body.use_cloud.unwrap_or(false) {
        request.use_cloud = true;
    }

    // Локальные файлы этого ПК: на «найди …» отвечаем списком сразу,
    // иначе подмешиваем найденные фрагменты в контекст модели.
    let hits = state.files.search(&text, 6).await.unwrap_or_default();
    if !hits.is_empty() {
        if looks_like_file_search(&text) {
            let payload = serde_json::json!({
                "files": hits
                    .iter()
                    .map(|hit| serde_json::json!({
                        "path": hit.path,
                        "name": hit.name,
                        "snippet": hit.snippet,
                    }))
                    .collect::<Vec<_>>()
            });
            return Ok(Json(QueryResponse {
                text: format_hits(&hits),
                action: Some("files".to_string()),
                action_payload: Some(payload),
                tool_calls: Vec::new(),
                reasoning: None,
                degraded: false,
                model_used: "files".to_string(),
                node_used: "local".to_string(),
                tokens_used: Some(0),
            }));
        }
        let context = build_context(&hits);
        request.text = format!("{context}\n\nВопрос пользователя: {text}");
    }

    let response = state.query.query(&request, body.model.as_deref()).await?;
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::auth::PwaToken;
    use crate::query::QueryService;
    use crate::router;
    use tensa_approval::{ApprovalConfig, SqliteApprovalStore};
    use tensa_core::SqliteNotesStore;

    /// Состояние с реальными хранилищами и шлюзом без провайдеров.
    async fn test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("tensa-query-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("каталог");
        let approval = SqliteApprovalStore::new(
            root.join("approval.db"),
            ApprovalConfig::default(),
            "00000000-0000-4000-8000-000000000001".to_string(),
        )
        .await
        .expect("approval");
        let notes = SqliteNotesStore::new(&root.join("notes.db"), "node".to_string())
            .await
            .expect("notes");
        AppState::new(
            Arc::new(approval),
            Arc::new(notes),
            PwaToken::new("test-token-1234567890"),
        )
        .with_query(Arc::new(QueryService::disabled()))
    }

    #[tokio::test]
    async fn rejects_empty_text() {
        let state = test_state().await;
        let app = router(state);
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/query")
            .header("authorization", "Bearer test-token-1234567890")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"   "}"#))
            .expect("запрос");
        let response = app.oneshot(request).await.expect("ответ");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn unavailable_without_providers() {
        let state = test_state().await;
        let app = router(state);
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/query")
            .header("authorization", "Bearer test-token-1234567890")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"привет"}"#))
            .expect("запрос");
        let response = app.oneshot(request).await.expect("ответ");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn requires_bearer_token() {
        let state = test_state().await;
        let app = router(state);
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/query")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"привет"}"#))
            .expect("запрос");
        let response = app.oneshot(request).await.expect("ответ");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
