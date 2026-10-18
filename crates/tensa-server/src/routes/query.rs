//! `POST /api/v1/query` — запрос к модели через ядро (Контракт 12).
//!
//! Провайдер выбирает [`crate::query::QueryService`]: облако OpenRouter по
//! ключу из секретов или локальная GGUF. Тело: `{ "text": "...", "use_cloud"?: bool,
//! "model"?: "..." }`; ответ — `QueryResponse` (text/model_used/tokens_used/...).

use axum::extract::State;
use axum::Json;
use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, NaiveTime, TimeZone};
use serde::Deserialize;
use tensa_router::models::{QueryRequest, QueryResponse};

use crate::calendar::{CalendarError, Event, NewEvent};
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

/// Создать событие из фразы «создай встречу завтра в 15:00 …».
fn parse_calendar_intent(text: &str) -> Option<NewEvent> {
    let lower = text.to_lowercase();
    let create = [
        "создай",
        "создать",
        "добавь",
        "добавить",
        "назначь",
        "назначить",
        "запланируй",
        "запланировать",
        "поставь",
        "сделай",
    ]
    .iter()
    .any(|word| lower.contains(word));
    let calendar = lower.contains("календар")
        || lower.contains("встреч")
        || lower.contains("событи")
        || lower.contains("напомин");
    if !(create && calendar) {
        return None;
    }

    let now = Local::now();
    let mut date = now.date_naive();
    if lower.contains("послезавтра") {
        date += ChronoDuration::days(2);
    } else if lower.contains("завтра") {
        date += ChronoDuration::days(1);
    }
    if let Some(parsed) = parse_date(&lower) {
        date = parsed;
    }
    let time = parse_time(&lower).unwrap_or(NaiveTime::from_hms_opt(10, 0, 0)?);
    let start_naive = date.and_time(time);
    let start = Local
        .from_local_datetime(&start_naive)
        .single()?
        .timestamp();
    Some(NewEvent {
        title: calendar_title(text),
        starts_at: start,
        ends_at: start + 3600,
        location: None,
        notes: Some(text.to_string()),
    })
}

/// Дата из «15.10.2026», «15.10» или «15 октября».
fn parse_date(text: &str) -> Option<NaiveDate> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| !(c.is_alphanumeric() || c == '.'))
                .to_lowercase()
        })
        .collect();
    let year_now = Local::now().year();
    for (index, word) in words.iter().enumerate() {
        let parts: Vec<&str> = word.split('.').filter(|part| !part.is_empty()).collect();
        if parts.len() >= 2 {
            let day = parts[0].parse::<u32>().ok();
            let month = parts[1].parse::<u32>().ok();
            let year = parts
                .get(2)
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(year_now);
            if let (Some(day), Some(month)) = (day, month) {
                if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                    return Some(date);
                }
            }
        }
        if let Ok(day) = word.parse::<u32>() {
            if let Some(month) = words.get(index + 1).and_then(|value| month_number(value)) {
                let year = words
                    .get(index + 2)
                    .and_then(|value| value.parse::<i32>().ok())
                    .unwrap_or(year_now);
                if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                    return Some(date);
                }
            }
        }
    }
    None
}

/// Номер месяца по русскому названию.
fn month_number(word: &str) -> Option<u32> {
    let word = word
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    let month = match word.as_str() {
        "января" | "январь" => 1,
        "февраля" | "февраль" => 2,
        "марта" | "март" => 3,
        "апреля" | "апрель" => 4,
        "мая" | "май" => 5,
        "июня" | "июнь" => 6,
        "июля" | "июль" => 7,
        "августа" | "август" => 8,
        "сентября" | "сентябрь" => 9,
        "октября" | "октябрь" => 10,
        "ноября" | "ноябрь" => 11,
        "декабря" | "декабрь" => 12,
        _ => return None,
    };
    Some(month)
}

/// Время из «15:00» или «в 15 часов».
fn parse_time(text: &str) -> Option<NaiveTime> {
    let words: Vec<&str> = text.split_whitespace().collect();
    for (index, word) in words.iter().enumerate() {
        let cleaned = word.trim_matches(|c: char| !(c.is_alphanumeric() || c == ':'));
        if let Some((hours, minutes)) = cleaned.split_once(':') {
            if let (Ok(hours), Ok(minutes)) = (hours.parse::<u32>(), minutes.parse::<u32>()) {
                if let Some(time) = NaiveTime::from_hms_opt(hours, minutes, 0) {
                    return Some(time);
                }
            }
        }
        if words
            .get(index + 1)
            .map(|next| next.to_lowercase())
            .as_deref()
            == Some("час")
            || words
                .get(index + 1)
                .map(|next| next.to_lowercase())
                .as_deref()
                == Some("часов")
        {
            if let Ok(hours) = cleaned.parse::<u32>() {
                if let Some(time) = NaiveTime::from_hms_opt(hours, 0, 0) {
                    return Some(time);
                }
            }
        }
    }
    None
}

