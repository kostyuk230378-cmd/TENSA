//! # tensa-llama — уровень 2 (ADR-003)
//!
//! Безопасная асинхронная обёртка над llama.cpp:
//! - RAII для ресурсов (модель, контекст, сессия);
//! - ошибки через `Result`, не паники;
//! - tokio-совместимый асинхронный API;
//! - контроль версий и автообновление с подтверждением (уровень 3).
//!
//! Статус нативного слоя: FFI-биндинги (`tensa-llama-sys`) ещё не подключены
//! (нужны LLVM/libclang и закреплённая git-версия llama.cpp). Поэтому
//! [`LlamaEngine::load`] в текущей сборке возвращает честную ошибку
//! [`LlmError::EngineUnavailable`]; весь остальной слой (конфиг, sampling,
//! оценка памяти, окно сессии, контроль версий) полностью функционален и
//! покрыт тестами.

pub mod context;
pub mod engine;
pub mod error;
pub mod model;
/// Нативный движок llama.cpp (фича `native`; TASK-ENV-001).
#[cfg(feature = "native")]
mod native;
pub mod sampling;
pub mod session;
pub mod update;

pub use context::{ensure_fits, estimate_kv_cache_bytes};
pub use engine::{
    ChatMessage, ChatRequest, ChatResponse, ChatStream, FinishReason, GenerateRequest,
    GenerateResponse, LlamaEngine, LlmConfig, LlmEngine, LlmHealth, Role, ToolCall, ToolDefinition,
};
pub use error::{LlmError, Result};
pub use model::{ModelInfo, ModelTier};
pub use sampling::{detect_stop, SamplingParams};
pub use session::ChatSession;
pub use update::{UpdateChecker, UpdateFlow, UpdateStage};

/// Нативный движок llama.cpp скомпилирован и доступен?
/// Отражает фичу `native` (уровень 1 собран: TASK-ENV-001).
pub const NATIVE_ENGINE: bool = tensa_llama_sys::NATIVE_READY;

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
