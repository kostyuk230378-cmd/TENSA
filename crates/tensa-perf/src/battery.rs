//! Батарея (ТЗ tensa-perf: батарея/windows — Win32, Linux/Android — sysfs).
//!
//! Отсутствие батареи — не ошибка: `None` в обоих полях (десктоп).

/// Снимок батареи.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatterySnapshot {
    /// Заряд, % (None — батареи нет или значение недоступно).
    pub percent: Option<u8>,
    /// Идёт ли зарядка (None — неизвестно).
    pub charging: Option<bool>,
}

/// Есть ли на платформе источник данных о батарее.
pub fn supported() -> bool {
    cfg!(any(windows, target_os = "linux", target_os = "android"))
}

/// Собрать снимок батареи.
#[cfg(windows)]
pub fn snapshot() -> BatterySnapshot {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    // SAFETY: SYSTEM_POWER_STATUS — POD-структура, нулевое значение допустимо.
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: вызов пишет в локальную структуру; указатель валиден на время вызова.
    let ok = unsafe { GetSystemPowerStatus(&mut status) };
    if ok == 0 {
        return BatterySnapshot {
            percent: None,
            charging: None,
        };
    }

    const BATTERY_FLAG_CHARGING: u8 = 8;
    const BATTERY_FLAG_NO_BATTERY: u8 = 128;
    const BATTERY_FLAG_UNKNOWN: u8 = 255;

    if status.BatteryFlag == BATTERY_FLAG_NO_BATTERY || status.BatteryFlag == BATTERY_FLAG_UNKNOWN {
        return BatterySnapshot {
            percent: None,
            charging: None,
        };
    }

    let percent = (status.BatteryLifePercent <= 100).then_some(status.BatteryLifePercent);
    let charging = match status.ACLineStatus {
        0 => Some(false),
        1 => Some(true),
        _ => Some(status.BatteryFlag & BATTERY_FLAG_CHARGING != 0),
    };

    BatterySnapshot { percent, charging }
}

/// Собрать снимок батареи (Linux/Android: `/sys/class/power_supply/BAT*`).
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn snapshot() -> BatterySnapshot {
    let base = std::path::Path::new("/sys/class/power_supply");
    let Ok(entries) = std::fs::read_dir(base) else {
        return BatterySnapshot {
            percent: None,
            charging: None,
        };
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("BAT") {
            continue;
        }
        let percent = std::fs::read_to_string(entry.path().join("capacity"))
            .ok()
            .and_then(|raw| raw.trim().parse::<u8>().ok())
            .filter(|value| *value <= 100);
        let charging = std::fs::read_to_string(entry.path().join("status"))
            .ok()
            .map(|raw| matches!(raw.trim(), "Charging" | "Full"));
        return BatterySnapshot { percent, charging };
    }

    BatterySnapshot {
        percent: None,
        charging: None,
    }
}

/// Собрать снимок батареи (прочие платформы: источник отсутствует).
#[cfg(not(any(windows, target_os = "linux", target_os = "android")))]
pub fn snapshot() -> BatterySnapshot {
    BatterySnapshot {
        percent: None,
        charging: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_consistent() {
        let snapshot = snapshot();
        if let Some(percent) = snapshot.percent {
            assert!(percent <= 100);
        }
        assert_eq!(
            supported(),
            cfg!(any(windows, target_os = "linux", target_os = "android"))
        );
    }
}
