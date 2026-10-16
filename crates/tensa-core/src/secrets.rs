//! Секреты (`~/.tensa/secrets.toml`; Контракт 4).
//!
//! Контракт 4: секреты хранятся отдельным файлом и не синхронизируются;
//! значения из окружения имеют приоритет над файлом (`OPENROUTER_API_KEY`,
//! `TELEGRAM_BOT_TOKEN`, `IMAP_PASSWORD`, `SYNCTHING_API_KEY`).
//! Секреты не логируются — `Debug` печатает только факт «задано/пусто».

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::models::WhatsAppConfig;

/// Секреты Tensa (`secrets.toml`).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Secrets {
    #[serde(default)]
    pub openrouter: OpenRouterSecrets,
    #[serde(default)]
    pub telegram: TelegramSecrets,
    #[serde(default)]
    pub email: EmailSecrets,
    #[serde(default)]
    pub whatsapp: Option<WhatsAppConfig>,
    #[serde(default)]
    pub syncthing: SyncthingSecrets,
}

/// Секреты OpenRouter.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct OpenRouterSecrets {
    #[serde(default)]
    pub credential: Option<String>,
}

/// Секреты Telegram-бота.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TelegramSecrets {
    #[serde(default)]
    pub credential: Option<String>,
    #[serde(default)]
    pub owner_chat_id: Option<String>,
}

/// Секреты почтовых аккаунтов (по id аккаунта).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct EmailSecrets {
    #[serde(default)]
    pub accounts: BTreeMap<String, EmailAccountSecrets>,
}

/// Секреты одного почтового аккаунта.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct EmailAccountSecrets {
    #[serde(default)]
    pub credential: Option<String>,
}

/// Секреты Syncthing (api key REST API).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SyncthingSecrets {
    #[serde(default)]
    pub credential: Option<String>,
}

/// Метка «задано/пусто» для Debug (значения секретов не печатаются).
fn flag(present: bool) -> &'static str {
    if present {
        "задано"
    } else {
        "пусто"
    }
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let accounts: Vec<&String> = self.email.accounts.keys().collect();
        f.debug_struct("Secrets")
            .field("openrouter", &flag(self.openrouter.credential.is_some()))
            .field("telegram", &flag(self.telegram.credential.is_some()))
            .field("telegram.owner_chat_id", &self.telegram.owner_chat_id)
            .field("email.accounts", &accounts)
            .field("whatsapp", &flag(self.whatsapp.is_some()))
            .field("syncthing", &flag(self.syncthing.credential.is_some()))
            .finish()
    }
}

/// Значение из окружения (непустое) — приоритет над файлом.
fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

impl Secrets {
    /// Ключ OpenRouter: окружение (`OPENROUTER_API_KEY`) → файл.
    pub fn openrouter_key(&self) -> Option<String> {
        env_value("OPENROUTER_API_KEY").or_else(|| self.openrouter.credential.clone())
    }

    /// Токен Telegram-бота: окружение (`TELEGRAM_BOT_TOKEN`) → файл.
    pub fn telegram_bot(&self) -> Option<String> {
        env_value("TELEGRAM_BOT_TOKEN").or_else(|| self.telegram.credential.clone())
    }

    /// Пароль почтового аккаунта: окружение (`IMAP_PASSWORD`) → файл.
    pub fn email_credential(&self, account_id: &str) -> Option<String> {
        env_value("IMAP_PASSWORD")
            .or_else(|| self.email.accounts.get(account_id)?.credential.clone())
    }

    /// Ключ Syncthing: окружение (`SYNCTHING_API_KEY`) → файл.
    pub fn syncthing_key(&self) -> Option<String> {
        env_value("SYNCTHING_API_KEY").or_else(|| self.syncthing.credential.clone())
    }

    /// Загрузить из TOML-файла (при отсутствии — пустые секреты).
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Config(format!("cannot read {}: {e}", path.display())))?;
        let secrets: Secrets = toml::from_str(&raw)?;
        Ok(secrets)
    }

    /// Сохранить в TOML-файл атомарно (tmp + rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        let body = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, path)?;
        tracing::info!("secrets saved");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_masking() {
        let mut secrets = Secrets::default();
        secrets.openrouter.credential = Some("sk-test-value".to_string());
        secrets.telegram.owner_chat_id = Some("42".to_string());
        secrets
            .email
            .accounts
            .entry("personal".to_string())
            .or_default()
            .credential = Some("mail-pass".to_string());

        let debug = format!("{secrets:?}");
        assert!(
            !debug.contains("sk-test-value"),
            "секрет не должен печататься"
        );
        assert!(!debug.contains("mail-pass"), "секрет не должен печататься");
        assert!(debug.contains("задано"));

        let root = std::env::temp_dir().join(format!("tensa-secrets-{}", uuid::Uuid::new_v4()));
        let path = root.join("secrets.toml");
        secrets.save(&path).expect("save");
        let loaded = Secrets::load(&path).expect("load");
        assert_eq!(
            loaded.openrouter.credential.as_deref(),
            Some("sk-test-value")
        );
        assert_eq!(loaded.telegram.owner_chat_id.as_deref(), Some("42"));
        assert_eq!(
            loaded
                .email
                .accounts
                .get("personal")
                .and_then(|a| a.credential.as_deref()),
            Some("mail-pass")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let root =
            std::env::temp_dir().join(format!("tensa-secrets-missing-{}", uuid::Uuid::new_v4()));
        let loaded = Secrets::load(&root.join("secrets.toml")).expect("load");
        assert!(loaded.openrouter.credential.is_none());
        assert!(loaded.telegram.credential.is_none());
    }
}
