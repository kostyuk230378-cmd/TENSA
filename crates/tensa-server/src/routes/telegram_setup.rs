//! Настройка Telegram-бота: проверка токена, chat_id владельца.
//!
//! Токен сохраняется только в `secrets.toml`; API не возвращает его обратно.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use tensa_core::{Secrets, TensaPaths};
use tensa_tools::telegram::TelegramClient;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Тело настройки Telegram.
#[derive(Debug, Deserialize)]
pub struct TelegramBody {
    /// Токен бота от @BotFather.
    pub credential: String,
    /// chat_id владельца (пусто — определить по последнему сообщению боту).
    #[serde(default)]
    pub owner_chat_id: Option<String>,
}

/// Тело определения chat_id.
#[derive(Debug, Deserialize)]
pub struct DetectBody {
    /// Токен бота.
    pub credential: String,
}

/// `GET /telegram/status` — настроен ли бот (без токена).
pub async fn status(State(state): State<AppState>) -> Result<Json<Value>> {
    let paths = TensaPaths::with_root(state.data_root.clone());
    let secrets = Secrets::load(&paths.secrets_file()).unwrap_or_default();
    let config = config_table(&state.config_path).unwrap_or_default();
    let owner = secrets
        .telegram
        .owner_chat_id
        .clone()
        .or_else(|| {
            config
                .get("telegram")
                .and_then(|table| table.get("owner_chat_id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    Ok(Json(json!({
        "configured": secrets.telegram.credential.is_some(),
        "owner_chat_id": owner,
    })))
}

/// `POST /telegram/detect` — узнать chat_id по последнему сообщению боту.
pub async fn detect(Json(body): Json<DetectBody>) -> Result<Json<Value>> {
    let client = TelegramClient::new(body.credential.trim())
        .map_err(|error| ServerError::BadRequest(format!("Telegram: {error}")))?;
    let chat_id = detect_chat(&client).await?;
    Ok(Json(json!({ "chat_id": chat_id })))
}

/// `POST /telegram` — проверить токен и сохранить настройки.
pub async fn save(
    State(state): State<AppState>,
    Json(body): Json<TelegramBody>,
) -> Result<Json<Value>> {
    let credential = body.credential.trim().to_string();
    if credential.is_empty() {
        return Err(ServerError::BadRequest("нужен токен бота".to_string()));
    }
    let client = TelegramClient::new(credential.clone())
        .map_err(|error| ServerError::BadRequest(format!("Telegram: {error}")))?;
    let me = client
        .get_me()
        .await
        .map_err(|error| ServerError::BadRequest(format!("Telegram: {error}")))?;
    let owner = match body.owner_chat_id.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => value,
        _ => detect_chat(&client).await?,
    };
    client
        .send_message(&owner, "Tensa: подключение настроено.", None)
        .await
        .map_err(|error| {
            ServerError::BadRequest(format!("Telegram: не могу написать в чат: {error}"))
        })?;

    let paths = TensaPaths::with_root(state.data_root.clone());
    let mut secrets = Secrets::load(&paths.secrets_file()).unwrap_or_default();
    secrets.telegram.credential = Some(credential);
    secrets.telegram.owner_chat_id = Some(owner.clone());
    secrets
        .save(&paths.secrets_file())
        .map_err(|error| ServerError::Storage(format!("secrets.toml: {error}")))?;

    let mut config = config_table(&state.config_path)?;
    let telegram = config
        .entry("telegram")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| ServerError::Storage("config.toml: [telegram] не таблица".to_string()))?;
    telegram.insert(
        "owner_chat_id".to_string(),
        toml::Value::String(owner.clone()),
    );
    save_table(&state.config_path, &config)?;

    Ok(Json(json!({
        "status": "ok",
        "owner_chat_id": owner,
        "bot_username": me.username,
    })))
}

async fn detect_chat(client: &TelegramClient) -> Result<String> {
    let updates = client
        .get_updates(None, 0)
        .await
        .map_err(|error| ServerError::BadRequest(format!("Telegram: {error}")))?;
    let chat_id = updates
        .iter()
        .rev()
        .find_map(|update| update.message.as_ref().map(|message| message.chat.id))
        .ok_or_else(|| {
            ServerError::BadRequest(
                "не вижу сообщений: напишите боту /start и повторите".to_string(),
            )
        })?;
    Ok(chat_id.to_string())
}

fn config_table(path: &std::path::Path) -> Result<toml::Table> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))?;
    toml::from_str(&raw).map_err(|error| ServerError::Storage(format!("config.toml: {error}")))
}

fn save_table(path: &std::path::Path, table: &toml::Table) -> Result<()> {
    let body = toml::to_string_pretty(table)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))?;
    std::fs::write(path, body)
        .map_err(|error| ServerError::Storage(format!("config.toml: {error}")))
}
