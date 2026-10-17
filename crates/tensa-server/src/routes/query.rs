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

/// Явная команда «отправь файл на почту»: (адрес, запрос файла).
fn parse_email_intent(text: &str) -> Option<(Option<String>, String)> {
    let lower = text.to_lowercase();
    let send = lower.contains("отправ") || lower.contains("send") || lower.contains("пошл");
    let mail = lower.contains("почт")
        || lower.contains("email")
        || lower.contains("e-mail")
        || lower.contains("mail");
    if !(send && mail) {
        return None;
    }
    let to = text.split_whitespace().find_map(|word| {
        let trimmed = word.trim_matches(|c: char| {
            !(c.is_alphanumeric() || c == '@' || c == '.' || c == '_' || c == '-')
        });
        if trimmed.contains('@') && trimmed.contains('.') {
            Some(trimmed.to_string())
        } else {
            None
        }
    });
    let stop = [
        "отправь",
        "отправить",
        "отправ",
        "send",
        "пошли",
        "пожалуйста",
        "мне",
        "на",
        "почту",
        "почта",
        "почтой",
        "email",
        "e-mail",
        "mail",
        "себе",
        "файл",
        "файлы",
        "документ",
        "документы",
    ];
    let mut parts: Vec<&str> = Vec::new();
    for word in text.split_whitespace() {
        if word.contains('@') || stop.contains(&word.to_lowercase().as_str()) {
            continue;
        }
        parts.push(word);
    }
    Some((to, parts.join(" ")))
}

/// Отправить найденный файл на почту (явная команда владельца).
async fn send_file_by_email(
    state: &AppState,
    original: &str,
    recipient: Option<String>,
    file_query: String,
) -> Result<Json<QueryResponse>> {
    use std::path::{Path, PathBuf};
    use tensa_tools::email::{EmailService, OutgoingEmail};

    let service = EmailService::from_files(&state.config_path)
        .map_err(|error| ServerError::Unavailable(format!("почта: {error}")))?;
    let query = if file_query.trim().is_empty() {
        original.to_string()
    } else {
        file_query
    };
    let hits = state
        .files
        .search(&query, 6)
        .await
        .map_err(|error| ServerError::Storage(format!("поиск файла: {error}")))?;
    let hit = hits.into_iter().find(|hit| is_document(&hit.path));
    let Some(hit) = hit else {
        return Ok(Json(QueryResponse {
            text: "Не нашёл файл для отправки — назови его точнее (например, «… 2 ВАРИАНТА.docx»)."
                .to_string(),
            action: None,
            action_payload: None,
            tool_calls: Vec::new(),
            reasoning: None,
            degraded: false,
            model_used: "email".to_string(),
            node_used: "local".to_string(),
            tokens_used: Some(0),
        }));
    };
    let name = Path::new(&hit.path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let to = recipient
        .or_else(|| {
            service
                .accounts()
                .first()
                .map(|account| account.email.clone())
        })
        .ok_or_else(|| {
            ServerError::Unavailable(
                "нет почтового аккаунта: добавьте [email.accounts.<id>] в config.toml".to_string(),
            )
        })?;
    let email = OutgoingEmail {
        account_id: None,
        to: to.clone(),
        cc: None,
        bcc: None,
        subject: format!("Tensa: {name}"),
        body: format!("Файл: {}\n\nОтправлено секретарём Tensa.", hit.path),
        attachment: Some(PathBuf::from(&hit.path)),
    };
    service
        .send_now(email)
        .await
        .map_err(|error| ServerError::Unavailable(format!("отправка: {error}")))?;
    Ok(Json(QueryResponse {
        text: format!("Отправил «{name}» на {to}."),
        action: Some("email".to_string()),
        action_payload: Some(serde_json::json!({ "file": hit.path, "to": to })),
        tool_calls: Vec::new(),
        reasoning: None,
        degraded: false,
        model_used: "email".to_string(),
        node_used: "local".to_string(),
        tokens_used: Some(0),
    }))
}

/// Документ, который можно отправить вложением.
fn is_document(path: &str) -> bool {
    let lower = path.to_lowercase();
    [".docx", ".pdf", ".xlsx", ".pptx", ".txt", ".md"]
        .iter()
        .any(|ext| lower.ends_with(ext))
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

    // Явная команда «отправь файл на почту» — сразу отправляем вложением.
    if let Some((recipient, file_query)) = parse_email_intent(&text) {
        return send_file_by_email(&state, &text, recipient, file_query).await;
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

    #[test]
    fn email_intent_parses() {
        let (to, query) =
            parse_email_intent("Отправь КОММЕРЧЕСКОЕ ПРЕДЛОЖЕНИЕ 2 ВАРИАНТА.docx мне на почту")
                .expect("команда почты");
        assert!(to.is_none());
        assert!(query.to_lowercase().contains("коммерческое"));
        assert!(query.to_lowercase().contains(".docx"));
        let (to, _) = parse_email_intent("отправь файл на email boss@example.com").expect("адрес");
        assert_eq!(to.as_deref(), Some("boss@example.com"));
        assert!(parse_email_intent("найди файлы с договором").is_none());
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
