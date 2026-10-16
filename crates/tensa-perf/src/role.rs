//! Определение роли узла и возможностей (ТЗ tensa-perf: heavy/thin;
//! пороги — из [`PerfConfig`]).
//!
//! Правило heavy по ТЗ: RAM ≥ порога, ядер ≥ порога, GPU (опционально),
//! батарея > порога разряда, thermal ≤ порога троттлинга. Иначе — thin.

use tensa_core::NodeRole;
use tensa_llama::ModelTier;
use tensa_nodes::ThermalStatus;

use crate::config::PerfConfig;
use crate::models::{DeviceProfile, NodeRoleDecision};

/// [derived] Минимум RAM для memory store (Lance + эмбеддер).
const MEMORY_MIN_RAM_MB: u64 = 4096;
/// [derived] Минимум RAM для почтового клиента.
const EMAIL_MIN_RAM_MB: u64 = 2048;

/// Порядок серьёзности термального статуса (для сравнения с порогом).
fn severity(status: ThermalStatus) -> u8 {
    match status {
        ThermalStatus::Nominal => 0,
        ThermalStatus::Fair => 1,
        ThermalStatus::Serious => 2,
        ThermalStatus::Critical => 3,
    }
}

/// Решить роль узла по профилю (чистая функция — тестируется без железа).
pub fn decide(config: &PerfConfig, profile: &DeviceProfile) -> NodeRoleDecision {
    let mut blockers: Vec<String> = Vec::new();

    if profile.ram_total_mb < config.heavy_ram_threshold_mb {
        blockers.push(format!(
            "RAM {} МБ < {} МБ",
            profile.ram_total_mb, config.heavy_ram_threshold_mb
        ));
    }
    if profile.cpu_cores < config.heavy_cpu_cores_threshold {
        blockers.push(format!(
            "ядер {} < {}",
            profile.cpu_cores, config.heavy_cpu_cores_threshold
        ));
    }
    if config.heavy_gpu_required && profile.gpu.is_none() {
        blockers.push("нет GPU (heavy_gpu_required = true)".to_string());
    }
    if let (Some(percent), Some(false)) = (profile.battery_percent, profile.battery_charging) {
        if percent <= config.thin_battery_threshold_percent {
            blockers.push(format!(
                "батарея {percent}% ≤ {}% (не на зарядке)",
                config.thin_battery_threshold_percent
            ));
        }
    }
    if severity(profile.thermal_status) > severity(config.thermal_throttle_threshold) {
        blockers.push(format!(
            "thermal {} > {}",
            profile.thermal_status.as_str(),
            config.thermal_throttle_threshold.as_str()
        ));
    }

    let heavy = blockers.is_empty();
    let role = if heavy {
        NodeRole::Heavy
    } else {
        NodeRole::Thin
    };
    let reason = if heavy {
        format!(
            "heavy: RAM {} МБ (≥ {}), ядер {} (≥ {}), thermal {}",
            profile.ram_total_mb,
            config.heavy_ram_threshold_mb,
            profile.cpu_cores,
            config.heavy_cpu_cores_threshold,
            profile.thermal_status.as_str()
        )
    } else {
        blockers.join("; ")
    };

    NodeRoleDecision {
        role,
        reason,
        can_run_llm: heavy,
        recommended_model_tier: recommend_model_tier(profile),
        can_run_memory: profile.ram_total_mb >= MEMORY_MIN_RAM_MB,
        can_run_telegram: true,
        can_run_email: profile.ram_total_mb >= EMAIL_MIN_RAM_MB,
    }
}

