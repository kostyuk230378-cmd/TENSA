//! Трейт SyncService (ТЗ tensa-sync, сигнатуры дословно) + события
//! заметок/контактов (Контракт 8: sync.proto: NoteEvent, ContactEvent).

use crate::error::Result;
use crate::status::SyncStatus;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tensa_approval::{ActionEvent, VectorClock};
use tensa_nodes::Node;
use uuid::Uuid;

/// Событие заметки (Контракт 8: NoteEvent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteEvent {
    pub note_id: Uuid,
    /// created | updated | deleted
    pub event_type: String,
    /// JSON-снимок заметки.
    pub note_data: serde_json::Value,
    pub timestamp: i64,
    pub node_id: String,
    pub vector_clock: VectorClock,
}

/// Событие контакта (Контракт 8: ContactEvent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactEvent {
    pub contact_id: Uuid,
    /// created | updated | deleted
    pub event_type: String,
    /// JSON-снимок контакта.
    pub contact_data: serde_json::Value,
    pub timestamp: i64,
    pub node_id: String,
    pub vector_clock: VectorClock,
}

/// Сервис синхронизации узла (ТЗ tensa-sync). Реализация: `GrpcSyncService`.
#[async_trait]
pub trait SyncService: Send + Sync {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn status(&self) -> Result<SyncStatus>;
    async fn sync_approval_events(&self) -> Result<Vec<ActionEvent>>;
    async fn push_approval_event(&self, event: ActionEvent) -> Result<()>;
    async fn sync_notes(&self) -> Result<Vec<NoteEvent>>;
    async fn push_note_event(&self, event: NoteEvent) -> Result<()>;
    async fn sync_contacts(&self) -> Result<Vec<ContactEvent>>;
    async fn push_contact_event(&self, event: ContactEvent) -> Result<()>;
    async fn send_heartbeat(&self) -> Result<()>;
    async fn list_online_nodes(&self) -> Result<Vec<Node>>;
    async fn acquire_connection(&self, resource: &str, account_id: Option<&str>) -> Result<()>;
    async fn release_connection(&self, resource: &str, account_id: Option<&str>) -> Result<()>;
}

/// Источник событий заметок для серверной стороны SyncService.
///
/// [derived] В tensa-core нет запроса «заметки, изменённые позже X»
/// (Контракт 3: list_notes(limit, offset)); адаптер реализуется интегратором
/// без изменения публичных трейтов tensa-core. До появления адаптера
/// используется `NoopSyncSource`.
#[async_trait]
pub trait NotesSyncSource: Send + Sync {
    /// События заметок, изменённых позже `last_updated_at`.
    async fn events_since(&self, last_updated_at: i64, limit: usize) -> Result<Vec<NoteEvent>>;

    /// Применить удалённое событие заметки (LWW по updated_at).
    async fn apply_event(&self, event: NoteEvent) -> Result<()>;
}

/// Источник событий контактов для серверной стороны SyncService.
#[async_trait]
pub trait ContactsSyncSource: Send + Sync {
    /// События контактов, изменённых позже `last_updated_at`.
    async fn events_since(&self, last_updated_at: i64, limit: usize) -> Result<Vec<ContactEvent>>;

    /// Применить удалённое событие контакта (LWW по updated_at).
    async fn apply_event(&self, event: ContactEvent) -> Result<()>;
}

/// Заглушка: событий нет, приём событий честно отклоняется
/// (не молчаливая потеря данных, а явная ошибка).
#[derive(Debug, Default)]
pub struct NoopSyncSource;

#[async_trait]
impl NotesSyncSource for NoopSyncSource {
    async fn events_since(&self, _last_updated_at: i64, _limit: usize) -> Result<Vec<NoteEvent>> {
        Ok(Vec::new())
    }

    async fn apply_event(&self, _event: NoteEvent) -> Result<()> {
        Err(crate::error::SyncError::Config(
            "notes sync adapter is not wired yet".into(),
        ))
    }
}

#[async_trait]
impl ContactsSyncSource for NoopSyncSource {
    async fn events_since(
        &self,
        _last_updated_at: i64,
        _limit: usize,
    ) -> Result<Vec<ContactEvent>> {
        Ok(Vec::new())
    }

    async fn apply_event(&self, _event: ContactEvent) -> Result<()> {
        Err(crate::error::SyncError::Config(
            "contacts sync adapter is not wired yet".into(),
        ))
    }
}
