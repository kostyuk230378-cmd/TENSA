//! Локальный индекс файлов: сканирование папок (Документы/Рабочий стол/Загрузки)
//! и поиск по имени и содержимому (SQLite FTS5).
//!
//! Зачем: секретарь должен сам находить документы на этом ПК («Найди договор
//! ВСТК»), а не просить пользователя их загрузить. Индекс живёт в
//! `<root>/files.db`; текст извлекается инструментом tensa-tools (PDF/DOCX/
//! PPTX/XLSX/TXT/MD). Каталоги и лимиты — из `[documents] folders` конфига
//! или значения по умолчанию.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    /// Каталоги, которые сканируются (корни локальных дисков).
    pub dirs: Vec<String>,
    /// Сколько файлов просмотрено в текущем/последнем проходе.
    pub scanned: u64,
    /// Сколько файлов добавлено/обновлено в текущем/последнем проходе.
    pub indexed_run: u64,
    /// Каталог, который обрабатывается сейчас.
    pub current_dir: String,
}

/// Индекс файлов (ленивое подключение к SQLite).
pub struct FilesIndex {
    db: PathBuf,
    pool: OnceCell<SqlitePool>,
    scanning: Arc<AtomicBool>,
    scanned: Arc<AtomicU64>,
    indexed_run: Arc<AtomicU64>,
    current_dir: Arc<std::sync::Mutex<String>>,
}

impl FilesIndex {
    /// Индекс в указанной базе (создаётся при первом использовании).
    pub fn new(db: PathBuf) -> Self {
        Self {
            db,
            pool: OnceCell::new(),
            scanning: Arc::new(AtomicBool::new(false)),
            scanned: Arc::new(AtomicU64::new(0)),
            indexed_run: Arc::new(AtomicU64::new(0)),
            current_dir: Arc::new(std::sync::Mutex::new(String::new())),
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
        // Сначала строгий запрос (все значимые слова), если пусто — мягкий (OR);
        // затем отсекаем хвосты: оставляем только файлы с максимальным покрытием
        // значимых слов, иначе README/доки забивают выдачу.
        let Some((strict, loose, words)) = fts_queries(query) else {
            return Ok(Vec::new());
        };
        let pool = self.pool().await?;
        let candidates = (limit * 5).clamp(10, 100);
        let mut rows = self
            .run_match(pool, &strict, candidates)
            .await
            .map_err(|e| FilesError::Storage(format!("поиск: {e}")))?;
        if rows.is_empty() {
            rows = self
                .run_match(pool, &loose, candidates)
                .await
                .map_err(|e| FilesError::Storage(format!("поиск: {e}")))?;
        }

        let scored: Vec<(usize, usize, FileHit)> = rows
            .into_iter()
            .map(|(path, name, snippet, text)| {
                let name_lower = name.to_lowercase();
                let text_lower = text.to_lowercase();
                let matches = words
                    .iter()
                    .filter(|word| name_lower.contains(*word) || text_lower.contains(*word))
                    .count();
                let name_matches = words
                    .iter()
                    .filter(|word| name_lower.contains(*word))
                    .count();
                (
                    matches,
                    name_matches,
                    FileHit {
                        path,
                        name,
                        snippet,
                    },
                )
            })
            .collect();

        let best = scored
            .iter()
            .map(|(matches, _, _)| *matches)
            .max()
            .unwrap_or(0);
        let mut kept: Vec<(usize, usize, FileHit)> = scored
            .into_iter()
            .filter(|(matches, _, _)| *matches >= best)
            .collect();
        kept.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        kept.truncate(limit);
        Ok(kept.into_iter().map(|(_, _, hit)| hit).collect())
    }

    /// Выполнить MATCH-запрос (вес имени 50, текста 1).
    async fn run_match(
        &self,
        pool: &SqlitePool,
        match_query: &str,
        limit: usize,
    ) -> std::result::Result<Vec<(String, String, String, String)>, sqlx::Error> {
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT f.path, f.name, snippet(files_fts, 1, '«', '»', '…', 18), f.text \
             FROM files_fts JOIN files f ON f.id = files_fts.rowid \
             WHERE files_fts MATCH ? ORDER BY bm25(files_fts, 50.0, 1.0) LIMIT ?",
        )
        .bind(match_query)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
    }

    /// Есть ли файл в индексе по точному пути.
    pub async fn search_exact(&self, path: &str) -> Result<bool> {
        let pool = self.pool().await?;
        let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM files WHERE path = ?")
            .bind(path)
            .fetch_optional(pool)
            .await
            .map_err(|e| FilesError::Storage(format!("проверка пути: {e}")))?;
        Ok(row.is_some())
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
        let current_dir = self
            .current_dir
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        Ok(FilesStatus {
            scanning: self.scanning.load(Ordering::Relaxed),
            files: count.0,
            updated_at: updated.0.unwrap_or(0),
            dirs: dirs.iter().map(|dir| dir.display().to_string()).collect(),
            scanned: self.scanned.load(Ordering::Relaxed),
            indexed_run: self.indexed_run.load(Ordering::Relaxed),
            current_dir,
        })
    }

