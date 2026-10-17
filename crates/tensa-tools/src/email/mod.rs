//! Инструмент Email (ТЗ tensa-tools: `email/`; Контракт 6, ADR-015).
//!
//! Назначение: отправка почты через цикл одобрения (ТЗ: сетевые действия —
//! только через `ApprovalStore`). LLM/секретарь ставят письмо в очередь
//! (`propose_send`), владелец подтверждает (PWA/бот), исполнитель отправляет
//! (`execute_approved`) и действие фиксируется как `executed`.
//!
//! Состав модуля по ТЗ: `autodetect` (провайдеры), `connections` (аккаунты
//! из config+secrets), `smtp` (отправка, lettre). `imap`/`oauth2` — следующая
//! итерация (чтение/поиск почты и OAuth2-аутентификация), отмечено в README.
//!
//! Аудит: все действия — в `<root>/logs/tools.log`.

pub mod autodetect;
pub mod connections;
pub mod error;
pub mod smtp;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tensa_approval::{Action, ActionKind, ActionOrigin, ApprovalStore, NewAction};
use uuid::Uuid;

use crate::audit;
use crate::error::{Result as ToolsResult, ToolsError};
use crate::Tool;

pub use connections::Account;
pub use error::EmailError;
pub use smtp::OutgoingEmail;

/// Имя инструмента в реестре.
pub const NAME: &str = "email";

/// Описание инструмента для LLM и интерфейса.
pub const DESCRIPTION: &str = "Email: отправка письма через одобрение владельца. \
Параметры: {\"to\": \"...\", \"subject\": \"...\", \"body\": \"...\", \"account_id\": \"...\"?} — \
ставит письмо в очередь одобрения; отправка — после подтверждения.";

/// Сервис почты: конфиг, секреты и очередь одобрения.
pub struct EmailService {
    root: PathBuf,
    config_path: PathBuf,
    config: tensa_core::Config,
    secrets: tensa_core::Secrets,
}

impl EmailService {
    /// Собрать сервис из файлов ядра (`config.toml` + `secrets.toml`).
    pub fn from_files(config_path: &Path) -> Result<Self, EmailError> {
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
        Ok(Self {
            root,
            config_path: config_path.to_path_buf(),
            config,
            secrets,
        })
    }

    /// Собрать сервис из готовых частей (тесты).
    pub fn from_parts(
        root: PathBuf,
        config: tensa_core::Config,
        secrets: tensa_core::Secrets,
    ) -> Self {
        Self {
            config_path: root.join("config.toml"),
            root,
            config,
            secrets,
        }
    }

    /// Корень данных (для аудита и хранилищ).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Путь к `config.toml`, из которого собран сервис.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Учётные записи из конфига.
    pub fn accounts(&self) -> Vec<Account> {
        connections::resolve_accounts(&self.config)
    }

    /// Креденшел аккаунта: окружение приоритетнее файла секретов (Контракт 4/6).
    fn credential(&self, account: &Account) -> Result<String, EmailError> {
        let from_env = std::env::var("SMTP_PASSWORD")
            .ok()
            .or_else(|| std::env::var("IMAP_PASSWORD").ok())
            .filter(|value| !value.trim().is_empty());
        from_env
            .or_else(|| {
                self.secrets
                    .email
                    .accounts
                    .get(&account.id)
                    .and_then(|secret| secret.credential.clone())
            })
            .ok_or_else(|| EmailError::MissingCredential(account.id.clone()))
    }

