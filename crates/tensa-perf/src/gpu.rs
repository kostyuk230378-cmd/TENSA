//! Детекция GPU (ТЗ tensa-perf: nvml-wrapper — опционально).
//!
//! Нативный слой NVML подключается вместе с нативным слоем llama.cpp
//! (задача TASK-ENV-001, ADR-003). До этого детекция честно возвращает
//! `None`, а [`detection_available`] — `false`: выдуманных данных нет
//! (`heavy_gpu_required` по умолчанию `false`, поэтому роли это не мешает).

use crate::models::GpuInfo;

/// Доступна ли реальная детекция GPU в текущей сборке.
pub fn detection_available() -> bool {
    false
}

/// Определить GPU (до TASK-ENV-001 — всегда `None`, см. модуль).
pub fn detect() -> Option<GpuInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_is_honest() {
        assert!(!detection_available());
        assert!(detect().is_none());
    }
}
