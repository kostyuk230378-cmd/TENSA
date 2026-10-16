//! Heartbeat-логика реестра: интервал 30 с, офлайн > 90 с (ТЗ).
//! Реестр хранит last_seen; здесь — валидация конфига и константы.

use crate::error::Result;

/// Интервал heartbeat по умолчанию (секунд).
pub const HEARTBEAT_INTERVAL_SECONDS: u64 = 30;
/// Таймаут офлайна по умолчанию (секунд).
pub const HEARTBEAT_TIMEOUT_SECONDS: u64 = 90;

/// Проверка согласованности heartbeat-конфигурации.
pub fn validate(interval_seconds: u64, timeout_seconds: u64) -> Result<()> {
    if interval_seconds == 0 {
        return Err(crate::error::NodesError::Config(
            "heartbeat interval must be > 0".into(),
        ));
    }
    if timeout_seconds <= interval_seconds {
        return Err(crate::error::NodesError::Config(
            "heartbeat timeout must be > interval".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_valid() {
        assert!(validate(HEARTBEAT_INTERVAL_SECONDS, HEARTBEAT_TIMEOUT_SECONDS).is_ok());
    }

    #[test]
    fn invalid_configs() {
        assert!(validate(0, 90).is_err());
        assert!(validate(90, 90).is_err());
        assert!(validate(90, 30).is_err());
    }
}
