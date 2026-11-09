//! Приложения операционной системы (Контракт 5, MVP): сканирование,
//! кэш `apps_cache.json`, запуск по имени, открытие URL.
//!
//! MVP-отступление от Контракта 5 (зафиксировано): полный белый список
//! `apps.toml` не введён — запуск инициируется владельцем (Bearer-запрос,
//! trusted origin), поэтому применяется только чёрный список
//! `[security.denylist]` из `~/.tensa/apps.toml`. Секретов здесь нет;
//! все запуски логируются.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Найденное приложение (Контракт 5: `ScannedApp`, упрощённо).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedApp {
    /// Имя (имя ярлыка/записи без расширения).
    pub name: String,
    /// Путь (Windows: `.lnk`; Linux: строка `Exec`).
    pub path: String,
    /// Источник (`start-menu` / `applications`).
    pub source: String,
}

/// Сколь глубокого вложения каталогов достаточно (Start Menu Programs).
const WALK_DEPTH: usize = 5;

/// Подсказки к мусорным ярлыкам (деинсталляторы не запускаем).
fn is_junk(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("uninstall") || lower.contains("удалить") || lower.contains("деинсталл")
}

/// Просканировать приложения текущей платформы.
pub fn scan_apps() -> Vec<ScannedApp> {
    #[cfg(windows)]
    {
        scan_windows()
    }
    #[cfg(not(windows))]
    {
        scan_linux()
    }
}

/// Путь к кэшу сканирования (Контракт 5: `~/.tensa/apps_cache.json`).
pub fn cache_path(root: &Path) -> PathBuf {
    root.join("apps_cache.json")
}

/// Прочитать кэш сканирования.
pub fn read_cache(root: &Path) -> Option<Vec<ScannedApp>> {
    let raw = std::fs::read_to_string(cache_path(root)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Сохранить кэш сканирования.
pub fn write_cache(root: &Path, apps: &[ScannedApp]) -> std::io::Result<()> {
    let body = serde_json::to_string_pretty(apps).map_err(std::io::Error::other)?;
    std::fs::write(cache_path(root), body)
}

/// Чёрный список имён из `~/.tensa/apps.toml`
/// (`[security.denylist] windows | linux`, в нижнем регистре).
pub fn denylist(root: &Path) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(root.join("apps.toml")) else {
        return Vec::new();
    };
    let Ok(table) = raw.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(security) = table.get("security").and_then(|value| value.as_table()) else {
        return Vec::new();
    };
    let Some(denylist) = security.get("denylist").and_then(|value| value.as_table()) else {
        return Vec::new();
    };
    #[cfg(windows)]
    let key = "windows";
    #[cfg(not(windows))]
    let key = "linux";
    denylist
        .get(key)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| item.to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// Запустить приложение по пути ярлыка/команде (после проверки чёрного списка).
pub fn launch(app: &ScannedApp) -> std::io::Result<()> {
    tracing::info!(name = %app.name, "apps: запуск приложения");
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &app.path])
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&app.path)
            .spawn()?;
        Ok(())
    }
}

/// Открыть URL штатным браузером (только http/https — защита от `file:` и
/// инъекций через прочие схемы).
pub fn open_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("URL должен начинаться с http:// или https://".to_string());
    }
    tracing::info!("apps: открытие URL");
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(windows)]
fn scan_windows() -> Vec<ScannedApp> {
    let mut apps = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(program_data) = std::env::var_os("ProgramData") {
        dirs.push(PathBuf::from(program_data).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    for dir in dirs {
        walk_shortcuts(&dir, 0, &mut apps);
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());
    apps
}

#[cfg(windows)]
fn walk_shortcuts(dir: &Path, depth: usize, apps: &mut Vec<ScannedApp>) {
    if depth > WALK_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_shortcuts(&path, depth + 1, apps);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
        {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() || is_junk(&name) {
                continue;
            }
            apps.push(ScannedApp {
                name,
                path: path.to_string_lossy().into_owned(),
                source: "start-menu".to_string(),
            });
        }
    }
}

#[cfg(not(windows))]
fn scan_linux() -> Vec<ScannedApp> {
    let mut apps = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/share/applications"));
    for dir in dirs {
        walk_desktops(&dir, 0, &mut apps);
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());
    apps
}

#[cfg(not(windows))]
fn walk_desktops(dir: &Path, depth: usize, apps: &mut Vec<ScannedApp>) {
    if depth > WALK_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_desktops(&path, depth + 1, apps);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "desktop")
        {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() || is_junk(&name) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Exec обязателен для запуска (Desktop Entry Specification).
            let Some((_, exec)) = parse_desktop(&content) else {
                continue;
            };
            apps.push(ScannedApp {
                name,
                path: exec,
                source: "applications".to_string(),
            });
        }
    }
}

