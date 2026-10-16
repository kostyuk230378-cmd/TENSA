//! Движок инференса (ТЗ tensa-llm: «Реализация: `LlamaEngine`»).
//!
//! Публичный API — дословно из ТЗ. Нативный FFI-слой (`tensa-llama-sys`)
//! ещё не подключён: `load` проверяет конфиг и файл модели, после чего
//! возвращает `EngineUnavailable` с объяснением — до этапа сборки llama.cpp
//! (ADR-003, уровень 1). Остальные методы возвращают `NotLoaded`.

use crate::error::{LlmError, Result};
use crate::model::{ModelInfo, ModelTier};
use crate::sampling::{detect_stop, SamplingParams};
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::RwLock;
use std::time::Instant;

/// Потоковый ответ chat_stream.
/// [derived] В ТЗ — `impl Stream`; для dyn-совместимости (`Arc<dyn LlmEngine>`
/// в координаторе) используется boxed-поток с тем же Item.
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

/// Конфигурация движка (ТЗ tensa-llm; значения по умолчанию — из ТЗ).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub model_path: PathBuf,
    pub model_tier: ModelTier,
    pub context_size: u32,
    pub threads: u32,
    pub batch_size: u32,
    pub gpu_layers: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub max_tokens: u32,
    pub repeat_penalty: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            model_tier: ModelTier::Xs,
            context_size: 4096,
            threads: 0,
            batch_size: 512,
            gpu_layers: 0,
            temperature: 0.7,
            top_p: 0.95,
            top_k: 40,
            max_tokens: 2048,
            repeat_penalty: 1.1,
        }
    }
}

impl LlmConfig {
    /// Валидация конфига (без обращения к нативному слою).
    pub fn validate(&self) -> Result<()> {
        if self.model_path.as_os_str().is_empty() {
            return Err(LlmError::Config("model_path is empty".into()));
        }
        if self.context_size == 0 {
            return Err(LlmError::Config("context_size must be > 0".into()));
        }
        if self.batch_size == 0 {
            return Err(LlmError::Config("batch_size must be > 0".into()));
        }
        if self.max_tokens == 0 {
            return Err(LlmError::Config("max_tokens must be > 0".into()));
        }
        SamplingParams {
            temperature: self.temperature,
            top_p: self.top_p,
            top_k: self.top_k,
            repeat_penalty: self.repeat_penalty,
        }
        .validate()?;
        Ok(())
    }
}

/// Запрос генерации (ТЗ).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop: Vec<String>,
}

/// Ответ генерации (ТЗ).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub text: String,
    pub tokens_generated: u32,
    pub tokens_prompt: u32,
    pub time_ms: u64,
    pub finish_reason: FinishReason,
}

/// Причина завершения генерации (ТЗ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    Error,
}

/// Сообщение чата (ТЗ).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

/// Роль сообщения (ТЗ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Запрос чата с инструментами (ТЗ: tool use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Описание инструмента для LLM (ТЗ: JSON-схема).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Вызов инструмента в ответе LLM.
/// [derived] ТЗ ссылается на тип, но не определяет поля.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub params: serde_json::Value,
}

/// Ответ чата (ТЗ).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub tool_calls: Vec<ToolCall>,
    pub tokens_generated: u32,
    pub time_ms: u64,
}

/// Состояние движка (ТЗ: + здоровье и модель).
/// [derived] Поля в ТЗ не заданы — минимальный набор для health/UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmHealth {
    pub loaded: bool,
    /// Нативный движок скомпилирован и доступен.
    pub native_engine: bool,
    pub model: Option<String>,
    pub context_size: u32,
    pub tier: Option<ModelTier>,
}

/// Трейт движка инференса (ТЗ tensa-llm, сигнатуры дословно).
#[async_trait]
pub trait LlmEngine: Send + Sync {
    async fn load(&self, config: LlmConfig) -> Result<()>;
    async fn unload(&self) -> Result<()>;
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse>;
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream>;
    async fn health(&self) -> Result<LlmHealth>;
    fn model_info(&self) -> Option<ModelInfo>;
}

/// Внутреннее состояние движка.
#[derive(Default)]
struct EngineState {
    config: Option<LlmConfig>,
    info: Option<ModelInfo>,
    /// Нативный движок llama.cpp (фича `native`). Arc + tokio-Mutex — чтобы
    /// блокирующая генерация шла в `spawn_blocking`, не удерживая блокировку
    /// состояния.
    #[cfg(feature = "native")]
    native: Option<std::sync::Arc<tokio::sync::Mutex<crate::native::NativeEngine>>>,
}

impl std::fmt::Debug for EngineState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineState")
            .field("config", &self.config)
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

