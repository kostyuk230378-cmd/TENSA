//! Протокол tool-calls секретаря (`system_prompt_tensa.md`): модель отвечает
//! ТОЛЬКО JSON — одно действие, `steps`, `ask_user` или `error`.
//!
//! Локальная GGUF-модель не обучена нативному function calling — системный
//! промпт задаёт протокол, парсер разбирает ответ; невалидный ответ
//! (нет JSON, неизвестный инструмент) → fallback `ask_user`.
//! Облачный путь (OpenRouter) использует нативные tool calls и парсер не требует.

use crate::models::ToolCall;
use serde_json::Value;

/// Инструменты, доступные модели (system_prompt_tensa.md + память ADR-021):
/// `(имя, описание с параметрами)`.
pub const SECRETARY_TOOLS: &[(&str, &str)] = &[
    ("search_files", "Поиск файлов по запросу {query, limit}"),
    ("open_file", "Открыть файл {path}"),
    (
        "list_documents",
        "Список проиндексированных документов {limit, offset}",
    ),
    (
        "send_email",
        "Отправить письмо {account_id, to, subject, body}",
    ),
    (
        "fetch_emails",
        "Получить письма {account_id, folder, limit}",
    ),
    ("search_emails", "Поиск писем {account_id, query}"),
    ("send_telegram", "Сообщение в Telegram {message}"),
    (
        "create_calendar_event",
        "Создать событие {title, start, end}",
    ),
    ("check_calendar", "Конфликты календаря {start, end}"),
    ("list_calendar_events", "Список событий {start, end}"),
    ("create_note", "Создать заметку {title, body, tags}"),
    ("search_notes", "Поиск заметок {query, limit}"),
    (
        "create_contact",
        "Создать контакт {name, email?, phone?, telegram?, whatsapp?}",
    ),
    ("find_contact", "Найти контакт {query}"),
    ("launch_app", "Запустить приложение {name}"),
    ("open_url", "Открыть URL {url}"),
    (
        "core_memory_replace",
        "Заменить блок core memory {block, value}",
    ),
    (
        "core_memory_append",
        "Дописать в блок core memory {block, value}",
    ),
    (
        "recall_memory_search",
        "Поиск по истории разговоров {query, limit}",
    ),
    ("archival_memory_search", "Поиск по архиву {query, limit}"),
    (
        "archival_memory_insert",
        "Сохранить факт в архив {content, tags}",
    ),
];

/// Системный промпт: протокол ответа + список инструментов.
pub fn build_system_prompt(tools: &[(&str, &str)]) -> String {
    let mut prompt = String::from(
        "Ты — секретарь Tensa. Отвечай ТОЛЬКО JSON без текста вне JSON и без Markdown.\n\
         Форматы ответа:\n\
         {\"action\": \"имя_инструмента\", \"params\": {...}, \"reasoning\": \"кратко\"}\n\
         {\"steps\": [{\"action\": \"...\", \"params\": {...}}, ...], \"reasoning\": \"кратко\"}\n\
         {\"action\": \"ask_user\", \"params\": {\"question\": \"...\"}, \"reasoning\": \"...\"}\n\
         {\"action\": \"error\", \"params\": {\"message\": \"...\"}, \"reasoning\": \"...\"}\n\
         Даты — ISO 8601 (пример: 2026-11-09T10:00:00). Незнакомый получатель — ask_user.\n\
         Не выдумывай инструменты и параметры. Доступные инструменты:\n",
    );
    for (name, description) in tools {
        prompt.push_str(&format!("- {name}: {description}\n"));
    }
    prompt
}

/// Результат разбора ответа модели.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedReply {
    /// Инструменты к выполнению (одно действие или steps).
    pub tool_calls: Vec<ToolCall>,
    /// Служебное действие (`ask_user` / `error`) без tool calls.
    pub action: Option<String>,
    /// Полезная нагрузка служебного действия (question/message).
    pub action_payload: Option<String>,
    /// Краткое объяснение модели.
    pub reasoning: Option<String>,
}

/// Разобрать ответ модели по протоколу.
///
/// `known_tools` — имена, которые сервер готов исполнить; ответ с
/// неизвестным инструментом не проходит и даёт fallback `ask_user`
/// (протокол: «инструмент не подходит — ask_user»).
pub fn parse_tool_reply(raw: &str, known_tools: &[&str]) -> ParsedReply {
    let known: Vec<&str> = known_tools.to_vec();
    match extract_json(raw).and_then(|text| serde_json::from_str::<Value>(text).ok()) {
        Some(value) => parse_value(value, &known).unwrap_or_else(|| ask_user_fallback(raw)),
        None => ask_user_fallback(raw),
    }
}

