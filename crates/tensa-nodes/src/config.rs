//! Конфигурация реестра узлов.

/// Настройки реестра.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodesConfig {
    /// Интервал heartbeat, секунд (30).
    pub heartbeat_interval_seconds: u64,
    /// Таймаут офлайна, секунд (90).
    pub heartbeat_timeout_seconds: u64,
    /// TTL аренды соединений, секунд (900 = 15 минут).
    pub lease_ttl_seconds: u64,
}

impl Default for NodesConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_seconds: crate::heartbeat::HEARTBEAT_INTERVAL_SECONDS,
            heartbeat_timeout_seconds: crate::heartbeat::HEARTBEAT_TIMEOUT_SECONDS,
            lease_ttl_seconds: crate::models::LEASE_TTL_SECONDS as u64,
        }
    }
}

impl NodesConfig {
    /// Валидация (heartbeat + согласованность TTL).
    pub fn validate(&self) -> crate::error::Result<()> {
        crate::heartbeat::validate(
            self.heartbeat_interval_seconds,
            self.heartbeat_timeout_seconds,
        )?;
        if self.lease_ttl_seconds < 60 {
            return Err(crate::error::NodesError::Config(
                "lease_ttl_seconds must be >= 60".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_valid() {
        NodesConfig::default().validate().expect("valid");
    }

    #[test]
    fn short_lease_rejected() {
        let cfg = NodesConfig {
            lease_ttl_seconds: 30,
            ..NodesConfig::default()
        };
        assert!(cfg.validate().is_err());
    }
}