/// Безопасная реализация движка (RAII-ресурсы появятся с нативным слоем).
#[derive(Debug, Default)]
pub struct LlamaEngine {
    inner: RwLock<EngineState>,
}

impl LlamaEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Общий предварительный этап загрузки: валидация конфига и файла.
    fn validate_load(&self, config: &LlmConfig) -> Result<()> {
        config.validate()?;
        if !config.model_path.exists() {
            return Err(LlmError::ModelNotFound(
                config.model_path.display().to_string(),
            ));
        }
        Ok(())
    }

    /// Проверить, что модель загружена (используется заглушечными ветками;
    /// в нативной сборке состояние проверяется через `native_handle`).
    #[cfg(not(feature = "native"))]
    fn ensure_loaded(&self) -> Result<()> {
        let st = self
            .inner
            .read()
            .map_err(|_| LlmError::LockPoisoned("engine".into()))?;
        if st.config.is_none() {
            return Err(LlmError::NotLoaded);
        }
        Ok(())
    }

    /// Проверка стоп-последовательности в накопленном тексте (общая логика
    /// для generate/chat_stream; используется нативным циклом генерации).
    pub fn check_stop(&self, text: &str, stop: &[String]) -> Option<(usize, usize)> {
        detect_stop(text, stop)
    }

    /// Взять Arc на нативный движок и конфиг (или `NotLoaded`).
    #[cfg(feature = "native")]
    fn native_handle(
        &self,
    ) -> Result<(
        std::sync::Arc<tokio::sync::Mutex<crate::native::NativeEngine>>,
        LlmConfig,
    )> {
        let st = self
            .inner
            .read()
            .map_err(|_| LlmError::LockPoisoned("engine".into()))?;
        let native = st.native.clone().ok_or(LlmError::NotLoaded)?;
        let config = st.config.clone().ok_or(LlmError::NotLoaded)?;
        Ok((native, config))
    }

    /// Загрузить модель через нативный слой llama.cpp (фича `native`).
    #[cfg(feature = "native")]
    async fn load_native(&self, config: LlmConfig) -> Result<()> {
        let load_config = config.clone();
        let native =
            tokio::task::spawn_blocking(move || crate::native::NativeEngine::load(&load_config))
                .await
                .map_err(|error| LlmError::Llama(format!("загрузка модели прервана: {error}")))??;

        let info = native.model_info(&config);
        let mut st = self
            .inner
            .write()
            .map_err(|_| LlmError::LockPoisoned("engine".into()))?;
        st.native = Some(std::sync::Arc::new(tokio::sync::Mutex::new(native)));
        st.info = Some(info);
        st.config = Some(config);
        tracing::info!("модель загружена (нативный llama.cpp)");
        Ok(())
    }

    /// Заглушка загрузки без нативного слоя (честная ошибка, ADR-003).
    #[cfg(not(feature = "native"))]
    async fn load_stub(&self, _config: LlmConfig) -> Result<()> {
        Err(LlmError::EngineUnavailable(
            "native llama.cpp FFI is not enabled in this build (feature `native`); \
             load() validates config/model file only (ADR-003)"
                .into(),
        ))
    }

    /// Генерация через нативный слой (фича `native`).
    #[cfg(feature = "native")]
    async fn generate_native(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        let (native, config) = self.native_handle()?;
        let max_tokens = request.max_tokens.unwrap_or(config.max_tokens);
        let temperature = request.temperature.unwrap_or(config.temperature);
        let (top_p, top_k, repeat_penalty) = (config.top_p, config.top_k, config.repeat_penalty);
        let prompt = request.prompt;
        let stop = request.stop;
        let started = Instant::now();

        let output = tokio::task::spawn_blocking(move || {
            let mut engine = native.blocking_lock();
            engine.generate(
                &prompt,
                max_tokens,
                temperature,
                top_p,
                top_k,
                repeat_penalty,
                &stop,
            )
        })
        .await
        .map_err(|error| LlmError::Llama(format!("генерация прервана: {error}")))??;

        Ok(GenerateResponse {
            text: output.text,
            tokens_generated: output.tokens_generated,
            tokens_prompt: output.tokens_prompt,
            time_ms: started.elapsed().as_millis() as u64,
            finish_reason: output.finish_reason,
        })
    }

    /// Генерация без нативного слоя (форма ответа — по ТЗ).
    #[cfg(not(feature = "native"))]
    async fn generate_stub(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        self.ensure_loaded()?;
        let started = Instant::now();
        Ok(GenerateResponse {
            text: String::new(),
            tokens_generated: 0,
            tokens_prompt: request.prompt.chars().count() as u32 / 4,
            time_ms: started.elapsed().as_millis() as u64,
            finish_reason: FinishReason::Error,
        })
    }

    /// Чат через нативный слой (chat-шаблон модели + генерация).
    #[cfg(feature = "native")]
    async fn chat_native(&self, request: ChatRequest) -> Result<ChatResponse> {
        let (native, config) = self.native_handle()?;
        let max_tokens = request.max_tokens.unwrap_or(config.max_tokens);
        let temperature = request.temperature.unwrap_or(config.temperature);
        let (top_p, top_k, repeat_penalty) = (config.top_p, config.top_k, config.repeat_penalty);
        let messages = request.messages;
        let started = Instant::now();

        let output = tokio::task::spawn_blocking(move || {
            let mut engine = native.blocking_lock();
            let prompt = engine.apply_chat_template(&messages)?;
            engine.generate(
                &prompt,
                max_tokens,
                temperature,
                top_p,
                top_k,
                repeat_penalty,
                &[],
            )
        })
        .await
        .map_err(|error| LlmError::Llama(format!("генерация прервана: {error}")))??;

        Ok(ChatResponse {
            message: ChatMessage {
                role: Role::Assistant,
                content: output.text,
            },
            // [derived] grammar-constrained tool calls — вместе с tensa-tools.
            tool_calls: Vec::new(),
            tokens_generated: output.tokens_generated,
            time_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Чат без нативного слоя (форма ответа — по ТЗ).
    #[cfg(not(feature = "native"))]
    async fn chat_stub(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.ensure_loaded()?;
        let started = Instant::now();
        let _ = request;
        Ok(ChatResponse {
            message: ChatMessage {
                role: Role::Assistant,
                content: String::new(),
            },
            tool_calls: Vec::new(),
            tokens_generated: 0,
            time_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Потоковая выдача через нативный слой.
    #[cfg(feature = "native")]
    async fn chat_stream_native(&self, request: ChatRequest) -> Result<ChatStream> {
        // [derived] Поштучная выдача появится с поточным циклом инференса;
        // контракт потока сохранён: куски текста + пустой финальный.
        let response = self.chat_native(request).await?;
        let chunks: Vec<Result<String>> = response
            .message
            .content
            .split_inclusive(char::is_whitespace)
            .map(|piece| Ok(piece.to_string()))
            .chain(std::iter::once(Ok(String::new())))
            .collect();
        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    /// Поток без нативного слоя.
    #[cfg(not(feature = "native"))]
    async fn chat_stream_stub(&self, request: ChatRequest) -> Result<ChatStream> {
        self.ensure_loaded()?;
        let _ = request;
        let stream = futures::stream::iter(Vec::<Result<String>>::new());
        Ok(Box::pin(stream))
    }
}

#[async_trait]
impl LlmEngine for LlamaEngine {
    async fn load(&self, config: LlmConfig) -> Result<()> {
        self.validate_load(&config)?;
        #[cfg(feature = "native")]
        let result = self.load_native(config).await;
        #[cfg(not(feature = "native"))]
        let result = self.load_stub(config).await;
        result
    }

    async fn unload(&self) -> Result<()> {
        let mut st = self
            .inner
            .write()
            .map_err(|_| LlmError::LockPoisoned("engine".into()))?;
        st.config = None;
        st.info = None;
        #[cfg(feature = "native")]
        {
            // Drop освобождает модель и контекст llama.cpp.
            st.native = None;
        }
        Ok(())
    }

    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        #[cfg(feature = "native")]
        let result = self.generate_native(request).await;
        #[cfg(not(feature = "native"))]
        let result = self.generate_stub(request).await;
        result
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        #[cfg(feature = "native")]
        let result = self.chat_native(request).await;
        #[cfg(not(feature = "native"))]
        let result = self.chat_stub(request).await;
        result
    }

    async fn chat_stream(&self, request: ChatRequest) -> Result<ChatStream> {
        #[cfg(feature = "native")]
        let result = self.chat_stream_native(request).await;
        #[cfg(not(feature = "native"))]
        let result = self.chat_stream_stub(request).await;
        result
    }

    async fn health(&self) -> Result<LlmHealth> {
        let st = self
            .inner
            .read()
            .map_err(|_| LlmError::LockPoisoned("engine".into()))?;
        Ok(LlmHealth {
            loaded: st.config.is_some(),
            native_engine: crate::NATIVE_ENGINE,
            model: st
                .config
                .as_ref()
                .map(|c| c.model_path.display().to_string()),
            context_size: st.config.as_ref().map(|c| c.context_size).unwrap_or(0),
            tier: st.config.as_ref().map(|c| c.model_tier),
        })
    }

    fn model_info(&self) -> Option<ModelInfo> {
        self.inner.read().ok().and_then(|st| st.info.clone())
    }
}
