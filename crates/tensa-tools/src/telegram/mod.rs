//! Инструмент Telegram (ТЗ tensa-tools: `telegram/`, ADR-011/014).
//!
//! Назначение: бот-секретарь уведомляет владельца и принимает решения
//! (подтвердить/отклонить) прямо из чата; LLM-инициированные отправки идут
//! через `ApprovalStore` (ТЗ: сетевые действия — только через одобрение,
//! trusted-источник `telegram` — можно без одобрения, но с аудитом).
//!
//! [derived] Вместо `teloxide` (план разведки) — тонкий клиент Bot API
//! поверх `reqwest` (уже в воркспейсе): принцип ТЗ «минимум зависимостей»,
//! версионный риск фреймворка отсутствует, схема Bot API стабильна.
//! [derived] Оформлен модулем (не подкрейтом), как и `wol`: выделение
//! подкрейтов `crates/tensa-tools/<tool>` — отдельная механическая задача.
//!
//! Источники настроек: токен бота — `TELEGRAM_BOT_TOKEN` → `secrets.toml`;
//! чат владельца — `secrets.telegram.owner_chat_id` → `config.telegram.owner_chat_id`.
//! Все действия журналируются в `<root>/logs/tools.log` (аудит ТЗ).

pub mod bot;
pub mod callbacks;
pub mod error;
pub mod handlers;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tensa_approval::{Action, ActionKind, ActionOrigin, ApprovalStore, NewAction};
use uuid::Uuid;

use crate::audit;
use crate::error::{Result as ToolsResult, ToolsError};
use crate::Tool;

pub use bot::{TelegramClient, Update};
pub use error::TelegramError;

/// Имя инструмента в реестре.
pub const NAME: &str = "telegram";

/// Описание инструмента для LLM и интерфейса.
pub const DESCRIPTION: &str = "Telegram: уведомление владельца и решения из чата. \
Параметры: {\"text\": \"...\"} — текст, который секретарь отправляет владельцу.";

/// Сервис Telegram: клиент Bot API, корень данных, чат владельца.
pub struct TelegramService {
    client: TelegramClient,
    root: PathBuf,
    config_path: PathBuf,
    owner_chat_id: String,
    node_id: String,
}

