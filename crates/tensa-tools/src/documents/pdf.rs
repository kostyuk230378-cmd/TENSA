//! Извлечение текста: PDF (ТЗ tensa-tools: `documents/src/pdf.rs`).
//!
//! [derived] Транспорт — `pdf-extract` (чистый Rust): ADR-020 называет
//! pdfplumber-rs основным, но он помечен Alpha, а план разведки прямо
//! предусматривает fallback pdf-extract — на сроке идём запасным путём.
//! Ограничение: текстовые PDF; сканы без OCR-слоя дают пустой результат
//! (OCR — отдельный этап, ТЗ `ocr/`).

use std::path::Path;

use super::error::DocumentError;

/// Извлечь текст из PDF-файла.
pub fn extract(path: &Path) -> Result<String, DocumentError> {
    pdf_extract::extract_text(path).map_err(|error| DocumentError::Pdf(error.to_string()))
}
