//! Tauri-обёртка десктопа (Контракт 10): IPC-команды для Svelte-фронтенда.
//!
//! Фаза 1: мастер первого запуска — `is_first_run`, `run_first_run_wizard`
//! и `wizard_step_*` по списку доп. ТЗ tensa-core. Ошибки отдаются строкой
//! (форма Контракта 10: `Result<T, String>`); секреты в аргументах команд
//! уходят в `secrets.toml` и не логируются.
//!
//! Остальные команды Контракта 10 (approvals, notes, query, ...) добавляются
//! по мере реализации фаз плана завершения.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tensa_core::{Config, FirstRunWizard, Language, MemoryInitializer, TensaPaths};
use tensa_memory::{
    CoreMemoryConfig, DefaultMemoryManager, HashEmbedder, MemoryManagerConfig,
    SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};

/// Состояние приложения: сессия мастера первого запуска (создаётся лениво
/// на первом шаге, сбрасывается после завершения — как в HTTP-бэкенде).
pub struct AppState {
    wizard: Mutex<Option<Arc<FirstRunWizard>>>,
}

/// Результат мастера для IPC (доп. ТЗ tensa-core: `WizardResult`).
#[derive(serde::Serialize)]
pub struct WizardResult {
    /// config.toml записан.
    pub config_saved: bool,
    /// secrets.toml записан.
    pub secrets_saved: bool,
    /// Core memory инициализирована.
    pub core_memory_initialized: bool,
}

/// Первый ли запуск (доп. ТЗ: признак — отсутствие config.toml).
#[tauri::command]
async fn is_first_run() -> Result<bool, String> {
    Ok(!TensaPaths::new().config_file().exists())
}

/// Слот сессии мастера (устойчиво к отравлению Mutex).
fn slot<'a>(
    state: &'a tauri::State<'_, AppState>,
) -> std::sync::MutexGuard<'a, Option<Arc<FirstRunWizard>>> {
    state
        .wizard
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Текущая сессия мастера: существующая или только что созданная.
async fn session(state: &tauri::State<'_, AppState>) -> Result<Arc<FirstRunWizard>, String> {
    let existing = slot(state).clone();
    if let Some(wizard) = existing {
        return Ok(wizard);
    }
    let wizard = Arc::new(build_wizard().await?);
    *slot(state) = Some(wizard.clone());
    Ok(wizard)
}

