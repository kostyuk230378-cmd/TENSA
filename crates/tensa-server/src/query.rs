//! Шлюз запросов к моделям (`POST /api/v1/query`, Контракт 12).
//!
//! Выбор провайдера:
//! - `[brain] mode = "hybrid"` (по умолчанию) — облако, если задан ключ
//!   OpenRouter (`secrets.toml` → `[openrouter] credential` или env
//!   `OPENROUTER_API_KEY`); при ошибке облака — локальная GGUF; без ключа —
//!   сразу локальная;
//! - `mode = "cloud"` — только облако; `mode = "local"` — только локальная.
//!
//! Локальная модель — первый `*.gguf` в `<root>/models` (или явный
//! `[brain] local_model`); загружается лениво при первом запросе и только
//! в сборке с фичей `native` (llama.cpp). Без фичи локальный провайдер
//! отсутствует — шлюз честно отвечает 503.

#[cfg(feature = "native")]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tensa_core::{Secrets, TensaPaths};
use tensa_router::cloud::{CloudBackend, HttpCloudBackend};
use tensa_router::local::{LocalBackend, LocalReply};
use tensa_router::models::{QueryRequest, QueryResponse};
use tensa_router::RouterError;

use crate::error::{Result, ServerError};

/// Адрес OpenRouter API по умолчанию.
const DEFAULT_CLOUD_BASE: &str = "https://openrouter.ai/api/v1";

/// Таймаут облачного запроса (LLM отвечает не мгновенно).
const CLOUD_TIMEOUT: Duration = Duration::from_secs(120);

/// Режим выбора провайдера (`[brain] mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Только локальная модель.
    Local,
    /// Только облако.
    Cloud,
    /// Облако с откатом на локальную модель.
    Hybrid,
}

impl QueryMode {
    /// Разобрать значение из конфига; неизвестное — hybrid.
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(|value| value.trim().to_ascii_lowercase()) {
            Some(value) if value == "local" => QueryMode::Local,
            Some(value) if value == "cloud" => QueryMode::Cloud,
            _ => QueryMode::Hybrid,
        }
    }
}

/// Шлюз запросов к моделям.
pub struct QueryService {
    cloud: Option<Arc<dyn CloudBackend>>,
    cloud_models: Vec<String>,
    local: Option<Arc<dyn LocalBackend>>,
    local_model: String,
    mode: QueryMode,
}

impl QueryService {
    /// Шлюз без провайдеров (для тестов состояния и случаев «не настроено»).
    pub fn disabled() -> Self {
        Self {
            cloud: None,
            cloud_models: Vec::new(),
            local: None,
            local_model: String::new(),
            mode: QueryMode::Hybrid,
        }
    }

