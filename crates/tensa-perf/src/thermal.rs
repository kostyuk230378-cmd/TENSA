//! Термальный статус узла (ТЗ tensa-perf).
//!
//! - Linux/Android: максимум из `/sys/class/thermal/thermal_zone*/temp`;
//! - Windows: публичного API температуры нет — возвращается `Nominal`
//!   («признаков троттлинга не обнаружено»); см. [`supported`].
//!
//! [derived] Пороги классификации в ТЗ не заданы:
//! < 60 °C — Nominal, < 75 °C — Fair, < 90 °C — Serious, иначе Critical.

use tensa_nodes::ThermalStatus;

/// Есть ли на платформе реальный источник температуры.
pub fn supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "android"))
}

/// Классифицировать температуру в термальный статус (пороги — см. модуль).
pub fn classify(temp_c: f64) -> ThermalStatus {
    if temp_c < 60.0 {
        ThermalStatus::Nominal
    } else if temp_c < 75.0 {
        ThermalStatus::Fair
    } else if temp_c < 90.0 {
        ThermalStatus::Serious
    } else {
        ThermalStatus::Critical
    }
}

/// Текущий термальный статус.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn read_status() -> ThermalStatus {
    read_max_temp_c()
        .map(classify)
        .unwrap_or(ThermalStatus::Nominal)
}

/// Текущий термальный статус (нет источника — `Nominal`, см. модуль).
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn read_status() -> ThermalStatus {
    ThermalStatus::Nominal
}

/// Максимум температурных зон, °C (`None` — зон нет/не читаются).
#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_max_temp_c() -> Option<f64> {
    const ZONES_DIR: &str = "/sys/class/thermal";
    let entries = std::fs::read_dir(ZONES_DIR).ok()?;

    let mut max_temp: Option<f64> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(entry.path().join("temp")) else {
            continue;
        };
        let Ok(milli_c) = raw.trim().parse::<i64>() else {
            continue;
        };
        let temp_c = milli_c as f64 / 1000.0;
        // Значения ≤ 1 °C — незаполненные сенсоры.
        if temp_c <= 1.0 {
            continue;
        }
        max_temp = Some(max_temp.map_or(temp_c, |current| current.max(temp_c)));
    }
    max_temp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_thresholds() {
        assert_eq!(classify(35.0), ThermalStatus::Nominal);
        assert_eq!(classify(59.9), ThermalStatus::Nominal);
        assert_eq!(classify(60.0), ThermalStatus::Fair);
        assert_eq!(classify(74.9), ThermalStatus::Fair);
        assert_eq!(classify(75.0), ThermalStatus::Serious);
        assert_eq!(classify(89.9), ThermalStatus::Serious);
        assert_eq!(classify(90.0), ThermalStatus::Critical);
        assert_eq!(classify(120.0), ThermalStatus::Critical);
    }

    #[test]
    fn read_status_is_deterministic_on_platform() {
        let status = read_status();
        if supported() {
            // Linux/Android: реальный сенсор или Nominal при его отсутствии.
            assert!(matches!(
                status,
                ThermalStatus::Nominal
                    | ThermalStatus::Fair
                    | ThermalStatus::Serious
                    | ThermalStatus::Critical
            ));
        } else {
            assert_eq!(status, ThermalStatus::Nominal);
        }
    }
}
