//! Модель: тиры и информация (ТЗ tensa-llm).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Тип (размер) локальной модели (ТЗ: ModelTier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ModelTier {
    Xs,
    S,
    M,
    L,
}

impl ModelTier {
    /// Каноническое строковое значение (Контракт 4: local_model_tier).
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelTier::Xs => "XS",
            ModelTier::S => "S",
            ModelTier::M => "M",
            ModelTier::L => "L",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "XS" | "xs" => Some(ModelTier::Xs),
            "S" | "s" => Some(ModelTier::S),
            "M" | "m" => Some(ModelTier::M),
            "L" | "l" => Some(ModelTier::L),
            _ => None,
        }
    }
}

/// Информация о загруженной модели.
/// [derived] Поля в ТЗ не заданы — минимальный набор для health/UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub path: PathBuf,
    pub tier: ModelTier,
    /// Размер файла, байт.
    pub size_bytes: u64,
    /// Строка квантования (например, "Q3_K_S").
    pub quantization: String,
    /// Размер контекста, токенов.
    pub context_size: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_str_roundtrip() {
        for t in [ModelTier::Xs, ModelTier::S, ModelTier::M, ModelTier::L] {
            assert_eq!(ModelTier::from_db(t.as_str()), Some(t));
        }
        assert_eq!(ModelTier::from_db("XL"), None);
    }

    #[test]
    fn tier_serde_uppercase() {
        let json = serde_json::to_string(&ModelTier::Xs).expect("ser");
        assert_eq!(json, "\"XS\"");
    }
}
