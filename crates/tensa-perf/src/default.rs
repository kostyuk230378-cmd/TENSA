//! Реализация профилировщика по умолчанию: `DefaultPerfProfiler`
//! (ТЗ tensa-perf: sysinfo + платформенные источники).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tensa_core::paths::TensaPaths;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::benchmark;
use crate::config::PerfConfig;
use crate::error::{PerfError, Result};
use crate::models::{BenchmarkResult, DeviceProfile, NodeRoleDecision};
use crate::profiler::PerfProfiler;
use crate::role;

/// Профилировщик по умолчанию.
pub struct DefaultPerfProfiler {
    config: PerfConfig,
    paths: TensaPaths,
    current: Arc<RwLock<Option<DeviceProfile>>>,
    monitor: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl DefaultPerfProfiler {
    /// Создать профилировщик (пути по умолчанию `~/.tensa/`).
    pub fn new(config: PerfConfig) -> Result<Self> {
        Self::with_paths(config, TensaPaths::new())
    }

    /// Создать профилировщик с явными путями (тесты, портативный режим).
    pub fn with_paths(config: PerfConfig, paths: TensaPaths) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            paths,
            current: Arc::new(RwLock::new(None)),
            monitor: Arc::new(Mutex::new(None)),
        })
    }

    /// Конфигурация профилировщика.
    pub fn config(&self) -> &PerfConfig {
        &self.config
    }

    /// Пути Tensa.
    pub fn paths(&self) -> &TensaPaths {
        &self.paths
    }
}

/// Собрать профиль устройства (блокирующая часть; вызывать через
/// `spawn_blocking`).
fn collect_profile() -> DeviceProfile {
    let cpu = crate::cpu::snapshot();
    let memory = crate::memory::snapshot();
    let disk = crate::memory::disk_snapshot();
    let battery = crate::battery::snapshot();

    DeviceProfile {
        ram_total_mb: memory.total_mb,
        ram_available_mb: memory.available_mb,
        cpu_cores: cpu.cores,
        cpu_frequency_mhz: cpu.frequency_mhz,
        cpu_usage_percent: cpu.usage_percent,
        gpu: crate::gpu::detect(),
        npu: crate::npu::detect(),
        disk_total_gb: disk.total_gb,
        disk_available_gb: disk.available_gb,
        battery_percent: battery.percent,
        battery_charging: battery.charging,
        thermal_status: crate::thermal::read_status(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

/// Собрать профиль в blocking-пуле tokio.
async fn profile_blocking() -> Result<DeviceProfile> {
    tokio::task::spawn_blocking(collect_profile)
        .await
        .map_err(|error| PerfError::Profile(format!("сбор профиля прерван: {error}")))
}

#[async_trait]
impl PerfProfiler for DefaultPerfProfiler {
    async fn profile(&self) -> Result<DeviceProfile> {
        let profile = profile_blocking().await?;
        *self.current.write().await = Some(profile.clone());
        Ok(profile)
    }

    async fn benchmark(&self) -> Result<BenchmarkResult> {
        let paths = self.paths.clone();
        let joined = tokio::task::spawn_blocking(move || benchmark::run(&paths.root()))
            .await
            .map_err(|error| PerfError::Benchmark(format!("бенчмарк прерван: {error}")))?;
        let result = joined?;
        benchmark::save_cached(&self.paths, &result)?;
        Ok(result)
    }

    async fn decide_role(&self, profile: &DeviceProfile) -> Result<NodeRoleDecision> {
        Ok(role::decide(&self.config, profile))
    }

    async fn start_monitoring(&self) -> Result<()> {
        let mut guard = self.monitor.lock().await;
        if guard.is_some() {
            return Err(PerfError::Monitoring("мониторинг уже запущен".to_string()));
        }

        let interval = Duration::from_secs(self.config.profile_interval_seconds.max(1));
        let current = Arc::clone(&self.current);
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                match tokio::task::spawn_blocking(collect_profile).await {
                    Ok(profile) => {
                        *current.write().await = Some(profile);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "профилирование в мониторинге прервано");
                    }
                }
            }
        });

        *guard = Some(handle);
        tracing::debug!(
            interval_seconds = self.config.profile_interval_seconds,
            "мониторинг профиля запущен"
        );
        Ok(())
    }

    async fn stop_monitoring(&self) -> Result<()> {
        let mut guard = self.monitor.lock().await;
        match guard.take() {
            Some(handle) => {
                handle.abort();
                tracing::debug!("мониторинг профиля остановлен");
                Ok(())
            }
            None => Err(PerfError::Monitoring("мониторинг не запущен".to_string())),
        }
    }

    async fn current_profile(&self) -> Result<DeviceProfile> {
        if let Some(profile) = self.current.read().await.clone() {
            return Ok(profile);
        }
        self.profile().await
    }
}
