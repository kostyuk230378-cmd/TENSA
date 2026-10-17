//! Извлечение текста: PPTX (ТЗ tensa-tools: `documents/src/pptx.rs`).
//!
//! PPTX — ZIP с `ppt/slides/slideN.xml`; текст — в `<a:t>`-узлах,
//! конец абзаца — `</a:p>`. [derived] Свой разбор (zip + quick-xml),
//! как и в плане разведки: специализированный pptx-крейт не нужен.

use std::io::Read;
use std::path::Path;

use super::docx::collect_text;
use super::error::DocumentError;

/// Извлечь текст из pptx (слайды по порядку).
pub fn extract(path: &Path) -> Result<String, DocumentError> {
    let bytes = std::fs::read(path)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| DocumentError::Archive(error.to_string()))?;

    // Слайды: ppt/slides/slideN.xml — сортируем по номеру.
    let mut slides: Vec<(u32, String)> = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| DocumentError::Archive(error.to_string()))?;
        let name = file
            .name()
            .map_err(|error| DocumentError::Archive(error.to_string()))?
            .to_string();
        let Some(number) = slide_number(&name) else {
            continue;
        };
        let mut xml = String::new();
        file.read_to_string(&mut xml)
            .map_err(|error| DocumentError::Archive(format!("{name}: {error}")))?;
        slides.push((number, xml));
    }
    slides.sort_by_key(|(number, _)| *number);
    if slides.is_empty() {
        return Err(DocumentError::Unsupported(
            "в pptx нет слайдов (ppt/slides)".to_string(),
        ));
    }

    let mut output = String::new();
    for (number, xml) in slides {
        let text = collect_text(&xml, "a:t", "a:p")?;
        if text.is_empty() {
            continue;
        }
        output.push_str(&format!("## Слайд {number}\n{text}\n\n"));
    }
    Ok(output.trim().to_string())
}

/// Номер слайда из имени части (`ppt/slides/slide12.xml` → 12).
fn slide_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("ppt/slides/slide")?;
    let number = rest.strip_suffix(".xml")?;
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn slide_number_parsing() {
        assert_eq!(slide_number("ppt/slides/slide1.xml"), Some(1));
        assert_eq!(slide_number("ppt/slides/slide12.xml"), Some(12));
        assert_eq!(slide_number("ppt/slides/_rels/slide1.xml.rels"), None);
        assert_eq!(slide_number("ppt/slides/slideX.xml"), None);
    }

    #[test]
    fn extracts_slides_in_order() {
        let dir = std::env::temp_dir().join(format!("tensa-pptx-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("sample.pptx");

        {
            let file = std::fs::File::create(&path).expect("файл");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            for (name, text) in [
                ("ppt/slides/slide2.xml", "Второй слайд"),
                ("ppt/slides/slide1.xml", "Первый слайд"),
            ] {
                zip.start_file(name, options).expect("запись");
                zip.write_all(
                    format!(
                        "<p:sld><p:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sld>"
                    )
                    .as_bytes(),
                )
                .expect("xml");
            }
            zip.finish().expect("финиш");
        }

        let text = extract(&path).expect("извлечение");
        assert!(text.contains("Первый слайд"), "текст: {text}");
        assert!(text.contains("Второй слайд"));
        let first = text.find("Первый слайд").expect("первый");
        let second = text.find("Второй слайд").expect("второй");
        assert!(first < second, "слайды должны идти по порядку: {text}");
        std::fs::remove_dir_all(dir).ok();
    }
}
