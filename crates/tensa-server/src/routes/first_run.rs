//! Мастер первого запуска (Контракт 12: `/first-run/*`; доп. ТЗ tensa-core,
//! ADR-021). Обработчики — тонкая HTTP-обёртка над `tensa_core::FirstRunWizard`.
//!
//! Поведение по Контракту 12 (`teh_doc/contracts/pwa/openapi.yaml`):
//! `GET /first-run` → `{ is_first_run }`; шаги `POST /first-run/step/*` →
//! `204 No Content`; `POST /first-run/step/finish` → `WizardResult`
//! (`config_saved`, `secrets_saved`, `core_memory_initialized`).
//!
//! Мастер — одна сессия на сервер: объект с накопленными ответами живёт в
//! `AppState.first_run`, создаётся при первом обращении к шагу и сбрасывается
//! после `finish` (поэтому `finish` без обязательных шагов даёт 400).
//!
//! Контракт 12 не определяет requestBody для шагов почты и WhatsApp — тела
//! принимаются опционально (формат — из доп. ТЗ tensa-core; секреты в теле
//! уходят в `secrets.toml` и не логируются).

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use tensa_core::{Config, EmailAccount, FirstRunWizard, Language, WhatsAppConfig};

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

/// Тело шага 4 «Telegram» (Контракт 12: `{ bot_token, owner_chat_id }`).
#[derive(Debug, Deserialize)]
pub struct TelegramBody {
    /// Токен бота (секрет — уходит в secrets.toml, не логируется).
    #[serde(default)]
    pub bot_token: String,
    /// Chat ID владельца.
    #[serde(default)]
    pub owner_chat_id: String,
}

/// Тело шага 5 «почта» (доп. ТЗ tensa-core: аккаунт с адресом; секрет —
/// в `credential`, уходит в secrets.toml).
#[derive(Debug, Deserialize)]
pub struct EmailBody {
    /// Учётная запись почты.
    pub account: EmailAccount,
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
    let secrets_path = crate::state::secrets_path(&state.data_root);
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

/// `POST /first-run/step/models` — шаг 3 (согласие на загрузку моделей).
pub async fn step_models(State(state): State<AppState>) -> Result<StatusCode> {
    session(&state).await?.step_models().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/telegram` — шаг 4.
pub async fn step_telegram(
    State(state): State<AppState>,
    Json(body): Json<TelegramBody>,
) -> Result<StatusCode> {
    session(&state)
        .await?
        .step_telegram(body.bot_token, body.owner_chat_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /first-run/step/email` — шаг 5 (тело опционально; без тела шаг
/// просто не добавляет аккаунт — Контракт 12 не определяет requestBody).
pub async fn step_email(
    State(state): State<AppState>,
    body: Option<Json<EmailBody>>,
) -> Result<StatusCode> {
    let wizard = session(&state).await?;
    if let Some(Json(body)) = body {
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
/// инициализация core memory (Контракт 12: ответ — `WizardResult`).
pub async fn step_finish(State(state): State<AppState>) -> Result<Json<Value>> {
    let wizard = session(&state).await?;
    let result = wizard.step_finish().await?;
    // Сессия завершена: следующий finish начинается с пустого мастера.
    *state.first_run.write().await = None;
    // Успешный finish означает, что оба файла записаны ядром (иначе — ошибка).
    Ok(Json(json!({
        "config_saved": true,
        "secrets_saved": true,
        "core_memory_initialized": result.core_memory_initialized,
    })))
}
