//! Детекция NPU (ТЗ tensa-perf: NNAPI/CoreML/DirectML).
//!
//! Платформенные бекенды (NNAPI — Android, CoreML — Apple, DirectML —
//! Windows, LiteRT — ТЗ tensa-llm) подключаются по мере реализации
//! соответствующих приложений. До этого [`detect`] честно возвращает
//! `None`, а [`detection_available`] — `false`.

use crate::models::NpuInfo;

/// Доступна ли реальная детекция NPU в текущей сборке.
pub fn detection_available() -> bool {
    false
}

/// Определить NPU (до платформенных бекендов — всегда `None`, см. модуль).
pub fn detect() -> Option<NpuInfo> {
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
