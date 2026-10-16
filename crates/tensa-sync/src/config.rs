//! Конфигурация синхронизации (ТЗ tensa-sync, раздел «Публичный API»).

/// Настройки синхронизации. Значения по умолчанию — из ТЗ.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncConfig {
    /// Порт gRPC-сервера узла.
    pub grpc_port: u16,
    /// Включить mTLS для gRPC (иначе шифрование Tailscale).
    pub grpc_tls: bool,
    /// Синхронизация только внутри Tailscale-сети.
    pub tailscale_enabled: bool,
    /// Имя узла в Tailscale.
    pub tailscale_hostname: String,
    /// Управление Syncthing включено.
    pub syncthing_enabled: bool,
    /// Порт REST API Syncthing.
    pub syncthing_port: u16,
    /// API-ключ Syncthing. Не логировать!
    #[serde(skip_serializing)]
    pub syncthing_api_key: String,
    /// Интервал heartbeat, секунд (30).
    pub heartbeat_interval_seconds: u64,
    /// Таймаут офлайна, секунд (90).
    pub heartbeat_timeout_seconds: u64,
    /// Интервал постоянной синхронизации ApprovalStore, секунд (60).
    pub sync_interval_seconds: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            grpc_port: 8766,
            grpc_tls: true,
            tailscale_enabled: true,
            tailscale_hostname: String::new(),
            syncthing_enabled: false,
            syncthing_port: 8384,
            syncthing_api_key: String::new(),
            heartbeat_interval_seconds: 30,
            heartbeat_timeout_seconds: 90,
            sync_interval_seconds: 60,
        }
    }
}
