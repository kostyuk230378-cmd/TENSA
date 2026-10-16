//! Трейт профилировщика (ТЗ tensa-perf: `PerfProfiler`, сигнатуры дословно).

use crate::error::Result;
use crate::models::{BenchmarkResult, DeviceProfile, NodeRoleDecision};

/// Провайдер профиля устройства, бенчмарка и роли узла.
#[async_trait::async_trait]
pub trait PerfProfiler: Send + Sync {
    /// Собрать текущий профиль устройства.
    async fn profile(&self) -> Result<DeviceProfile>;

    /// Выполнить бенчмарк (память, диск, CPU) и сохранить кэш.
    async fn benchmark(&self) -> Result<BenchmarkResult>;

    /// Решить роль узла по профилю (heavy/thin + возможности).
    async fn decide_role(&self, profile: &DeviceProfile) -> Result<NodeRoleDecision>;

    /// Запустить фоновый мониторинг (интервал — `PerfConfig`).
    async fn start_monitoring(&self) -> Result<()>;

    /// Остановить фоновый мониторинг.
    async fn stop_monitoring(&self) -> Result<()>;

    /// Текущий (последний собранный) профиль; при отсутствии — собрать.
    async fn current_profile(&self) -> Result<DeviceProfile>;
}
