//! Извлечение текста: Markdown (ТЗ tensa-tools: `documents/src/markdown.rs`).
//!
//! [derived] Для индексации и предпросмотра Markdown читается как обычный
//! текст (разметка не мешает полнотекстовому поиску). Полноценный рендер
//! (pulldown-cmark) появится вместе с отображением документов в PWA.

use std::path::Path;

use super::error::DocumentError;

/// Прочитать markdown как текст.
pub fn extract(path: &Path) -> Result<String, DocumentError> {
    super::txt::extract(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_markdown_source() {
        let dir = std::env::temp_dir().join(format!("tensa-md-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("readme.md");
        std::fs::write(&path, "# Заголовок\n\nАбзац **текста**.").expect("запись");

        let text = extract(&path).expect("чтение");
        assert!(text.contains("# Заголовок"));
        std::fs::remove_dir_all(dir).ok();
    }
}