/// Разбор валидного JSON; `None` — ответ не по протоколу (fallback).
fn parse_value(value: Value, known: &[&str]) -> Option<ParsedReply> {
    let object = value.as_object()?;
    let reasoning = obj_str(object, "reasoning");

    // Несколько действий.
    if let Some(steps) = object.get("steps").and_then(|value| value.as_array()) {
        let mut tool_calls = Vec::new();
        for step in steps {
            let action = step.get("action")?.as_str()?;
            if !known.contains(&action) {
                return None;
            }
            let params = step.get("params").cloned().unwrap_or(Value::Null);
            tool_calls.push(ToolCall {
                tool: action.to_string(),
                params,
            });
        }
        return Some(ParsedReply {
            tool_calls,
            action: None,
            action_payload: None,
            reasoning,
        });
    }

    // Одно действие.
    let action = object.get("action")?.as_str()?.to_string();
    let params = object
        .get("params")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    match action.as_str() {
        "ask_user" => Some(ParsedReply {
            tool_calls: Vec::new(),
            action: Some("ask_user".to_string()),
            action_payload: params
                .get("question")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            reasoning,
        }),
        "error" => Some(ParsedReply {
            tool_calls: Vec::new(),
            action: Some("error".to_string()),
            action_payload: params
                .get("message")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            reasoning,
        }),
        other => {
            if !known.contains(&other) {
                return None;
            }
            Some(ParsedReply {
                tool_calls: vec![ToolCall {
                    tool: other.to_string(),
                    params,
                }],
                action: None,
                action_payload: None,
                reasoning,
            })
        }
    }
}

/// Строка из JSON-объекта по ключу.
fn obj_str(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

/// Fallback по протоколу плана: модель не ответила JSON — уточняем у владельца.
fn ask_user_fallback(raw: &str) -> ParsedReply {
    let truncated: String = raw.trim().chars().take(500).collect();
    ParsedReply {
        tool_calls: Vec::new(),
        action: Some("ask_user".to_string()),
        action_payload: Some(truncated),
        reasoning: Some("модель не вернула ответ по протоколу — уточнение".to_string()),
    }
}

/// Первый сбалансированный JSON-объект в тексте (учёт строк и экранирования).
/// Возвращает срез от `{` до парной `}`.
fn extract_json(raw: &str) -> Option<&str> {
    let mut start: Option<usize> = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in raw.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return start.map(|from| &raw[from..=idx]);
                }
            }
            '"' if start.is_some() => in_string = true,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TOOLS: &[&str] = &["core_memory_append", "send_email", "search_files"];

    fn parse(raw: &str) -> ParsedReply {
        parse_tool_reply(raw, TOOLS)
    }

    #[test]
    fn single_action_parses() {
        let parsed = parse(
            r#"{"action": "core_memory_append", "params": {"block": "tasks", "value": "Купить хлеб"}, "reasoning": "новая задача"}"#,
        );
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].tool, "core_memory_append");
        assert_eq!(parsed.tool_calls[0].params["value"], json!("Купить хлеб"));
        assert_eq!(parsed.action, None);
        assert_eq!(parsed.reasoning.as_deref(), Some("новая задача"));
    }

    #[test]
    fn steps_parse_in_order() {
        let parsed = parse(
            r#"{"steps": [{"action": "search_files", "params": {"query": "смета"}}, {"action": "send_email", "params": {"to": "a@b.c"}}], "reasoning": "план"}"#,
        );
        assert_eq!(parsed.tool_calls.len(), 2);
        assert_eq!(parsed.tool_calls[0].tool, "search_files");
        assert_eq!(parsed.tool_calls[1].tool, "send_email");
        assert!(parsed.action.is_none());
    }

    #[test]
    fn ask_user_parses_with_question() {
        let parsed = parse(
            r#"{"action": "ask_user", "params": {"question": "Кому?"}, "reasoning": "мало данных"}"#,
        );
        assert!(parsed.tool_calls.is_empty());
        assert_eq!(parsed.action.as_deref(), Some("ask_user"));
        assert_eq!(parsed.action_payload.as_deref(), Some("Кому?"));
    }

    #[test]
    fn error_parses_with_message() {
        let parsed = parse(r#"{"action": "error", "params": {"message": "адрес некорректен"}}"#);
        assert_eq!(parsed.action.as_deref(), Some("error"));
        assert_eq!(parsed.action_payload.as_deref(), Some("адрес некорректен"));
    }

    #[test]
    fn json_inside_markdown_fence_parses() {
        let parsed = parse("Вот ответ:\n```json\n{\"action\": \"search_files\", \"params\": {\"query\": \"договор\"}}\n```\n");
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].tool, "search_files");
    }

    #[test]
    fn json_after_leading_text_parses() {
        let parsed = parse(
            "Размышляю...\n{\"action\": \"search_files\", \"params\": {\"query\": \"{цитаты}\"}}",
        );
        assert_eq!(parsed.tool_calls.len(), 1);
    }

    #[test]
    fn invalid_json_falls_back_to_ask_user() {
        let parsed = parse("Я не понял запрос, уточните, пожалуйста.");
        assert!(parsed.tool_calls.is_empty());
        assert_eq!(parsed.action.as_deref(), Some("ask_user"));
        assert!(parsed
            .action_payload
            .as_deref()
            .unwrap()
            .contains("уточните"));
    }

    #[test]
    fn unknown_tool_falls_back_to_ask_user() {
        let parsed = parse(r#"{"action": "format_disk", "params": {}}"#);
        assert!(parsed.tool_calls.is_empty());
        assert_eq!(parsed.action.as_deref(), Some("ask_user"));
    }

    #[test]
    fn empty_reply_falls_back_to_ask_user() {
        let parsed = parse("   ");
        assert_eq!(parsed.action.as_deref(), Some("ask_user"));
    }

    #[test]
    fn system_prompt_lists_tools_and_protocol() {
        let prompt = build_system_prompt(SECRETARY_TOOLS);
        assert!(prompt.contains("ТОЛЬКО JSON"));
        assert!(prompt.contains("core_memory_append"));
        assert!(prompt.contains("steps"));
        assert!(prompt.contains("ask_user"));
    }
}
