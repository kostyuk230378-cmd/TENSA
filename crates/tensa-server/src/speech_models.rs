//! Каталог моделей распознавания речи и их загрузка.
//!
//! Модели — GGML Whisper (совместимы с `tensa-whisper`), до 800 МБ.
//! Скачиваются в `<root>/models`, активная модель хранится в
//! `<root>/models/asr-active.txt` (имя файла).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// Каталог моделей рядом с exe установки; fallback — данные.
fn install_models_dir(root: &Path) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("models")))
        .unwrap_or_else(|| root.join("models"))
}

/// Запись каталога.
#[derive(Clone, Copy)]
struct CatalogEntry {
    id: &'static str,
    file: &'static str,
    name: &'static str,
    description: &'static str,
    size_mb: u64,
    languages: &'static str,
    accuracy: u8,
    speed: u8,
    engine: &'static str,
    url: &'static str,
}

/// Модели до 800 МБ, поддерживающие русский язык.
const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "tiny",
        file: "ggml-tiny.bin",
        name: "Whisper Tiny",
        description: "Самая быстрая, но низкая точность. Для коротких команд.",
        size_mb: 75,
        languages: "99 языков",
        accuracy: 1,
        speed: 5,
        engine: "whisper",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
    },
    CatalogEntry {
        id: "small-q5_1",
        file: "ggml-small-q5_1.bin",
        name: "Whisper Small q5_1",
        description: "Текущая: хороший баланс точности и скорости.",
        size_mb: 181,
        languages: "99 языков",
        accuracy: 4,
        speed: 4,
        engine: "whisper",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
    },
    CatalogEntry {
        id: "small",
        file: "ggml-small.bin",
        name: "Whisper Small",
        description: "Полная small: точнее квантованной, но заметно медленнее.",
        size_mb: 487,
        languages: "99 языков",
        accuracy: 5,
        speed: 3,
        engine: "whisper",
        url: "https://blob.handy.computer/ggml-small.bin",
    },
    CatalogEntry {
        id: "nemotron-3.5",
        file: "nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf",
        name: "Nemotron Streaming 3.5",
        description: "Потоковая мультиязычная модель, 28 языков. Локальное распознавание, движок Parakeet/Nemotron.",
        size_mb: 716,
        languages: "28 языков",
        accuracy: 4,
        speed: 5,
        engine: "parakeet",
        url: "https://blob.handy.computer/handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf/6d44e540bc31b0de1dbe174a3cea87f53a7f22fb/nemotron-3.5-asr-streaming-0.6b-Q8_0.gguf",
    },
    CatalogEntry {
        id: "parakeet-tdt-v3",
        file: "parakeet-tdt-0.6b-v3-Q8_0.gguf",
        name: "Parakeet TDT 0.6B v3",
        description: "Быстрая и точная, 25 европейских языков. Локальное распознавание, движок Parakeet/Nemotron.",
        size_mb: 705,
        languages: "25 языков",
        accuracy: 5,
        speed: 4,
        engine: "parakeet",
        url: "https://blob.handy.computer/handy-computer/parakeet-tdt-0.6b-v3-gguf/85ac09ea12fc4b1112fa76810059364bc6adc9de/parakeet-tdt-0.6b-v3-Q8_0.gguf",
    },
    CatalogEntry {
        id: "canary-1b-v2",
        file: "canary-1b-v2-Q5_K_M.gguf",
        name: "Canary 1B v2",
        description: "25 языков, перевод на английский. Локальное распознавание, движок Canary.",
        size_mb: 797,
        languages: "25 языков",
        accuracy: 5,
        speed: 4,
        engine: "canary",
        url: "https://blob.handy.computer/handy-computer/canary-1b-v2-gguf/58d13c2c0102229aad45f7e19a77ddc42b41dd9a/canary-1b-v2-Q5_K_M.gguf",
    },
];

/// Состояние загрузки.
#[derive(Clone, Default)]
struct DownloadState {
    downloaded: u64,
    total: u64,
    error: Option<String>,
    done: bool,
}

/// Представление модели для UI.
#[derive(Clone, Serialize)]
pub struct SpeechModelView {
    /// Идентификатор.
    pub id: String,
    /// Имя файла.
    pub file: String,
    /// Название.
    pub name: String,
    /// Описание.
    pub description: String,
    /// Размер, МБ.
    pub size_mb: u64,
    /// Языки.
    pub languages: String,
    /// Точность 1–5.
    pub accuracy: u8,
    /// Скорость 1–5.
    pub speed: u8,
    /// Движок (whisper/parakeet/canary).
    pub engine: String,
    /// Можно выбрать для текущего движка.
    pub can_activate: bool,
    /// Установлена.
    pub installed: bool,
    /// Активна.
    pub active: bool,
    /// Идёт скачивание.
    pub downloading: bool,
    /// Прогресс 0–100.
    pub progress: u8,
    /// Ошибка загрузки.
    pub error: Option<String>,
}

/// Каталог и загрузки.
pub struct SpeechModels {
    models: PathBuf,
    downloads: Arc<Mutex<HashMap<String, DownloadState>>>,
}

