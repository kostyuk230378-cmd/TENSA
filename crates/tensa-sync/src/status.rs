//! Статус синхронизации (ТЗ tensa-sync).

use serde::{Deserialize, Serialize};

/// Сводный статус сервиса синхронизации.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Сервис запущен.
    pub running: bool,
    /// Время последней успешной синхронизации.
    pub last_sync_at: Option<i64>,
    /// Узлов онлайн.
    pub peers_online: usize,
    /// Всего известных узлов.
    pub peers_total: usize,
    /// Статус Syncthing (если управление включено).
    pub syncthing_status: Option<SyncthingStatus>,
}

/// Статус Syncthing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncthingStatus {
    pub running: bool,
    pub connections: usize,
    pub last_error: Option<String>,
}
