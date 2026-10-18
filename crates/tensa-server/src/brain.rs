//! Модель ассистента: локальная Gemma GGUF или OpenRouter.
//!
//! Локальный файл скачивается по скрытой ссылке, проверяется SHA256 и кладётся
//! в `<root>/models`. Ключ OpenRouter сохраняется в `secrets.toml`, модель —
//! `google/gemma-4-26b-a4b-it`; QueryService перезагружается вызывающим кодом.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use tensa_core::{Secrets, TensaPaths};

/// Имя локального GGUF.
pub const LOCAL_FILE: &str = "gemma-4-E2B-it-XS-TS-Q3_K_S.gguf";
/// Скрытая ссылка на локальную модель (репозиторий, не прямая страница файла).
const LOCAL_URL: &str =
    "https://huggingface.co/TheStageAI/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-XS-TS-Q3_K_S.gguf";
/// SHA256 локального файла.
const LOCAL_SHA256: &str = "d1e358e0a9f945084e8757090684ef698f9e99f41693dcd465f5aeaa3564c3b5";
/// Модель OpenRouter.
pub const CLOUD_MODEL: &str = "google/gemma-4-26b-a4b-it";

#[derive(Clone, Default)]
struct DownloadState {
    downloaded: u64,
    total: u64,
    error: Option<String>,
    done: bool,
}

/// Сервис моделей ассистента.
pub struct BrainService {
    root: PathBuf,
    models_dir: PathBuf,
    config_path: PathBuf,
    download: Arc<Mutex<DownloadState>>,
}

impl BrainService {
    /// Сервис для корня данных и config.toml.
    pub fn new(root: &Path, config_path: PathBuf) -> Self {
        let models_dir = install_models_dir().unwrap_or_else(|| root.join("models"));
        Self {
            root: root.to_path_buf(),
            models_dir,
            config_path,
            download: Arc::new(Mutex::new(DownloadState::default())),
        }
    }

    /// Заглушка для тестов.
    pub fn disabled() -> Self {
        Self {
            root: PathBuf::new(),
            models_dir: PathBuf::new(),
            config_path: PathBuf::new(),
            download: Arc::new(Mutex::new(DownloadState::default())),
        }
    }

    /// Путь локального GGUF.
    pub fn local_path(&self) -> PathBuf {
        self.models_dir.join(LOCAL_FILE)
    }

    /// Статус локальной и облачной модели.
    pub async fn status(&self) -> Value {
        let path = self.local_path();
        let installed = path.is_file();
        let size_mb = std::fs::metadata(&path)
            .map(|meta| meta.len() / (1024 * 1024))
            .unwrap_or(0);
        let download = self.download.lock().await.clone();
        let progress = if download.total > 0 {
            download
                .downloaded
                .min(download.total)
                .saturating_mul(100)
                .checked_div(download.total)
                .unwrap_or(0) as u8
        } else if download.done {
            100
        } else {
            0
        };
        let local = json!({
            "installed": installed,
            "name": LOCAL_FILE,
            "size_mb": size_mb,
            "downloading": !download.done && download.error.is_none() && download.total > 0,
            "progress": progress,
            "error": download.error,
        });

        let secrets = Secrets::load(&TensaPaths::with_root(self.root.clone()).secrets_file()).ok();
        let configured = secrets
            .as_ref()
            .and_then(|secrets| secrets.openrouter_key())
            .is_some();
        let mode = self.mode();
        let active = if mode == "cloud" && configured {
            "cloud"
        } else if installed {
            "local"
        } else if configured {
            "cloud"
        } else {
            "none"
        };
        json!({
            "local": local,
            "cloud": {
                "configured": configured,
                "model": CLOUD_MODEL,
            },
            "mode": mode,
            "active": active,
        })
    }

    /// Запустить скачивание локальной модели.
    pub async fn download_local(&self) -> Result<(), String> {
        if self.local_path().is_file() {
            return Ok(());
        }
        {
            let state = self.download.lock().await;
            if !state.done && state.error.is_none() && state.total > 0 {
                return Ok(());
            }
        }
        *self.download.lock().await = DownloadState::default();
        let target = self.local_path();
        let state = self.download.clone();
        tokio::spawn(async move {
            let result = download_file(&target, state.clone()).await;
            let mut current = state.lock().await;
            match result {
                Ok(()) => {
                    current.done = true;
                    if current.total == 0 {
                        current.total = 1;
                        current.downloaded = 1;
                    }
                }
                Err(error) => current.error = Some(error),
            }
        });
        Ok(())
    }

