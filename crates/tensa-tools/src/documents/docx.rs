//! Извлечение текста: DOCX (ТЗ tensa-tools: `documents/src/docx.rs`).
//!
//! DOCX — ZIP с `word/document.xml`; текст лежит в `<w:t>`-узлах, конец
//! абзаца — `</w:p>`. [derived] Разбор на `zip` + `quick-xml` напрямую:
//! для извлечения текста полноценный docx-парсер не нужен (меньше зависимостей).

use std::io::Read;
use std::path::Path;

use super::error::DocumentError;

/// Извлечь текст из docx.
pub fn extract(path: &Path) -> Result<String, DocumentError> {
    let bytes = std::fs::read(path)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| DocumentError::Archive(error.to_string()))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|error| DocumentError::Archive(format!("word/document.xml: {error}")))?
        .read_to_string(&mut xml)
        .map_err(|error| DocumentError::Archive(format!("word/document.xml: {error}")))?;

    collect_text(&xml, "w:t", "w:p")
}

/// Собрать текст: содержимое тегов `text_tag`, перевод строки на `para_tag`.
pub(super) fn collect_text(
    xml: &str,
    text_tag: &str,
    para_tag: &str,
) -> Result<String, DocumentError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut output = String::new();
    let mut inside_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                if event.name().as_ref() == text_tag {
                    inside_text = true;
                }
            }
            Ok(Event::End(event)) => {
                let name = event.name();
                if name.as_ref() == text_tag {
                    inside_text = false;
                } else if name.as_ref() == para_tag {
                    output.push('\n');
                }
            }
            Ok(Event::Text(event)) => {
                if inside_text {
                    let text = quick_xml::escape::unescape(&event)
                        .map_err(|error| DocumentError::Xml(error.to_string()))?;
                    output.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(DocumentError::Xml(error.to_string())),
        }
    }

    Ok(output.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn collects_paragraphs_and_runs() {
        let xml = "<w:document><w:body>\
            <w:p><w:r><w:t>Привет</w:t></w:r><w:r><w:t>, мир</w:t></w:r></w:p>\
            <w:p><w:r><w:t>Второй абзац</w:t></w:r></w:p>\
            </w:body></w:document>";
        let text = collect_text(xml, "w:t", "w:p").expect("разбор");
        assert_eq!(text, "Привет, мир\nВторой абзац");
    }

    #[test]
    fn extracts_from_real_zip_container() {
        let dir = std::env::temp_dir().join(format!("tensa-docx-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("sample.docx");

        {
            let file = std::fs::File::create(&path).expect("файл");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("word/document.xml", options)
                .expect("запись");
            zip.write_all(
                "<w:document><w:body><w:p><w:r><w:t>Тестовый docx</w:t></w:r></w:p></w:body></w:document>"
                    .as_bytes(),
            )
            .expect("xml");
            zip.finish().expect("финиш");
        }

        let text = extract(&path).expect("извлечение");
        assert!(text.contains("Тестовый docx"), "текст: {text}");
        std::fs::remove_dir_all(dir).ok();
    }
}
