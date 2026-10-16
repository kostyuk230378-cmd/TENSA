//! Вывод CLI: цветной текст, таблицы, JSON, тихий режим (ТЗ tensa-cli).

use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use serde::Serialize;

use crate::error::{CliError, Result};

/// Режим вывода, общий для всех команд (`--json`, `--quiet`).
#[derive(Debug, Clone, Copy)]
pub struct Output {
    json: bool,
    quiet: bool,
}

impl Output {
    /// Режим вывода из глобальных флагов.
    pub fn new(json: bool, quiet: bool) -> Self {
        Self { json, quiet }
    }

    /// Включён ли JSON-режим.
    pub fn json_mode(&self) -> bool {
        self.json
    }

    /// Включён ли тихий режим.
    pub fn quiet(&self) -> bool {
        self.quiet
    }

    /// Строка данных (в тихом режиме подавляется).
    pub fn message(&self, text: impl AsRef<str>) {
        if !self.quiet {
            println!("{}", text.as_ref());
        }
    }

    /// Успешное завершение операции (зелёная галочка).
    pub fn success(&self, text: impl AsRef<str>) {
        if !self.quiet {
            println!("{} {}", "✔".green(), text.as_ref());
        }
    }

    /// Предупреждение (жёлтое, stderr; печатается и в тихом режиме).
    pub fn warning(&self, text: impl AsRef<str>) {
        eprintln!("{} {}", "⚠".yellow(), text.as_ref());
    }

    /// JSON-вывод: это данные, печатается и в тихом режиме.
    pub fn json<T: Serialize>(&self, value: &T) -> Result<()> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|e| CliError::Message(format!("сериализация JSON: {e}")))?;
        println!("{text}");
        Ok(())
    }

    /// Таблица (в тихом режиме не печатается).
    pub fn table(&self, headers: &[&str], rows: &[Vec<String>]) {
        if self.quiet {
            return;
        }
        print!("{}", table_string(headers, rows));
    }

    /// Пары «ключ — значение» (человеческий режим, выровнено по колонке).
    pub fn key_values(&self, pairs: &[(String, String)]) {
        if self.quiet {
            return;
        }
        print!("{}", key_values_string(pairs));
    }
}

/// Отрисовка таблицы (вынесено отдельно — тестируется без печати).
pub fn table_string(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(headers.to_vec());
    for row in rows {
        table.add_row(row.clone());
    }
    format!("{table}\n")
}

/// Отрисовка пар «ключ — значение» с выровненной колонкой ключей.
pub fn key_values_string(pairs: &[(String, String)]) -> String {
    let width = pairs
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (key, value) in pairs {
        out.push_str(&format!("{key:<width$}  {value}\n", width = width));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_contains_headers_and_cells() {
        let text = table_string(
            &["id", "status"],
            &[vec!["42".to_string(), "pending".to_string()]],
        );
        assert!(text.contains("id"));
        assert!(text.contains("status"));
        assert!(text.contains("pending"));
        assert!(text.contains('─'), "должны быть рамки таблицы: {text}");
    }

    #[test]
    fn key_values_aligned() {
        let text = key_values_string(&[
            ("root".to_string(), "/tmp/tensa".to_string()),
            ("node_id".to_string(), "abc".to_string()),
        ]);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("root   "));
        assert!(lines[0].ends_with("/tmp/tensa"));
        assert_eq!(lines[1], "node_id  abc");
    }

    #[test]
    fn quiet_suppresses_printing_but_not_json() {
        // Печать здесь не проверяем (нет перехвата stdout в unit),
        // но фиксируем контракт флагов.
        let output = Output::new(false, true);
        assert!(output.quiet());
        assert!(!output.json_mode());
        let output = Output::new(true, true);
        assert!(output.json_mode());
    }
}