impl TelegramService {
    /// Собрать сервис из файлов ядра (`config.toml` + `secrets.toml`).
    pub fn from_files(config_path: &Path) -> Result<Self, TelegramError> {
        let root = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let config = if config_path.exists() {
            tensa_core::Config::load(config_path)?
        } else {
            tensa_core::Config::default()
        };
        let secrets_path = root.join("secrets.toml");
        let secrets = if secrets_path.exists() {
            tensa_core::Secrets::load(&secrets_path)?
        } else {
            tensa_core::Secrets::default()
        };

        // Токен: окружение приоритетнее файла секретов (Контракт 4).
        let credential = std::env::var("TELEGRAM_BOT_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| secrets.telegram.credential.clone())
            .ok_or(TelegramError::MissingCredential)?;

        let owner_chat_id = secrets
            .telegram
            .owner_chat_id
            .clone()
            .or_else(|| {
                config
                    .telegram
                    .get("owner_chat_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .ok_or(TelegramError::MissingOwnerChat)?;

        Self::from_parts(
            root,
            config_path.to_path_buf(),
            credential,
            owner_chat_id,
            config.general.node_id.clone(),
        )
    }

    /// Собрать сервис из готовых частей (тесты и демо).
    pub fn from_parts(
        root: PathBuf,
        config_path: PathBuf,
        credential: String,
        owner_chat_id: String,
        node_id: String,
    ) -> Result<Self, TelegramError> {
        Ok(Self {
            client: TelegramClient::new(credential)?,
            root,
            config_path,
            owner_chat_id,
            node_id,
        })
    }

    /// Корень данных (для аудита и хранилищ).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Путь к `config.toml`, из которого собран сервис.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Чат владельца.
    pub fn owner_chat_id(&self) -> &str {
        &self.owner_chat_id
    }

    /// Сообщение из чата владельца?
    pub fn is_owner(&self, chat_id: i64) -> bool {
        chat_id.to_string() == self.owner_chat_id
    }

    /// Клиент Bot API (нужен обработчикам).
    pub fn client(&self) -> &TelegramClient {
        &self.client
    }

    /// Короткая строка состояния узла (для `/status`).
    pub fn status_line(&self) -> String {
        let node = self.node_id.chars().take(8).collect::<String>();
        format!("Tensa: узел {node}…, корень {}", self.root.display())
    }

    /// Отправить текст владельцу (trusted-путь: ручной вызов/owner-команды) + аудит.
    pub async fn send_to_owner(&self, text: &str) -> Result<(), TelegramError> {
        self.client
            .send_message(&self.owner_chat_id, text, None)
            .await?;
        audit::record(&self.root, NAME, "send to owner", "ok");
        Ok(())
    }

    /// Поставить в очередь одобрения отправку владельцу (origin — LLM).
    ///
    /// Владелец увидит действие в PWA («Одобрения») или в боте (`/approvals`);
    /// после одобрения отправка исполняется через [`Self::execute_approved`].
    pub async fn propose_send(
        &self,
        text: &str,
        store: &dyn ApprovalStore,
    ) -> Result<Action, TelegramError> {
        if text.trim().is_empty() {
            return Err(TelegramError::InvalidParams("пустой текст".to_string()));
        }
        let action = store
            .request_action(NewAction {
                kind: ActionKind::TelegramSend,
                payload: serde_json::json!({ "text": text }),
                created_by: ActionOrigin::Llm,
                expires_in_hours: None,
            })
            .await?;
        audit::record(
            &self.root,
            NAME,
            &format!("propose {}", action.id),
            "pending",
        );
        Ok(action)
    }

    /// Исполнить одобренную отправку: `Approved` → отправка → `Executed`.
    ///
    /// До сети доходит только действие в статусе `approved`: ожидающие
    /// и отклонённые отсекаются локально (без обращения к Bot API).
    pub async fn execute_approved(
        &self,
        store: &dyn ApprovalStore,
        id: Uuid,
    ) -> Result<Action, TelegramError> {
        let action = store
            .get_by_id(id)
            .await?
            .ok_or_else(|| TelegramError::InvalidParams(format!("действие {id} не найдено")))?;
        if action.kind.as_str() != "telegram_send" {
            return Err(TelegramError::InvalidParams(format!(
                "действие {id} не telegram_send (а {})",
                action.kind.as_str()
            )));
        }
        if action.status.as_str() != "approved" {
            return Err(TelegramError::InvalidParams(format!(
                "исполнять можно только approved (сейчас {})",
                action.status.as_str()
            )));
        }
        let text = action
            .payload
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| TelegramError::InvalidParams("payload без text".to_string()))?
            .to_string();

        self.client
            .send_message(&self.owner_chat_id, &text, None)
            .await?;
        let executed = store.execute(id).await?;
        audit::record(&self.root, NAME, &format!("execute {}", action.id), "ok");
        Ok(executed)
    }

    /// Long polling: принимать команды и кнопки владельца до остановки процесса.
    ///
    /// Ошибки отдельных обновлений не роняют цикл (уходят в `tracing`).
    pub async fn run_polling(
        self: Arc<Self>,
        store: Arc<dyn ApprovalStore>,
    ) -> Result<(), TelegramError> {
        // Стартовый getMe — только информативный: сбой сети/прокси не должен
        // убивать процесс (цикл ниже сам повторяет запросы).
        match self.client.get_me().await {
            Ok(me) => tracing::info!(
                bot = me.username.as_deref().unwrap_or("(без username)"),
                owner = %self.owner_chat_id,
                "telegram: бот запущен (long polling)"
            ),
            Err(error) => tracing::warn!(
                error = %error,
                "telegram: getMe не удался на старте — продолжаем опрос"
            ),
        }
        audit::record(&self.root, NAME, "bot start", "ok");

        let mut offset: Option<i64> = None;
        loop {
            match self.client.get_updates(offset, 25).await {
                Ok(updates) => {
                    for update in updates {
                        offset = Some(update.update_id + 1);
                        if let Err(error) =
                            handlers::handle_update(self.as_ref(), store.as_ref(), update).await
                        {
                            tracing::warn!(error = %error, "telegram: обработка обновления не удалась");
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "telegram: getUpdates не удался — повтор через 5 c");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

/// Инструмент `telegram` для [`crate::ToolRegistry`].
pub struct TelegramTool {
    service: Arc<TelegramService>,
}

impl TelegramTool {
    /// Обернуть собранный сервис в инструмент реестра.
    pub fn new(service: Arc<TelegramService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl Tool for TelegramTool {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    /// Отправить текст владельцу (ручной/trusted-вызов) + аудит.
    ///
    /// Гейтинг для LLM-источника — централизованный (ApprovalStore):
    /// используйте [`TelegramService::propose_send`] + `execute_approved`.
    async fn execute(&self, params: Value) -> ToolsResult<Value> {
        let text = params.get("text").and_then(Value::as_str).ok_or_else(|| {
            ToolsError::Telegram(TelegramError::InvalidParams(
                "ожидается {\"text\": \"...\"}".to_string(),
            ))
        })?;
        self.service.send_to_owner(text).await?;
        Ok(serde_json::json!({ "sent": true, "to": "owner" }))
    }

    /// Отправка сообщений — сетевое действие (ТЗ: через одобрение).
    fn requires_approval(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> TelegramService {
        TelegramService::from_parts(
            std::env::temp_dir().join("tensa-telegram-unit"),
            PathBuf::from("config.toml"),
            "unit-credential".to_string(),
            "42".to_string(),
            "00000000-0000-0000-0000-000000000000".to_string(),
        )
        .expect("конструктор")
    }

    #[test]
    fn owner_matching() {
        let service = service();
        assert!(service.is_owner(42));
        assert!(!service.is_owner(7));
    }

    #[test]
    fn status_line_contains_node_prefix() {
        let line = service().status_line();
        assert!(line.contains("00000000"), "строка: {line}");
    }

    #[test]
    fn tool_metadata() {
        let tool = TelegramTool::new(Arc::new(service()));
        assert_eq!(tool.name(), "telegram");
        assert!(tool.requires_approval());
        assert!(tool.description().contains("Telegram"));
    }

    #[tokio::test]
    async fn tool_rejects_params_without_text() {
        let tool = TelegramTool::new(Arc::new(service()));
        let result = tool.execute(serde_json::json!({ "foo": 1 })).await;
        assert!(result.is_err(), "без text должно быть ошибкой (до сети)");
    }
}
