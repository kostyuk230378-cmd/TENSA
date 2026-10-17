//! Локальный индекс файлов: сканирование папок (Документы/Рабочий стол/Загрузки)
//! и поиск по имени и содержимому (SQLite FTS5).
//!
//! Зачем: секретарь должен сам находить документы на этом ПК («Найди договор
//! ВСТК»), а не просить пользователя их загрузить. Индекс живёт в
//! `<root>/files.db`; текст извлекается инструментом tensa-tools (PDF/DOCX/
//! PPTX/XLSX/TXT/MD). Каталоги и лимиты — из `[documents] folders` конфига
//! или значения по умолчанию.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tokio::sync::OnceCell;

/// Ошибка индекса (текст — для лога/ответа API).
#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    /// Ошибка базы/файловой системы.
    #[error("файлы: {0}")]
    Storage(String),
}

/// Результат операций с индексом.
pub type Result<T> = std::result::Result<T, FilesError>;

/// Найденный файл.
#[derive(Debug, Clone, Serialize)]
pub struct FileHit {
    /// Полный путь.
    pub path: String,
    /// Имя файла.
    pub name: String,
    /// Фрагмент текста вокруг совпадения.
    pub snippet: String,
}

/// Состояние индекса.
#[derive(Debug, Clone, Serialize)]
pub struct FilesStatus {
    /// Идёт ли сканирование сейчас.
    pub scanning: bool,
    /// Сколько файлов в индексе.
    pub files: i64,
    /// Время последней индексации (unix seconds), 0 — пусто.
    pub updated_at: i64,
    /// Каталоги, которые сканируются.
    pub dirs: Vec<String>,
}

/// Индекс файлов (ленивое подключение к SQLite).
pub struct FilesIndex {
    db: PathBuf,
    pool: OnceCell<SqlitePool>,
    scanning: Arc<AtomicBool>,
}

