//! Координатор TensaCore (ТЗ tensa-core; доп. ТЗ: первый запуск и память).
//!
//! ТЗ-конструктор — [`TensaCore::new`] по пути к `config.toml` (Контракт 4:
//! один файл `~/.tensa/config.toml`, корень данных — каталог конфига).
//! Подсистемы, чьи крейты сами зависят от `tensa-core` (memory, sync,
//! router, perf, tools), подключаются снаружи через трейты и `Arc<dyn ...>`
//! — без циклов зависимостей.

use crate::config::Config;
use crate::error::{CoreError, Result};
use crate::first_run::FirstRunWizard;
use crate::health::{ComponentHealth, HealthStatus};
use crate::init::{InitParams, MemoryInitializer};
use crate::paths::TensaPaths;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::info;

/// Ядро Tensa: конфиг, хранилище одобрений, память, мастер первого запуска.
#[derive(Clone)]
pub struct TensaCore {
    paths: TensaPaths,
    config_path: PathBuf,
    config: Arc<RwLock<Config>>,
    approval_store: Arc<dyn tensa_approval::ApprovalStore>,
    memory: Option<Arc<dyn MemoryInitializer>>,
    started_at: Option<Instant>,
    running: Arc<AtomicBool>,
}

impl TensaCore {
    /// ТЗ-конструктор: собрать ядро по пути к `config.toml`.
    /// Открывает (или создаёт) ApprovalStore в `<root>/approval.db`.
    pub async fn new(config_path: &Path) -> Result<Self> {
        let root = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let paths = TensaPaths::with_root(root);
        paths.ensure_dirs()?;

        let config = if config_path.exists() {
            Config::load(config_path)?
        } else {
            Config::default()
        };

        let approval_config = tensa_approval::ApprovalConfig::from(&config.approval);
        let store = tensa_approval::SqliteApprovalStore::new(
            paths.approval_db(),
            approval_config,
            config.general.node_id.clone(),
        )
        .await?;

        Ok(Self {
            paths,
            config_path: config_path.to_path_buf(),
            config: Arc::new(RwLock::new(config)),
            approval_store: Arc::new(store),
            memory: None,
            started_at: None,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Собрать ядро из готовых частей (тесты, портативный режим).
    pub fn from_parts(
        config: Config,
        approval_store: Arc<dyn tensa_approval::ApprovalStore>,
    ) -> Self {
        let paths = TensaPaths::new();
        let config_path = paths.config_file();
        Self {
            paths,
            config_path,
            config: Arc::new(RwLock::new(config)),
            approval_store,
            memory: None,
            started_at: None,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Подключить инициализатор core memory (реализует tensa-memory).
    pub fn with_memory(mut self, memory: Arc<dyn MemoryInitializer>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Пути Tensa (`~/.tensa/...`).
    pub fn paths(&self) -> &TensaPaths {
        &self.paths
    }

    /// Путь к `config.toml`.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Первый ли это запуск?
    /// [derived] признак — отсутствие `config.toml` (доп. ТЗ: «заполненный
    /// config.toml + secrets.toml + core memory» как результат мастера).
    pub async fn is_first_run(&self) -> Result<bool> {
        Ok(!self.config_path.exists())
    }

    /// Создать мастер первого запуска.
    /// [derived] ТЗ: «запустить first-run wizard» — возвращаем объект
    /// мастера, шаги проходятся UI по одному (что и требует мастер).
    pub async fn run_first_run_wizard(&self) -> Result<FirstRunWizard> {
        let node_id = self.config.read().await.general.node_id.clone();
        FirstRunWizard::new(
            self.config_path.clone(),
            self.paths.secrets_file(),
            self.memory.clone(),
            node_id,
        )
        .await
    }

    /// Инициализировать core memory (ТЗ: `init_memory(params)`).
    pub async fn init_memory(&self, params: InitParams) -> Result<()> {
        match &self.memory {
            Some(memory) => memory.init_default(&params).await,
            None => Err(CoreError::Config(
                "memory initializer не подключён (with_memory)".into(),
            )),
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

    /// ТЗ: `reload_config()` — перечитать конфиг из файла.
    pub async fn reload_config(&self) -> Result<()> {
        let config = Config::load(&self.config_path)?;
        *self.config.write().await = config;
        info!("config reloaded from file");
        Ok(())
    }

    /// Применить конфиг извне (тесты, UI) — с валидацией.
    pub async fn update_config(&self, config: Config) -> Result<()> {
        config.validate()?;
        *self.config.write().await = config;
        info!("config updated");
        Ok(())
    }

    /// Health-статус ядра (ТЗ: HealthStatus).
    pub async fn health(&self, version: &str) -> Result<HealthStatus> {
        let mut components: HashMap<String, ComponentHealth> = HashMap::new();
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

    async fn runtime() -> TensaCore {
        let store = SqliteApprovalStore::init_with_config(ApprovalConfig::default())
            .await
            .expect("store");
        TensaCore::from_parts(Config::default(), Arc::new(store))
    }

    #[tokio::test]
    async fn start_stop_lifecycle() {
        let mut core = runtime().await;
        assert!(!core.is_running());
        core.start().await.expect("start");
        assert!(core.is_running());
        core.start().await.expect("start2");
        core.stop().await.expect("stop");
        assert!(!core.is_running());
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
    async fn update_config_validates() {
        let core = runtime().await;
        let mut cfg = core.config().await;
        cfg.general.node_id = "not-a-uuid".to_string();
        assert!(core.update_config(cfg).await.is_err());
    }

    #[tokio::test]
    async fn reload_config_reads_file() {
        let root = std::env::temp_dir().join(format!("tensa-core-reload-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("dirs");
        let config_path = root.join("config.toml");
        let node_id = uuid::Uuid::new_v4().to_string();
        Config::default_with_node(&node_id)
            .save(&config_path)
            .expect("save");

        let core = TensaCore::new(&config_path).await.expect("core");
        assert!(!core.is_first_run().await.expect("first run"));
        core.reload_config().await.expect("reload");
        assert_eq!(core.config().await.general.node_id, node_id);
        let _ = std::fs::remove_dir_all(root);
    }
}