    /// Собрать шлюз из конфига, секретов и путей данных.
    pub fn from_config(brain: &toml::Table, secrets: &Secrets, paths: &TensaPaths) -> Self {
        let mode = QueryMode::parse(brain.get("mode").and_then(|value| value.as_str()));

        let cloud_models = brain
            .get("cloud_models")
            .and_then(|value| value.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|model| model.as_str())
                    .map(|model| model.to_string())
                    .collect::<Vec<_>>()
            })
            .filter(|models| !models.is_empty())
            .unwrap_or_else(|| {
                tensa_router::config::default_cloud_models()
                    .into_iter()
                    .map(|model| model.model)
                    .collect()
            });

        let cloud = secrets.openrouter_key().and_then(|key| {
            match HttpCloudBackend::new(DEFAULT_CLOUD_BASE, Some(key), CLOUD_TIMEOUT) {
                Ok(backend) => Some(Arc::new(backend) as Arc<dyn CloudBackend>),
                Err(error) => {
                    tracing::warn!(error = %error, "облачный бэкенд не создан");
                    None
                }
            }
        });

        let (local, local_model) = build_local(brain, paths);

        Self {
            cloud,
            cloud_models,
            local,
            local_model,
            mode,
        }
    }

    /// Режим выбора провайдера.
    pub fn mode(&self) -> QueryMode {
        self.mode
    }

    /// Настроено ли облако (есть ключ).
    pub fn has_cloud(&self) -> bool {
        self.cloud
            .as_ref()
            .map(|backend| backend.available())
            .unwrap_or(false)
    }

    /// Доступен ли локальный провайдер (модель найдена и сборка с `native`).
    pub fn has_local(&self) -> bool {
        self.local.is_some()
    }

    /// Выполнить запрос: `model` — принудительная облачная модель.
    pub async fn query(
        &self,
        request: &QueryRequest,
        model: Option<&str>,
    ) -> Result<QueryResponse> {
        if let Some(model) = model {
            return self.via_cloud(request, model, false).await;
        }
        match self.mode {
            QueryMode::Cloud => {
                let model = self
                    .cloud_models
                    .first()
                    .map(String::as_str)
                    .ok_or_else(no_provider)?;
                self.via_cloud(request, model, false).await
            }
            QueryMode::Local => self.via_local(request).await,
            QueryMode::Hybrid => {
                if self.has_cloud() {
                    match self.via_cloud_chain(request).await {
                        Ok(response) => Ok(response),
                        Err(cloud_error) => match self.via_local(request).await {
                            Ok(mut response) => {
                                response.degraded = true;
                                Ok(response)
                            }
                            Err(_) => Err(cloud_error),
                        },
                    }
                } else {
                    self.via_local(request).await
                }
            }
        }
    }

    /// Перебор облачных моделей по порядку конфига.
    async fn via_cloud_chain(&self, request: &QueryRequest) -> Result<QueryResponse> {
        let cloud = self.cloud.as_ref().ok_or_else(no_provider)?;
        if !cloud.available() {
            return Err(no_provider());
        }
        let mut last_error: Option<RouterError> = None;
        for model in &self.cloud_models {
            match cloud.chat(model, request).await {
                Ok(reply) => return Ok(cloud_response(reply, model, false)),
                Err(error) => {
                    tracing::warn!(model = %model, error = %error, "облачная модель не ответила");
                    last_error = Some(error);
                }
            }
        }
        Err(ServerError::Unavailable(format!(
            "облако не ответило: {}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "нет моделей в [brain] cloud_models".to_string())
        )))
    }

    /// Запрос к конкретной облачной модели.
    async fn via_cloud(
        &self,
        request: &QueryRequest,
        model: &str,
        degraded: bool,
    ) -> Result<QueryResponse> {
        let cloud = self.cloud.as_ref().ok_or_else(no_provider)?;
        if !cloud.available() {
            return Err(no_provider());
        }
        let reply = cloud
            .chat(model, request)
            .await
            .map_err(|error| ServerError::Unavailable(format!("облако: {error}")))?;
        Ok(cloud_response(reply, model, degraded))
    }

    /// Запрос к локальной модели.
    async fn via_local(&self, request: &QueryRequest) -> Result<QueryResponse> {
        let local = self.local.as_ref().ok_or_else(|| {
            ServerError::Unavailable(
                "локальная модель не настроена: положите GGUF в ~/.tensa/models/ \
                 (сборка с фичей native) или задайте ключ OpenRouter"
                    .to_string(),
            )
        })?;
        let reply = local
            .generate(request)
            .await
            .map_err(|error| ServerError::Unavailable(format!("локальная модель: {error}")))?;
        Ok(local_response(reply, &self.local_model))
    }
}

/// Ошибка «нет ни одного провайдера».
fn no_provider() -> ServerError {
    ServerError::Unavailable(
        "модель не настроена: добавьте ключ OpenRouter в secrets.toml \
         ([openrouter] credential = \"sk-or-…\") или положите GGUF в ~/.tensa/models/"
            .to_string(),
    )
}

/// Ответ из облачного бэкенда.
fn cloud_response(
    reply: tensa_router::cloud::CloudReply,
    model: &str,
    degraded: bool,
) -> QueryResponse {
    QueryResponse {
        text: clean_model_text(&reply.text),
        action: None,
        action_payload: None,
        tool_calls: reply.tool_calls,
        reasoning: reply.reasoning,
        degraded,
        model_used: model.to_string(),
        node_used: "local".to_string(),
        tokens_used: reply.tokens_used,
    }
}

/// Ответ из локального бэкенда.
fn local_response(reply: LocalReply, model: &str) -> QueryResponse {
    QueryResponse {
        text: clean_model_text(&reply.text),
        action: None,
        action_payload: None,
        tool_calls: reply.tool_calls,
        reasoning: reply.reasoning,
        degraded: false,
        model_used: model.to_string(),
        node_used: "local".to_string(),
        tokens_used: Some(u64::from(reply.tokens_generated)),
    }
}

