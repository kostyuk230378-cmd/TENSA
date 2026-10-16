//! First-run wizard (ADR-021, дополнение к ТЗ tensa-core, Контракт 13).
//!
//! Wizard — состояние и результат; диалог с пользователем делает UI
//! (десктоп/PWA/CLI), передавая сюда готовые ответы по шагам.

use crate::error::Result;
use crate::models::Language;
use serde::{Deserialize, Serialize};

/// Шаги мастера (ТЗ: 8 шагов).
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
    /// Шаг 4: Telegram-бот (токен — в secrets.toml, здесь только chat id).
    pub telegram_owner_chat_id: Option<String>,
    pub telegram_enabled: bool,
    /// Шаг 5: почта настроена (аккаунты — в config/secrets).
    pub email_enabled: bool,
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

/// Результат мастера (ТЗ: заполненный config + secrets + инициализированная память).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardResult {
    pub completed: bool,
    pub answers: WizardAnswers,
    /// Что скачать по шагу 3 (имена файлов в ~/.tensa/models/).
    pub models_to_download: Vec<String>,
}

/// Сборка результата мастера из ответов.
pub fn finish(answers: WizardAnswers) -> Result<WizardResult> {
    if !answers.complete() {
        return Err(crate::error::CoreError::FirstRunNotCompleted);
    }
    let models_to_download = if answers.download_models.unwrap_or(false) {
        vec![
            "llama-server".to_string(),
            "gemma-4-E2B-it-XS-TS-Q3_K_S.gguf".to_string(),
            "multilingual-e5-small".to_string(),
            "ggml-base.bin".to_string(),
        ]
    } else {
        Vec::new()
    };
    tracing::info!(owner_name = ?answers.owner_name, "first-run wizard completed");
    Ok(WizardResult {
        completed: true,
        answers,
        models_to_download,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers() -> WizardAnswers {
        WizardAnswers {
            language: Some(Language::Ru),
            owner_name: Some("Иван".into()),
            owner_title: Some("Шеф".into()),
            assistant_name: Some("Tensa".into()),
            timezone: Some("Europe/Moscow".into()),
            ..Default::default()
        }
    }

    #[test]
    fn complete_requires_language_and_names() {
        assert!(answers().complete());
        let mut a = answers();
        a.language = None;
        assert!(!a.complete());
        let mut a = answers();
        a.assistant_name = None;
        assert!(!a.complete());
    }

    #[test]
    fn finish_fails_on_incomplete() {
        let a = WizardAnswers::default();
        assert!(finish(a).is_err());
    }

    #[test]
    fn finish_collects_models_when_requested() {
        let mut a = answers();
        a.download_models = Some(true);
        let r = finish(a).expect("finish");
        assert!(r.completed);
        assert!(r.models_to_download.iter().any(|m| m.ends_with(".gguf")));
    }

    #[test]
    fn wizard_step_serialization() {
        let json = serde_json::to_string(&WizardStep::Introduction).expect("ser");
        assert_eq!(json, "\"introduction\"");
    }
}
