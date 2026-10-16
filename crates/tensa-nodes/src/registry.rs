//! Трейт NodeRegistry (ТЗ tensa-nodes, сигнатуры дословно).

use crate::error::Result;
use crate::models::{Heartbeat, Node};
use async_trait::async_trait;
use uuid::Uuid;

/// Реестр узлов Tensa в сети (Контракт 7).
#[async_trait]
pub trait NodeRegistry: Send + Sync {
    /// Зарегистрировать узел (или обновить при повторной регистрации).
    async fn register(&self, node: Node) -> Result<()>;

    /// Удалить узел из реестра.
    async fn unregister(&self, node_id: Uuid) -> Result<()>;

    /// Обновить heartbeat узла (последняя активность + телеметрия).
    async fn update_heartbeat(&self, heartbeat: Heartbeat) -> Result<()>;

    /// Узел по id.
    async fn get(&self, node_id: Uuid) -> Result<Option<Node>>;

    /// Онлайн-узлы.
    async fn list_online(&self) -> Result<Vec<Node>>;

    /// Все узлы.
    async fn list_all(&self) -> Result<Vec<Node>>;

    /// Онлайн heavy-узел для инференса (с доступной RAM).
    async fn find_heavy(&self) -> Result<Option<Node>>;

    /// Онлайн-узлы с указанным ресурсом.
    async fn find_with_resource(&self, resource: &str) -> Result<Vec<Node>>;

    /// Захватить соединение к ресурсу (TTL 15 минут; конфликт — Err).
    async fn acquire_connection(
        &self,
        node_id: Uuid,
        resource: &str,
        account_id: Option<&str>,
    ) -> Result<()>;

    /// Освободить соединение.
    async fn release_connection(
        &self,
        node_id: Uuid,
        resource: &str,
        account_id: Option<&str>,
    ) -> Result<()>;

    /// Кто держит активное соединение к ресурсу.
    async fn get_active_connection(
        &self,
        resource: &str,
        account_id: Option<&str>,
    ) -> Result<Option<Uuid>>;

    /// Перевести молчащие узлы в офлайн; возвращает количество.
    async fn cleanup_stale(&self, timeout_seconds: u64) -> Result<usize>;
}