/// Очистить ответ модели от reasoning/control-токенов.
fn clean_model_text(text: &str) -> String {
    let mut value = text.trim();
    if let Some(position) = value.find("<|channel|>final") {
        value = &value[position + "<|channel|>final".len()..];
    } else if let Some(position) = value.find("channel>thought") {
        value = &value[..position];
    }
    for marker in ["Thinking Process:", "**Thinking Process:**", "Thinking:"] {
        if let Some(position) = value.find(marker) {
            value = &value[..position];
        }
    }
    let cleaned = value
        .replace("<|channel|>", "")
        .replace("channel>thought", "")
        .replace("channel>final", "")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        "Модель не дала ответа — уточните запрос.".to_string()
    } else {
        cleaned
    }
}

/// Собрать локальный провайдер (только сборка с `native`).
#[cfg(feature = "native")]
fn build_local(brain: &toml::Table, paths: &TensaPaths) -> (Option<Arc<dyn LocalBackend>>, String) {
    let explicit = brain
        .get("local_model")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);
    let model_path = explicit.or_else(|| find_gguf(paths)).or_else(|| None);
    let Some(model_path) = model_path else {
        return (None, String::new());
    };
    if !model_path.is_file() {
        tracing::warn!(path = %model_path.display(), "локальная модель не найдена");
        return (None, String::new());
    }

    let config = tensa_llm::LlmConfig {
        model_path,
        ..Default::default()
    };
    let name = config
        .model_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "local".to_string());
    let engine = Arc::new(tensa_llm::LlamaEngine::new());
    let local: Arc<dyn LocalBackend> = Arc::new(NativeLocal::new(engine, config));
    (Some(local), name)
}

/// Локальный провайдер без фичи `native` отсутствует.
#[cfg(not(feature = "native"))]
fn build_local(
    _brain: &toml::Table,
    _paths: &TensaPaths,
) -> (Option<Arc<dyn LocalBackend>>, String) {
    (None, String::new())
}

/// Первый `*.gguf` в каталоге моделей.
///
/// Приоритет — проектная модель TheStageAI `gemma-4-E2B-it-XS-TS-Q3_K_S.gguf`
/// (~1.73 ГБ, самый лёгкий квант); иначе — любой другой GGUF (по имени).
#[cfg(feature = "native")]
fn find_gguf(_paths: &TensaPaths) -> Option<PathBuf> {
    const PROJECT_MODEL: &str = "gemma-4-E2B-it-XS-TS-Q3_K_S.gguf";
    let model_dir = install_models_dir();
    let project = model_dir.join(PROJECT_MODEL);
    if project.is_file() {
        return Some(project);
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&model_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                    .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Каталог моделей рядом с установленным exe (а не ~/.tensa).
#[cfg(feature = "native")]
fn install_models_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("models")))
        .unwrap_or_else(|| PathBuf::from("models"))
}

/// Ленивая локальная модель: грузит GGUF при первом запросе.
#[cfg(feature = "native")]
struct NativeLocal {
    engine: Arc<dyn tensa_llm::LlmEngine>,
    config: tensa_llm::LlmConfig,
    loaded: tokio::sync::Mutex<bool>,
}

#[cfg(feature = "native")]
impl NativeLocal {
    fn new(engine: Arc<dyn tensa_llm::LlmEngine>, config: tensa_llm::LlmConfig) -> Self {
        Self {
            engine,
            config,
            loaded: tokio::sync::Mutex::new(false),
        }
    }
}

#[cfg(feature = "native")]
#[async_trait::async_trait]
impl LocalBackend for NativeLocal {
    async fn available(&self) -> bool {
        self.engine
            .health()
            .await
            .map(|health| health.loaded)
            .unwrap_or(false)
    }

