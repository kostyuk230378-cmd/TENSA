//! `ConfigStore` (Контракт 9): плоская конфигурация поверх tensa-core.
//!
//! [derived] UDL-словарь `Config` плоский (node_id/language/.../brain-поля),
//! конфиг ядра — вложенный, а `[brain]` — «сырая» TOML-таблица; здесь
//! выполняется сопоставление полей в обе стороны.

use toml::Value;

use crate::convert::{data_root, load_core_config};
use crate::error::TensaError;
use crate::types::Config;

/// FFI-обёртка конфигурации (`interface ConfigStore` в UDL).
pub struct ConfigStore {
    config_path: String,
}

impl ConfigStore {
    /// Открыть конфигурацию по пути к `config.toml`.
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        Ok(Self { config_path })
    }

    /// Текущая конфигурация (плоское представление UDL).
    pub async fn get_config(&self) -> Result<Config, TensaError> {
        let core = load_core_config(&self.config_path)?;
        Ok(flat(&core))
    }

    /// Записать конфигурацию (валидация + бэкап выполняются ядром).
    pub async fn update_config(&self, config: Config) -> Result<(), TensaError> {
        let mut core = load_core_config(&self.config_path)?;

        if !config.node_id.trim().is_empty() {
            core.general.node_id = config.node_id;
        }
        core.general.language = parse_language(&config.language);
        core.general.device_type = parse_device_type(&config.device_type);
        core.general.role = parse_role(&config.role);

        if !config.brain_mode.trim().is_empty() {
            core.brain
                .insert("mode".to_string(), Value::String(config.brain_mode));
        }
        if !config.local_model.trim().is_empty() {
            core.brain
                .insert("local_model".to_string(), Value::String(config.local_model));
        }
        if !config.cloud_model.trim().is_empty() {
            core.brain.insert(
                "cloud_models".to_string(),
                Value::Array(vec![Value::String(config.cloud_model)]),
            );
        }

        core.save(std::path::Path::new(&self.config_path))?;
        Ok(())
    }

    /// Перечитать конфигурацию с диска (проверка валидности).
    ///
    /// [derived] Обёртка состояния не кэширует конфиг (каждый вызов читает
    /// файл), поэтому reload — это строгая проверка чтения.
    pub async fn reload_config(&self) -> Result<(), TensaError> {
        let _ = load_core_config(&self.config_path)?;
        let _ = data_root(&self.config_path);
        Ok(())
    }
}

/// Вложенный конфиг ядра → плоское представление UDL.
fn flat(core: &tensa_core::Config) -> Config {
    let brain_str = |key: &str| -> String {
        core.brain
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let cloud_model = core
        .brain
        .get("cloud_models")
        .and_then(Value::as_array)
        .and_then(|models| models.first())
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Config {
        node_id: core.general.node_id.clone(),
        language: name(&core.general.language),
        device_type: name(&core.general.device_type),
        role: name(&core.general.role),
        brain_mode: brain_str("mode"),
        local_model: brain_str("local_model"),
        cloud_model,
    }
}

/// Serde-имя enum-значения (lowercase-строки ядра).
fn name<T: serde::Serialize>(value: &T) -> String {
    crate::convert::name_of(value)
}

fn parse_language(raw: &str) -> tensa_core::Language {
    match raw.trim() {
        "en" => tensa_core::Language::En,
        _ => tensa_core::Language::Ru,
    }
}

fn parse_device_type(raw: &str) -> tensa_core::DeviceType {
    match raw.trim() {
        "android" => tensa_core::DeviceType::Android,
        _ => tensa_core::DeviceType::Desktop,
    }
}

fn parse_role(raw: &str) -> tensa_core::NodeRole {
    match raw.trim() {
        "heavy" => tensa_core::NodeRole::Heavy,
        "thin" => tensa_core::NodeRole::Thin,
        _ => tensa_core::NodeRole::Auto,
    }
}

// Подавляем предупреждение о неиспользуемом импорте пути в тестовых сборках.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_roundtrip() {
        assert_eq!(name(&parse_language("en")), "en");
        assert_eq!(name(&parse_language("ru")), "ru");
        assert_eq!(name(&parse_language("что-то")), "ru");
    }
}
