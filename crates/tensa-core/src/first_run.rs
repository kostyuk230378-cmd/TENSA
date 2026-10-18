//! First-run wizard (доп. ТЗ tensa-core, ADR-021, Контракт 13).
//!
//! Мастер — состояние и шаги; диалог с пользователем ведёт UI
//! (десктоп/PWA/CLI), передавая сюда готовые ответы. [`FirstRunWizard::step_finish`]
//! записывает `config.toml` + `secrets.toml` и инициализирует core memory
//! через [`MemoryInitializer`] (обратный трейт: tensa-memory зависит от
//! tensa-core, прямая зависимость дала бы цикл).

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::config::Config;
use crate::error::{CoreError, Result};
use crate::init::{validate_params, InitParams, MemoryInitializer};
use crate::models::{EmailAccount, Language, WhatsAppConfig};
use crate::secrets::Secrets;

/// Шаги мастера (доп. ТЗ: 8 шагов).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WizardStep {
    Language,
    Introduction,
    ModelDownload,
    Telegram,
    Email,
    WhatsApp,
    Tailscale,
    Finish,
}

/// Ответы мастера (накапливаются по мере прохождения шагов).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WizardAnswers {
    /// Шаг 1: язык интерфейса.
    pub language: Option<Language>,
    /// Шаг 2: имя владельца.
    pub owner_name: Option<String>,
    /// Шаг 2: как обращаться к владельцу.
    pub owner_title: Option<String>,
    /// Шаг 2: имя секретаря.
    pub assistant_name: Option<String>,
    /// Шаг 2: часовой пояс.
    pub timezone: Option<String>,
    /// Шаг 3: согласие на загрузку моделей.
    pub download_models: Option<bool>,
    /// Шаг 4: Telegram-бот (секрет — в secrets.toml, здесь только chat id).
    pub telegram_owner_chat_id: Option<String>,
    pub telegram_enabled: bool,
    /// Шаг 5: почтовые аккаунты (без секретов — они ушли в secrets).
    #[serde(default)]
    pub email_accounts: Vec<EmailAccount>,
    /// Шаг 6: WhatsApp (опционально).
    pub whatsapp_enabled: bool,
    /// Шаг 7: Tailscale.
    pub tailscale_enabled: bool,
    pub tailscale_hostname: Option<String>,
}

impl WizardAnswers {
    /// Все обязательные шаги пройдены? (язык + знакомство; остальное опционально).
    pub fn complete(&self) -> bool {
        self.language.is_some()
            && self.owner_name.as_deref().is_some_and(|s| !s.is_empty())
            && self
                .assistant_name
                .as_deref()
                .is_some_and(|s| !s.is_empty())
    }
}

/// Результат мастера (доп. ТЗ: заполненные config + secrets +
/// инициализированная core memory).
#[derive(Debug, Clone)]
pub struct WizardResult {
    pub config: Config,
    pub secrets: Secrets,
    pub core_memory_initialized: bool,
}

/// Что скачивает шаг «модели» (доп. ТЗ, шаг 3; файлы в `~/.tensa/models/`).
pub fn models_to_download() -> Vec<String> {
    vec![
        "llama-server".to_string(),
        "gemma-4-E2B-it-XS-TS-Q3_K_S.gguf".to_string(),
        "multilingual-e5-small".to_string(),
        "ggml-small-q5_1.bin".to_string(),
    ]
}

/// Мастер первого запуска.
pub struct FirstRunWizard {
    config_path: PathBuf,
    secrets_path: PathBuf,
    node_id: String,
    memory: Option<Arc<dyn MemoryInitializer>>,
    answers: RwLock<WizardAnswers>,
    secrets: RwLock<Secrets>,
}

impl FirstRunWizard {
    /// Создать мастер (пути — `~/.tensa/config.toml` и `secrets.toml`).
    pub async fn new(
        config_path: PathBuf,
        secrets_path: PathBuf,
        memory: Option<Arc<dyn MemoryInitializer>>,
        node_id: String,
    ) -> Result<Self> {
        Ok(Self {
            config_path,
            secrets_path,
            node_id,
            memory,
            answers: RwLock::new(WizardAnswers::default()),
            secrets: RwLock::new(Secrets::default()),
        })
    }

    /// Текущие ответы (клон).
    pub async fn answers(&self) -> WizardAnswers {
        self.answers.read().await.clone()
    }

    /// Запустить мастер до конца (эквивалент [`Self::step_finish`];
    /// [derived] в UI-сценарии шаги проходятся по одному).
    pub async fn run(&self) -> Result<WizardResult> {
        self.step_finish().await
    }

