//! Инструмент Documents (ТЗ tensa-tools: `documents/`): извлечение текста
//! из PDF/DOCX/XLSX/PPTX/TXT/MD для поиска и индексации памяти.
//!
//! Принцип ТЗ: чтение локального файла — не сетевое действие, поэтому
//! `requires_approval() == false`, но каждое извлечение пишется в аудит
//! (`<root>/logs/tools.log`). Сканы без текстового слоя требуют OCR —
//! это отдельный этап (`ocr/`).

pub mod detect;
pub mod docx;
pub mod error;
pub mod markdown;
pub mod pdf;
pub mod pptx;
pub mod txt;
pub mod xlsx;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::Value;

use crate::audit;
use crate::error::{Result as ToolsResult, ToolsError};
use crate::Tool;

pub use detect::DocumentKind;
pub use error::DocumentError;

/// Имя инструмента в реестре.
pub const NAME: &str = "documents";

/// Описание инструмента для LLM и интерфейса.
pub const DESCRIPTION: &str = "Documents: извлечь текст из PDF/DOCX/XLSX/PPTX/TXT/MD. \
Параметры: {\"path\": \"...\"} — вернуть тип, число символов и начало текста.";

/// Результат извлечения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedDocument {
    /// Тип документа.
    pub kind: DocumentKind,
    /// Извлечённый текст.
    pub text: String,
}

/// Извлечь текст из файла (тип определяется автоматически).
pub fn extract_text(path: &Path) -> Result<ExtractedDocument, DocumentError> {
    let kind = detect::detect(path)?;
    let text = match kind {
        DocumentKind::Pdf => pdf::extract(path)?,
        DocumentKind::Docx => docx::extract(path)?,
        DocumentKind::Xlsx => xlsx::extract(path)?,
        DocumentKind::Pptx => pptx::extract(path)?,
        DocumentKind::Txt => txt::extract(path)?,
        DocumentKind::Markdown => markdown::extract(path)?,
    };
    Ok(ExtractedDocument { kind, text })
}

/// Инструмент `documents` для [`crate::ToolRegistry`].
///
/// Корень данных нужен для аудита (ТЗ: все действия — в `tools.log`).
pub struct DocumentsTool {
    root: PathBuf,
}

impl DocumentsTool {
    /// Создать инструмент с корнем данных (для журнала аудита).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl Tool for DocumentsTool {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    /// Извлечь текст и вернуть метаданные + предпросмотр.
    async fn execute(&self, params: Value) -> ToolsResult<Value> {
        let raw_path = params.get("path").and_then(Value::as_str).ok_or_else(|| {
            ToolsError::Documents(DocumentError::Unsupported(
                "ожидается {\"path\": \"...\"}".to_string(),
            ))
        })?;

        let extracted = extract_text(Path::new(raw_path))?;
        let chars = extracted.text.chars().count();
        let preview: String = extracted.text.chars().take(500).collect();

        audit::record(
            &self.root,
            NAME,
            &format!("extract {raw_path} ({})", extracted.kind.as_str()),
            "ok",
        );
        Ok(serde_json::json!({
            "kind": extracted.kind.as_str(),
            "chars": chars,
            "preview": preview,
        }))
    }

    /// Локальное чтение файла — без одобрения (ТЗ: approval — для сетевых).
    fn requires_approval(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_reads_txt() {
        let dir = std::env::temp_dir().join(format!("tensa-docs-mod-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("sample.txt");
        std::fs::write(&path, "текст документа").expect("запись");

        let extracted = extract_text(&path).expect("извлечение");
        assert_eq!(extracted.kind, DocumentKind::Txt);
        assert!(extracted.text.contains("текст документа"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn tool_returns_metadata_and_audits() {
        let dir = std::env::temp_dir().join(format!("tensa-docs-tool-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("sample.txt");
        std::fs::write(&path, "документ для инструмента").expect("запись");

        let tool = DocumentsTool::new(&dir);
        assert_eq!(tool.name(), "documents");
        assert!(!tool.requires_approval());

        let result = tool
            .execute(serde_json::json!({ "path": path.to_string_lossy() }))
            .await
            .expect("execute");
        assert_eq!(result["kind"], "txt");
        assert!(result["chars"].as_u64().unwrap_or(0) > 0);
        assert!(result["preview"]
            .as_str()
            .is_some_and(|preview| preview.contains("документ")));

        let journal = std::fs::read_to_string(dir.join("logs").join("tools.log")).expect("аудит");
        assert!(journal.contains("extract"), "журнал: {journal}");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn tool_rejects_missing_path() {
        let tool = DocumentsTool::new(std::env::temp_dir());
        assert!(tool.execute(serde_json::json!({})).await.is_err());
    }
}