/// Разобрать `.desktop`: возвращает `(Name, Exec)` (Name[ru] приоритетнее).
/// [cfg] Используется сканером Linux и юнит-тестами на любой платформе.
#[cfg(any(test, not(windows)))]
fn parse_desktop(content: &str) -> Option<(String, String)> {
    let mut name = String::new();
    let mut name_ru = String::new();
    let mut exec = String::new();
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("Name=") {
            name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Name[ru]=") {
            name_ru = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Exec=") {
            exec = value.trim().to_string();
        }
    }
    if exec.is_empty() {
        return None;
    }
    let display = if name_ru.is_empty() { name } else { name_ru };
    Some((display, exec))
}

/// Убрать подстановки аргументов из `Exec` (`%f`, `%F`, `%u`, …).
#[cfg(any(test, not(windows)))]
fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|token| !(token.len() == 2 && token.starts_with('%')))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Чёрный список из apps.toml читается по платформенной секции.
    #[test]
    fn denylist_reads_platform_section() {
        let root = std::env::temp_dir().join(format!("tensa-apps-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("корень");
        std::fs::write(
            root.join("apps.toml"),
            "[security.denylist]\nwindows = [\"Regedit\"]\nlinux = [\"rm\"]\n",
        )
        .expect("apps.toml");
        let list = denylist(&root);
        #[cfg(windows)]
        assert_eq!(list, vec!["regedit".to_string()]);
        #[cfg(not(windows))]
        assert_eq!(list, vec!["rm".to_string()]);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Без apps.toml чёрный список пуст (не ошибка).
    #[test]
    fn denylist_without_file_is_empty() {
        let root = std::env::temp_dir().join(format!("tensa-apps-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("корень");
        assert!(denylist(&root).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// Кэш переживает roundtrip.
    #[test]
    fn cache_roundtrip() {
        let root = std::env::temp_dir().join(format!("tensa-apps-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("корень");
        let apps = vec![ScannedApp {
            name: "Блокнот".to_string(),
            path: "C:\\Windows\\notepad.exe".to_string(),
            source: "start-menu".to_string(),
        }];
        write_cache(&root, &apps).expect("запись кэша");
        assert_eq!(read_cache(&root), Some(apps));
        let _ = std::fs::remove_dir_all(root);
    }

    /// Чтение кэша без файла — None (не ошибка).
    #[test]
    fn cache_missing_is_none() {
        let root = std::env::temp_dir().join(format!("tensa-apps-{}", uuid::Uuid::new_v4()));
        assert_eq!(read_cache(&root), None);
    }

    /// URL валидируется по схеме (http/https).
    #[test]
    fn open_url_rejects_non_http_schemes() {
        assert!(open_url("file:///C:/Windows/System32/calc.exe").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
        assert!(open_url("   ").is_err());
    }

    /// `.desktop`: Name[ru] приоритетнее, Exec без подстановок.
    #[test]
    fn parse_desktop_prefers_russian_name_and_cleans_exec() {
        let content = "[Desktop Entry]\nName=Text Editor\nName[ru]=Текстовый редактор\nExec=gedit %F\nType=Application\n";
        let parsed = parse_desktop(content).expect("запись с Exec");
        assert_eq!(parsed.0, "Текстовый редактор");
        assert_eq!(clean_exec(&parsed.1), "gedit");
    }

    /// `.desktop` без Exec не пригоден для запуска.
    #[test]
    fn parse_desktop_without_exec_is_none() {
        let content = "[Desktop Entry]\nName=Only Name\nType=Link\n";
        assert!(parse_desktop(content).is_none());
    }

    /// Сканирование не паникует и возвращает корректный список.
    #[test]
    fn scan_apps_does_not_panic() {
        let apps = scan_apps();
        // Формат записей корректен: имя и путь непустые.
        assert!(apps
            .iter()
            .all(|app| !app.name.is_empty() && !app.path.is_empty()));
    }
}
