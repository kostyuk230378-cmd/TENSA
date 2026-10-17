//! Определение типа документа (ТЗ tensa-tools: `documents/src/detect.rs`).
//!
//! Сначала расширение файла (быстро и предсказуемо), затем «магия» содержимого:
//! `%PDF` — PDF, `PK\x03\x04` — ZIP-контейнер, внутри которого docx/xlsx/pptx
//! различаются по составу частей (`word/`, `xl/`, `ppt/`).

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::error::DocumentError;

/// Тип документа.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    /// PDF.
    Pdf,
    /// Microsoft Word (docx).
    Docx,
    /// Microsoft Excel (xlsx).
    Xlsx,
    /// Microsoft PowerPoint (pptx).
    Pptx,
    /// Обычный текст.
    Txt,
    /// Markdown.
    Markdown,
}

impl DocumentKind {
    /// Строковое имя (для JSON-ответов и логов).
    pub fn as_str(self) -> &'static str {
        match self {
            DocumentKind::Pdf => "pdf",
            DocumentKind::Docx => "docx",
            DocumentKind::Xlsx => "xlsx",
            DocumentKind::Pptx => "pptx",
            DocumentKind::Txt => "txt",
            DocumentKind::Markdown => "markdown",
        }
    }
}

/// Определить тип документа.
pub fn detect(path: &Path) -> Result<DocumentKind, DocumentError> {
    if !path.is_file() {
        return Err(DocumentError::NotFound(path.display().to_string()));
    }

    if let Some(kind) = by_extension(path) {
        return Ok(kind);
    }
    by_magic(path)
}

/// Тип по расширению.
fn by_extension(path: &Path) -> Option<DocumentKind> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => Some(DocumentKind::Pdf),
        "docx" => Some(DocumentKind::Docx),
        "xlsx" => Some(DocumentKind::Xlsx),
        "pptx" => Some(DocumentKind::Pptx),
        "txt" | "log" => Some(DocumentKind::Txt),
        "md" | "markdown" => Some(DocumentKind::Markdown),
        _ => None,
    }
}

/// Тип по содержимому (запасной путь для файлов без расширения).
fn by_magic(path: &Path) -> Result<DocumentKind, DocumentError> {
    let mut header = [0u8; 8];
    let mut file = File::open(path)?;
    let read = file.read(&mut header)?;

    if read >= 4 && &header[..4] == b"%PDF" {
        return Ok(DocumentKind::Pdf);
    }
    if read >= 4 && header[..4] == [0x50, 0x4B, 0x03, 0x04] {
        // ZIP: различаем docx/xlsx/pptx по частям контейнера.
        let bytes = std::fs::read(path)?;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|error| DocumentError::Archive(error.to_string()))?;
        if archive.by_name("word/document.xml").is_ok() {
            return Ok(DocumentKind::Docx);
        }
        if archive.by_name("xl/workbook.xml").is_ok() {
            return Ok(DocumentKind::Xlsx);
        }
        if archive.by_name("ppt/presentation.xml").is_ok() {
            return Ok(DocumentKind::Pptx);
        }
        return Err(DocumentError::Unsupported(
            "ZIP без признаков docx/xlsx/pptx".to_string(),
        ));
    }

    Err(DocumentError::Unsupported(format!(
        "неопределённый тип файла: {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tensa-docs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join(name);
        let mut file = File::create(&path).expect("файл");
        file.write_all(bytes).expect("запись");
        path
    }

    #[test]
    fn extension_wins() {
        let path = temp_file("note.txt", b"hello");
        assert_eq!(detect(&path).expect("тип"), DocumentKind::Txt);
        let path = temp_file("readme.md", b"# hi");
        assert_eq!(detect(&path).expect("тип"), DocumentKind::Markdown);
    }

    #[test]
    fn magic_detects_pdf_without_extension() {
        let path = temp_file("document", b"%PDF-1.7\n...");
        assert_eq!(detect(&path).expect("тип"), DocumentKind::Pdf);
    }

    #[test]
    fn unknown_format_is_rejected() {
        let path = temp_file("data.bin", b"\x01\x02\x03\x04");
        assert!(detect(&path).is_err());
        let path = temp_file("noext", b"\x01\x02\x03\x04");
        assert!(detect(&path).is_err());
    }

    #[test]
    fn missing_file_is_not_found() {
        let path = std::env::temp_dir().join("tensa-docs-missing-file.pdf");
        assert!(detect(&path).is_err());
    }
}
