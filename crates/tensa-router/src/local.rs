//! Локальный инференс: бэкенд над `tensa_llm::LlmEngine`.
//!
//! Точка внедрения [`LocalBackend`] позволяет тестировать маршрутизацию без
//! нативного слоя (он подключается на TASK-ENV-001) — [derived], в ТЗ
//! зафиксирован только общий принцип «локальная по умолчанию».

use std::sync::Arc;

use async_trait::async_trait;
use tensa_llm::{ChatMessage, ChatRequest, LlmEngine, Role};

use crate::error::Result;
use crate::models::{QueryRequest, ToolCall};

/// Ответ локальной модели.
#[derive(Debug, Clone)]
pub struct LocalReply {
    pub text: String,
    pub tokens_generated: u32,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Option<String>,
}

impl LocalReply {
    /// Простой текстовый ответ (для заглушек и тестов).
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tokens_generated: 0,
            tool_calls: Vec::new(),
            reasoning: None,
        }
    }
}

/// Локальный бэкенд инференса.
#[async_trait]
pub trait LocalBackend: Send + Sync {
    /// Загружена ли модель и готова ли она к генерации.
    async fn available(&self) -> bool;

    /// Сгенерировать ответ на запрос (чат-стиль).
    async fn generate(&self, request: &QueryRequest) -> Result<LocalReply>;
}

/// Продовая реализация поверх [`LlmEngine`] (llama.cpp).
pub struct EngineLocalBackend {
    engine: Arc<dyn LlmEngine>,
    model_name: String,
}

impl EngineLocalBackend {
    /// Создать бэкенд вокруг движка.
    pub fn new(engine: Arc<dyn LlmEngine>, model_name: impl Into<String>) -> Self {
        Self {
            engine,
            model_name: model_name.into(),
        }
    }

    /// Имя модели (для `QueryResponse.model_used`).
    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[async_trait]
impl LocalBackend for EngineLocalBackend {
    async fn available(&self) -> bool {
        matches!(self.engine.health().await, Ok(health) if health.loaded)
    }

    async fn generate(&self, request: &QueryRequest) -> Result<LocalReply> {
        let chat = ChatRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: request.text.clone(),
            }],
            tools: Vec::new(),
            max_tokens: None,
            temperature: None,
        };
        let response = self.engine.chat(chat).await?;
        Ok(LocalReply {
            text: response.message.content,
            tokens_generated: response.tokens_generated,
            tool_calls: response
                .tool_calls
                .into_iter()
                .map(|call| ToolCall {
                    tool: call.name,
                    params: call.params,
                })
                .collect(),
            reasoning: None,
        })
    }
}

/// Детерминированная заглушка локального бэкенда (тесты и CI).
pub struct StubLocalBackend {
    /// Считается ли модель загруженной.
    pub available: bool,
    /// Ответ на любой запрос.
    pub reply: LocalReply,
}

impl StubLocalBackend {
    /// Загруженная модель с фиксированным ответом.
    pub fn loaded(text: impl Into<String>) -> Self {
        Self {
            available: true,
            reply: LocalReply::text(text),
        }
    }

    /// Незагруженная модель.
    pub fn unavailable() -> Self {
        Self {
            available: false,
            reply: LocalReply::text(""),
        }
    }
}

#[async_trait]
impl LocalBackend for StubLocalBackend {
    async fn available(&self) -> bool {
        self.available
    }

    async fn generate(&self, _request: &QueryRequest) -> Result<LocalReply> {
        Ok(self.reply.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_reports_availability() {
        assert!(StubLocalBackend::loaded("ok").available().await);
        assert!(!StubLocalBackend::unavailable().available().await);
    }

    #[tokio::test]
    async fn stub_returns_fixed_reply() {
        let backend = StubLocalBackend::loaded("привет");
        let reply = backend
            .generate(&QueryRequest::text("тест"))
            .await
            .expect("ответ");
        assert_eq!(reply.text, "привет");
    }
}
