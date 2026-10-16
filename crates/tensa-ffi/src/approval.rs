//! `ApprovalStore` (Контракт 9): очередь одобрения поверх tensa-approval.

use tensa_approval::{
    ActionKind, ActionOrigin, ApprovalStore as CoreApprovalStore, NewAction as CoreNewAction,
    SqliteApprovalStore,
};

use crate::convert::{
    data_root, json_str, load_core_config, name_of, parse_uuid, timestamp_str, uuid_str,
};
use crate::error::TensaError;
use crate::types::{Action, NewAction};

/// FFI-обёртка очереди одобрения (`interface ApprovalStore` в UDL).
pub struct ApprovalStore {
    inner: SqliteApprovalStore,
}

impl ApprovalStore {
    /// Открыть хранилище по пути к `config.toml` (база — `<root>/approval.db`).
    ///
    /// UDL: `[Async, Throws=TensaError] constructor(string config_path);`
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        let config = load_core_config(&config_path)?;
        let store = SqliteApprovalStore::new(
            data_root(&config_path).join("approval.db"),
            tensa_approval::ApprovalConfig::from(&config.approval),
            config.general.node_id.clone(),
        )
        .await?;
        Ok(Self { inner: store })
    }

    /// Поставить новое действие в очередь.
    pub async fn request_action(&self, action: NewAction) -> Result<Action, TensaError> {
        let payload: serde_json::Value = serde_json::from_str(&action.payload)
            .map_err(|error| TensaError::internal("new_action.payload", error))?;
        let created = self
            .inner
            .request_action(CoreNewAction {
                kind: parse_kind(&action.kind)?,
                payload,
                created_by: ActionOrigin::User,
                expires_in_hours: None,
            })
            .await?;
        Ok(to_action(created))
    }

    /// Подтвердить действие.
    pub async fn approve(&self, id: String, approved_by: String) -> Result<Action, TensaError> {
        let id = parse_uuid("approval.approve", &id)?;
        let action = self.inner.approve(id, parse_origin(&approved_by)).await?;
        Ok(to_action(action))
    }

    /// Отклонить действие.
    pub async fn reject(&self, id: String, rejected_by: String) -> Result<Action, TensaError> {
        let id = parse_uuid("approval.reject", &id)?;
        let action = self.inner.reject(id, parse_origin(&rejected_by)).await?;
        Ok(to_action(action))
    }

    /// Ожидающие решения действия.
    pub async fn get_pending(&self) -> Result<Vec<Action>, TensaError> {
        let pending = self.inner.get_pending().await?;
        Ok(pending.into_iter().map(to_action).collect())
    }

    /// Действие по id (включая решённые).
    pub async fn get_by_id(&self, id: String) -> Result<Option<Action>, TensaError> {
        let id = parse_uuid("approval.get_by_id", &id)?;
        let found = self.inner.get_by_id(id).await?;
        Ok(found.map(to_action))
    }
}

/// Конвертация действия ядра в UDL-представление.
fn to_action(action: tensa_approval::Action) -> Action {
    Action {
        id: uuid_str(action.id),
        kind: name_of(&action.kind),
        payload: json_str(&action.payload),
        status: name_of(&action.status),
        created_at: action.created_at,
        expires_at: action.expires_at,
        approved_at: timestamp_str(action.approved_at),
        executed_at: timestamp_str(action.executed_at),
        result: action.result.as_ref().map(json_str),
    }
}

/// Строка вида действия → enum ядра (канонические значения Контракта 1).
fn parse_kind(raw: &str) -> Result<ActionKind, TensaError> {
    match raw.trim() {
        "email_send" => Ok(ActionKind::EmailSend),
        "telegram_send" => Ok(ActionKind::TelegramSend),
        "whatsapp_send" => Ok(ActionKind::WhatsAppSend),
        "calendar_create" => Ok(ActionKind::CalendarCreate),
        "app_launch" => Ok(ActionKind::AppLaunch),
        "file_open" => Ok(ActionKind::FileOpen),
        other => Err(TensaError::internal(
            "approval.kind",
            format!("неизвестный вид: {other}"),
        )),
    }
}

/// Строка инициатора → enum ядра (неизвестное — пользователь из интерфейса).
fn parse_origin(raw: &str) -> ActionOrigin {
    raw.trim().parse().unwrap_or(ActionOrigin::User)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_parsing_matches_contract() {
        assert_eq!(
            parse_kind("email_send").map(|k| k.as_str()).ok(),
            Some("email_send")
        );
        assert!(parse_kind("nope").is_err());
    }
}
