//! Координатор TensaCore (ТЗ tensa-core).
//!
//! Этап 1 (текущий): каркас с зависимостями только от реализованных крейтов
//! (tensa-approval). Подсистемы, чьи крейты ещё не созданы (memory, nodes,
//! sync-сервис, router, perf, tools), подключаются как `Arc<dyn Trait>`
//! после появления своих крейтов — добавление поля не меняет публичный API
//! старт/стоп/health.

use crate::config::Config;
use crate::error::Result;
use crate::health::{ComponentHealth, HealthStatus};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::info;

/// Время старта ядра (для аптайма).
#[derive(Clone)]
pub struct CoreRuntime {
    config: Arc<RwLock<Config>>,
    approval_store: Arc<dyn tensa_approval::ApprovalStore>,
    started_at: Option<Instant>,
    running: Arc<AtomicBool>,
}

impl CoreRuntime {
    /// Собрать ядро из конфига и хранилища одобрений.
    pub fn new(config: Config, approval_store: Arc<dyn tensa_approval::ApprovalStore>) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            approval_store,
            started_at: None,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Текущий конфиг (клон).
    pub async fn config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Хранилище одобрений.
    pub fn approval_store(&self) -> Arc<dyn tensa_approval::ApprovalStore> {
        self.approval_store.clone()
    }

    /// Ядро запущено?
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Запуск ядра (ТЗ: 11 шагов — здесь шаги реализованных подсистем,
    /// остальные добавляются по мере появления крейтов).
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        info!("core start: config -> db -> approval");
        // Конфиг уже загружен (в конструкторе); approval-БД открыта.
        self.started_at = Some(Instant::now());
        self.running.store(true, Ordering::SeqCst);
        info!("core running");
        Ok(())
    }

    /// Остановка (обратный порядок шагов).
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }
        info!("core stop: approval -> db -> config");
        self.running.store(false, Ordering::SeqCst);
        self.started_at = None;
        info!("core stopped");
        Ok(())
    }

    /// Hot-reload конфига (ТЗ: reload_config).
    pub async fn reload_config(&mut self, config: Config) -> Result<()> {
        config.validate()?;
        *self.config.write().await = config;
        info!("config reloaded");
        Ok(())
    }

    /// Health-статус ядра (ТЗ: HealthStatus).
    pub async fn health(&self, version: &str) -> Result<HealthStatus> {
        let mut components: HashMap<String, ComponentHealth> = HashMap::new();
        // approval: БД доступна, если get_pending отвечает.
        let approval_healthy = self.approval_store.get_pending().await.is_ok();
        components.insert(
            "approval".to_string(),
            ComponentHealth {
                healthy: approval_healthy,
                detail: (!approval_healthy).then(|| "approval store unreachable".to_string()),
            },
        );
        components.insert(
            "config".to_string(),
            ComponentHealth {
                healthy: true,
                detail: None,
            },
        );
        let started_at = self
            .started_at
            .map(|t| chrono::Utc::now().timestamp() - t.elapsed().as_secs() as i64);
        Ok(match started_at {
            Some(ts) => {
                let mut status = HealthStatus::from_components(version, ts, components);
                status.healthy &= self.is_running();
                status
            }
            None => {
                // Ядро не запущено — не healthy, аптайм 0.
                let mut status = HealthStatus::from_components(
                    version,
                    chrono::Utc::now().timestamp(),
                    components,
                );
                status.healthy = false;
                status.uptime = "0s".to_string();
                status
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tensa_approval::{ApprovalConfig, SqliteApprovalStore};

    async fn runtime() -> CoreRuntime {
        let store = SqliteApprovalStore::init_with_config(ApprovalConfig::default())
            .await
            .expect("store");
        CoreRuntime::new(Config::default(), Arc::new(store))
    }

    #[tokio::test]
    async fn start_stop_lifecycle() {
        let mut core = runtime().await;
        assert!(!core.is_running());
        core.start().await.expect("start");
        assert!(core.is_running());
        // Повторный старт — идемпотентен.
        core.start().await.expect("start2");
        core.stop().await.expect("stop");
        assert!(!core.is_running());
        // Повторная остановка — идемпотентна.
        core.stop().await.expect("stop2");
    }

    #[tokio::test]
    async fn health_when_running() {
        let mut core = runtime().await;
        core.start().await.expect("start");
        let h = core.health("test").await.expect("health");
        assert!(h.healthy);
        assert!(h.components.contains_key("approval"));
        assert!(h.components.contains_key("config"));
    }

    #[tokio::test]
    async fn reload_config_validates() {
        let mut core = runtime().await;
        let mut cfg = core.config().await;
        cfg.general.node_id = "not-a-uuid".to_string();
        assert!(core.reload_config(cfg).await.is_err());
    }
}