    async fn generate(&self, request: &QueryRequest) -> tensa_router::Result<LocalReply> {
        {
            let mut loaded = self.loaded.lock().await;
            if !*loaded {
                self.engine.load(self.config.clone()).await?;
                *loaded = true;
            }
        }
        let chat = tensa_llm::ChatRequest {
            messages: vec![tensa_llm::ChatMessage {
                role: tensa_llm::Role::User,
                content: request.text.clone(),
            }],
            tools: Vec::new(),
            // Короткий ответ: /query — интерактивный чат, а не пакетная генерация.
            max_tokens: Some(256),
            temperature: None,
        };
        let response = self.engine.chat(chat).await?;
        Ok(LocalReply {
            text: response.message.content,
            tokens_generated: response.tokens_generated,
            tool_calls: response
                .tool_calls
                .into_iter()
                .map(|call| tensa_router::models::ToolCall {
                    tool: call.name,
                    params: call.params,
                })
                .collect(),
            reasoning: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tensa_router::cloud::{CloudReply, StubCloudBackend};
    use tensa_router::local::StubLocalBackend;

    /// Сервис с заданными провайдерами (для тестов).
    fn service(
        cloud: Option<Arc<dyn CloudBackend>>,
        models: Vec<&str>,
        local: Option<Arc<dyn LocalBackend>>,
        mode: QueryMode,
    ) -> QueryService {
        QueryService {
            cloud,
            cloud_models: models.into_iter().map(str::to_string).collect(),
            local,
            local_model: "test-local".to_string(),
            mode,
        }
    }

    #[tokio::test]
    async fn hybrid_prefers_cloud() {
        let cloud = Arc::new(StubCloudBackend::new(true));
        cloud.push(Ok(CloudReply::text("из облака")));
        let local = Arc::new(StubLocalBackend::loaded("из модели"));
        let service = service(
            Some(cloud.clone()),
            vec!["test/model"],
            Some(local),
            QueryMode::Hybrid,
        );
        let response = service
            .query(&QueryRequest::text("привет"), None)
            .await
            .expect("ответ");
        assert_eq!(response.text, "из облака");
        assert_eq!(response.model_used, "test/model");
        assert!(!response.degraded);
    }

    #[tokio::test]
    async fn hybrid_falls_back_to_local() {
        let cloud = Arc::new(StubCloudBackend::new(true));
        cloud.push(Err(RouterError::Cloud("нет сети".to_string())));
        let local = Arc::new(StubLocalBackend::loaded("локальный ответ"));
        let service = service(
            Some(cloud),
            vec!["test/model"],
            Some(local),
            QueryMode::Hybrid,
        );
        let response = service
            .query(&QueryRequest::text("привет"), None)
            .await
            .expect("ответ");
        assert_eq!(response.text, "локальный ответ");
        assert!(response.degraded, "откат должен помечаться");
    }

    #[tokio::test]
    async fn explicit_model_goes_to_cloud() {
        let cloud = Arc::new(StubCloudBackend::new(true));
        cloud.push(Ok(CloudReply::text("модель X")));
        let local = Arc::new(StubLocalBackend::loaded("локально"));
        let service = service(
            Some(cloud.clone()),
            vec!["test/model"],
            Some(local),
            QueryMode::Local,
        );
        let response = service
            .query(&QueryRequest::text("привет"), Some("openai/gpt-4o-mini"))
            .await
            .expect("ответ");
        assert_eq!(response.text, "модель X");
        assert_eq!(cloud.calls(), vec!["openai/gpt-4o-mini".to_string()]);
    }

    #[tokio::test]
    async fn local_mode_does_not_touch_cloud() {
        let cloud = Arc::new(StubCloudBackend::new(true));
        let local = Arc::new(StubLocalBackend::loaded("только локально"));
        let service = service(
            Some(cloud.clone()),
            vec!["test/model"],
            Some(local),
            QueryMode::Local,
        );
        let response = service
            .query(&QueryRequest::text("привет"), None)
            .await
            .expect("ответ");
        assert_eq!(response.text, "только локально");
        assert!(cloud.calls().is_empty(), "облако не должно вызываться");
    }

    #[tokio::test]
    async fn no_provider_is_unavailable() {
        let service = service(None, Vec::new(), None, QueryMode::Hybrid);
        let error = service
            .query(&QueryRequest::text("привет"), None)
            .await
            .expect_err("должна быть ошибка");
        assert!(
            matches!(error, ServerError::Unavailable(_)),
            "ожидалась Unavailable, получено: {error:?}"
        );
        assert_eq!(error.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }
}