/// [derived] Тир локальной модели по объёму RAM: ≤ 8 ГБ — XS, 8–16 — S,
/// 16–32 — M, ≥ 32 — L. Точные требования уточняются на TASK-ENV-001
/// (после появления реальных GGUF-моделей).
fn recommend_model_tier(profile: &DeviceProfile) -> ModelTier {
    match profile.ram_total_mb {
        0..=8191 => ModelTier::Xs,
        8192..=16383 => ModelTier::S,
        16384..=32767 => ModelTier::M,
        _ => ModelTier::L,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GpuInfo;

    fn profile(ram_total_mb: u64, cpu_cores: u32) -> DeviceProfile {
        DeviceProfile {
            ram_total_mb,
            ram_available_mb: ram_total_mb / 2,
            cpu_cores,
            cpu_frequency_mhz: 3000,
            cpu_usage_percent: 10.0,
            gpu: None,
            npu: None,
            disk_total_gb: 512,
            disk_available_gb: 256,
            battery_percent: None,
            battery_charging: None,
            thermal_status: ThermalStatus::Nominal,
            os: "test".to_string(),
            arch: "test".to_string(),
        }
    }

    #[test]
    fn heavy_when_all_thresholds_pass() {
        let decision = decide(&PerfConfig::default(), &profile(16384, 8));
        assert_eq!(decision.role, NodeRole::Heavy);
        assert!(decision.can_run_llm);
        assert!(decision.reason.starts_with("heavy"));
        assert_eq!(decision.recommended_model_tier, ModelTier::M);
    }

    #[test]
    fn thin_when_ram_below_threshold() {
        let decision = decide(&PerfConfig::default(), &profile(4096, 8));
        assert_eq!(decision.role, NodeRole::Thin);
        assert!(!decision.can_run_llm);
        assert!(decision.reason.contains("RAM"));
        assert_eq!(decision.recommended_model_tier, ModelTier::Xs);
        assert!(decision.can_run_email);
    }

    #[test]
    fn thin_when_cores_below_threshold() {
        let decision = decide(&PerfConfig::default(), &profile(32768, 2));
        assert_eq!(decision.role, NodeRole::Thin);
        assert!(decision.reason.contains("ядер"));
        assert_eq!(decision.recommended_model_tier, ModelTier::L);
    }

    #[test]
    fn thin_when_battery_low_and_discharging() {
        let mut device = profile(16384, 8);
        device.battery_percent = Some(15);
        device.battery_charging = Some(false);
        let decision = decide(&PerfConfig::default(), &device);
        assert_eq!(decision.role, NodeRole::Thin);
        assert!(decision.reason.contains("батарея"));
    }

    #[test]
    fn heavy_when_battery_low_but_charging() {
        let mut device = profile(16384, 8);
        device.battery_percent = Some(15);
        device.battery_charging = Some(true);
        let decision = decide(&PerfConfig::default(), &device);
        assert_eq!(decision.role, NodeRole::Heavy);
    }

    #[test]
    fn thin_when_thermal_above_threshold() {
        let mut device = profile(16384, 8);
        device.thermal_status = ThermalStatus::Critical;
        let decision = decide(&PerfConfig::default(), &device);
        assert_eq!(decision.role, NodeRole::Thin);
        assert!(decision.reason.contains("thermal"));
    }

    #[test]
    fn thermal_at_threshold_is_allowed() {
        let mut device = profile(16384, 8);
        device.thermal_status = ThermalStatus::Serious;
        let decision = decide(&PerfConfig::default(), &device);
        assert_eq!(decision.role, NodeRole::Heavy);
    }

    #[test]
    fn gpu_requirement_blocks_when_configured() {
        let config = PerfConfig {
            heavy_gpu_required: true,
            ..PerfConfig::default()
        };
        let decision = decide(&config, &profile(16384, 8));
        assert_eq!(decision.role, NodeRole::Thin);

        let mut device = profile(16384, 8);
        device.gpu = Some(GpuInfo {
            name: "Test".to_string(),
            vram_mb: 8192,
            backend: crate::models::GpuBackend::Cuda,
        });
        let decision = decide(&config, &device);
        assert_eq!(decision.role, NodeRole::Heavy);
    }

    #[test]
    fn memory_flag_follows_ram() {
        assert!(!decide(&PerfConfig::default(), &profile(2048, 4)).can_run_memory);
        assert!(decide(&PerfConfig::default(), &profile(8192, 4)).can_run_memory);
        assert!(!decide(&PerfConfig::default(), &profile(1024, 4)).can_run_email);
    }
}
