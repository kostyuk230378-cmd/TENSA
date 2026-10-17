//! Извлечение текста: TXT (ТЗ tensa-tools: `documents/src/txt.rs`).
//!
//! Простое чтение файла с потерей невалидных UTF-8 байтов: для индексации
//! и предпросмотра этого достаточно (кодировки Windows-1251 и т. п. —
//! отдельный этап в `ocr`/нормализации, если понадобится).

use std::path::Path;

use super::error::DocumentError;

/// Прочитать текст файла как UTF-8 (lossy).
pub fn extract(path: &Path) -> Result<String, DocumentError> {
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_utf8_text() {
        let dir = std::env::temp_dir().join(format!("tensa-txt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("note.txt");
        std::fs::write(&path, "Строка один\nСтрока два").expect("запись");

        let text = extract(&path).expect("чтение");
        assert!(text.contains("Строка один"));
        std::fs::remove_dir_all(dir).ok();
    }
}