impl SpeechModels {
    /// Каталог для корня данных.
    pub fn new(root: &Path) -> Self {
        Self {
            models: install_models_dir(root),
            downloads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Заглушка без каталога.
    pub fn disabled() -> Self {
        Self {
            models: PathBuf::new(),
            downloads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Каталог моделей.
    fn models_dir(&self) -> PathBuf {
        self.models.clone()
    }

    /// Путь к файлу активной модели.
    fn active_file(&self) -> PathBuf {
        self.models_dir().join("asr-active.txt")
    }

    /// Найти установленный файл.
    fn installed_path(&self, entry: &CatalogEntry) -> Option<PathBuf> {
        let roots = vec![self.models_dir(), self.models_dir().join("whisper")];
        roots
            .into_iter()
            .map(|dir| dir.join(entry.file))
            .find(|path| path.is_file())
    }

    /// Активная модель: файл из asr-active.txt либо первый установленный.
    fn active_entry(&self) -> Option<&'static CatalogEntry> {
        let active = std::fs::read_to_string(self.active_file())
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(active) = active {
            if let Some(entry) = CATALOG.iter().find(|entry| entry.file == active) {
                if self.installed_path(entry).is_some() {
                    return Some(entry);
                }
            }
        }
        ["small-q5_1", "small", "tiny"]
            .iter()
            .filter_map(|id| CATALOG.iter().find(|entry| entry.id == *id))
            .find(|entry| self.installed_path(entry).is_some())
    }

    /// Список моделей и активный id.
    pub async fn list(&self) -> (Vec<SpeechModelView>, Option<String>) {
        let active = self.active_entry().map(|entry| entry.id.to_string());
        let downloads = self.downloads.lock().await.clone();
        let models = CATALOG
            .iter()
            .map(|entry| {
                let state = downloads.get(entry.id);
                let installed = self.installed_path(entry).is_some();
                let downloading = state
                    .map(|state| !state.done && state.error.is_none())
                    .unwrap_or(false);
                let progress = state
                    .map(|state| {
                        if state.total > 0 {
                            state
                                .downloaded
                                .min(state.total)
                                .saturating_mul(100)
                                .checked_div(state.total)
                                .unwrap_or(0) as u8
                        } else if state.done {
                            100
                        } else {
                            0
                        }
                    })
                    .unwrap_or(0);
                SpeechModelView {
                    id: entry.id.to_string(),
                    file: entry.file.to_string(),
                    name: entry.name.to_string(),
                    description: entry.description.to_string(),
                    size_mb: entry.size_mb,
                    languages: entry.languages.to_string(),
                    accuracy: entry.accuracy,
                    speed: entry.speed,
                    engine: entry.engine.to_string(),
                    can_activate: true,
                    installed,
                    active: active.as_deref() == Some(entry.id),
                    downloading,
                    progress,
                    error: state.and_then(|state| state.error.clone()),
                }
            })
            .collect();
        (models, active)
    }

    /// Начать скачивание модели.
    pub async fn download(&self, id: &str) -> Result<(), String> {
        let entry = CATALOG
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "неизвестная модель".to_string())?;
        if self.installed_path(entry).is_some() {
            return Ok(());
        }
        {
            let downloads = self.downloads.lock().await;
            if let Some(state) = downloads.get(entry.id) {
                if !state.done && state.error.is_none() {
                    return Ok(());
                }
            }
        }
        let mut downloads = self.downloads.lock().await;
        downloads.insert(entry.id.to_string(), DownloadState::default());
        drop(downloads);

        let dir = self.models_dir();
        let state = self.downloads.clone();
        let entry = *entry;
        tokio::spawn(async move {
            let result = download_file(entry, &dir, state.clone()).await;
            let mut downloads = state.lock().await;
            if let Some(current) = downloads.get_mut(entry.id) {
                match result {
                    Ok(()) => {
                        current.done = true;
                        current.progress_done();
                    }
                    Err(error) => current.error = Some(error),
                }
            }
        });
        Ok(())
    }

    /// Сделать модель активной.
    pub async fn activate(&self, id: &str) -> Result<(), String> {
        let entry = CATALOG
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| "неизвестная модель".to_string())?;
        if self.installed_path(entry).is_none() {
            return Err("модель не скачана".to_string());
        }
        std::fs::create_dir_all(self.models_dir())
            .map_err(|error| format!("каталог моделей: {error}"))?;
        std::fs::write(self.active_file(), entry.file)
            .map_err(|error| format!("запись активной модели: {error}"))?;
        Ok(())
    }
}

impl DownloadState {
    /// Отметить завершение.
    fn progress_done(&mut self) {
        if self.total == 0 {
            self.total = 1;
            self.downloaded = 1;
        }
    }
}

/// Скачать файл с прогрессом.
async fn download_file(
    entry: CatalogEntry,
    dir: &Path,
    state: Arc<Mutex<HashMap<String, DownloadState>>>,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("каталог: {error}"))?;
    let response = reqwest::get(entry.url)
        .await
        .map_err(|error| format!("сеть: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let total = response
        .content_length()
        .unwrap_or(entry.size_mb * 1024 * 1024);
    {
        let mut downloads = state.lock().await;
        if let Some(current) = downloads.get_mut(entry.id) {
            current.total = total;
        }
    }
    let target = dir.join(entry.file);
    let tmp = dir.join(format!(".{}.part", entry.file));
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|error| format!("файл: {error}"))?;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("поток: {error}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("запись: {error}"))?;
        let mut downloads = state.lock().await;
        if let Some(current) = downloads.get_mut(entry.id) {
            current.downloaded = current.downloaded.saturating_add(chunk.len() as u64);
        }
    }
    file.flush()
        .await
        .map_err(|error| format!("сброс: {error}"))?;
    tokio::fs::rename(&tmp, &target)
        .await
        .map_err(|error| format!("переименование: {error}"))?;
    Ok(())
}
