//! Ошибки инструмента WoL (thiserror).

use thiserror::Error;

/// Ошибки разбора параметров и отправки magic packet.
#[derive(Debug, Error)]
pub enum WolError {
    /// MAC-адрес не разобран.
    #[error("invalid mac address '{input}': {reason}")]
    InvalidMac {
        /// Исходная строка (как пришла в параметрах).
        input: String,
        /// Причина отказа.
        reason: &'static str,
    },

    /// Параметры вызова не являются объектом JSON.
    #[error("params must be a JSON object, got {0}")]
    InvalidParams(&'static str),

    /// Обязательный параметр отсутствует.
    #[error("missing required parameter '{0}'")]
    MissingParameter(&'static str),

    /// Параметр имеет неверный тип или значение.
    #[error("invalid parameter '{name}': expected {expected}, got {actual}")]
    InvalidParameter {
        /// Имя параметра.
        name: &'static str,
        /// Ожидаемая форма.
        expected: &'static str,
        /// Фактическое значение (тип или содержимое).
        actual: String,
    },

    /// Адрес отправки не разобран как IPv4-адрес.
    #[error("invalid broadcast address '{0}': IPv4 address expected")]
    InvalidBroadcast(String),

    /// Ошибка сокета: создание, настройка broadcast или отправка.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
