//! Модели крейта (ТЗ tensa-router: типы дословно).
//! Общие типы переиспользуются (не дублировать).

pub use tensa_core::Language;
pub use tensa_llm::ModelTier;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::error::Result;

/// Режим мозга (ТЗ: BrainMode).
/// [derived] Значение по умолчанию в ТЗ не задано; принцип проекта —
/// «локальная по умолчанию» (ТЗ tensa-router, принципы).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrainMode {
    #[default]
    Local,
    Cloud,
    Auto,
}

/// Приоритет запроса (ТЗ: QueryPriority).
/// [derived] Значение по умолчанию в ТЗ не задано — Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryPriority {
    Low,
    #[default]
    Normal,
    High,
}

/// Запрос к ядру (ТЗ: QueryRequest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRequest {
    pub text: String,
    pub language: Language,
    #[serde(default)]
    pub context: HashMap<String, String>,
    pub use_cloud: bool,
    pub priority: QueryPriority,
}

impl QueryRequest {
    /// Простой текстовый запрос (русский, автоприоритет).
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: Language::Ru,
            context: HashMap::new(),
            use_cloud: false,
            priority: QueryPriority::Normal,
        }
    }
}

/// Вызов инструмента в ответе (ТЗ tensa-router: ToolCall).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub params: serde_json::Value,
}

/// Ответ ядра (ТЗ: QueryResponse, поля дословно).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResponse {
    pub text: String,
    pub action: Option<String>,
    pub action_payload: Option<serde_json::Value>,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Option<String>,
    pub degraded: bool,
    pub model_used: String,
    pub node_used: String,
    pub tokens_used: Option<u64>,
}

/// Кусочек потокового ответа.
/// [derived] Тип из Контракта 8 (query.proto: text, done); в списке типов ТЗ
/// tensa-router отсутствует, но используется в `query_stream`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryChunk {
    pub text: String,
    pub done: bool,
}

/// Поток ответа.
/// [derived] В ТЗ — `impl Stream`; для dyn-совместимости (`Arc<dyn BrainRouter>`)
/// используется boxed-поток с тем же Item (как `ChatStream` в tensa-llm).
pub type QueryStream = Pin<Box<dyn futures::Stream<Item = Result<QueryChunk>> + Send>>;

/// Здоровье роутера (ТЗ: RouterHealth).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouterHealth {
    pub local_available: bool,
    pub cloud_available: bool,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
    pub current_node: String,
}

/// Статус бюджета облака (ТЗ: BudgetStatus).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub daily_tokens_used: u64,
    pub daily_tokens_limit: u64,
    pub monthly_usd_used: f64,
    pub monthly_usd_limit: f64,
    pub warn: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_helper_defaults() {
        let request = QueryRequest::text("привет");
        assert_eq!(request.language, Language::Ru);
        assert_eq!(request.priority, QueryPriority::Normal);
        assert!(!request.use_cloud);
    }

    #[test]
    fn response_serde_roundtrip() {
        let response = QueryResponse {
            text: "ответ".to_string(),
            action: Some("email_send".to_string()),
            action_payload: Some(serde_json::json!({"to": "a@b"})),
            tool_calls: vec![ToolCall {
                tool: "email_send".to_string(),
                params: serde_json::json!({}),
            }],
            reasoning: None,
            degraded: false,
            model_used: "gemma-4-E2B-it".to_string(),
            node_used: "local".to_string(),
            tokens_used: Some(42),
        };
        let json = serde_json::to_string(&response).expect("сериализация");
        let back: QueryResponse = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(response, back);
    }

    #[test]
    fn brain_mode_default_is_local() {
        assert_eq!(BrainMode::default(), BrainMode::Local);
    }
}
