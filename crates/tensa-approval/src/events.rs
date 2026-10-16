//! События (event sourcing, Контракт 1: таблица `action_events`).

use crate::vector_clock::VectorClock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Тип события жизненного цикла действия.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Created,
    Approved,
    Rejected,
    Executed,
    Failed,
    Expired,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Created => "created",
            EventType::Approved => "approved",
            EventType::Rejected => "rejected",
            EventType::Executed => "executed",
            EventType::Failed => "failed",
            EventType::Expired => "expired",
        }
    }

    pub fn from_db(s: &str) -> crate::error::Result<Self> {
        s.parse()
    }
}

/// Разбор строковых значений из БД.
impl std::str::FromStr for EventType {
    type Err = crate::error::ApprovalError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "created" => EventType::Created,
            "approved" => EventType::Approved,
            "rejected" => EventType::Rejected,
            "executed" => EventType::Executed,
            "failed" => EventType::Failed,
            "expired" => EventType::Expired,
            other => {
                return Err(crate::error::ApprovalError::Config(format!(
                    "unknown event type: {other}"
                )))
            }
        })
    }
}

/// Событие действия для синхронизации между узлами (gRPC, ADR-012).
/// Идемпотентность — `action_id` + `vector_clock`; порядок — `event_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvent {
    pub event_id: u64,
    pub action_id: Uuid,
    pub event_type: EventType,
    pub event_data: serde_json::Value,
    pub timestamp: i64,
    pub node_id: String,
    pub vector_clock: VectorClock,
}
