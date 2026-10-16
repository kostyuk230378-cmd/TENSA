//! Сбор CPU-метрик (sysinfo; ТЗ tensa-perf).
//!
//! Загрузка CPU считается как дельта между двумя выборками, поэтому
//! [`snapshot`] занимает ~200 мс и вызывается из blocking-контекста.

use sysinfo::System;

/// Снимок CPU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuSnapshot {
    /// Логических ядер.
    pub cores: u32,
    /// Максимальная частота среди ядер, МГц (0 — неизвестно).
    pub frequency_mhz: u32,
    /// Средняя загрузка, % (0–100).
    pub usage_percent: f32,
}

/// Собрать снимок CPU (блокирует ~200 мс).
pub fn snapshot() -> CpuSnapshot {
    let mut system = System::new();
    system.refresh_cpu_all();

    let cores = system.cpus().len().max(1) as u32;
    let frequency_mhz = system
        .cpus()
        .iter()
        .map(|cpu| cpu.frequency())
        .max()
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;

    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();
    let usage_percent = system.global_cpu_usage();

    CpuSnapshot {
        cores,
        frequency_mhz,
        usage_percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_sane() {
        let snapshot = snapshot();
        assert!(snapshot.cores >= 1);
        assert!((0.0..=100.0).contains(&snapshot.usage_percent));
    }
}
