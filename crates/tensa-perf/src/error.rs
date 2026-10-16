//! Ошибки крейта (ТЗ tensa-perf).

use thiserror::Error;

/// Результат операций tensa-perf.
pub type Result<T> = std::result::Result<T, PerfError>;

/// Ошибки профилирования, бенчмарка и мониторинга.
#[derive(Debug, Error)]
pub enum PerfError {
    /// Некорректная конфигурация.
    #[error("Ошибка конфигурации: {0}")]
    Config(String),

    /// Ошибка сбора профиля устройства.
    #[error("Ошибка профилирования: {0}")]
    Profile(String),

    /// Ошибка бенчмарка.
    #[error("Ошибка бенчмарка: {0}")]
    Benchmark(String),

    /// Ошибка управления мониторингом.
    #[error("Ошибка мониторинга: {0}")]
    Monitoring(String),

    /// Ошибка ввода-вывода.
    #[error("Ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка JSON (кэш бенчмарка).
    #[error("Ошибка JSON: {0}")]
    Json(#[from] serde_json::Error),
}