    /// Шаг 1: язык.
    pub async fn step_language(&self, language: Language) -> Result<()> {
        self.answers.write().await.language = Some(language);
        Ok(())
    }

    /// Шаг 2: знакомство (имя, обращение, имя секретаря, часовой пояс).
    pub async fn step_introduction(
        &self,
        owner_name: String,
        owner_title: String,
        secretary_name: String,
        timezone: String,
    ) -> Result<()> {
        let language = self.answers.read().await.language.unwrap_or(Language::Ru);
        let params = InitParams {
            owner_name: owner_name.clone(),
            owner_title: owner_title.clone(),
            secretary_name: secretary_name.clone(),
            language,
            timezone: timezone.clone(),
        };
        validate_params(&params)?;

        let mut answers = self.answers.write().await;
        answers.owner_name = Some(owner_name);
        answers.owner_title = Some(owner_title);
        answers.assistant_name = Some(secretary_name);
        answers.timezone = Some(timezone);
        Ok(())
    }

    /// Шаг 3: загрузка моделей (отметка согласия).
    pub async fn step_models(&self) -> Result<()> {
        self.answers.write().await.download_models = Some(true);
        Ok(())
    }

    /// Шаг 4: Telegram-бот (секрет уходит в secrets.toml).
    pub async fn step_telegram(&self, bot_credential: String, owner_chat_id: String) -> Result<()> {
        if bot_credential.trim().is_empty() {
            return Err(CoreError::Config("telegram credential пуст".into()));
        }
        if owner_chat_id.trim().is_empty() {
            return Err(CoreError::Config("telegram owner_chat_id пуст".into()));
        }
        {
            let mut secrets = self.secrets.write().await;
            secrets.telegram.credential = Some(bot_credential);
            secrets.telegram.owner_chat_id = Some(owner_chat_id.clone());
        }
        let mut answers = self.answers.write().await;
        answers.telegram_enabled = true;
        answers.telegram_owner_chat_id = Some(owner_chat_id);
        Ok(())
    }

    /// Шаг 5: почта (секрет аккаунта уходит в secrets.toml, ответы — без секретов).
    pub async fn step_email(&self, account: EmailAccount) -> Result<()> {
        if account.id.trim().is_empty()
            || account.email.trim().is_empty()
            || account.imap_host.trim().is_empty()
            || account.smtp_host.trim().is_empty()
        {
            return Err(CoreError::Config(
                "email account: пустые id/email/imap_host/smtp_host".into(),
            ));
        }

        let mut account = account;
        let credential = account.credential.take();
        if let Some(credential) = credential {
            let mut secrets = self.secrets.write().await;
            secrets
                .email
                .accounts
                .entry(account.id.clone())
                .or_default()
                .credential = Some(credential);
        }
        self.answers.write().await.email_accounts.push(account);
        Ok(())
    }

    /// Шаг 6: WhatsApp (опционально).
    pub async fn step_whatsapp(&self, config: Option<WhatsAppConfig>) -> Result<()> {
        if let Some(config) = config {
            if config.credential.trim().is_empty() || config.phone_number_id.trim().is_empty() {
                return Err(CoreError::Config("whatsapp: пустые поля".into()));
            }
            self.secrets.write().await.whatsapp = Some(config);
            self.answers.write().await.whatsapp_enabled = true;
        }
        Ok(())
    }

    /// Шаг 7: Tailscale (флаг; сама сеть настраивается отдельно).
    pub async fn step_tailscale(&self) -> Result<()> {
        self.answers.write().await.tailscale_enabled = true;
        Ok(())
    }