impl FilesIndex {
    /// Индекс в указанной базе (создаётся при первом использовании).
    pub fn new(db: PathBuf) -> Self {
        Self {
            db,
            pool: OnceCell::new(),
            scanning: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Отключённый индекс (тесты и состояние без файлов).
    pub fn disabled() -> Self {
        Self::new(std::env::temp_dir().join("tensa-files-disabled.db"))
    }

    /// Флаг «идёт сканирование» (для `/files/status`).
    pub fn scanning_flag(&self) -> Arc<AtomicBool> {
        self.scanning.clone()
    }

    /// Подключиться и создать схему (однократно).
    async fn pool(&self) -> Result<&SqlitePool> {
        self.pool
            .get_or_try_init(|| async {
                if let Some(parent) = self.db.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        FilesError::Storage(format!("каталог {}: {e}", parent.display()))
                    })?;
                }
                let options = SqliteConnectOptions::new()
                    .filename(&self.db)
                    .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options)
                    .await
                    .map_err(|e| {
                        FilesError::Storage(format!("подключение {}: {e}", self.db.display()))
                    })?;
                for statement in SCHEMA {
                    sqlx::query(statement)
                        .execute(&pool)
                        .await
                        .map_err(|e| FilesError::Storage(format!("схема: {e}")))?;
                }
                Ok(pool)
            })
            .await
    }

    /// Найти файлы по запросу (имя + содержимое).
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<FileHit>> {
        let match_query = fts_query(query);
        let Some(match_query) = match_query else {
            return Ok(Vec::new());
        };
        let pool = self.pool().await?;
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT f.path, f.name, snippet(files_fts, 1, '«', '»', '…', 18) \
             FROM files_fts JOIN files f ON f.id = files_fts.rowid \
             WHERE files_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(match_query)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| FilesError::Storage(format!("поиск: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(path, name, snippet)| FileHit { path, name, snippet })
            .collect())
    }

    /// Статистика индекса.
    pub async fn status(&self, dirs: &[PathBuf]) -> Result<FilesStatus> {
        let pool = self.pool().await?;
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
            .fetch_one(pool)
            .await
            .map_err(|e| FilesError::Storage(format!("счётчик: {e}")))?;
        let updated: (Option<i64>,) = sqlx::query_as("SELECT MAX(indexed_at) FROM files")
            .fetch_one(pool)
            .await
            .map_err(|e| FilesError::Storage(format!("время: {e}")))?;
        Ok(FilesStatus {
            scanning: self.scanning.load(Ordering::Relaxed),
            files: count.0,
            updated_at: updated.0.unwrap_or(0),
            dirs: dirs
                .iter()
                .map(|dir| dir.display().to_string())
                .collect(),
        })
    }

    /// Полное (инкрементальное) сканирование каталогов.
    pub async fn scan(&self, dirs: &[PathBuf]) -> Result<usize> {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return Ok(0);
        }
        let result = self.scan_inner(dirs).await;
        self.scanning.store(false, Ordering::SeqCst);
        result
    }

    /// Внутренний проход по каталогам.
    async fn scan_inner(&self, dirs: &[PathBuf]) -> Result<usize> {
        let mut queue: Vec<(PathBuf, u8)> = dirs.iter().map(|dir| (dir.clone(), 0)).collect();
        let mut visited = 0usize;
        let mut indexed = 0usize;

        while let Some((dir, depth)) = queue.pop() {
            if depth > MAX_DEPTH {
                continue;
            }
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                if visited >= MAX_FILES {
                    break;
                }
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(_) => continue,
                };
                if file_type.is_dir() {
                    if should_skip_dir(&name) {
                        continue;
                    }
                    if depth < MAX_DEPTH {
                        queue.push((path, depth + 1));
                    }
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }
                if name.starts_with('.') || !supported(&path) {
                    continue;
                }
                let metadata = match entry.metadata() {
                    Ok(metadata) => metadata,
                    Err(_) => continue,
                };
                if metadata.len() > MAX_FILE_BYTES {
                    continue;
                }
                visited += 1;
                let mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs() as i64)
                    .unwrap_or(0);
                if self.is_fresh(&path, metadata.len(), mtime).await? {
                    continue;
                }
                let path_for_extract = path.clone();
                let text = tokio::task::spawn_blocking(move || {
                    tensa_tools::documents::extract_text(&path_for_extract)
                        .map(|document| document.text)
                })
                .await
                .ok()
                .and_then(|result| result.ok())
                .unwrap_or_default();
                self.upsert(&path, &name, metadata.len(), mtime, &text)
                    .await?;
                indexed += 1;
            }
        }
        Ok(indexed)
    }

    /// Файл не менялся с прошлой индексации?
    async fn is_fresh(&self, path: &Path, size: u64, mtime: i64) -> Result<bool> {
        let pool = self.pool().await?;
        let row: Option<(i64, i64)> =
            sqlx::query_as("SELECT size, mtime FROM files WHERE path = ?")
                .bind(path.to_string_lossy().to_string())
                .fetch_optional(pool)
                .await
                .map_err(|e| FilesError::Storage(format!("проверка: {e}")))?;
        Ok(matches!(row, Some((old_size, old_mtime)) if old_size == size as i64 && old_mtime == mtime))
    }

    /// Записать/обновить файл и его текст в FTS.
    async fn upsert(&self, path: &Path, name: &str, size: u64, mtime: i64, text: &str) -> Result<()> {
        let pool = self.pool().await?;
        let path_text = path.to_string_lossy().to_string();
        let dir = path
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_string())
            .unwrap_or_default();

        let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM files WHERE path = ?")
            .bind(&path_text)
            .fetch_optional(pool)
            .await
            .map_err(|e| FilesError::Storage(format!("id: {e}")))?;
        if let Some((id,)) = existing {
            sqlx::query("DELETE FROM files_fts WHERE rowid = ?")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FilesError::Storage(format!("fts delete: {e}")))?;
            sqlx::query("DELETE FROM files WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| FilesError::Storage(format!("delete: {e}")))?;
        }

        let inserted: (i64,) = sqlx::query_as(
            "INSERT INTO files (path, name, dir, ext, size, mtime, text, indexed_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&path_text)
        .bind(name)
        .bind(&dir)
        .bind(&ext)
        .bind(size as i64)
        .bind(mtime)
        .bind(text)
        .bind(chrono::Utc::now().timestamp())
        .fetch_one(pool)
        .await
        .map_err(|e| FilesError::Storage(format!("insert: {e}")))?;
        sqlx::query("INSERT INTO files_fts (rowid, name, text) VALUES (?, ?, ?)")
            .bind(inserted.0)
            .bind(name)
            .bind(text)
            .execute(pool)
            .await
            .map_err(|e| FilesError::Storage(format!("fts insert: {e}")))?;
        Ok(())
    }

    /// Прямая запись текста (тесты и импорт извне).
    pub async fn upsert_text(&self, path: &str, name: &str, text: &str) -> Result<()> {
        let path = PathBuf::from(path);
        self.upsert(&path, name, text.len() as u64, 0, text).await
    }
}

