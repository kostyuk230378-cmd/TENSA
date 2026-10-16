//! Health-статус ядра (ТЗ tensa-core, раздел «Health»).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Состояние одного компонента.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Компонент работает.
    pub healthy: bool,
    /// Пояснение (при деградации/ошибке).
    pub detail: Option<String>,
}

/// Сводный статус здоровья ядра.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub version: String,
    /// Аптайм в секундах (строка формата — на UI).
    pub uptime: String,
    pub degraded: bool,
    pub degraded_reason: Option<String>,
    /// Компонент -> состояние.
    pub components: HashMap<String, ComponentHealth>,
}

impl HealthStatus {
    /// Собрать статус из компонентов: healthy — все здоровы;
    /// degraded — есть нездоровые необязательные.
    pub fn from_components(
        version: &str,
        started_at: i64,
        components: HashMap<String, ComponentHealth>,
    ) -> Self {
        let all_healthy = components.values().all(|c| c.healthy);
        let degraded_parts: Vec<String> = components
            .iter()
            .filter(|(_, c)| !c.healthy)
            .map(|(name, _)| name.clone())
            .collect();
        let uptime_secs = (chrono::Utc::now().timestamp() - started_at).max(0);
        Self {
            healthy: all_healthy,
            version: version.to_string(),
            uptime: format_uptime(uptime_secs),
            degraded: !all_healthy,
            degraded_reason: if all_healthy {
                None
            } else {
                Some(format!("unhealthy: {}", degraded_parts.join(", ")))
            },
            components,
        }
    }
}

fn format_uptime(secs: i64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m {s}s")
    } else {
        format!("{m}m {s}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_healthy() {
        let mut comps = HashMap::new();
        comps.insert(
            "db".into(),
            ComponentHealth {
                healthy: true,
                detail: None,
            },
        );
        let h = HealthStatus::from_components("0.1.0", 0, comps);
        assert!(h.healthy);
        assert!(!h.degraded);
        assert!(h.degraded_reason.is_none());
    }

    #[test]
    fn degraded_lists_components() {
        let mut comps = HashMap::new();
        comps.insert(
            "db".into(),
            ComponentHealth {
                healthy: true,
                detail: None,
            },
        );
        comps.insert(
            "llm".into(),
            ComponentHealth {
                healthy: false,
                detail: Some("model not loaded".into()),
            },
        );
        let h = HealthStatus::from_components("0.1.0", 0, comps);
        assert!(!h.healthy);
        assert!(h.degraded);
        assert!(h.degraded_reason.expect("reason").contains("llm"));
    }

    #[test]
    fn uptime_formatting() {
        assert_eq!(format_uptime(65), "1m 5s");
        assert_eq!(format_uptime(3661), "1h 1m 1s");
        assert_eq!(format_uptime(90061), "1d 1h 1m");
    }
}
