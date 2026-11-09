//! Мастер первого запуска (Контракт 12: `/first-run/*`; доп. ТЗ tensa-core,
//! ADR-021). Обработчики — тонкая HTTP-обёртка над `tensa_core::FirstRunWizard`.
//!
//! Поведение по Контракту 12 (`teh_doc/contracts/pwa/openapi.yaml`):
//! `GET /first-run` → `{ is_first_run }`; шаги `POST /first-run/step/*` →
//! `204 No Content`; `POST /first-run/step/finish` → `WizardResult`
//! (`config_saved`, `secrets_saved`, `core_memory_initialized`).
//!
//! Расширения сверх минимума контракта (тела опциональны — requestBody в
//! контракте для этих шагов не определён):
//! - шаг 3 `{ download: bool }` — фоновое скачивание GGUF ассистента и
//!   Whisper Small q5_1 (прогресс — `GET /brain/status`, `GET /models/speech`);
//! - шаги 4/5 `validate: bool` (по умолчанию `true`) — живая проверка
//!   Telegram (getMe + тестовое сообщение) и SMTP до записи в сессию;
//! - шаг 8 `{ brain, openrouter_credential }` — после записи config.toml
//!   ядром: ключ OpenRouter — в secrets.toml, режим мозга — в config.
//!
//! Мастер — одна сессия на сервер: объект с накопленными ответами живёт в
//! `AppState.first_run`, создаётся при первом обращении к шагу и сбрасывается
//! после `finish` (поэтому `finish` без обязательных шагов даёт 400).
//! Секреты в телах уходят в `secrets.toml` и не логируются.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use tensa_core::{Config, EmailAccount, FirstRunWizard, Language, Secrets, WhatsAppConfig};
use tensa_tools::email::autodetect::SmtpTls;
use tensa_tools::email::connections::Account as SmtpAccount;
use tensa_tools::email::smtp;
use tensa_tools::telegram::TelegramClient;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Тело шага 1 «язык» (Контракт 12: `{ language }`).
#[derive(Debug, Deserialize)]
pub struct LanguageBody {
    /// Код языка: `ru` или `en`.
    pub language: String,
}

/// Тело шага 2 «знакомство» (Контракт 12: имя, обращение, секретарь, пояс).
#[derive(Debug, Deserialize)]
pub struct IntroductionBody {
    /// Имя владельца.
    #[serde(default)]
    pub owner_name: String,
    /// Обращение к владельцу.
    #[serde(default)]
    pub owner_title: String,
    /// Имя секретаря.
    #[serde(default)]
    pub secretary_name: String,
    /// Часовой пояс (IANA, например `Europe/Moscow`).
    #[serde(default)]
    pub timezone: String,
}

/// Тело шага 3 «модели» (опционально по Контракту 12).
#[derive(Debug, Deserialize)]
pub struct ModelsBody {
    /// Скачивать локальные модули: GGUF ассистента + Whisper Small q5_1.
    #[serde(default)]
    pub download: bool,
}

/// Тело шага 4 «Telegram» (Контракт 12: `{ bot_token, owner_chat_id }`).
#[derive(Debug, Deserialize)]
pub struct TelegramBody {
    /// Токен бота (секрет — уходит в secrets.toml, не логируется).
    #[serde(default)]
    pub bot_token: String,
    /// Chat ID владельца.
    #[serde(default)]
    pub owner_chat_id: String,
    /// Живая проверка: getMe + тестовое сообщение (по умолчанию — да).
    #[serde(default = "default_true")]
    pub validate: bool,
}

/// Тело шага 5 «почта» (доп. ТЗ tensa-core: аккаунт с адресом; секрет —
/// в `credential`, уходит в secrets.toml).
#[derive(Debug, Deserialize)]
pub struct EmailBody {
    /// Учётная запись почты.
    pub account: EmailAccount,
    /// Живая проверка SMTP (по умолчанию — да; IMAP — с Фазы 2).
    #[serde(default = "default_true")]
    pub validate: bool,
}

/// Конфигурация WhatsApp в теле шага 6 (имена полей — из UDL, Контракт 9).
#[derive(Debug, Deserialize)]
pub struct WhatsAppBodyConfig {
    /// Токен доступа (секрет — уходит в secrets.toml).
    #[serde(default)]
    pub access_token: String,
    /// Идентификатор номера.
    #[serde(default)]
    pub phone_number_id: String,
    /// Идентификатор WABA.
    #[serde(default)]
    pub waba_id: String,
}

