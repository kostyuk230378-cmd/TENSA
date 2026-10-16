//! # tensa-perf
//!
//! Профилирование устройства и динамическое распределение ролей heavy/thin
//! (ТЗ tensa-perf; ADR-004 CPU-инференс, ADR-012/ADR-013 синхронизация и
//! выбор узла).
//!
//! - [`PerfConfig`] — пороги и интервалы (Контракт 4);
//! - [`DeviceProfile`] — RAM/CPU/GPU/NPU/диск/батарея/thermal/платформа;
//! - [`PerfProfiler`] + [`DefaultPerfProfiler`] — профиль, бенчмарк,
//!   решение о роли, фоновый мониторинг;
//! - [`role::decide`] — чистое решение heavy/thin по [`PerfConfig`].
//!
//! Статус источников: CPU/RAM/диски — `sysinfo`; батарея — Win32
//! (`GetSystemPowerStatus`) и sysfs; thermal — sysfs (Linux/Android);
//! GPU (NVML) и NPU подключаются вместе с нативным слоем (TASK-ENV-001),
//! до этого [`gpu::detection_available`]/[`npu::detection_available`] —
//! `false`, выдуманных данных нет.
//!
//! Ограничения ТЗ: не маршрутизирует, не синхронизирует, не логирует
//! секреты.

pub mod battery;
pub mod benchmark;
pub mod config;
pub mod cpu;
pub mod default;
pub mod error;
pub mod gpu;
pub mod memory;
pub mod models;
pub mod npu;
pub mod profiler;
pub mod role;
pub mod thermal;

pub use config::PerfConfig;
pub use default::DefaultPerfProfiler;
pub use error::{PerfError, Result};
pub use models::{
    BenchmarkResult, DeviceProfile, GpuBackend, GpuInfo, NodeRoleDecision, NpuBackend, NpuInfo,
};
pub use profiler::PerfProfiler;

// Общие типы — единый источник (не дублировать).
pub use tensa_core::NodeRole;
pub use tensa_llama::ModelTier;
pub use tensa_nodes::ThermalStatus;

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
