//! Tauri-обёртка десктопа (Контракт 10): IPC-команды для Svelte-фронтенда.
//!
//! Фаза 1: мастер первого запуска — `is_first_run`, `run_first_run_wizard`
//! и `wizard_step_*` по списку доп. ТЗ tensa-core. Ошибки отдаются строкой
//! (форма Контракта 10: `Result<T, String>`); секреты в аргументах команд
//! уходят в `secrets.toml` и не логируются.
//!
//! Расширения: шаги 4/5 выполняют живые проверки (Telegram getMe +
//! тестовое сообщение, SMTP) до записи в сессию; шаг 8 принимает выбор
//! модели ассистента (`local`/`cloud`) и ключ OpenRouter — применяются
//! после записи config.toml ядром. Скачивание моделей из Tauri-мастера
//! недоступно (сервисы в tensa-server) — модули ставятся из «Настроек»
//! PWA после установки.
//!
//! Остальные команды Контракта 10 (approvals, notes, query, ...) добавляются
//! по мере реализации фаз плана завершения.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tensa_core::{
    Config, EmailAccount, FirstRunWizard, Language, MemoryInitializer, Secrets, TensaPaths,
};
use tensa_memory::{
    CoreMemoryConfig, DefaultMemoryManager, HashEmbedder, MemoryManagerConfig,
    SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};
use tensa_tools::email::autodetect::SmtpTls;
use tensa_tools::email::connections::Account as SmtpAccount;
use tensa_tools::email::smtp;
use tensa_tools::telegram::TelegramClient;

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
    finish(&state, None, None).await
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

/// Шаг 3: согласие на загрузку моделей.
///
/// Скачивание из Tauri-мастера недоступно (сервисы загрузки живут в
/// HTTP-бэкенде): модули ставятся из «Настроек» PWA после установки.
#[tauri::command]
async fn wizard_step_models(state: tauri::State<'_, AppState>) -> Result<(), String> {
    tracing::info!("мастер: согласие на модели сохранено; скачивание — из «Настроек» PWA");
    session(&state)
        .await?
        .step_models()
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 4: Telegram-бот — живая проверка (getMe + тестовое сообщение),
/// токен — в secrets.toml.
#[tauri::command]
async fn wizard_step_telegram(
    state: tauri::State<'_, AppState>,
    bot_token: String,
    owner_chat_id: String,
    validate: Option<bool>,
) -> Result<(), String> {
    if bot_token.trim().is_empty() {
        return Err("нужен токен бота".to_string());
    }
    if validate != Some(false) {
        let client =
            TelegramClient::new(bot_token.trim()).map_err(|error| format!("Telegram: {error}"))?;
        client
            .get_me()
            .await
            .map_err(|error| format!("Telegram: {error}"))?;
        let owner = if owner_chat_id.trim().is_empty() {
            return Err("укажите chat id владельца (или напишите боту /start)".to_string());
        } else {
            owner_chat_id.trim().to_string()
        };
        client
            .send_message(&owner, "Tensa: подключение настроено.", None)
            .await
            .map_err(|error| format!("Telegram: не могу написать в чат: {error}"))?;
    }
    session(&state)
        .await?
        .step_telegram(bot_token, owner_chat_id)
        .await
        .map_err(|error| error.to_string())
}

/// Шаг 5: почта — живая проверка SMTP, аккаунт — в config, секрет — в
/// secrets.toml (IMAP-проверка — с Фазы 2).
#[tauri::command]
async fn wizard_step_email(
    state: tauri::State<'_, AppState>,
    account: EmailAccount,
    validate: Option<bool>,
) -> Result<(), String> {
    if validate != Some(false) {
        let credential = account
            .credential
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "нужен пароль приложения".to_string())?;
        let smtp_account = SmtpAccount {
            id: "test".to_string(),
            email: account.email.clone(),
            label: account.label.clone(),
            imap_host: account.imap_host.clone(),
            imap_port: account.imap_port,
            smtp_host: account.smtp_host.clone(),
            smtp_port: account.smtp_port,
            smtp_tls: match account.smtp_port {
                465 => SmtpTls::Implicit,
                _ => SmtpTls::StartTls,
            },
        };
        smtp::test_connection(&smtp_account, credential.trim())
            .await
            .map_err(|error| format!("SMTP: {error}"))?;
    }
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

/// Шаг 8: завершение — config.toml, secrets.toml, core memory; затем
/// модель ассистента (`local`/`cloud`) и ключ OpenRouter.
#[tauri::command]
async fn wizard_step_finish(
    state: tauri::State<'_, AppState>,
    brain: Option<String>,
    openrouter_credential: Option<String>,
) -> Result<WizardResult, String> {
    finish(&state, brain, openrouter_credential).await
}

/// Общая логика завершения: записать файлы, инициализировать память,
/// применить выбор мозга, сбросить сессию.
async fn finish(
    state: &tauri::State<'_, AppState>,
    brain: Option<String>,
    openrouter_credential: Option<String>,
) -> Result<WizardResult, String> {
    let wizard = session(state).await?;
    let result = wizard
        .step_finish()
        .await
        .map_err(|error| error.to_string())?;
    apply_brain(brain, openrouter_credential).await?;
    *slot(state) = None;
    // Успешный finish означает, что оба файла записаны ядром.
    Ok(WizardResult {
        config_saved: true,
        secrets_saved: true,
        core_memory_initialized: result.core_memory_initialized,
    })
}

/// Применить выбор модели ассистента: ключ — в secrets.toml (валидация
/// формата), режим мозга — в config.toml (файл уже записан ядром).
async fn apply_brain(
    brain: Option<String>,
    openrouter_credential: Option<String>,
) -> Result<(), String> {
    let paths = TensaPaths::new();
    if let Some(key) = openrouter_credential
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if key.len() < 20 || !key.starts_with("sk-or-") {
            return Err("ключ OpenRouter должен начинаться с sk-or-".to_string());
        }
        let mut secrets = Secrets::load(&paths.secrets_file())
            .map_err(|error| format!("secrets.toml: {error}"))?;
        secrets.openrouter.credential = Some(key.to_string());
        secrets
            .save(&paths.secrets_file())
            .map_err(|error| format!("secrets.toml: {error}"))?;
    }
    let mode = match brain.as_deref() {
        Some("cloud") | Some("local") => brain.expect("проверено выше"),
        _ => return Ok(()),
    };
    let mut config = Config::load(&paths.config_file()).map_err(|error| error.to_string())?;
    config
        .brain
        .insert("mode".to_string(), toml::Value::String(mode.to_string()));
    config
        .save(&paths.config_file())
        .map_err(|error| error.to_string())
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
