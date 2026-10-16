//! Модели крейта (ТЗ tensa-perf: типы дословно).
//! Общие типы переиспользуются из других крейтов (не дублировать).

pub use tensa_core::NodeRole;
pub use tensa_llama::ModelTier;
pub use tensa_nodes::ThermalStatus;

use serde::{Deserialize, Serialize};

/// Бэкенд GPU.
/// [derived] В ТЗ перечислен только инструмент (nvml-wrapper), значения —
/// минимум для UI/логов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuBackend {
    Cuda,
    Rocm,
    Metal,
    Vulkan,
    DirectMl,
    Unknown,
}

/// Информация о GPU (ТЗ tensa-perf: `GpuInfo`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u64,
    pub backend: GpuBackend,
}

/// Бэкенд NPU.
/// [derived] Значения в ТЗ не перечислены (платформы: NNAPI/CoreML/DirectML,
/// LiteRT — ТЗ tensa-llm).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NpuBackend {
    Nnapi,
    CoreMl,
    DirectMl,
    Litert,
}

/// Информация о NPU (ТЗ tensa-perf: `NpuInfo`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpuInfo {
    pub name: String,
    pub backend: NpuBackend,
}

/// Профиль устройства (ТЗ tensa-perf: `DeviceProfile`, поля дословно).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub ram_total_mb: u64,
    pub ram_available_mb: u64,
    pub cpu_cores: u32,
    pub cpu_frequency_mhz: u32,
    pub cpu_usage_percent: f32,
    pub gpu: Option<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub disk_total_gb: u64,
    pub disk_available_gb: u64,
    pub battery_percent: Option<u8>,
    pub battery_charging: Option<bool>,
    pub thermal_status: ThermalStatus,
    pub os: String,
    pub arch: String,
}

/// Результат бенчмарка (ТЗ tensa-perf: `BenchmarkResult`, поля дословно).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub memory_bandwidth_mbps: f64,
    pub disk_read_mbps: f64,
    pub disk_write_mbps: f64,
    pub cpu_score: u32,
    pub timestamp: i64,
}

/// Решение о роли узла (ТЗ tensa-perf: `NodeRoleDecision`, поля дословно).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRoleDecision {
    pub role: NodeRole,
    pub reason: String,
    pub can_run_llm: bool,
    pub recommended_model_tier: ModelTier,
    pub can_run_memory: bool,
    pub can_run_telegram: bool,
    pub can_run_email: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> DeviceProfile {
        DeviceProfile {
            ram_total_mb: 16384,
            ram_available_mb: 8192,
            cpu_cores: 8,
            cpu_frequency_mhz: 3600,
            cpu_usage_percent: 12.5,
            gpu: Some(GpuInfo {
                name: "Test GPU".to_string(),
                vram_mb: 8192,
                backend: GpuBackend::Cuda,
            }),
            npu: None,
            disk_total_gb: 1024,
            disk_available_gb: 512,
            battery_percent: None,
            battery_charging: None,
            thermal_status: ThermalStatus::Nominal,
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
        }
    }

    #[test]
    fn profile_serde_roundtrip() {
        let profile = sample_profile();
        let json = serde_json::to_string(&profile).expect("сериализация");
        let back: DeviceProfile = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(profile, back);
    }

    #[test]
    fn decision_serde_uses_canonical_names() {
        let decision = NodeRoleDecision {
            role: NodeRole::Heavy,
            reason: "heavy: тест".to_string(),
            can_run_llm: true,
            recommended_model_tier: ModelTier::S,
            can_run_memory: true,
            can_run_telegram: true,
            can_run_email: true,
        };
        let json = serde_json::to_string(&decision).expect("сериализация");
        assert!(json.contains("\"heavy\""));
        assert!(json.contains("\"S\""));
    }
}
