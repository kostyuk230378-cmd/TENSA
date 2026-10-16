//! Аудит действий инструментов (ТЗ tensa-tools: `~/.tensa/logs/tools.log`).
//!
//! Каждая запись — строка JSON: время, инструмент, действие, результат.
//! Секреты в журнал не попадают: пишутся только имена действий и статусы.
//! Ошибка журнала не срывает само действие — она уходит в `tracing`
//! (аудит не должен ломать работу секретаря).

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Каталог журналов в корне данных (`<root>/logs`).
pub fn logs_dir(root: &Path) -> PathBuf {
    root.join("logs")
}

/// Путь журнала инструментов (`<root>/logs/tools.log`).
pub fn tools_log_path(root: &Path) -> PathBuf {
    logs_dir(root).join("tools.log")
}

/// Записать событие инструмента в журнал.
pub fn record(root: &Path, tool: &str, action: &str, result: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let line = serde_json::json!({
        "ts": timestamp,
        "tool": tool,
        "action": action,
        "result": result,
    })
    .to_string();

    let path = tools_log_path(root);
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "{line}")
    };
    if let Err(error) = write() {
        tracing::warn!(
            error = %error,
            path = %path.display(),
            "аудит: не удалось записать журнал инструментов"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_appends_json_line() {
        let root = std::env::temp_dir().join(format!("tensa-audit-{}", uuid::Uuid::new_v4()));
        record(&root, "telegram", "send to 42", "ok");
        record(&root, "telegram", "approve abc", "denied");

        let text = std::fs::read_to_string(tools_log_path(&root)).expect("журнал");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).expect("JSON");
        assert_eq!(first["tool"], "telegram");
        assert_eq!(first["action"], "send to 42");
        assert_eq!(first["result"], "ok");
        assert!(first["ts"].as_u64().is_some());

        std::fs::remove_dir_all(root).ok();
    }
}
