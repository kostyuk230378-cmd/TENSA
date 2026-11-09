//! `FirstRunWizard` (Контракт 9, доп. ТЗ tensa-core): мастер первого запуска
//! поверх `tensa_core::FirstRunWizard`.
//!
//! Инициализация core memory — через `tensa_memory::DefaultMemoryManager`
//! (SQLite: `memory.db` для core/recall, `memory/archival.db` для архива).
//! Эмбеддер архива — `HashEmbedder`: реальная модель e5-small подключается
//! за фичей `fastembed` (CLI) и для инициализации дефолтных блоков не нужна.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tensa_memory::{
    CoreMemoryConfig, DefaultMemoryManager, HashEmbedder, MemoryManagerConfig,
    SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};

use crate::convert::{data_root, load_core_config};
use crate::error::TensaError;
use crate::types::{EmailAccount, WhatsAppConfig, WizardResult};

/// FFI-обёртка мастера первого запуска (`interface FirstRunWizard` в UDL).
pub struct FirstRunWizard {
    config_path: String,
    inner: tensa_core::FirstRunWizard,
}

impl FirstRunWizard {
    /// Открыть мастер (`constructor(string config_path)`).
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        let root = data_root(&config_path);
        let paths = tensa_core::TensaPaths::with_root(root.clone());
        paths
            .ensure_dirs()
            .map_err(|error| TensaError::config("wizard: каталоги данных", error))?;
        let node_id = load_core_config(&config_path)?.general.node_id;
        let core_store = SqliteCoreMemoryStore::new(
            &root.join("memory.db"),
            CoreMemoryConfig::default(),
            node_id.clone(),
        )
        .await?;
        let recall_store = SqliteRecallMemoryStore::new(&root.join("memory.db")).await?;
        let archival_store = SqliteArchivalMemoryStore::new(
            &paths.memory_dir().join("archival.db"),
            Arc::new(HashEmbedder::new(384)),
        )
        .await?;
        let manager = DefaultMemoryManager::new(
            Arc::new(core_store),
            Arc::new(recall_store),
            Arc::new(archival_store),
            MemoryManagerConfig::default(),
            node_id.clone(),
        )
        .await?;
        let inner = tensa_core::FirstRunWizard::new(
            PathBuf::from(&config_path),
            paths.secrets_file(),
            Some(Arc::new(manager)),
            node_id,
        )
        .await?;
        Ok(Self { config_path, inner })
    }

    /// Первый ли запуск: `config.toml` отсутствует
    /// (UDL: `[Async, Throws=TensaError] boolean is_first_run();`).
    pub async fn is_first_run(&self) -> Result<bool, TensaError> {
        Ok(!Path::new(&self.config_path).exists())
    }

    /// Шаг 1: язык (`ru` | `en`).
    pub async fn step_language(&self, language: String) -> Result<(), TensaError> {
        let language = match language.trim().to_lowercase().as_str() {
            "ru" => tensa_core::Language::Ru,
            "en" => tensa_core::Language::En,
            other => {
                return Err(TensaError::config(
                    "wizard.step_language: неизвестный язык",
                    format!("«{other}»: ru | en"),
                ))
            }
        };
        self.inner.step_language(language).await?;
        Ok(())
    }

    /// Шаг 2: знакомство (имя владельца, обращение, имя секретаря, пояс).
    pub async fn step_introduction(
        &self,
        owner_name: String,
        owner_title: String,
        secretary_name: String,
        timezone: String,
    ) -> Result<(), TensaError> {
        self.inner
            .step_introduction(owner_name, owner_title, secretary_name, timezone)
            .await?;
        Ok(())
    }

    /// Шаг 3: модели (согласие на загрузку).
    pub async fn step_models(&self) -> Result<(), TensaError> {
        self.inner.step_models().await?;
        Ok(())
    }

    /// Шаг 4: Telegram (токен бота уходит в secrets.toml).
    pub async fn step_telegram(
        &self,
        bot_token: String,
        owner_chat_id: String,
    ) -> Result<(), TensaError> {
        self.inner.step_telegram(bot_token, owner_chat_id).await?;
        Ok(())
    }

    /// Шаг 5: почта — не реализован на FFI-поверхности.
    ///
    /// UDL-словарь `EmailAccount` (Контракт 9) не содержит `email` и
    /// `credential`, без которых ядро не принимает аккаунт (валидация
    /// `step_email`). Контракты не меняются без утверждения Архитектора —
    /// расширение словаря предложено в отчёте; до согласования метод
    /// отвечает «ещё не реализовано».
    pub async fn step_email(&self, _account: EmailAccount) -> Result<(), TensaError> {
        Err(TensaError::not_implemented(
            "wizard.step_email: UDL EmailAccount без email/credential",
        ))
    }

    /// Шаг 6: WhatsApp (опционально; `access_token` уходит в secrets.toml).
    pub async fn step_whatsapp(&self, config: Option<WhatsAppConfig>) -> Result<(), TensaError> {
        let config = config.map(|config| tensa_core::WhatsAppConfig {
            credential: config.access_token,
            phone_number_id: config.phone_number_id,
            waba_id: config.waba_id,
        });
        self.inner.step_whatsapp(config).await?;
        Ok(())
    }

    /// Шаг 7: Tailscale.
    pub async fn step_tailscale(&self) -> Result<(), TensaError> {
        self.inner.step_tailscale().await?;
        Ok(())
    }

    /// Шаг 8: завершение — config.toml, secrets.toml и core memory
    /// (UDL: `[Async, Throws=TensaError] WizardResult step_finish();`).
    pub async fn step_finish(&self) -> Result<WizardResult, TensaError> {
        let result = self.inner.step_finish().await?;
        // Успешный finish означает, что оба файла записаны ядром.
        Ok(WizardResult {
            config_saved: true,
            secrets_saved: true,
            core_memory_initialized: result.core_memory_initialized,
        })
    }
}
