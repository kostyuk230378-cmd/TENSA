//! Конфигурация роутера (ТЗ tensa-router: `RouterConfig` и значения по
//! умолчанию; Контракт 4).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tensa_core::paths::TensaPaths;

use crate::error::{Result, RouterError};
use crate::models::{BrainMode, ModelTier};

/// Конфигурация локальной модели (ТЗ: `LocalModelConfig`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalModelConfig {
    pub model: String,
    pub tier: ModelTier,
    pub file: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub quantization: String,
    pub capabilities: Vec<String>,
}

impl Default for LocalModelConfig {
    /// Значения — из Контракта 4 (`local_model_*`: gemma-4-E2B-it XS, Q3_K_S).
    /// [derived] `size_bytes = 0` — размер уточняется при загрузке модели.
    fn default() -> Self {
        let file = "gemma-4-E2B-it-XS-TS-Q3_K_S.gguf".to_string();
        let path = TensaPaths::new().models_dir().join(&file);
        Self {
            model: "gemma-4-E2B-it".to_string(),
            tier: ModelTier::Xs,
            file,
            path,
            size_bytes: 0,
            quantization: "Q3_K_S".to_string(),
            capabilities: vec![
                "chat".to_string(),
                "tools".to_string(),
                "ru".to_string(),
                "en".to_string(),
            ],
        }
    }
}

/// Конфигурация облачной модели (ТЗ: `CloudModelConfig`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CloudModelConfig {
    pub model: String,
    pub priority: u8,
    pub free: bool,
}

/// Облачные модели по умолчанию.
/// [derived] Имена моделей — из FIN.md («google/gemma-4-26b-a4b-it
/// (free/paid)»); точные идентификаторы уточняются при интеграции.
pub fn default_cloud_models() -> Vec<CloudModelConfig> {
    vec![
        CloudModelConfig {
            model: "google/gemma-4-26b-a4b-it:free".to_string(),
            priority: 1,
            free: true,
        },
        CloudModelConfig {
            model: "google/gemma-4-26b-a4b-it".to_string(),
            priority: 2,
            free: false,
        },
    ]
}

/// Конфигурация роутера (ТЗ: `RouterConfig`, поля и значения — по ТЗ).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouterConfig {
    pub mode: BrainMode,
    pub local_model: LocalModelConfig,
    pub cloud_models: Vec<CloudModelConfig>,
    pub auto_fallback: bool,
    /// Таймаут бесплатной облачной модели, секунды (ТЗ: 30).
    pub cloud_free_timeout_seconds: u64,
    /// Повторы бесплатной облачной модели (ТЗ: 2).
    pub cloud_free_max_retries: u32,
    /// Падать обратно на локальную модель (ТЗ: true).
    pub cloud_fallback_to_local: bool,
    /// Дневной лимит токенов OpenRouter (ТЗ: 100 000).
    pub openrouter_daily_limit_tokens: u64,
    /// Месячный лимит USD OpenRouter (ТЗ: 10.0).
    pub openrouter_monthly_limit_usd: f64,
    /// Предупреждение при достижении, % (ТЗ: 80).
    pub openrouter_warn_at_percent: u8,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            mode: BrainMode::default(),
            local_model: LocalModelConfig::default(),
            cloud_models: default_cloud_models(),
            auto_fallback: true,
            cloud_free_timeout_seconds: 30,
            cloud_free_max_retries: 2,
            cloud_fallback_to_local: true,
            openrouter_daily_limit_tokens: 100_000,
            openrouter_monthly_limit_usd: 10.0,
            openrouter_warn_at_percent: 80,
        }
    }
}

impl RouterConfig {
    /// Проверить конфигурацию.
    pub fn validate(&self) -> Result<()> {
        if self.local_model.model.is_empty() {
            return Err(RouterError::Config(
                "local_model.model не должен быть пустым".to_string(),
            ));
        }
        if self.cloud_free_timeout_seconds == 0 {
            return Err(RouterError::Config(
                "cloud_free_timeout_seconds должен быть ≥ 1".to_string(),
            ));
        }
        if self.openrouter_daily_limit_tokens == 0 {
            return Err(RouterError::Config(
                "openrouter_daily_limit_tokens должен быть > 0".to_string(),
            ));
        }
        if self.openrouter_monthly_limit_usd <= 0.0 {
            return Err(RouterError::Config(
                "openrouter_monthly_limit_usd должен быть > 0".to_string(),
            ));
        }
        if !(1..=100).contains(&self.openrouter_warn_at_percent) {
            return Err(RouterError::Config(
                "openrouter_warn_at_percent должен быть в 1..=100".to_string(),
            ));
        }
        if self.mode == BrainMode::Cloud && self.cloud_models.is_empty() {
            return Err(RouterError::Config(
                "mode = cloud требует непустой cloud_models".to_string(),
            ));
        }
        for model in &self.cloud_models {
            if model.model.is_empty() {
                return Err(RouterError::Config(
                    "cloud_models: пустое имя модели".to_string(),
                ));
            }
            if model.priority == 0 {
                return Err(RouterError::Config(
                    "cloud_models: priority должен быть ≥ 1".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let config = RouterConfig::default();
        assert_eq!(config.mode, BrainMode::Local);
        assert_eq!(config.local_model.tier, ModelTier::Xs);
        assert!(config.auto_fallback);
        assert_eq!(config.cloud_free_timeout_seconds, 30);
        assert_eq!(config.cloud_free_max_retries, 2);
        assert!(config.cloud_fallback_to_local);
        assert_eq!(config.openrouter_daily_limit_tokens, 100_000);
        assert_eq!(config.openrouter_monthly_limit_usd, 10.0);
        assert_eq!(config.openrouter_warn_at_percent, 80);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn cloud_mode_requires_models() {
        let config = RouterConfig {
            mode: BrainMode::Cloud,
            cloud_models: Vec::new(),
            ..RouterConfig::default()
        };
        assert!(matches!(config.validate(), Err(RouterError::Config(_))));
    }

    #[test]
    fn validation_rejects_bad_values() {
        let config = RouterConfig {
            cloud_free_timeout_seconds: 0,
            ..RouterConfig::default()
        };
        assert!(config.validate().is_err());

        let config = RouterConfig {
            openrouter_warn_at_percent: 0,
            ..RouterConfig::default()
        };
        assert!(config.validate().is_err());

        let config = RouterConfig {
            openrouter_monthly_limit_usd: 0.0,
            ..RouterConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let config = RouterConfig::default();
        let json = serde_json::to_string(&config).expect("сериализация");
        let back: RouterConfig = serde_json::from_str(&json).expect("десериализация");
        assert_eq!(config, back);
    }
}
