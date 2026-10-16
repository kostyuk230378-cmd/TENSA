//! Журнал синхронизации: фиксация раундов и ошибок (без секретов).
//! Полный gRPC-транспорт появится с фичей `grpc` и крейтом tensa-nodes.

use crate::error::Result;
use crate::status::SyncStatus;
use tracing::info;
/// Протокол одного раунда синхронизации (вызывается по таймеру
/// `sync_interval_seconds` или по событию).
pub trait SyncRound {
    /// Имя раунда (для лога): "approval", "notes", "contacts".
    fn name(&self) -> &'static str;

    /// Выполнить раунд. Возвращает число перенесённых записей.
    fn run(&self) -> impl std::future::Future<Output = Result<usize>> + Send;
}

/// Учётчик состояния синхронизации — то, что видит `SyncStatus`.
#[derive(Debug, Default)]
pub struct SyncLedger {
    running: bool,
    last_sync_at: Option<i64>,
    rounds_ok: u64,
    rounds_failed: u64,
}

impl SyncLedger {
    pub fn mark_started(&mut self) {
        self.running = true;
        info!("sync service started");
    }

    pub fn mark_stopped(&mut self) {
        self.running = false;
        info!("sync service stopped");
    }

    pub fn mark_round(&mut self, ok: bool, now_ts: i64) {
        if ok {
            self.rounds_ok += 1;
            self.last_sync_at = Some(now_ts);
        } else {
            self.rounds_failed += 1;
        }
    }

    pub fn snapshot(&self, peers_online: usize, peers_total: usize) -> SyncStatus {
        SyncStatus {
            running: self.running,
            last_sync_at: self.last_sync_at,
            peers_online,
            peers_total,
            syncthing_status: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn rounds_ok(&self) -> u64 {
        self.rounds_ok
    }

    pub fn rounds_failed(&self) -> u64 {
        self.rounds_failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_lifecycle() {
        let mut l = SyncLedger::default();
        assert!(!l.is_running());
        l.mark_started();
        assert!(l.is_running());
        l.mark_round(true, 100);
        l.mark_round(false, 101);
        l.mark_round(true, 102);
        assert_eq!(l.rounds_ok(), 2);
        assert_eq!(l.rounds_failed(), 1);
        let s = l.snapshot(1, 2);
        assert!(s.running);
        assert_eq!(s.last_sync_at, Some(102));
        assert_eq!(s.peers_online, 1);
        assert_eq!(s.peers_total, 2);
        l.mark_stopped();
        assert!(!l.is_running());
    }
}