/// Заголовок события из текста команды.
fn calendar_title(text: &str) -> String {
    let mut title = text.to_string();
    for phrase in [
        "создай",
        "создать",
        "добавь",
        "добавить",
        "назначь",
        "назначить",
        "запланируй",
        "запланировать",
        "поставь",
        "сделай",
        "заметку",
        "заметка",
        "в календаре",
        "в календарь",
        "календаре",
        "календарь",
        "на завтра",
        "послезавтра",
        "завтра",
        "сегодня",
    ] {
        title = title.replace(phrase, " ");
    }
    let words: Vec<&str> = title
        .split_whitespace()
        .filter(|word| {
            let cleaned =
                word.trim_matches(|c: char| !(c.is_alphanumeric() || c == '.' || c == ':'));
            !cleaned.contains(':')
                && parse_date(cleaned).is_none()
                && !matches!(cleaned.to_lowercase().as_str(), "на" | "в")
        })
        .collect();
    let mut title = words
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_string();
    if title.is_empty() {
        return "Событие".to_string();
    }
    // Убираем ведущий командный глагол в любом регистре («Создай …»).
    let lower = title.to_lowercase();
    for command in [
        "создай",
        "создать",
        "добавь",
        "добавить",
        "назначь",
        "назначить",
        "запланируй",
        "запланировать",
        "поставь",
        "сделай",
    ] {
        if lower.starts_with(command) {
            title = title[command.len()..].trim_start().to_string();
            break;
        }
    }
    if title.to_lowercase().starts_with("встречу ") {
        title = format!("Встреча{}", &title["встречу".len()..]);
    }
    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Событие".to_string(),
    }
}

/// Записать ICS события в ~/.tensa/calendar и открыть календарь Windows.
async fn open_calendar_event(state: &AppState, event: &Event) -> Option<std::path::PathBuf> {
    let ics = state.calendar.ics_event(event.id).await.ok().flatten()?;
    let dir = state.data_root.join("calendar");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{}.ics", event.id));
    std::fs::write(&path, ics).ok()?;
    #[cfg(windows)]
    {
        let open = std::env::var("TENSA_CALENDAR_OPEN").as_deref() != Ok("0");
        if open {
            let _ = std::process::Command::new("explorer").arg(&path).spawn();
        }
    }
    Some(path)
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

    // «Создай встречу в календаре…» — сразу событие и ICS для календаря Windows.
    if let Some(new_event) = parse_calendar_intent(&text) {
        let event = match state.calendar.create(new_event).await {
            Ok(event) => event,
            Err(CalendarError::Conflict(existing)) => {
                let title = existing
                    .first()
                    .map(|event| event.title.clone())
                    .unwrap_or_else(|| "другое событие".to_string());
                return Ok(Json(QueryResponse {
                    text: format!("На это время уже есть событие «{title}». Второе не создавал."),
                    action: Some("calendar_conflict".to_string()),
                    action_payload: Some(serde_json::json!({
                        "conflicts": existing.iter().map(|event| event.id).collect::<Vec<_>>()
                    })),
                    tool_calls: Vec::new(),
                    reasoning: None,
                    degraded: false,
                    model_used: "calendar".to_string(),
                    node_used: "local".to_string(),
                    tokens_used: Some(0),
                }));
            }
            Err(error) => return Err(ServerError::BadRequest(error.to_string())),
        };
        let ics = open_calendar_event(&state, &event).await;
        let when = chrono::DateTime::from_timestamp(event.starts_at, 0)
            .map(|date| {
                date.with_timezone(&Local)
                    .format("%d.%m.%Y %H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        let opened = ics.is_some();
        return Ok(Json(QueryResponse {
            text: format!(
                "Создал событие «{}» на {when}. {}",
                event.title,
                if opened {
                    "Открыл календарь Windows; если он спросит — подтвердите добавление."
                } else {
                    "Событие сохранено в календаре Tensa."
                }
            ),
            action: Some("calendar".to_string()),
            action_payload: Some(serde_json::json!({
                "id": event.id,
                "title": event.title,
                "starts_at": event.starts_at,
                "ends_at": event.ends_at,
                "ics": ics.map(|path| path.display().to_string()),
            })),
            tool_calls: Vec::new(),
            reasoning: None,
            degraded: false,
            model_used: "calendar".to_string(),
            node_used: "local".to_string(),
            tokens_used: Some(0),
        }));
    }

    // Локальные файлы этого ПК: на «найди …» отвечаем списком сразу,
    // иначе подмешиваем найденные фрагменты в контекст модели.
    let hits = state.files.search(&text, 6).await.unwrap_or_default();
    let mut seen_paths = std::collections::HashSet::new();
    let hits: Vec<_> = hits
        .into_iter()
        .filter(|hit| seen_paths.insert(hit.path.to_lowercase()))
        .collect();
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
