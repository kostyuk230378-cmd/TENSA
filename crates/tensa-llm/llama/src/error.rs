//! Ошибки движка (thiserror). Никаких паник — только `Result` (ADR-003).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, LlmError>;

/// Ошибки инференса.
#[derive(Debug, Error)]
pub enum LlmError {
    /// Файл модели не найден.
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// Модель повреждена или нечитаема.
    #[error("model corrupted: {0}")]
    ModelCorrupted(String),

    /// Недостаточно RAM для контекста/модели.
    #[error("insufficient memory: required {required_mb} MB, available {available_mb} MB")]
    OutOfMemory { required_mb: u64, available_mb: u64 },

    /// Нативный движок недоступен (FFI-слой не подключён/не собран).
    #[error("native engine unavailable: {0}")]
    EngineUnavailable(String),

    /// Модель не загружена (операция до `load`).
    #[error("model is not loaded")]
    NotLoaded,

    /// Ошибка llama.cpp.
    #[error("llama.cpp error: {0}")]
    Llama(String),

    /// Ошибка обновления (ADR-003, уровень 3).
    #[error("update error: {0}")]
    Update(String),

    /// Некорректная конфигурация/параметры.
    #[error("config error: {0}")]
    Config(String),

    /// Ошибка ввода-вывода.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка сериализации.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
