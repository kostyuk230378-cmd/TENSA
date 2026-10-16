//! Ошибки CLI (thiserror): без паник, `unwrap()`/`expect()` в прод-коде.

use thiserror::Error;

/// Удобный алиас результата CLI.
pub type Result<T> = std::result::Result<T, CliError>;

/// Ошибки tensa-cli.
#[derive(Debug, Error)]
pub enum CliError {
    /// Ошибка ядра (конфиг, хранилища, мастер первого запуска).
    #[error("ошибка ядра: {0}")]
    Core(#[from] tensa_core::CoreError),

    /// Ошибка хранилища одобрений.
    #[error("ошибка одобрений: {0}")]
    Approval(#[from] tensa_approval::ApprovalError),

    /// Ошибка реестра узлов.
    #[error("ошибка узлов: {0}")]
    Nodes(#[from] tensa_nodes::NodesError),

    /// Ошибка профилирования/бенчмарка.
    #[error("ошибка профилирования: {0}")]
    Perf(#[from] tensa_perf::PerfError),

    /// Ошибка ввода-вывода.
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    /// Неразобранный UUID в аргументе (ТЗ-тесты: «неверный ID»).
    #[error("некорректный id «{raw}»: {source}")]
    BadId {
        /// Как аргумент пришёл.
        raw: String,
        /// Причина отказа разбора.
        source: uuid::Error,
    },

    /// Команда объявлена в CLI, но реализация ещё не готова (честная заглушка).
    #[error("не реализовано в tensa-cli (MVP): {0}")]
    Unimplemented(String),

    /// Прочие ошибки с готовым сообщением.
    #[error("{0}")]
    Message(String),
}

impl CliError {
    /// Код возврата процесса (clap для ошибок разбора использует 2).
    pub fn exit_code(&self) -> i32 {
        1
    }
}

/// Разобрать UUID из аргумента с понятной ошибкой.
pub fn parse_id(raw: &str) -> Result<uuid::Uuid> {
    uuid::Uuid::parse_str(raw).map_err(|source| CliError::BadId {
        raw: raw.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_id_reports_argument() {
        let error = parse_id("не-uuid").expect_err("должен быть отказ");
        let text = error.to_string();
        assert!(text.contains("не-uuid"), "текст: {text}");
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn uuid_parses() {
        let id = parse_id("6f1c1c62-1a2b-4c3d-9e8f-0123456789ab").expect("валидный uuid");
        assert_eq!(id.to_string(), "6f1c1c62-1a2b-4c3d-9e8f-0123456789ab");
    }
}