    /// Сделать локальную модель активной.
    pub fn activate_local(&self) -> Result<(), String> {
        if !self.local_path().is_file() {
            return Err("локальная модель не скачана".to_string());
        }
        self.write_config("local", false)
    }

    /// Включить облачную модель с уже сохранённым ключом.
    pub fn activate_cloud(&self) -> Result<(), String> {
        let secrets = Secrets::load(&TensaPaths::with_root(self.root.clone()).secrets_file())
            .map_err(|error| format!("secrets.toml: {error}"))?;
        if secrets.openrouter_key().is_none() {
            return Err("ключ OpenRouter не задан".to_string());
        }
        self.write_config("cloud", true)
    }

    /// Сохранить ключ OpenRouter и включить облачную модель.
    pub fn connect_cloud(&self, credential: &str) -> Result<(), String> {
        let key = credential.trim();
        if key.len() < 20 || !key.starts_with("sk-or-") {
            return Err("ключ OpenRouter должен начинаться с sk-or-".to_string());
        }
        let paths = TensaPaths::with_root(self.root.clone());
        let mut secrets = Secrets::load(&paths.secrets_file())
            .map_err(|error| format!("secrets.toml: {error}"))?;
        secrets.openrouter.credential = Some(key.to_string());
        secrets
            .save(&paths.secrets_file())
            .map_err(|error| format!("сохранение ключа: {error}"))?;
        self.write_config("cloud", true)
    }

    fn mode(&self) -> String {
        self.config_table()
            .get("brain")
            .and_then(|brain| brain.get("mode"))
            .and_then(|mode| mode.as_str())
            .unwrap_or("hybrid")
            .to_string()
    }

    fn config_table(&self) -> toml::Table {
        std::fs::read_to_string(&self.config_path)
            .ok()
            .and_then(|raw| toml::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn write_config(&self, mode: &str, cloud: bool) -> Result<(), String> {
        let raw = std::fs::read_to_string(&self.config_path)
            .map_err(|error| format!("config.toml: {error}"))?;
        let mut table: toml::Table =
            toml::from_str(&raw).map_err(|error| format!("config.toml: {error}"))?;
        let mut brain = table
            .remove("brain")
            .and_then(|value| match value {
                toml::Value::Table(table) => Some(table),
                _ => None,
            })
            .unwrap_or_default();
        brain.insert("mode".to_string(), toml::Value::String(mode.to_string()));
        brain.insert(
            "local_model".to_string(),
            toml::Value::String(self.local_path().display().to_string()),
        );
        if cloud {
            brain.insert(
                "cloud_models".to_string(),
                toml::Value::Array(vec![toml::Value::String(CLOUD_MODEL.to_string())]),
            );
        }
        table.insert("brain".to_string(), toml::Value::Table(brain));
        let body =
            toml::to_string_pretty(&table).map_err(|error| format!("config.toml: {error}"))?;
        std::fs::write(&self.config_path, body)
            .map_err(|error| format!("запись config.toml: {error}"))
    }
}

/// Каталог моделей рядом с установленным exe (не зависит от машины).
fn install_models_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("models")))
}

/// Скачать файл, проверить SHA256 и переименовать `.part`.
async fn download_file(target: &Path, state: Arc<Mutex<DownloadState>>) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("каталог: {error}"))?;
    }
    let response = reqwest::get(LOCAL_URL)
        .await
        .map_err(|error| format!("сеть: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    {
        let mut current = state.lock().await;
        current.total = total;
    }
    let tmp = target.with_extension("gguf.part");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|error| format!("файл: {error}"))?;
    let mut hasher = Sha256::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("поток: {error}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("запись: {error}"))?;
        hasher.update(&chunk);
        let mut current = state.lock().await;
        current.downloaded = current.downloaded.saturating_add(chunk.len() as u64);
    }
    file.flush()
        .await
        .map_err(|error| format!("сброс: {error}"))?;
    let digest = format!("{:x}", hasher.finalize());
    if digest != LOCAL_SHA256 {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(format!("SHA256 не совпал: {digest}"));
    }
    tokio::fs::rename(&tmp, target)
        .await
        .map_err(|error| format!("переименование: {error}"))?;
    Ok(())
}