    /// Полное (инкрементальное) сканирование каталогов.
    pub async fn scan(&self, dirs: &[PathBuf]) -> Result<usize> {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return Ok(0);
        }
        self.scanned.store(0, Ordering::Relaxed);
        self.indexed_run.store(0, Ordering::Relaxed);
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
            if let Ok(mut current) = self.current_dir.lock() {
                *current = dir.display().to_string();
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
                self.scanned.fetch_add(1, Ordering::Relaxed);
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
                // Битые PDF/DOCX могут паниковать в парсере: изолируем панику,
                // иначе один файл уронил бы всё ядро (SSE/чат «падают»).
                let text = tokio::task::spawn_blocking(move || {
                    let extracted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        tensa_tools::documents::extract_text(&path_for_extract)
                            .map(|document| document.text)
                    }));
                    match extracted {
                        Ok(Ok(text)) => Some(text),
                        Ok(Err(_)) => None,
                        Err(_) => {
                            tracing::warn!(
                                path = %path_for_extract.display(),
                                "парсер документа упал — файл пропущен"
                            );
                            None
                        }
                    }
                })
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
                self.upsert(&path, &name, metadata.len(), mtime, &text)
                    .await?;
                indexed += 1;
                self.indexed_run.fetch_add(1, Ordering::Relaxed);
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
        Ok(
            matches!(row, Some((old_size, old_mtime)) if old_size == size as i64 && old_mtime == mtime),
        )
    }

    /// Записать/обновить файл и его текст в FTS.
    async fn upsert(
        &self,
        path: &Path,
        name: &str,
        size: u64,
        mtime: i64,
        text: &str,
    ) -> Result<()> {
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
const MAX_DEPTH: u8 = 64;
/// Предохранитель от бесконечного прохода.
const MAX_FILES: usize = 2_000_000;
/// Максимальный размер файла (50 МБ).
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;

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
            | "programdata"
            | "target"
            | "$recycle.bin"
            | "system volume information"
            | "recovery"
            | "msocache"
            | "perflogs"
            | "$windows.~ws"
            | "$windows.~bt"
            | "config.msi"
            | ".cache"
            | ".cargo"
            | ".rustup"
            | ".tensa"
            | "tensa-tools"
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

/// Корни всех локальных (fixed) дисков — стандартный набор для индексации.
///
/// CD/DVD, сетевые и съёмные диски не обходим (медленные/нестабильные).
/// Системные каталоги исключает [`should_skip_dir`].
#[cfg(windows)]
pub fn default_scan_roots() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let mut roots = Vec::new();
    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: передаём валидную NUL-терминированную строку.
        let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
        if drive_type == 3 /* DRIVE_FIXED */ && std::path::Path::new(&root).is_dir() {
            roots.push(PathBuf::from(root));
        }
    }
    roots
}

/// Не-Windows: корень файловой системы.
#[cfg(not(windows))]
pub fn default_scan_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}

/// Построить FTS5-запрос: термины длиной от 3 символов, префиксный поиск, OR.
fn fts_queries(raw: &str) -> Option<(String, String, Vec<String>)> {
    // Служебные слова поиска не должны попадать в выдачу (иначе «файлы»/
    // «найди» находят сам репозиторий вместо документов пользователя).
    const STOPWORDS: [&str; 14] = [
        "найди",
        "найти",
        "поищи",
        "поиск",
        "покажи",
        "файл",
        "файлы",
        "файлов",
        "документ",
        "документы",
        "документов",
        "где",
        "find",
        "search",
    ];
    let mut terms: Vec<String> = Vec::new();
    let mut plain: Vec<String> = Vec::new();
    for word in raw.split(|c: char| !c.is_alphanumeric()) {
        let word = word.trim();
        if word.chars().count() < 3 {
            continue;
        }
        let lowered = word.to_lowercase();
        if STOPWORDS.iter().any(|stop| *stop == lowered) {
            continue;
        }
        let escaped = word.replace('"', "\"\"");
        terms.push(format!("\"{escaped}\"*"));
        plain.push(word.to_lowercase());
    }
    if terms.is_empty() {
        return None;
    }
    terms.dedup();
    plain.dedup();
    Some((terms.join(" AND "), terms.join(" OR "), plain))
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
        "найди"
            | "найти"
            | "поищи"
            | "поиск"
            | "где"
            | "покажи"
            | "find"
            | "search"
            | "locate"
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
    fn fts_query_builds_strict_and_loose() {
        let (strict, loose, words) = fts_queries("Найди документ ВСТК").expect("запрос");
        assert_eq!(strict, "\"ВСТК\"*");
        assert_eq!(loose, "\"ВСТК\"*");
        assert_eq!(words, vec!["встк".to_string()]);
        let (strict, loose, _) = fts_queries("Договор ВСТК").expect("запрос");
        assert_eq!(strict, "\"Договор\"* AND \"ВСТК\"*");
        assert_eq!(loose, "\"Договор\"* OR \"ВСТК\"*");
        assert!(fts_queries("и в").is_none());
    }

    #[tokio::test]
    async fn search_cuts_irrelevant_tail() {
        let index = index();
        index
            .upsert_text(
                r"C:\Docs\Договор ВСТК.docx",
                "Договор ВСТК.docx",
                "договор поставки встк",
            )
            .await
            .expect("a");
        index
            .upsert_text(
                r"C:\Docs\Другое.docx",
                "Другое.docx",
                "договор аренды офиса",
            )
            .await
            .expect("b");
        let hits = index.search("Договор ВСТК", 10).await.expect("поиск");
        assert_eq!(hits.len(), 1, "файл без ВСТК не должен попадать");
        assert!(hits[0].name.contains("ВСТК"));
    }
}
