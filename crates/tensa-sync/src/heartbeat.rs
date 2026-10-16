//! Heartbeat узлов (ТЗ: каждые 30 секунд, офлайн > 90 секунд;
//! Контракт 8: NodeService.StreamHeartbeats).

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::warn;

/// Heartbeat одного узла.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    /// node_id узла.
    pub node_id: String,
    /// Роль узла (Контракт 8: heavy/thin/auto). [derived] Строки вместо
    /// генерируемого enum — прото-типы появят с gRPC-слоем.
    pub role: String,
    /// Версия ПО узла.
    pub version: String,
    /// Unix timestamp отправки.
    pub timestamp: i64,
    /// Загрузка узла 0.0–1.0 (для выбора heavy при инференсе).
    pub load: Option<f64>,
}

/// Локальное состояние heartbeat'ов всех известных узлов.
/// Потокобезопасность обеспечивает владелец (в крейте — мьютекс снаружи).
#[derive(Debug, Default)]
pub struct HeartbeatState {
    /// node_id -> последний heartbeat.
    last_seen: BTreeMap<String, NodeHeartbeat>,
}

impl HeartbeatState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Зарегистрировать входящий heartbeat.
    pub fn observe(&mut self, hb: NodeHeartbeat) {
        self.last_seen.insert(hb.node_id.clone(), hb);
    }

    /// Узел онлайн? (heartbeat свежее timeout_seconds).
    pub fn is_online(&self, node_id: &str, timeout_seconds: u64, now_ts: i64) -> bool {
        match self.last_seen.get(node_id) {
            Some(hb) => now_ts - hb.timestamp <= timeout_seconds as i64,
            None => false,
        }
    }

    /// Список онлайн-узлов.
    pub fn online_nodes(&self, timeout_seconds: u64, now_ts: i64) -> Vec<&NodeHeartbeat> {
        self.last_seen
            .values()
            .filter(|hb| now_ts - hb.timestamp <= timeout_seconds as i64)
            .collect()
    }

    /// Узлы, считавшиеся онлайн, но молчащие дольше timeout — кандидаты
    /// на инвалидацию (освобождение их соединений, ADR-015).
    pub fn stale_nodes(&self, timeout_seconds: u64, now_ts: i64) -> Vec<String> {
        self.last_seen
            .iter()
            .filter(|(_, hb)| now_ts - hb.timestamp > timeout_seconds as i64)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Отметить устаревшие узлы в логе (вызывается из такта watchdog).
    pub fn report_stale(&self, timeout_seconds: u64, now_ts: i64) {
        for id in self.stale_nodes(timeout_seconds, now_ts) {
            warn!(node_id = %id, "node heartbeat timed out");
        }
    }

    pub fn total(&self) -> usize {
        self.last_seen.len()
    }

    /// Получить последний heartbeat узла.
    pub fn get(&self, node_id: &str) -> Option<&NodeHeartbeat> {
        self.last_seen.get(node_id)
    }
}

/// Проверка конфигурации heartbeat (ТЗ: interval < timeout).
pub fn validate_config(interval_seconds: u64, timeout_seconds: u64) -> Result<()> {
    if interval_seconds == 0 {
        return Err(crate::error::SyncError::Config(
            "heartbeat_interval_seconds must be > 0".into(),
        ));
    }
    if timeout_seconds <= interval_seconds {
        return Err(crate::error::SyncError::Config(
            "heartbeat_timeout_seconds must be > heartbeat_interval_seconds".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hb(node: &str, ts: i64) -> NodeHeartbeat {
        NodeHeartbeat {
            node_id: node.into(),
            role: "heavy".into(),
            version: "0.1.0".into(),
            timestamp: ts,
            load: Some(0.2),
        }
    }

    #[test]
    fn online_offline_by_timeout() {
        let mut state = HeartbeatState::new();
        state.observe(hb("a", 100));
        assert!(state.is_online("a", 90, 150));
        assert!(!state.is_online("a", 90, 191));
        assert!(!state.is_online("ghost", 90, 150));
    }

    #[test]
    fn stale_detection() {
        let mut state = HeartbeatState::new();
        state.observe(hb("a", 100));
        state.observe(hb("b", 180));
        let stale = state.stale_nodes(90, 200);
        assert_eq!(stale, vec!["a".to_string()]);
    }

    #[test]
    fn config_validation() {
        assert!(validate_config(30, 90).is_ok());
        assert!(validate_config(0, 90).is_err());
        assert!(validate_config(90, 90).is_err());
    }

    #[test]
    fn heartbeat_json_roundtrip() {
        let hb = hb("n1", 42);
        let json = serde_json::to_string(&hb).expect("ser");
        let back: NodeHeartbeat = serde_json::from_str(&json).expect("de");
        assert_eq!(back.node_id, "n1");
        assert_eq!(back.timestamp, 42);
    }
}
