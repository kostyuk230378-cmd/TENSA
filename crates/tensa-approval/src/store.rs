//! Трейт ApprovalStore (ТЗ tensa-approval, сигнатуры дословно).

use crate::error::Result;
use crate::events::ActionEvent;
use crate::models::{Action, ActionOrigin, NewAction};
use async_trait::async_trait;
use uuid::Uuid;

/// Хранилище действий, требующих одобрения владельца.
///
/// Реализации не исполняют действия и не делают сетевых запросов —
/// только ведут жизненный цикл и журнал событий.
#[async_trait]
pub trait ApprovalStore: Send + Sync {
    /// Создаёт новое действие в очереди (статус pending).
    async fn request_action(&self, action: NewAction) -> Result<Action>;

    /// Одобряет действие: pending → approved.
    async fn approve(&self, id: Uuid, approved_by: ActionOrigin) -> Result<Action>;

    /// Отклоняет действие: pending → rejected.
    async fn reject(&self, id: Uuid, rejected_by: ActionOrigin) -> Result<Action>;

    /// Помечает исполнение: approved → executed (исполнителем снаружи).
    async fn execute(&self, id: Uuid) -> Result<Action>;

    /// Текущая очередь ожидания.
    async fn get_pending(&self) -> Result<Vec<Action>>;

    /// Действие по id.
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Action>>;

    /// Переводит просроченные pending-действия в expired; возвращает количество.
    async fn expire_old(&self) -> Result<usize>;

    /// Применяет чужое событие синхронизации (идемпотентно).
    async fn apply_event(&self, event: ActionEvent) -> Result<()>;

    /// События после указанного event_id (для инкрементальной синхронизации).
    async fn get_events_since(&self, last_event_id: u64) -> Result<Vec<ActionEvent>>;
}
