//! Управление соединениями к разделяемым ресурсам (ADR-015: почта напрямую,
//! один активный доступ; ТЗ: node_connections, TTL 15 минут, очередь при
//! конфликте, освобождение при офлайн).

use crate::error::{Result, SyncError};
use std::collections::BTreeMap;
use tracing::info;

/// TTL аренды соединения по ТЗ.
pub const LEASE_TTL_SECONDS: i64 = 15 * 60;

/// Кому выдана аренда.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    /// node_id держателя.
    pub holder: String,
    /// account_id (для почты — конкретный ящик), если ресурс мног acct.
    pub account_id: Option<String>,
    /// Unix timestamp выдачи.
    pub acquired_at: i64,
}

/// Реестр аренд по ресурсам (ресурс: "email", "calendar", ...).
#[derive(Debug, Default)]
pub struct ConnectionManager {
    leases: BTreeMap<String, Lease>,
    /// Очередь ожидания: ресурс -> node_id в порядке запросов.
    waiting: BTreeMap<String, Vec<String>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Запросить соединение. Возвращает Ok(lease), если свободно;
    /// Err(NodeUnavailable) — ресурс занят (запрашивающий встаёт в очередь).
    pub fn acquire(
        &mut self,
        resource: &str,
        node_id: &str,
        account_id: Option<&str>,
        now_ts: i64,
    ) -> Result<Lease> {
        // Просроченные аренды освобождаем лениво.
        self.evict_expired(now_ts);

        match self.leases.get(resource) {
            Some(lease) if lease.holder != node_id => {
                let queue = self.waiting.entry(resource.to_string()).or_default();
                if !queue.iter().any(|n| n == node_id) {
                    queue.push(node_id.to_string());
                }
                Err(SyncError::NodeUnavailable(format!(
                    "resource '{resource}' held by {}",
                    lease.holder
                )))
            }
            Some(lease) => Ok(lease.clone()), // повторный запрос того же узла
            None => {
                let lease = Lease {
                    holder: node_id.to_string(),
                    account_id: account_id.map(|s| s.to_string()),
                    acquired_at: now_ts,
                };
                self.leases.insert(resource.to_string(), lease.clone());
                info!(resource, node_id, "connection lease acquired");
                Ok(lease)
            }
        }
    }

    /// Освободить соединение; следующему в очереди — попытка вне этого метода.
    pub fn release(&mut self, resource: &str, node_id: &str, now_ts: i64) -> Result<()> {
        self.evict_expired(now_ts);
        match self.leases.get(resource) {
            Some(lease) if lease.holder == node_id => {
                self.leases.remove(resource);
                info!(resource, node_id, "connection lease released");
                Ok(())
            }
            Some(lease) => Err(SyncError::Config(format!(
                "resource '{resource}' held by {}, not {node_id}",
                lease.holder
            ))),
            None => Err(SyncError::Config(format!(
                "resource '{resource}' not acquired"
            ))),
        }
    }

    /// Узел стал офлайн — все его аренды освобождаются (ТЗ).
    /// Возвращает освобождённые ресурсы.
    pub fn release_node(&mut self, node_id: &str) -> Vec<String> {
        let freed: Vec<String> = self
            .leases
            .iter()
            .filter(|(_, l)| l.holder == node_id)
            .map(|(r, _)| r.clone())
            .collect();
        for r in &freed {
            self.leases.remove(r);
        }
        // Из очередей ожидания тоже убираем.
        for queue in self.waiting.values_mut() {
            queue.retain(|n| n != node_id);
        }
        freed
    }

    /// Следующий в очереди на ресурс (после освобождения).
    pub fn next_in_queue(&mut self, resource: &str) -> Option<String> {
        self.waiting.get_mut(resource)?.pop()
    }

    /// Ленивое освобождение просроченных аренд.
    fn evict_expired(&mut self, now_ts: i64) {
        let expired: Vec<String> = self
            .leases
            .iter()
            .filter(|(_, l)| now_ts - l.acquired_at > LEASE_TTL_SECONDS)
            .map(|(r, _)| r.clone())
            .collect();
        for r in expired {
            if let Some(l) = self.leases.remove(&r) {
                tracing::warn!(resource = %r, holder = %l.holder, "lease expired, evicted");
            }
        }
    }

    /// Активная аренда ресурса.
    pub fn lease(&self, resource: &str) -> Option<&Lease> {
        self.leases.get(resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_release_cycle() {
        let mut m = ConnectionManager::new();
        let l = m
            .acquire("email", "node-1", Some("acc1"), 100)
            .expect("acquire");
        assert_eq!(l.holder, "node-1");
        assert!(m.release("email", "node-1", 110).is_ok());
        assert!(m.lease("email").is_none());
    }

    #[test]
    fn conflict_queues() {
        let mut m = ConnectionManager::new();
        m.acquire("email", "node-1", None, 100).expect("1");
        let err = m.acquire("email", "node-2", None, 101).expect_err("busy");
        assert!(matches!(err, SyncError::NodeUnavailable(_)));
        assert_eq!(m.next_in_queue("email"), Some("node-2".into()));
        m.release("email", "node-1", 105).expect("release");
        // node-2 может взять теперь.
        assert!(m.acquire("email", "node-2", None, 106).is_ok());
    }

    #[test]
    fn ttl_eviction() {
        let mut m = ConnectionManager::new();
        m.acquire("email", "node-1", None, 100).expect("1");
        // TTL 15 мин: на 100+901 аренда просрочена.
        assert!(m.lease("email").is_some());
        m.acquire("email", "node-2", None, 100 + LEASE_TTL_SECONDS + 1)
            .expect("2");
        assert_eq!(m.lease("email").expect("new").holder, "node-2");
    }

    #[test]
    fn offline_frees_leases() {
        let mut m = ConnectionManager::new();
        m.acquire("email", "node-1", None, 100).expect("1");
        m.acquire("calendar", "node-1", None, 100).expect("2");
        let freed = m.release_node("node-1");
        assert_eq!(freed.len(), 2);
        assert!(m.lease("email").is_none());
    }

    #[test]
    fn wrong_release_rejected() {
        let mut m = ConnectionManager::new();
        m.acquire("email", "node-1", None, 100).expect("1");
        assert!(m.release("email", "node-2", 101).is_err());
    }
}
