//! Слой-адаптер над ApprovalStore для синхронизации (Контракт 8: SyncService).
//!
//! Крейт по ТЗ «не хранит данные» — данные остаются в `tensa-approval`;
//! здесь только извлечение событий и применение удалённых.

use crate::error::Result;
use async_trait::async_trait;
use tensa_approval::{ActionEvent, ApprovalStore};

/// Синхронизационная часть ApprovalStore (события).
#[async_trait]
pub trait ApprovalSync: Send + Sync {
    /// Все события с указанного event_id (для отправки peer'у).
    async fn events_since(&self, last_event_id: u64) -> Result<Vec<ActionEvent>>;

    /// Применить события от peer'а (идемпотентно, LWW).
    async fn apply_remote(&self, events: Vec<ActionEvent>) -> Result<usize>;
}

#[async_trait]
impl<T: ApprovalStore + ?Sized> ApprovalSync for T {
    async fn events_since(&self, last_event_id: u64) -> Result<Vec<ActionEvent>> {
        Ok(ApprovalStore::get_events_since(self, last_event_id).await?)
    }

    async fn apply_remote(&self, events: Vec<ActionEvent>) -> Result<usize> {
        let mut applied = 0usize;
        for event in events {
            ApprovalStore::apply_event(self, event).await?;
            applied += 1;
        }
        Ok(applied)
    }
}

/// Локальная запись позиции синхронизации (Контракт 1: sync_state).
/// Хранится у владельца состояния, крейт только считает дельту.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SyncCursor {
    /// node_id -> последний применённый event_id.
    pub peers: std::collections::BTreeMap<String, u64>,
}

impl SyncCursor {
    pub fn last_for(&self, node_id: &str) -> u64 {
        self.peers.get(node_id).copied().unwrap_or(0)
    }

    pub fn advance(&mut self, node_id: &str, event_id: u64) {
        let slot = self.peers.entry(node_id.to_string()).or_insert(0);
        if event_id > *slot {
            *slot = event_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tensa_approval::{ActionKind, ApprovalConfig, NewAction, SqliteApprovalStore};

    #[tokio::test]
    async fn events_flow_between_nodes() {
        let a = SqliteApprovalStore::init_with_config(ApprovalConfig::default())
            .await
            .expect("a");
        let b = SqliteApprovalStore::init_with_config(ApprovalConfig::default())
            .await
            .expect("b");

        a.request_action(NewAction {
            kind: ActionKind::EmailSend,
            payload: serde_json::json!({ "to": "x@y.z" }),
            created_by: tensa_approval::ActionOrigin::Llm,
            expires_in_hours: None,
        })
        .await
        .expect("request");

        let events = a.events_since(0).await.expect("events");
        assert_eq!(events.len(), 1);
        let applied = b.apply_remote(events).await.expect("apply");
        assert_eq!(applied, 1);
        // Повторная отправка — идемпотентно.
        let events2 = a.events_since(0).await.expect("events2");
        let applied2 = b.apply_remote(events2).await.expect("apply2");
        assert_eq!(applied2, 1); // событие записано, дубликат отсечён внутри apply_event
    }

    #[test]
    fn cursor_monotonic() {
        let mut c = SyncCursor::default();
        c.advance("n1", 5);
        c.advance("n1", 3); // назад не движется
        assert_eq!(c.last_for("n1"), 5);
        c.advance("n1", 7);
        assert_eq!(c.last_for("n1"), 7);
        assert_eq!(c.last_for("ghost"), 0);
    }
}