/// Тело шага 6 «WhatsApp» (опционально по Контракту 12; `config: null` —
/// пропустить шаг).
#[derive(Debug, Deserialize)]
pub struct WhatsAppBody {
    /// Конфигурация Cloud API или null.
    #[serde(default)]
    pub config: Option<WhatsAppBodyConfig>,
}

/// Тело шага 8 «завершение» (опционально): модель ассистента и ключ.
#[derive(Debug, Deserialize)]
pub struct FinishBody {
    /// Модель ассистента: `local` или `cloud`.
    #[serde(default)]
    pub brain: Option<String>,
    /// Ключ OpenRouter (секрет — уходит в secrets.toml).
    #[serde(default)]
    pub openrouter_credential: Option<String>,
}

/// Дефолт для полей `validate` (живая проверка включена по умолчанию).
fn default_true() -> bool {
    true
}

/// `GET /first-run` — первый ли запуск (признак: `config.toml` отсутствует,
/// как в `TensaCore::is_first_run`).
pub async fn status(State(state): State<AppState>) -> Result<Json<Value>> {
    let is_first_run = !state.config_path.exists();
    Ok(Json(json!({ "is_first_run": is_first_run })))
}

/// Сессия мастера: существующая или только что созданная.
async fn session(state: &AppState) -> Result<Arc<FirstRunWizard>> {
    let mut slot = state.first_run.write().await;
    if let Some(wizard) = slot.as_ref() {
        return Ok(wizard.clone());
    }
    let node_id = if state.config_path.exists() {
        Config::load(&state.config_path)?.general.node_id
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    let secrets_path = tensa_core::TensaPaths::with_root(state.data_root.clone()).secrets_file();
    let wizard = Arc::new(
        FirstRunWizard::new(
            state.config_path.clone(),
            secrets_path,
            state.first_run_memory.clone(),
            node_id,
        )
        .await?,
    );
    *slot = Some(wizard.clone());
    Ok(wizard)
}

/// `POST /first-run/step/language` — шаг 1.
pub async fn step_language(
    State(state): State<AppState>,
    Json(body): Json<LanguageBody>,
) -> Result<StatusCode> {
    let language = match body.language.trim().to_lowercase().as_str() {
        "ru" => Language::Ru,
        "en" => Language::En,
        other => {
            return Err(ServerError::BadRequest(format!(
                "неизвестный язык «{other}»: ru | en"
            )))
        }
    };
    session(&state).await?.step_language(language).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/introduction` — шаг 2.
pub async fn step_introduction(
    State(state): State<AppState>,
    Json(body): Json<IntroductionBody>,
) -> Result<StatusCode> {
    session(&state)
        .await?
        .step_introduction(
            body.owner_name,
            body.owner_title,
            body.secretary_name,
            body.timezone,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/models` — шаг 3: фоновое скачивание локальных
/// модулей (GGUF ассистента + Whisper Small q5_1); прогресс —
/// `GET /brain/status` и `GET /models/speech`.
pub async fn step_models(
    State(state): State<AppState>,
    body: Option<Json<ModelsBody>>,
) -> Result<StatusCode> {
    if let Some(Json(body)) = body {
        if body.download {
            state
                .brain
                .download_local()
                .await
                .map_err(ServerError::Unavailable)?;
            state
                .speech_models
                .download("small-q5_1")
                .await
                .map_err(ServerError::Unavailable)?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/telegram` — шаг 4: живая проверка токена
/// (getMe + тестовое сообщение владельцу) и запись в сессию мастера.
pub async fn step_telegram(
    State(state): State<AppState>,
    Json(body): Json<TelegramBody>,
) -> Result<StatusCode> {
    if body.bot_token.trim().is_empty() {
        return Err(ServerError::BadRequest("нужен токен бота".to_string()));
    }
    if body.validate {
        let client = TelegramClient::new(body.bot_token.trim())
            .map_err(|error| ServerError::BadRequest(format!("Telegram: {error}")))?;
        client
            .get_me()
            .await
            .map_err(|error| ServerError::BadRequest(format!("Telegram: {error}")))?;
        let owner = if body.owner_chat_id.trim().is_empty() {
            crate::routes::telegram_setup::detect_chat(&client).await?
        } else {
            body.owner_chat_id.trim().to_string()
        };
        client
            .send_message(&owner, "Tensa: подключение настроено.", None)
            .await
            .map_err(|error| {
                ServerError::BadRequest(format!("Telegram: не могу написать в чат: {error}"))
            })?;
    }
    session(&state)
        .await?
        .step_telegram(body.bot_token, body.owner_chat_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/email` — шаг 5: живая проверка SMTP и запись
/// аккаунта в сессию мастера (IMAP-проверка — с Фазы 2).
pub async fn step_email(
    State(state): State<AppState>,
    body: Option<Json<EmailBody>>,
) -> Result<StatusCode> {
    let wizard = session(&state).await?;
    if let Some(Json(body)) = body {
        if body.validate {
            let account = &body.account;
            let credential = account
                .credential
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| ServerError::BadRequest("нужен пароль приложения".to_string()))?;
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
                .map_err(|error| ServerError::BadRequest(format!("SMTP: {error}")))?;
        }
        wizard.step_email(body.account).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/whatsapp` — шаг 6 (тело опционально; null — пропустить).
pub async fn step_whatsapp(
    State(state): State<AppState>,
    body: Option<Json<WhatsAppBody>>,
) -> Result<StatusCode> {
    let wizard = session(&state).await?;
    if let Some(Json(body)) = body {
        let config = body.config.map(|config| WhatsAppConfig {
            credential: config.access_token,
            phone_number_id: config.phone_number_id,
            waba_id: config.waba_id,
        });
        wizard.step_whatsapp(config).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/tailscale` — шаг 7.
pub async fn step_tailscale(State(state): State<AppState>) -> Result<StatusCode> {
    session(&state).await?.step_tailscale().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/finish` — шаг 8: запись config.toml и secrets.toml,
/// инициализация core memory; затем — модель ассистента и ключ OpenRouter
/// (Контракт 12: ответ — `WizardResult`).
pub async fn step_finish(
    State(state): State<AppState>,
    body: Option<Json<FinishBody>>,
) -> Result<Json<Value>> {
    // Ошибки выбора мозга — до записи файлов (валидация формата/наличия).
    if let Some(Json(body)) = &body {
        if body.brain.as_deref() == Some("cloud") {
            let provided = body
                .openrouter_credential
                .as_deref()
                .is_some_and(|key| key.len() >= 20 && key.starts_with("sk-or-"));
            let existing = Secrets::load(
                &tensa_core::TensaPaths::with_root(state.data_root.clone()).secrets_file(),
            )
            .unwrap_or_default()
            .openrouter
            .credential
            .is_some();
            if !provided && !existing {
                return Err(ServerError::BadRequest(
                    "для облачной модели нужен ключ OpenRouter (sk-or-…)".to_string(),
                ));
            }
        }
    }

    let wizard = session(&state).await?;
    let result = wizard.step_finish().await?;

    // Выбор модели ассистента: config.toml уже записан ядром.
    if let Some(Json(body)) = body {
        apply_brain(&state, body.brain, body.openrouter_credential).await;
    }
    // Сессия завершена: следующий finish начинается с пустого мастера.
    *state.first_run.write().await = None;
    // Успешный finish означает, что оба файла записаны ядром.
    Ok(Json(json!({
        "config_saved": true,
        "secrets_saved": true,
        "core_memory_initialized": result.core_memory_initialized,
    })))
}

/// Применить выбор модели ассистента: ключ — в secrets.toml (с валидацией
/// формата), режим — в config.toml. Ошибки активации не срывают финиш —
/// режим можно включить в «Настройках».
async fn apply_brain(state: &AppState, brain: Option<String>, credential: Option<String>) {
    if let Some(key) = credential
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if key.len() < 20 || !key.starts_with("sk-or-") {
            tracing::warn!("ключ OpenRouter не сохранён: неверный формат");
            return;
        }
        let paths = tensa_core::TensaPaths::with_root(state.data_root.clone());
        match Secrets::load(&paths.secrets_file())
            .map_err(|error| error.to_string())
            .and_then(|mut secrets| {
                secrets.openrouter.credential = Some(key.to_string());
                secrets
                    .save(&paths.secrets_file())
                    .map_err(|error| error.to_string())
            }) {
            Ok(()) => {}
            Err(error) => {
                tracing::warn!(error = %error, "ключ OpenRouter не сохранён");
                return;
            }
        }
    }
    match brain.as_deref() {
        Some("cloud") => {
            if let Err(error) = state.brain.activate_cloud() {
                tracing::warn!(error = %error, "облачная модель не активирована");
            }
        }
        Some("local") => {
            if let Err(_error) = state.brain.activate_local() {
                tracing::warn!("локальная модель ещё не скачана — активируйте в «Настройках»");
            }
        }
        _ => {}
    }
}
