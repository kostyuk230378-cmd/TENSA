//! Извлечение текста: XLSX (ТЗ tensa-tools: `documents/src/xlsx.rs`).
//!
//! Парсер — `calamine` (план разведки: calamine 0.36). Листы выводятся
//! секциями `# Лист`, строки — через табуляцию, пустые строки пропускаются
//! (для индексации нужен текст, а не сетка Excel).

use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};

use super::error::DocumentError;

/// Извлечь текст из xlsx (все листы).
pub fn extract(path: &Path) -> Result<String, DocumentError> {
    let mut workbook =
        open_workbook_auto(path).map_err(|error| DocumentError::Xlsx(error.to_string()))?;

    let names: Vec<String> = workbook
        .sheets_metadata()
        .iter()
        .map(|sheet| sheet.name.clone())
        .collect();
    if names.is_empty() {
        return Err(DocumentError::Unsupported("в xlsx нет листов".to_string()));
    }

    let mut output = String::new();
    for name in names {
        let range = workbook
            .worksheet_range(&name)
            .map_err(|error| DocumentError::Xlsx(format!("лист «{name}»: {error}")))?;
        let mut sheet_text = String::new();
        for row in range.rows() {
            let line = row
                .iter()
                .map(cell_to_string)
                .collect::<Vec<_>>()
                .join("\t")
                .trim_end()
                .to_string();
            if !line.trim().is_empty() {
                sheet_text.push_str(&line);
                sheet_text.push('\n');
            }
        }
        if !sheet_text.is_empty() {
            output.push_str(&format!("# {name}\n{sheet_text}\n"));
        }
    }

    if output.trim().is_empty() {
        return Err(DocumentError::Unsupported(
            "в xlsx нет текстовых данных".to_string(),
        ));
    }
    Ok(output.trim().to_string())
}

/// Значение ячейки в строку.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Минимальный валидный xlsx (inline-строки), чтобы проверить calamine.
    fn write_minimal_xlsx(path: &Path) {
        let file = std::fs::File::create(path).expect("файл");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        let parts: &[(&str, &str)] = &[
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
            ),
            (
                "xl/workbook.xml",
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="Лист1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1" t="inlineStr"><is><t>Привет</t></is></c><c r="B1"><v>42</v></c></row>
<row r="2"><c r="A2" t="inlineStr"><is><t>Мир</t></is></c></row>
</sheetData>
</worksheet>"#,
            ),
        ];
        for (name, body) in parts {
            zip.start_file(*name, options).expect("запись");
            zip.write_all(body.as_bytes()).expect("xml");
        }
        zip.finish().expect("финиш");
    }

    #[test]
    fn extracts_cells_and_sheet_names() {
        let dir = std::env::temp_dir().join(format!("tensa-xlsx-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("sample.xlsx");
        write_minimal_xlsx(&path);

        let text = extract(&path).expect("извлечение");
        assert!(text.contains("Лист1"), "текст: {text}");
        assert!(text.contains("Привет"), "текст: {text}");
        assert!(text.contains("42"));
        assert!(text.contains("Мир"));
        std::fs::remove_dir_all(dir).ok();
    }
}
