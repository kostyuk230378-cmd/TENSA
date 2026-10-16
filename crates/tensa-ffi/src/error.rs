//! Ошибки FFI-поверхности (Контракт 9: `[Error] enum TensaError`).
//!
//! [derived] Варианты UDL не несут payload (только имена) — исходный текст
//! ошибки уходит в лог (`tracing`), наружу отдаётся класс ошибки.
//! Форма вариантов — с пустыми скобками: этого требует кодоген uniffi
//! для UDL-enum без полей.

use thiserror::Error;

/// Классы ошибок Tensa (имена — дословно из UDL).
#[derive(Debug, Error)]
pub enum TensaError {
    /// Ошибка конфигурации.
    #[error("config error")]
    ConfigError {},
    /// Сетевая ошибка.
    #[error("network error")]
    NetworkError {},
    /// Ошибка очереди одобрения.
    #[error("approval error")]
    ApprovalError {},
    /// Ошибка памяти.
    #[error("memory error")]
    MemoryError {},
    /// Ошибка почты.
    #[error("email error")]
    EmailError {},
    /// Ошибка календаря.
    #[error("calendar error")]
    CalendarError {},
    /// Ошибка приложений.
    #[error("app error")]
    AppError {},
    /// Ошибка узлов.
    #[error("node error")]
    NodeError {},
    /// Внутренняя ошибка.
    #[error("internal error")]
    InternalError {},
}

impl TensaError {
    /// Ошибка с записью исходной причины в лог.
    pub fn config(context: &str, error: impl std::fmt::Display) -> Self {
        tracing::error!(context, error = %error, "tensa-ffi: ошибка конфигурации");
        Self::ConfigError {}
    }

    /// Внутренняя ошибка с записью исходной причины в лог.
    pub fn internal(context: &str, error: impl std::fmt::Display) -> Self {
        tracing::error!(context, error = %error, "tensa-ffi: внутренняя ошибка");
        Self::InternalError {}
    }

    /// Ответ заглушки: функциональность появится на своём этапе.
    pub fn not_implemented(what: &str) -> Self {
        tracing::warn!(what, "tensa-ffi: интерфейс ещё не реализован");
        Self::InternalError {}
    }
}

impl From<tensa_core::CoreError> for TensaError {
    /// Ядро: конфигурационные ошибки — `ConfigError`, остальное — `InternalError`.
    fn from(error: tensa_core::CoreError) -> Self {
        let mapped = match error {
            tensa_core::CoreError::Config(_) => Self::ConfigError {},
            _ => {
                tracing::error!(error = %error, "tensa-ffi: ошибка ядра");
                Self::InternalError {}
            }
        };
        mapped
    }
}

impl From<tensa_approval::ApprovalError> for TensaError {
    fn from(error: tensa_approval::ApprovalError) -> Self {
        tracing::error!(error = %error, "tensa-ffi: ошибка одобрений");
        Self::ApprovalError {}
    }
}

impl From<tensa_nodes::NodesError> for TensaError {
    fn from(error: tensa_nodes::NodesError) -> Self {
        tracing::error!(error = %error, "tensa-ffi: ошибка узлов");
        Self::NodeError {}
    }
}

impl From<tensa_memory::MemoryError> for TensaError {
    fn from(error: tensa_memory::MemoryError) -> Self {
        tracing::error!(error = %error, "tensa-ffi: ошибка памяти");
        Self::MemoryError {}
    }
}
