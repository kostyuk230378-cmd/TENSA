//! Модели данных (Контракт 1: таблица `actions`, Rust-структуры).
//! Имена и поля — дословно из ТЗ tensa-approval.

use crate::error::{ApprovalError, Result};
use crate::vector_clock::VectorClock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Действие, требующее одобрения владельца.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: Uuid,
    pub kind: ActionKind,
    pub payload: serde_json::Value,
    pub status: ActionStatus,
    pub created_at: i64,
    pub created_by: ActionOrigin,
    pub created_on_node: String,
    pub expires_at: i64,
    pub approved_at: Option<i64>,
    pub approved_by: Option<ActionOrigin>,
    pub approved_on_node: Option<String>,
    pub executed_at: Option<i64>,
    pub executed_on_node: Option<String>,
    pub result: Option<serde_json::Value>,
    pub operation_id: Uuid,
    pub vector_clock: VectorClock,
}

/// Тип действия (Контракт 8: `ActionKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    EmailSend,
    TelegramSend,
    WhatsAppSend,
    CalendarCreate,
    AppLaunch,
    FileOpen,
}

impl ActionKind {
    /// Каноническое строковое значение для БД (как в Контракте 1).
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionKind::EmailSend => "email_send",
            ActionKind::TelegramSend => "telegram_send",
            ActionKind::WhatsAppSend => "whatsapp_send",
            ActionKind::CalendarCreate => "calendar_create",
            ActionKind::AppLaunch => "app_launch",
            ActionKind::FileOpen => "file_open",
        }
    }

    pub fn from_db(s: &str) -> Result<Self> {
        s.parse()
    }

    /// Выключатель в `ApprovalConfig` для этого типа действия.
    pub fn config_flag(&self, config: &crate::config::ApprovalConfig) -> bool {
        match self {
            ActionKind::EmailSend => config.allow_email_send,
            ActionKind::TelegramSend => config.allow_telegram_send,
            ActionKind::WhatsAppSend => config.allow_whatsapp_send,
            _ => true,
        }
    }
}

/// Статус действия (Контракт 8: `ActionStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Failed,
    Expired,
}

impl ActionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionStatus::Pending => "pending",
            ActionStatus::Approved => "approved",
            ActionStatus::Rejected => "rejected",
            ActionStatus::Executed => "executed",
            ActionStatus::Failed => "failed",
            ActionStatus::Expired => "expired",
        }
    }

    pub fn from_db(s: &str) -> Result<Self> {
        s.parse()
    }
}

/// Разбор строковых значений из БД (SQLx/контрактные имена).
impl std::str::FromStr for ActionKind {
    type Err = ApprovalError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "email_send" => ActionKind::EmailSend,
            "telegram_send" => ActionKind::TelegramSend,
            "whatsapp_send" => ActionKind::WhatsAppSend,
            "calendar_create" => ActionKind::CalendarCreate,
            "app_launch" => ActionKind::AppLaunch,
            "file_open" => ActionKind::FileOpen,
            other => {
                return Err(ApprovalError::Config(format!(
                    "unknown action kind: {other}"
                )))
            }
        })
    }
}

/// Источник создания действия (Контракт 1: `created_by`).
/// По ТЗ этот же тип используется в approve/reject (`approved_by`); значения
/// user/telegram/mobile/desktop из Контракта 1 маппятся в User/Telegram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOrigin {
    Llm,
    User,
    Telegram,
    Email,
    System,
}

impl ActionOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionOrigin::Llm => "llm",
            ActionOrigin::User => "user",
            ActionOrigin::Telegram => "telegram",
            ActionOrigin::Email => "email",
            ActionOrigin::System => "system",
        }
    }

    pub fn from_db(s: &str) -> Result<Self> {
        s.parse()
    }
}

/// Разбор строковых значений из БД.
impl std::str::FromStr for ActionStatus {
    type Err = ApprovalError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "pending" => ActionStatus::Pending,
            "approved" => ActionStatus::Approved,
            "rejected" => ActionStatus::Rejected,
            "executed" => ActionStatus::Executed,
            "failed" => ActionStatus::Failed,
            "expired" => ActionStatus::Expired,
            other => return Err(ApprovalError::Config(format!("unknown status: {other}"))),
        })
    }
}

/// Разбор строковых значений из БД.
impl std::str::FromStr for ActionOrigin {
    type Err = ApprovalError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "llm" => ActionOrigin::Llm,
            "user" => ActionOrigin::User,
            "telegram" => ActionOrigin::Telegram,
            "email" => ActionOrigin::Email,
            "system" => ActionOrigin::System,
            // mobile/desktop — источники одобрения с других узлов; [derived]
            // маппинг на ближайший вариант ТЗ.
            "mobile" | "desktop" => ActionOrigin::User,
            other => return Err(ApprovalError::Config(format!("unknown origin: {other}"))),
        })
    }
}

/// Заявка на создание действия.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAction {
    pub kind: ActionKind,
    pub payload: serde_json::Value,
    pub created_by: ActionOrigin,
    pub expires_in_hours: Option<u64>,
}
