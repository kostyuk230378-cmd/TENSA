//! Клиент Bot API (ТЗ tensa-tools: `telegram/src/bot.rs`).
//!
//! [derived] Вместо teloxide (из плана разведки) — тонкий клиент Bot API
//! поверх `reqwest` (уже в воркспейсе): принцип ТЗ «минимум зависимостей»,
//! риск версионного API фреймворка отсутствует, схема методов стабильна.
//! Токен живёт в URL и в журнал не попадает (`Debug` не выводится).
//!
//! Реализованы методы, нужные секретарю: `getMe`, `sendMessage`,
//! `editMessageText`, `answerCallbackQuery`, `getUpdates` (long polling).

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::error::TelegramError;

/// База Bot API.
const API_BASE: &str = "https://api.telegram.org";

/// Клиент Bot API с заданным токеном бота.
pub struct TelegramClient {
    credential: String,
    http: reqwest::Client,
}

impl TelegramClient {
    /// Собрать клиент по токену бота.
    pub fn new(credential: impl Into<String>) -> Result<Self, TelegramError> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(TelegramError::from)?;
        Ok(Self {
            credential: credential.into(),
            http,
        })
    }

    /// URL метода Bot API (токен — секрет: наружу не логируется).
    fn url(&self, method: &str) -> String {
        format!("{API_BASE}/bot{}/{method}", self.credential)
    }

    /// Вызвать метод и разобрать конверт `{ ok, result }`.
    async fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        body: Option<&Value>,
        timeout: Option<Duration>,
    ) -> Result<T, TelegramError> {
        let mut request = self.http.post(self.url(method));
        if let Some(body) = body {
            request = request.json(body);
        }
        if let Some(timeout) = timeout {
            request = request.timeout(timeout);
        }

        let response = request.send().await.map_err(TelegramError::from)?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(TelegramError::from)?;

        let envelope: ApiEnvelope<T> =
            serde_json::from_str(&text).map_err(|error| TelegramError::Protocol {
                status,
                detail: format!("конверт Bot API не разобран: {error}"),
            })?;
        if !envelope.ok {
            return Err(TelegramError::Api {
                status,
                description: envelope.description.unwrap_or_default(),
            });
        }
        envelope.result.ok_or_else(|| TelegramError::Protocol {
            status,
            detail: "в конверте нет result".to_string(),
        })
    }

    /// `getMe` — сведения о боте (проверка токена).
    pub async fn get_me(&self) -> Result<User, TelegramError> {
        self.call("getMe", None, None).await
    }

    /// `sendMessage` — отправить текст (без parse_mode: чужой разметкой не рискуем).
    pub async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_markup: Option<Value>,
    ) -> Result<Message, TelegramError> {
        let mut body = serde_json::json!({ "chat_id": chat_id, "text": text });
        if let Some(markup) = reply_markup {
            body["reply_markup"] = markup;
        }
        self.call("sendMessage", Some(&body), None).await
    }

    /// `editMessageText` — заменить текст сообщения (после решения по кнопке).
    pub async fn edit_message_text(
        &self,
        chat_id: &str,
        message_id: i64,
        text: &str,
    ) -> Result<Value, TelegramError> {
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
        });
        self.call("editMessageText", Some(&body), None).await
    }

    /// `answerCallbackQuery` — погасить «часики» у нажатой кнопки.
    pub async fn answer_callback_query(
        &self,
        callback_query_id: &str,
        text: Option<&str>,
    ) -> Result<Value, TelegramError> {
        let mut body = serde_json::json!({ "callback_query_id": callback_query_id });
        if let Some(text) = text {
            body["text"] = Value::String(text.to_string());
        }
        self.call("answerCallbackQuery", Some(&body), None).await
    }

    /// `getUpdates` — long polling (timeout держим чуть выше серверного).
    pub async fn get_updates(
        &self,
        offset: Option<i64>,
        timeout_secs: u64,
    ) -> Result<Vec<Update>, TelegramError> {
        let mut body = serde_json::json!({
            "timeout": timeout_secs,
            "allowed_updates": ["message", "callback_query"],
        });
        if let Some(offset) = offset {
            body["offset"] = serde_json::json!(offset);
        }
        self.call(
            "getUpdates",
            Some(&body),
            Some(Duration::from_secs(timeout_secs + 15)),
        )
        .await
    }
}

