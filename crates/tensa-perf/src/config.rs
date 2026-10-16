//! Конфигурация профилирования (ТЗ tensa-perf; Контракт 4).

use serde::{Deserialize, Serialize};
use tensa_nodes::ThermalStatus;

use crate::error::{PerfError, Result};

/// Порог термального троттлинга по умолчанию (ТЗ: heavy при thermal ≤ Serious).
fn default_thermal_threshold() -> ThermalStatus {
    ThermalStatus::Serious
}

/// Конфигурация профилировщика (ТЗ tensa-perf: `PerfConfig`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfConfig {
    /// Интервал фонового мониторинга, секунды (ТЗ: 60).
    pub profile_interval_seconds: u64,
    /// Порог RAM для heavy-узла, МБ (ТЗ: 8192).
    pub heavy_ram_threshold_mb: u64,
    /// Порог ядер CPU для heavy-узла (ТЗ: 4).
    pub heavy_cpu_cores_threshold: u32,
    /// Требовать GPU для heavy-узла (ТЗ: false).
    pub heavy_gpu_required: bool,
    /// Порог батареи (разряд) для thin-узла, % (ТЗ: 20).
    pub thin_battery_threshold_percent: u8,
    /// Предельная термическая ступень для heavy (ТЗ: thermal ≤ Serious).
    #[serde(default = "default_thermal_threshold")]
    pub thermal_throttle_threshold: ThermalStatus,
    /// Запускать бенчмарк при старте (ТЗ: true).
    pub benchmark_on_startup: bool,
}

impl Default for PerfConfig {
    fn default() -> Self {
        Self {
            profile_interval_seconds: 60,
            heavy_ram_threshold_mb: 8192,
            heavy_cpu_cores_threshold: 4,
            heavy_gpu_required: false,
            thin_battery_threshold_percent: 20,
            thermal_throttle_threshold: default_thermal_threshold(),
            benchmark_on_startup: true,
        }
    }
}

impl PerfConfig {
    /// Проверить конфигурацию (пороги из ТЗ: неотрицательные, интервал ≥ 1).
    pub fn validate(&self) -> Result<()> {
        if self.profile_interval_seconds == 0 {
            return Err(PerfError::Config(
                "profile_interval_seconds должен быть ≥ 1".to_string(),
            ));
        }
        if self.heavy_ram_threshold_mb == 0 {
            return Err(PerfError::Config(
                "heavy_ram_threshold_mb должен быть > 0".to_string(),
            ));
        }
        if self.heavy_cpu_cores_threshold == 0 {
            return Err(PerfError::Config(
                "heavy_cpu_cores_threshold должен быть > 0".to_string(),
            ));
        }
        if self.thin_battery_threshold_percent > 100 {
            return Err(PerfError::Config(
                "thin_battery_threshold_percent должен быть ≤ 100".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let config = PerfConfig::default();
        assert_eq!(config.profile_interval_seconds, 60);
        assert_eq!(config.heavy_ram_threshold_mb, 8192);
        assert_eq!(config.heavy_cpu_cores_threshold, 4);
        assert!(!config.heavy_gpu_required);
        assert_eq!(config.thin_battery_threshold_percent, 20);
        assert_eq!(config.thermal_throttle_threshold, ThermalStatus::Serious);
        assert!(config.benchmark_on_startup);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validation_rejects_bad_values() {
        let config = PerfConfig {
            profile_interval_seconds: 0,
            ..PerfConfig::default()
        };
        assert!(matches!(config.validate(), Err(PerfError::Config(_))));

        let config = PerfConfig {
            heavy_ram_threshold_mb: 0,
            ..PerfConfig::default()
        };
        assert!(config.validate().is_err());

        let config = PerfConfig {
            heavy_cpu_cores_threshold: 0,
            ..PerfConfig::default()
        };
        assert!(config.validate().is_err());

        let config = PerfConfig {
            thin_battery_threshold_percent: 101,
            ..PerfConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let config = PerfConfig::default();
        let json = serde_json::to_string(&config).expect("сериализация");
        let back: PerfConfig = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(config, back);
    }
}