/// Схема: файлы + FTS5 по имени и тексту.
const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS files (id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL, \
     name TEXT NOT NULL, dir TEXT NOT NULL, ext TEXT NOT NULL, size INTEGER NOT NULL, \
     mtime INTEGER NOT NULL, text TEXT NOT NULL, indexed_at INTEGER NOT NULL);",
    "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(name, text);",
];

/// Максимальная глубина обхода.
const MAX_DEPTH: u8 = 6;
/// Максимум файлов за проход.
const MAX_FILES: usize = 5000;
/// Максимальный размер файла (25 МБ).
const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

/// Служебные каталоги, которые не индексируем.
fn should_skip_dir(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "node_modules"
            | ".git"
            | "appdata"
            | "windows"
            | "program files"
            | "program files (x86)"
            | "target"
            | "$recycle.bin"
            | ".cache"
            | "onedrivetemp"
    ) || name.starts_with('.')
}

/// Поддерживаемые для извлечения текста расширения.
fn supported(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "pdf" | "docx" | "pptx" | "xlsx" | "txt" | "md" | "csv" | "log" | "json"
    )
}

/// Построить FTS5-запрос: термины длиной от 3 символов, префиксный поиск, OR.
fn fts_query(raw: &str) -> Option<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in raw.split(|c: char| !c.is_alphanumeric()) {
        let word = word.trim();
        if word.chars().count() < 3 {
            continue;
        }
        let escaped = word.replace('"', "\"\"");
        terms.push(format!("\"{escaped}\"*"));
    }
    if terms.is_empty() {
        return None;
    }
    terms.dedup();
    Some(terms.join(" OR "))
}

/// Человеческий список найденных файлов (прямой ответ без LLM).
pub fn format_hits(hits: &[FileHit]) -> String {
    let mut out = format!("Нашёл {} файл(ов) на этом ПК:\n", hits.len());
    for (index, hit) in hits.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", index + 1, hit.path));
        if !hit.snippet.trim().is_empty() {
            let snippet: String = hit.snippet.chars().take(220).collect();
            out.push_str(&format!("   {}\n", snippet.replace('\n', " ")));
        }
    }
    out
}

/// Похоже ли сообщение на поиск файла («найди договор», «где файл», «find»).
pub fn looks_like_file_search(text: &str) -> bool {
    let first = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let first = first.trim_matches(|c: char| !c.is_alphanumeric());
    matches!(
        first,
        "найди" | "найти" | "поищи" | "поиск" | "где" | "покажи" | "find" | "search" | "locate"
    )
}

/// Контекст для модели: несколько фрагментов найденных файлов.
pub fn build_context(hits: &[FileHit]) -> String {
    let mut out = String::from("Локальные файлы на этом ПК, найденные по запросу:\n");
    for hit in hits.iter().take(3) {
        let snippet: String = hit.snippet.chars().take(400).collect();
        out.push_str(&format!("- {}: {}\n", hit.path, snippet.replace('\n', " ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Индекс во временной базе.
    fn index() -> FilesIndex {
        let dir = std::env::temp_dir().join(format!("tensa-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        FilesIndex::new(dir.join("files.db"))
    }

    #[tokio::test]
    async fn search_finds_name_and_text() {
        let index = index();
        index
            .upsert_text(
                r"C:\Users\User\Documents\Договор ВСТК.docx",
                "Договор ВСТК.docx",
                "Договор поставки ВСТК от 1 октября. Стороны: ООО Ромашка и ВСТК.",
            )
            .await
            .expect("запись");
        let hits = index.search("договор", 10).await.expect("поиск");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.contains("Договор ВСТК.docx"));
        assert!(hits[0].snippet.contains("ВСТК"));
    }

    #[tokio::test]
    async fn search_returns_empty_for_short_query() {
        let index = index();
        assert!(index.search("ok", 10).await.expect("поиск").is_empty());
    }

    #[test]
    fn formatting_and_heuristics() {
        let hits = vec![FileHit {
            path: r"C:\Docs\Договор ВСТК.docx".to_string(),
            name: "Договор ВСТК.docx".to_string(),
            snippet: "…Договор поставки…".to_string(),
        }];
        let text = format_hits(&hits);
        assert!(text.contains("Договор ВСТК.docx"));
        assert!(looks_like_file_search("Найди документ Договор ВСТК"));
        assert!(looks_like_file_search("где файл договора"));
        assert!(!looks_like_file_search("напиши письмо поставщику"));
    }

    #[test]
    fn fts_query_builds_prefix_terms() {
        assert_eq!(
            fts_query("Найди документ ВСТК").as_deref(),
            Some("\"Найди\"* OR \"документ\"* OR \"ВСТК\"*")
        );
        assert!(fts_query("и в").is_none());
    }
}
