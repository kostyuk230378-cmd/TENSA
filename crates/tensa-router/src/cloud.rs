//! Облачный инференс: OpenRouter-совместимый HTTP-клиент (ТЗ tensa-router:
//! «Облачная: Gemma 4 26B A4B (free/paid)», бюджет OpenRouter).
//!
//! Секреты: ключ читается из окружения (`OPENROUTER_API_KEY`) и никогда не
//! логируется (ТЗ: «не логировать секреты»).

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{Result, RouterError};
use crate::models::{QueryRequest, ToolCall};

/// Ответ облачной модели.
#[derive(Debug, Clone, PartialEq)]
pub struct CloudReply {
    pub text: String,
    pub tokens_used: Option<u64>,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Option<String>,
}

impl CloudReply {
    /// Простой текстовый ответ (для заглушек и тестов).
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tokens_used: None,
            tool_calls: Vec::new(),
            reasoning: None,
        }
    }
}

/// Облачный бэкенд инференса.
#[async_trait]
pub trait CloudBackend: Send + Sync {
    /// Настроен ли бэкенд (есть API-ключ). Без сетевых вызовов.
    fn available(&self) -> bool;

    /// Чат-запрос к конкретной модели.
    async fn chat(&self, model: &str, request: &QueryRequest) -> Result<CloudReply>;
}

/// HTTP-клиент OpenRouter (base_url + ключ).
pub struct HttpCloudBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl HttpCloudBackend {
    /// Создать бэкенд. `api_key = None` — бэкенд считается ненастроенным.
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| RouterError::Cloud(format!("не создать HTTP-клиент: {error}")))?;
        Ok(Self {
            client,
            base_url: base_url.into(),
            api_key,
        })
    }

    /// Бэкенд из окружения (ключ `OPENROUTER_API_KEY`).
    pub fn from_env(timeout: Duration) -> Result<Self> {
        Self::new(
            "https://openrouter.ai/api/v1",
            std::env::var("OPENROUTER_API_KEY").ok(),
            timeout,
        )
    }
}

#[derive(Serialize)]
struct ChatBody<'a> {
    model: &'a str,
    messages: Vec<ChatBodyMessage<'a>>,
}

#[derive(Serialize)]
struct ChatBodyMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatCompletion {
    #[serde(default)]
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ChatToolCall>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Deserialize)]
struct ChatToolCall {
    function: ChatFunctionCall,
}

#[derive(Deserialize)]
struct ChatFunctionCall {
    name: String,
    #[serde(default)]
    arguments: String,
}

#[derive(Deserialize)]
struct ChatUsage {
    #[serde(default)]
    total_tokens: Option<u64>,
}

#[async_trait]
impl CloudBackend for HttpCloudBackend {
    fn available(&self) -> bool {
        self.api_key.is_some()
    }

    async fn chat(&self, model: &str, request: &QueryRequest) -> Result<CloudReply> {
        let Some(api_key) = self.api_key.as_deref() else {
            return Err(RouterError::ApiKey(
                "OPENROUTER_API_KEY не задан".to_string(),
            ));
        };

        let body = ChatBody {
            model,
            messages: vec![ChatBodyMessage {
                role: "user",
                content: &request.text,
            }],
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                // Тело запроса и ключ не логируются (ТЗ).
                RouterError::Cloud(format!("сеть: {error}"))
            })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(RouterError::ApiKey(format!("HTTP {status}")));
        }
        if !status.is_success() {
            return Err(RouterError::Cloud(format!("HTTP {status}")));
        }

        let completion: ChatCompletion = response
            .json()
            .await
            .map_err(|error| RouterError::Cloud(format!("разбор ответа: {error}")))?;

        let choice = completion
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| RouterError::Cloud("пустой choices в ответе".to_string()))?;

        Ok(CloudReply {
            text: choice.message.content.unwrap_or_default(),
            tokens_used: completion.usage.and_then(|usage| usage.total_tokens),
            tool_calls: choice
                .message
                .tool_calls
                .into_iter()
                .map(|call| ToolCall {
                    tool: call.function.name,
                    params: serde_json::from_str(&call.function.arguments)
                        .unwrap_or(serde_json::Value::String(call.function.arguments)),
                })
                .collect(),
            reasoning: choice.message.reasoning,
        })
    }
}

/// Скриптовая заглушка облачного бэкенда (тесты и CI).
pub struct StubCloudBackend {
    available: bool,
    script: Mutex<VecDeque<Result<CloudReply>>>,
    calls: Mutex<Vec<String>>,
}

impl StubCloudBackend {
    /// Создать заглушку (`available` — считается ли бэкенд настроенным).
    pub fn new(available: bool) -> Self {
        Self {
            available,
            script: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Добавить следующий ответ (в порядке вызовов).
    pub fn push(&self, reply: Result<CloudReply>) {
        self.lock_script().push_back(reply);
    }

    /// Модели, к которым обращались (в порядке вызовов).
    pub fn calls(&self) -> Vec<String> {
        self.lock_calls().clone()
    }

    fn lock_script(&self) -> std::sync::MutexGuard<'_, VecDeque<Result<CloudReply>>> {
        match self.script.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn lock_calls(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        match self.calls.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[async_trait]
impl CloudBackend for StubCloudBackend {
    fn available(&self) -> bool {
        self.available
    }

    async fn chat(&self, model: &str, _request: &QueryRequest) -> Result<CloudReply> {
        self.lock_calls().push(model.to_string());
        match self.lock_script().pop_front() {
            Some(reply) => reply,
            None => Err(RouterError::Cloud("скрипт заглушки исчерпан".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_scripts_replies_and_tracks_calls() {
        let backend = StubCloudBackend::new(true);
        backend.push(Ok(CloudReply::text("раз")));
        backend.push(Err(RouterError::Cloud("сеть".to_string())));

        assert!(backend.available());
        let first = backend
            .chat("m1", &QueryRequest::text("a"))
            .await
            .expect("ответ");
        assert_eq!(first.text, "раз");
        assert!(backend.chat("m2", &QueryRequest::text("b")).await.is_err());
        assert_eq!(backend.calls(), vec!["m1".to_string(), "m2".to_string()]);
    }

    #[test]
    fn http_backend_without_key_is_unavailable() {
        let backend = HttpCloudBackend::new("https://example.com", None, Duration::from_secs(1));
        let backend = backend.expect("клиент");
        assert!(!backend.available());
    }
}
