//! Профилирование локального узла: RAM, CPU, GPU/NPU, батарея, thermal.
//!
//! Реальный сбор телеметрии выполняет `tensa-perf` (ТЗ tensa-perf,
//! `DefaultPerfProfiler`: sysinfo + платформенные источники) — во избежание
//! дублирования кода; сюда кадр попадает из оркестратора (tensa-core).
//! В этом крейте остаётся детерминированная заглушка для тестов и CI.
//! [derived] Схема подписи (NodeProfile) — из ТЗ.

use crate::models::{NodeResources, ThermalStatus};

/// Снимок телеметрии локального узла (то, что уходит в Heartbeat).
#[derive(Debug, Clone, PartialEq)]
pub struct NodeProfile {
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
}

/// Провайдер профиля узла.
pub trait NodeProfiler: Send + Sync {
    /// Собрать текущий профиль.
    fn profile(&self) -> NodeProfile;
}

/// Заглушка: детерминированные значения (для тестов и CI).
pub struct StubProfiler {
    pub profile_value: NodeProfile,
}

impl StubProfiler {
    pub fn desktop() -> Self {
        Self {
            profile_value: NodeProfile {
                ram_total_mb: 16384,
                ram_available_mb: 8192,
                cpu_cores: 8,
                cpu_usage_percent: 15.0,
                has_gpu: true,
                has_npu: false,
                battery_percent: None,
                battery_charging: None,
                thermal_status: ThermalStatus::Nominal,
                resources: NodeResources {
                    telegram: false,
                    email: true,
                    whatsapp: false,
                    llm_local: true,
                    llm_cloud: true,
                    memory_store: true,
                    approval_store: true,
                },
            },
        }
    }

    pub fn android() -> Self {
        Self {
            profile_value: NodeProfile {
                ram_total_mb: 4096,
                ram_available_mb: 2048,
                cpu_cores: 8,
                cpu_usage_percent: 30.0,
                has_gpu: false,
                has_npu: true,
                battery_percent: Some(75),
                battery_charging: Some(false),
                thermal_status: ThermalStatus::Fair,
                resources: NodeResources {
                    telegram: true,
                    email: false,
                    whatsapp: false,
                    llm_local: false,
                    llm_cloud: true,
                    memory_store: false,
                    approval_store: true,
                },
            },
        }
    }
}

impl NodeProfiler for StubProfiler {
    fn profile(&self) -> NodeProfile {
        self.profile_value.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_desktop_profile() {
        let p = StubProfiler::desktop().profile();
        assert_eq!(p.cpu_cores, 8);
        assert!(p.has_gpu);
        assert!(p.battery_percent.is_none());
        assert_eq!(p.thermal_status, ThermalStatus::Nominal);
        assert!(p.resources.has("llm_local"));
        assert!(!p.resources.has("whatsapp"));
    }

    #[test]
    fn stub_android_profile() {
        let p = StubProfiler::android().profile();
        assert_eq!(p.battery_percent, Some(75));
        assert!(!p.battery_charging.expect("charging flag"));
        assert!(p.resources.has("telegram"));
        assert!(!p.resources.has("llm_local"));
    }
}
