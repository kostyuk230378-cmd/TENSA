//! Sampling: параметры и стоп-последовательности (ТЗ tensa-llm: sampling).
//! Чистая логика — применима нативным циклом генерации.

use crate::engine::LlmConfig;
use crate::error::{LlmError, Result};

/// Параметры сэмплинга (из конфига, с разовыми переопределениями).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplingParams {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub repeat_penalty: f32,
}

impl SamplingParams {
    /// Из конфига движка; `temperature` может быть переопределена запросом.
    pub fn from_config(config: &LlmConfig, temperature: Option<f32>) -> Self {
        Self {
            temperature: temperature.unwrap_or(config.temperature),
            top_p: config.top_p,
            top_k: config.top_k,
            repeat_penalty: config.repeat_penalty,
        }
    }

    /// Валидация диапазонов.
    pub fn validate(&self) -> Result<()> {
        if !self.temperature.is_finite() || self.temperature < 0.0 {
            return Err(LlmError::Config(format!(
                "temperature must be finite and >= 0 (got {})",
                self.temperature
            )));
        }
        if !(self.top_p > 0.0 && self.top_p <= 1.0) {
            return Err(LlmError::Config(format!(
                "top_p must be in (0, 1] (got {})",
                self.top_p
            )));
        }
        if self.top_k == 0 {
            return Err(LlmError::Config("top_k must be > 0".into()));
        }
        if !self.repeat_penalty.is_finite() || self.repeat_penalty <= 0.0 {
            return Err(LlmError::Config(format!(
                "repeat_penalty must be finite and > 0 (got {})",
                self.repeat_penalty
            )));
        }
        Ok(())
    }
}

/// Найти самую раннюю стоп-последовательность в тексте.
/// Возвращает (позицию, длину) или None. Пустые стоп-строки игнорируются.
pub fn detect_stop(text: &str, stop: &[String]) -> Option<(usize, usize)> {
    stop.iter()
        .filter(|s| !s.is_empty())
        .filter_map(|s| text.find(s.as_str()).map(|pos| (pos, s.len())))
        .min_by_key(|(pos, _)| *pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LlmConfig {
        LlmConfig::default()
    }

    #[test]
    fn from_config_uses_defaults_and_override() {
        let p = SamplingParams::from_config(&config(), None);
        assert_eq!(p.temperature, 0.7);
        assert_eq!(p.top_p, 0.95);
        assert_eq!(p.top_k, 40);
        assert_eq!(p.repeat_penalty, 1.1);

        let p = SamplingParams::from_config(&config(), Some(0.0));
        assert_eq!(p.temperature, 0.0, "переопределение из запроса");
    }

    #[test]
    fn validate_accepts_defaults() {
        SamplingParams::from_config(&config(), None)
            .validate()
            .expect("defaults valid");
    }

    #[test]
    fn validate_rejects_bad_ranges() {
        let mut p = SamplingParams::from_config(&config(), None);
        p.temperature = -0.1;
        assert!(matches!(p.validate(), Err(LlmError::Config(_))));
        p = SamplingParams::from_config(&config(), None);
        p.temperature = f32::NAN;
        assert!(p.validate().is_err());
        p = SamplingParams::from_config(&config(), None);
        p.top_p = 1.5;
        assert!(p.validate().is_err());
        p = SamplingParams::from_config(&config(), None);
        p.top_p = 0.0;
        assert!(p.validate().is_err());
        p = SamplingParams::from_config(&config(), None);
        p.top_k = 0;
        assert!(p.validate().is_err());
        p = SamplingParams::from_config(&config(), None);
        p.repeat_penalty = 0.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn stop_detection_earliest_match() {
        let stop = vec!["###".to_string(), "</s>".to_string()];
        // find возвращает байтовый offset: «текст » = 5 кириллических
        // символов (2 байта каждый) + пробел = 11 байт.
        assert_eq!(detect_stop("текст </s> дальше ###", &stop), Some((11, 4)));
        assert_eq!(detect_stop("без стопов", &stop), None);
    }

    #[test]
    fn stop_detection_ignores_empty() {
        let stop = vec![String::new(), "z".to_string()];
        assert_eq!(detect_stop("az", &stop), Some((1, 1)));
        assert_eq!(detect_stop("abc", &[]), None);
    }
}