/// Собрать мастер: пути `~/.tensa/`, инициализатор core memory из
/// `tensa-memory` (SQLite-хранилища; HashEmbedder для архива — как в CLI).
async fn build_wizard() -> Result<FirstRunWizard, String> {
    let paths = TensaPaths::new();
    paths.ensure_dirs().map_err(|error| error.to_string())?;
    let config_path = paths.config_file();
    let node_id = if config_path.exists() {
        Config::load(&config_path)
            .map_err(|error| error.to_string())?
            .general
            .node_id
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    let root = paths.root();
    let core_store = SqliteCoreMemoryStore::new(
        &root.join("memory.db"),
        CoreMemoryConfig::default(),
        node_id.clone(),
    )
    .await
    .map_err(|error| error.to_string())?;
    let recall_store = SqliteRecallMemoryStore::new(&root.join("memory.db"))
        .await
        .map_err(|error| error.to_string())?;
    let archival_store = SqliteArchivalMemoryStore::new(
        &paths.memory_dir().join("archival.db"),
        Arc::new(HashEmbedder::new(384)),
    )
    .await
    .map_err(|error| error.to_string())?;
    let manager = DefaultMemoryManager::new(
        Arc::new(core_store),
        Arc::new(recall_store),
        Arc::new(archival_store),
        MemoryManagerConfig::default(),
        node_id.clone(),
    )
    .await
    .map_err(|error| error.to_string())?;
    let memory: Arc<dyn MemoryInitializer> = Arc::new(manager);
    FirstRunWizard::new(config_path, paths.secrets_file(), Some(memory), node_id)
        .await
        .map_err(|error| error.to_string())
}

/// Запустить мастер до конца с накопленными ответами
/// (доп. ТЗ: `run_first_run_wizard() -> WizardResult`).
#[tauri::command]
async fn run_first_run_wizard(state: tauri::State<'_, AppState>) -> Result<WizardResult, String> {
    finish(&state).await
}

/// Шаг 1: язык интерфейса (`ru` | `en`).
#[tauri::command]
async fn wizard_step_language(
    state: tauri::State<'_, AppState>,
    language: String,
) -> Result<(), String> {
    let language = match language.trim().to_lowercase().as_str() {
        "ru" => Language::Ru,
        "en" => Language::En,
        other => return Err(format!("неизвестный язык «{other}»: ru | en")),
    };
    session(&state)
        .await?
        .step_language(language)
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 2: знакомство (имя владельца, обращение, имя секретаря, пояс).
#[tauri::command]
async fn wizard_step_introduction(
    state: tauri::State<'_, AppState>,
    owner_name: String,
    owner_title: String,
    secretary_name: String,
    timezone: String,
) -> Result<(), String> {
    session(&state)
        .await?
        .step_introduction(owner_name, owner_title, secretary_name, timezone)
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 3: загрузка моделей (согласие).
#[tauri::command]
async fn wizard_step_models(state: tauri::State<'_, AppState>) -> Result<(), String> {
    session(&state)
        .await?
        .step_models()
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 4: Telegram-бот (токен — в secrets.toml).
#[tauri::command]
async fn wizard_step_telegram(
    state: tauri::State<'_, AppState>,
    bot_token: String,
    owner_chat_id: String,
) -> Result<(), String> {
    session(&state)
        .await?
        .step_telegram(bot_token, owner_chat_id)
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 5: почта (аккаунт — в config.toml, секрет — в secrets.toml).
#[tauri::command]
async fn wizard_step_email(
    state: tauri::State<'_, AppState>,
    account: tensa_core::EmailAccount,
) -> Result<(), String> {
    session(&state)
        .await?
        .step_email(account)
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 6: WhatsApp (опционально; null — пропустить).
#[tauri::command]
async fn wizard_step_whatsapp(
    state: tauri::State<'_, AppState>,
    config: Option<tensa_core::WhatsAppConfig>,
) -> Result<(), String> {
    session(&state)
        .await?
        .step_whatsapp(config)
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 7: Tailscale.
#[tauri::command]
async fn wizard_step_tailscale(state: tauri::State<'_, AppState>) -> Result<(), String> {
    session(&state)
        .await?
        .step_tailscale()
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 8: завершение (config.toml + secrets.toml + core memory).
#[tauri::command]
async fn wizard_step_finish(state: tauri::State<'_, AppState>) -> Result<WizardResult, String> {
    finish(&state).await
}

/// Общая логика завершения: записать файлы, инициализировать память,
/// сбросить сессию.
async fn finish(state: &tauri::State<'_, AppState>) -> Result<WizardResult, String> {
    let wizard = session(state).await?;
    let result = wizard
        .step_finish()
        .await
        .map_err(|error| error.to_string())?;
    *slot(state) = None;
    // Успешный finish означает, что оба файла записаны ядром.
    Ok(WizardResult {
        config_saved: true,
        secrets_saved: true,
        core_memory_initialized: result.core_memory_initialized,
    })
}

/// Запуск Tauri-приложения (вызывается из `main.rs`; ошибки — значением).
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .manage(AppState {
            wizard: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            is_first_run,
            run_first_run_wizard,
            wizard_step_language,
            wizard_step_introduction,
            wizard_step_models,
            wizard_step_telegram,
            wizard_step_email,
            wizard_step_whatsapp,
            wizard_step_tailscale,
            wizard_step_finish
        ])
        .run(tauri::generate_context!())
}

/// Путь к config.toml по умолчанию (для UI-подсказок и тестов).
pub fn default_config_path() -> PathBuf {
    TensaPaths::new().config_file()
}