    /// Поставить письмо в очередь одобрения (origin — LLM).
    pub async fn propose_send(
        &self,
        email: OutgoingEmail,
        store: &dyn ApprovalStore,
    ) -> Result<Action, EmailError> {
        if email.to.trim().is_empty() {
            return Err(EmailError::InvalidParams("поле to пустое".to_string()));
        }
        let payload = serde_json::json!({
            "account_id": email.account_id,
            "to": email.to,
            "cc": email.cc,
            "bcc": email.bcc,
            "subject": email.subject,
            "body": email.body,
        });
        let action = store
            .request_action(NewAction {
                kind: ActionKind::EmailSend,
                payload,
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

    /// Исполнить одобренную отправку: `Approved` → SMTP → `Executed`.
    ///
    /// До сети доходит только действие в статусе `approved`.
    pub async fn execute_approved(
        &self,
        store: &dyn ApprovalStore,
        id: Uuid,
    ) -> Result<Action, EmailError> {
        let action = store
            .get_by_id(id)
            .await?
            .ok_or_else(|| EmailError::InvalidParams(format!("действие {id} не найдено")))?;
        if action.kind.as_str() != "email_send" {
            return Err(EmailError::InvalidParams(format!(
                "действие {id} не email_send (а {})",
                action.kind.as_str()
            )));
        }
        if action.status.as_str() != "approved" {
            return Err(EmailError::InvalidParams(format!(
                "исполнять можно только approved (сейчас {})",
                action.status.as_str()
            )));
        }

        let email = outgoing_from_payload(&action.payload)?;
        let accounts = self.accounts();
        let account = connections::find_account(&accounts, email.account_id.as_deref())?.clone();
        let credential = self.credential(&account)?;

        smtp::send(&account, &credential, &email).await?;
        let executed = store.execute(id).await?;
        audit::record(&self.root, NAME, &format!("execute {}", action.id), "ok");
        Ok(executed)
    }
}

/// Письмо из payload действия (`email_send`).
fn outgoing_from_payload(payload: &Value) -> Result<OutgoingEmail, EmailError> {
    let field = |name: &str| {
        payload
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let to = field("to").ok_or_else(|| EmailError::InvalidParams("payload без to".to_string()))?;
    let optional = |name: &str| {
        payload
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    Ok(OutgoingEmail {
        account_id: optional("account_id"),
        to,
        cc: optional("cc"),
        bcc: optional("bcc"),
        subject: field("subject").unwrap_or_default(),
        body: field("body").unwrap_or_default(),
    })
}

/// Инструмент `email` для [`crate::ToolRegistry`].
///
/// Держит хранилище одобрений: `execute` структурно не может отправить письмо
/// мимо очереди — он её наполняет (ТЗ: сетевые действия через ApprovalStore).
pub struct EmailTool {
    service: Arc<EmailService>,
    store: Arc<dyn ApprovalStore>,
}

impl EmailTool {
    /// Собрать инструмент из сервиса и очереди одобрения.
    pub fn new(service: Arc<EmailService>, store: Arc<dyn ApprovalStore>) -> Self {
        Self { service, store }
    }
}

#[async_trait]
impl Tool for EmailTool {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    /// Поставить письмо в очередь одобрения (возвращает id действия).
    async fn execute(&self, params: Value) -> ToolsResult<Value> {
        let to = params
            .get("to")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ToolsError::Email(EmailError::InvalidParams(
                    "ожидается {\"to\": ..., \"subject\": ..., \"body\": ...}".to_string(),
                ))
            })?
            .to_string();
        let email = OutgoingEmail {
            account_id: params
                .get("account_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            to,
            cc: params.get("cc").and_then(Value::as_str).map(str::to_string),
            bcc: params
                .get("bcc")
                .and_then(Value::as_str)
                .map(str::to_string),
            subject: params
                .get("subject")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            body: params
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        };
        let action = self
            .service
            .propose_send(email, self.store.as_ref())
            .await?;
        Ok(serde_json::json!({
            "queued": true,
            "action_id": action.id,
            "status": action.status.as_str(),
        }))
    }

    /// Отправка почты — сетевое действие (ТЗ: через одобрение).
    fn requires_approval(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_email() -> OutgoingEmail {
        OutgoingEmail {
            account_id: Some("work".to_string()),
            to: "friend@example.com".to_string(),
            cc: None,
            bcc: None,
            subject: "тема".to_string(),
            body: "текст".to_string(),
        }
    }

    #[test]
    fn payload_round_trip() {
        let email = sample_email();
        let payload = serde_json::json!({
            "account_id": email.account_id,
            "to": email.to,
            "cc": email.cc,
            "bcc": email.bcc,
            "subject": email.subject,
            "body": email.body,
        });
        let restored = outgoing_from_payload(&payload).expect("разбор");
        assert_eq!(restored, email);
    }

    #[test]
    fn payload_without_to_is_error() {
        assert!(outgoing_from_payload(&serde_json::json!({ "subject": "s" })).is_err());
    }
}
