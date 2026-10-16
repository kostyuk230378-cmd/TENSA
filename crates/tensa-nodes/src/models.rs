//! Модели крейта (ТЗ tensa-nodes, Контракт 7: типы дословно).
//! DeviceType/NodeRole переиспользуются из tensa-core (не дублировать).

pub use tensa_core::{DeviceType, NodeRole};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Платформа узла (Контракт 8: Platform, сокращённо по ТЗ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Linux,
    Android,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Android => "android",
        }
    }
}

/// Термальный статус узла.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalStatus {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl ThermalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThermalStatus::Nominal => "nominal",
            ThermalStatus::Fair => "fair",
            ThermalStatus::Serious => "serious",
            ThermalStatus::Critical => "critical",
        }
    }
}

/// Ресурсы узла (Контракт 7: JSON).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeResources {
    pub telegram: bool,
    pub email: bool,
    pub whatsapp: bool,
    pub llm_local: bool,
    pub llm_cloud: bool,
    pub memory_store: bool,
    pub approval_store: bool,
}

impl NodeResources {
    /// Есть ли ресурс (по каноническому имени из Контракта 7).
    pub fn has(&self, resource: &str) -> bool {
        match resource {
            "telegram" => self.telegram,
            "email" => self.email,
            "whatsapp" => self.whatsapp,
            "llm_local" => self.llm_local,
            "llm_cloud" => self.llm_cloud,
            "memory_store" => self.memory_store,
            "approval_store" => self.approval_store,
            _ => false,
        }
    }
}

/// Узел реестра.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_id: Uuid,
    pub name: String,
    pub device_type: DeviceType,
    pub platform: Platform,
    pub role: NodeRole,
    pub tailscale_ip: String,
    pub grpc_port: u16,
    pub last_seen: i64,
    pub online: bool,
    pub ram_total_mb: u64,
    pub ram_available_mb: u64,
    pub cpu_cores: u32,
    pub cpu_usage_percent: f32,
    pub has_gpu: bool,
    pub has_npu: bool,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,
    pub thermal_status: ThermalStatus,
    pub resources: NodeResources,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Heartbeat узла (Контракт 7/8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub node_id: Uuid,
    pub ram_available_mb: u64,
    pub cpu_usage_percent: f32,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,
    pub thermal_status: ThermalStatus,
    pub timestamp: i64,
}

/// Активное соединение узла к ресурсу (ADR-015).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConnection {
    pub id: i64,
    pub node_id: Uuid,
    pub resource: String,
    pub account_id: Option<String>,
    pub active: bool,
    pub acquired_at: i64,
    pub expires_at: i64,
}

/// TTL аренды соединения (ТЗ: 15 минут).
pub const LEASE_TTL_SECONDS: i64 = 15 * 60;
