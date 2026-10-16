//! Преобразования между типами ядра и типами UDL (Контракт 9).
//!
//! [derived] UDL оперирует строками там, где ядро использует Uuid/JSON/enum —
//! функции ниже собирают такие строки единообразно.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::error::TensaError;

/// UUID в строку (в UDL идентификаторы — `string`).
pub fn uuid_str(id: Uuid) -> String {
    id.to_string()
}

/// Имя enum-значения из serde-представления (snake_case, как в контрактах).
pub fn name_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|json| json.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// JSON-значение в строку (payload/result в UDL — `string`).
pub fn json_str(value: &Value) -> String {
    value.to_string()
}

/// Необязательный timestamp в строку.
pub fn timestamp_str(value: Option<i64>) -> Option<String> {
    value.map(|ts| ts.to_string())
}

/// Разобрать UUID из строки UDL.
pub fn parse_uuid(context: &str, raw: &str) -> Result<Uuid, TensaError> {
    Uuid::parse_str(raw).map_err(|error| TensaError::internal(context, error))
}

/// Каталог данных по пути к `config.toml` (корень — каталог файла, Контракт 4).
pub fn data_root(config_path: &str) -> PathBuf {
    Path::new(config_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Конфиг ядра из файла; файла нет — конфигурация по умолчанию
/// (файл создаётся только явными действиями пользователя).
pub fn load_core_config(config_path: &str) -> Result<tensa_core::Config, TensaError> {
    let path = Path::new(config_path);
    if path.exists() {
        Ok(tensa_core::Config::load(path)?)
    } else {
        Ok(tensa_core::Config::default())
    }
}
