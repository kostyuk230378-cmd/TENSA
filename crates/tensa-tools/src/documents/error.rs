//! Ошибки модуля документов (ТЗ tensa-tools: `documents/src/error.rs`).

use thiserror::Error;

/// Ошибки извлечения текста из документов.
#[derive(Debug, Error)]
pub enum DocumentError {
    /// Файл не найден или недоступен.
    #[error("documents: файл недоступен: {0}")]
    NotFound(String),

    /// Формат не поддерживается (или не определён).
    #[error("documents: неподдерживаемый формат: {0}")]
    Unsupported(String),

    /// Ошибка ввода-вывода.
    #[error("documents: ошибка чтения: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка извлечения PDF.
    #[error("documents: PDF: {0}")]
    Pdf(String),

    /// Ошибка чтения XLSX (calamine).
    #[error("documents: XLSX: {0}")]
    Xlsx(String),

    /// Ошибка ZIP-контейнера (docx/pptx).
    #[error("documents: архив docx/pptx: {0}")]
    Archive(String),

    /// Ошибка разбора XML (docx/pptx).
    #[error("documents: XML: {0}")]
    Xml(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_message_mentions_documents() {
        let text = DocumentError::Unsupported("foo.zzz".to_string()).to_string();
        assert!(text.contains("documents"));
        assert!(text.contains("foo.zzz"));
    }
}