    /// Шаг 8: завершение — запись config.toml и secrets.toml + core memory.
    pub async fn step_finish(&self) -> Result<WizardResult> {
        let answers = self.answers.read().await.clone();
        if !answers.complete() {
            return Err(CoreError::FirstRunNotCompleted);
        }

        // 1) Конфиг: существующий или свежий.
        let mut config = if self.config_path.exists() {
            Config::load(&self.config_path)?
        } else {
            Config::default_with_node(&self.node_id)
        };
        if let Some(language) = answers.language {
            config.general.language = language;
        }
        if answers.telegram_enabled {
            config
                .telegram
                .insert("enabled".to_string(), toml::Value::Boolean(true));
            if let Some(chat_id) = &answers.telegram_owner_chat_id {
                config.telegram.insert(
                    "owner_chat_id".to_string(),
                    toml::Value::String(chat_id.clone()),
                );
            }
        }
        if !answers.email_accounts.is_empty() {
            let mut accounts = toml::Table::new();
            for account in &answers.email_accounts {
                let mut entry = toml::Table::new();
                entry.insert(
                    "label".to_string(),
                    toml::Value::String(account.label.clone()),
                );
                entry.insert(
                    "email".to_string(),
                    toml::Value::String(account.email.clone()),
                );
                entry.insert(
                    "imap_host".to_string(),
                    toml::Value::String(account.imap_host.clone()),
                );
                entry.insert(
                    "imap_port".to_string(),
                    toml::Value::Integer(i64::from(account.imap_port)),
                );
                entry.insert(
                    "smtp_host".to_string(),
                    toml::Value::String(account.smtp_host.clone()),
                );
                entry.insert(
                    "smtp_port".to_string(),
                    toml::Value::Integer(i64::from(account.smtp_port)),
                );
                accounts.insert(account.id.clone(), toml::Value::Table(entry));
            }
            config
                .email
                .insert("accounts".to_string(), toml::Value::Table(accounts));
        }
        if answers.whatsapp_enabled {
            config
                .whatsapp
                .insert("enabled".to_string(), toml::Value::Boolean(true));
        }
        if answers.tailscale_enabled {
            config.sync.tailscale_enabled = true;
            if let Some(hostname) = &answers.tailscale_hostname {
                config.sync.tailscale_hostname = hostname.clone();
            }
        }
        config.save(&self.config_path)?;

        // 2) Секреты.
        let secrets = self.secrets.read().await.clone();
        secrets.save(&self.secrets_path)?;

        // 3) Core memory (если подключён инициализатор).
        let mut core_memory_initialized = false;
        if let Some(memory) = &self.memory {
            let params = InitParams {
                owner_name: answers.owner_name.clone().unwrap_or_default(),
                owner_title: answers.owner_title.clone().unwrap_or_default(),
                secretary_name: answers.assistant_name.clone().unwrap_or_default(),
                language: answers.language.unwrap_or(Language::Ru),
                timezone: answers.timezone.clone().unwrap_or_default(),
            };
            validate_params(&params)?;
            memory.init_default(&params).await?;
            core_memory_initialized = true;
        }

        tracing::info!(
            owner = ?answers.owner_name,
            telegram = answers.telegram_enabled,
            email_accounts = answers.email_accounts.len(),
            "first-run wizard completed"
        );
        Ok(WizardResult {
            config,
            secrets,
            core_memory_initialized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths() -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("tensa-wizard-{}", uuid::Uuid::new_v4()));
        (root.join("config.toml"), root.join("secrets.toml"))
    }

    async fn wizard() -> (FirstRunWizard, PathBuf) {
        let (config_path, secrets_path) = temp_paths();
        let root = config_path.parent().expect("root").to_path_buf();
        let wizard = FirstRunWizard::new(
            config_path,
            secrets_path,
            None,
            uuid::Uuid::new_v4().to_string(),
        )
        .await
        .expect("wizard");
        (wizard, root)
    }

    #[tokio::test]
    async fn finish_requires_mandatory_steps() {
        let (wizard, root) = wizard().await;
        assert!(matches!(
            wizard.step_finish().await,
            Err(CoreError::FirstRunNotCompleted)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn full_flow_writes_config_and_secrets() {
        let (config_path, secrets_path) = temp_paths();
        let root = config_path.parent().expect("root").to_path_buf();
        let wizard = FirstRunWizard::new(
            config_path.clone(),
            secrets_path.clone(),
            None,
            uuid::Uuid::new_v4().to_string(),
        )
        .await
        .expect("wizard");
        wizard.step_language(Language::Ru).await.expect("язык");
        wizard
            .step_introduction(
                "Иван".into(),
                "Шеф".into(),
                "Tensa".into(),
                "Europe/Moscow".into(),
            )
            .await
            .expect("знакомство");
        wizard
            .step_telegram("bot-credential".into(), "42".into())
            .await
            .expect("telegram");

        let result = wizard.step_finish().await.expect("финиш");
        assert!(
            !result.core_memory_initialized,
            "инициализатор не подключён"
        );

        let config = Config::load(&config_path).expect("config");
        assert_eq!(config.general.language, Language::Ru);
        assert_eq!(
            config.telegram.get("enabled"),
            Some(&toml::Value::Boolean(true))
        );
        let secrets = crate::secrets::Secrets::load(&secrets_path).expect("secrets");
        assert_eq!(
            secrets.telegram.credential.as_deref(),
            Some("bot-credential")
        );
        assert_eq!(secrets.telegram.owner_chat_id.as_deref(), Some("42"));
        let _ = std::fs::remove_dir_all(root);
    }
}
