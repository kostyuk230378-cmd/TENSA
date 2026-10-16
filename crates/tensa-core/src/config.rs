//! Конфигурация (Контракт 4, ТЗ tensa-core): `~/.tensa/config.toml`.
//!
//! Секреты в config.toml не хранятся — они в secrets.toml (отдельный файл,
//! в этой структуре отсутствует; поля-секреты конфига — только идентификаторы).

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Полная конфигурация Tensa (Контракт 4).
/// [derived] Секции brain/voice/memory/documents сейчас — «сырые» таблицы
/// TOML: их типизированные крейты (router/llm/memory/tools) появятся позже,
/// и их структуры должны определяться их ТЗ, а не здесь.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub brain: toml::Table,
    pub memory: toml::Table,
    pub voice: toml::Table,
    pub sync: crate::config::SyncSection,
    pub approval: crate::config::ApprovalSection,
    pub telegram: toml::Table,
    pub email: toml::Table,
    pub calendar: toml::Table,
    pub apps: toml::Table,
    pub security: toml::Table,
    pub updates: toml::Table,
    pub documents: toml::Table,
}

/// Общие настройки ([general], Контракт 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// UUID узла.
    pub node_id: String,
    /// Язык интерфейса.
    pub language: crate::models::Language,
    /// Тип устройства.
    pub device_type: crate::models::DeviceType,
    /// Роль узла.
    pub role: crate::models::NodeRole,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::new_v4().to_string(),
            language: crate::models::Language::Ru,
            device_type: crate::models::DeviceType::Desktop,
            role: crate::models::NodeRole::Auto,
        }
    }
}

/// Секция [sync] — типизированный срез полей SyncConfig из tensa-sync
/// (контракт 4 задаёт те же поля).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncSection {
    pub enabled: bool,
    pub grpc_port: u16,
    pub grpc_tls: bool,
    pub tailscale_enabled: bool,
    pub tailscale_hostname: String,
    pub syncthing_enabled: bool,
    pub syncthing_port: u16,
}

/// Секция [approval] — типизированный срез ApprovalConfig из tensa-approval
/// (Контракт 4: те же поля).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalSection {
    pub action_ttl_hours: u64,
    pub auto_approve_trusted: bool,
    pub allow_email_send: bool,
    pub allow_telegram_send: bool,
    pub allow_whatsapp_send: bool,
    pub confirm_unknown_recipients: bool,
    pub max_pending_actions: usize,
}

impl Default for ApprovalSection {
    fn default() -> Self {
        let c = tensa_approval::ApprovalConfig::default();
        Self {
            action_ttl_hours: c.action_ttl_hours,
            auto_approve_trusted: c.auto_approve_trusted,
            allow_email_send: c.allow_email_send,
            allow_telegram_send: c.allow_telegram_send,
            allow_whatsapp_send: c.allow_whatsapp_send,
            confirm_unknown_recipients: c.confirm_unknown_recipients,
            max_pending_actions: c.max_pending_actions,
        }
    }
}

/// Версия конфига (Контракт 4: миграции при обновлении).
pub const CONFIG_VERSION: i64 = 1;

impl Config {
    /// Загрузить из TOML-файла.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("cannot read {}: {e}", path.display())))?;
        let config: Config = toml::from_str(&raw)?;
        config.validate()?;
        Ok(config)
    }

    /// Сохранить в TOML-файл атомарно (tmp + rename), с бэкапом.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let body = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.exists() {
            let backup = path.with_extension("toml.bak");
            std::fs::copy(path, backup)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Создать конфиг с дефолтами (Контракт 4: создание при отсутствии).
    pub fn default_with_node(node_id: &str) -> Self {
        Self {
            general: GeneralConfig {
                node_id: node_id.to_string(),
                ..GeneralConfig::default()
            },
            ..Config::default()
        }
    }

    /// Валидация при загрузке (Контракт 4).
    pub fn validate(&self) -> Result<()> {
        uuid::Uuid::parse_str(&self.general.node_id)
            .map_err(|e| CoreError::Config(format!("general.node_id is not a UUID: {e}")))?;
        if self.general.node_id.is_empty() {
            return Err(CoreError::Config("general.node_id is empty".into()));
        }
        Ok(())
    }
}

/// Трейт хранилища конфигурации (ТЗ tensa-core, сигнатуры дословно).
#[async_trait::async_trait]
pub trait ConfigStore: Send + Sync {
    async fn load(&self, path: &Path) -> Result<Config>;
    async fn save(&self, config: &Config) -> Result<()>;
    async fn reload(&self) -> Result<Config>;
    async fn get(&self) -> Result<Config>;
    async fn update(&self, config: Config) -> Result<()>;
}

/// Реализация трейта ConfigStore (ТЗ) — файловый вариант.
pub struct FileConfigStore {
    path: std::path::PathBuf,
    current: tokio::sync::RwLock<Option<Config>>,
}

impl FileConfigStore {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            current: tokio::sync::RwLock::new(None),
        }
    }
}

#[async_trait::async_trait]
impl ConfigStore for FileConfigStore {
    async fn load(&self, path: &Path) -> Result<Config> {
        let cfg = Config::load(path)?;
        *self.current.write().await = Some(cfg.clone());
        Ok(cfg)
    }

    async fn save(&self, config: &Config) -> Result<()> {
        config.save(&self.path)?;
        *self.current.write().await = Some(config.clone());
        Ok(())
    }

    async fn reload(&self) -> Result<Config> {
        let cfg = Config::load(&self.path)?;
        *self.current.write().await = Some(cfg.clone());
        Ok(cfg)
    }

    async fn get(&self) -> Result<Config> {
        if let Some(cfg) = self.current.read().await.clone() {
            return Ok(cfg);
        }
        let cfg = Config::load(&self.path)?;
        *self.current.write().await = Some(cfg.clone());
        Ok(cfg)
    }

    async fn update(&self, config: Config) -> Result<()> {
        config.save(&self.path)?;
        *self.current.write().await = Some(config);
        Ok(())
    }
}