/// Конверт ответа Bot API.
///
/// `Option`-поля сериализуются как необязательные и без `#[serde(default)]`
/// (serde сам трактует отсутствующее значение как `None`); явный `default`
/// навесил бы лишнее требование `T: Default` на дженерик.
#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

/// Обновление из `getUpdates`.
#[derive(Debug, Clone, Deserialize)]
pub struct Update {
    /// Идентификатор обновления (для `offset`).
    pub update_id: i64,
    /// Сообщение, если пришло.
    #[serde(default)]
    pub message: Option<Message>,
    /// Нажатие inline-кнопки, если пришло.
    #[serde(default)]
    pub callback_query: Option<CallbackQuery>,
}

/// Сообщение.
#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    /// Идентификатор сообщения.
    pub message_id: i64,
    /// Чат.
    pub chat: Chat,
    /// Отправитель (у сообщений из чатов — есть).
    #[serde(default)]
    pub from: Option<User>,
    /// Текст, если это текст.
    #[serde(default)]
    pub text: Option<String>,
}

/// Чат.
#[derive(Debug, Clone, Deserialize)]
pub struct Chat {
    /// Числовой идентификатор чата.
    pub id: i64,
    /// Тип чата (`private`, `group`, ...).
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// Пользователь.
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    /// Числовой идентификатор.
    pub id: i64,
    /// Имя пользователя, если есть.
    #[serde(default)]
    pub username: Option<String>,
    /// Имя, если есть.
    #[serde(default)]
    pub first_name: Option<String>,
}

/// Нажатие inline-кнопки.
#[derive(Debug, Clone, Deserialize)]
pub struct CallbackQuery {
    /// Идентификатор запроса (для `answerCallbackQuery`).
    pub id: String,
    /// Данные кнопки (`approve:<uuid>` / `reject:<uuid>`).
    #[serde(default)]
    pub data: Option<String>,
    /// Сообщение, под которым нажата кнопка.
    #[serde(default)]
    pub message: Option<Message>,
    /// Кто нажал.
    #[serde(default)]
    pub from: Option<User>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_fixture_parses() {
        // Фрагмент реального getUpdates-ответа (текст и callback).
        let raw = r#"{
            "update_id": 1001,
            "message": {
                "message_id": 5,
                "from": { "id": 42, "username": "owner", "first_name": "Иван" },
                "chat": { "id": 42, "type": "private" },
                "text": "/approvals"
            }
        }"#;
        let update: Update = serde_json::from_str(raw).expect("разбор");
        assert_eq!(update.update_id, 1001);
        let message = update.message.expect("сообщение");
        assert_eq!(message.text.as_deref(), Some("/approvals"));
        assert_eq!(message.chat.id, 42);
    }

    #[test]
    fn callback_fixture_parses() {
        let raw = r#"{
            "update_id": 1002,
            "callback_query": {
                "id": "cb-1",
                "data": "approve:550e8400-e29b-41d4-a716-446655440000",
                "from": { "id": 42 },
                "message": {
                    "message_id": 7,
                    "chat": { "id": 42, "type": "private" },
                    "text": "Действие"
                }
            }
        }"#;
        let update: Update = serde_json::from_str(raw).expect("разбор");
        let callback = update.callback_query.expect("callback");
        assert_eq!(
            callback.data.as_deref(),
            Some("approve:550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(callback.message.expect("сообщение").message_id, 7);
    }

    #[test]
    fn unknown_untolerated_shapes_are_ignored() {
        // Обновление без message/callback (например, edit) — не ошибка.
        let update: Update = serde_json::from_str(r#"{ "update_id": 3 }"#).expect("разбор");
        assert!(update.message.is_none());
        assert!(update.callback_query.is_none());
    }
}
