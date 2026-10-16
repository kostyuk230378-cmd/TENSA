//! Serde-представление UDL-enum'ов `BlockType`/`MemoryOrigin`.
//!
//! [derived] Реализации живут отдельным модулем (а не derive в `types.rs`),
//! потому что варианты UDL-enum объявлены в форме `Persona {}` — derive
//! сериализовал бы их объектом `{"Persona":{}}`. Здесь представление —
//! каноническая строка ядра (`"persona"`, `"llm"`, ...), что совпадает
//! с `as_str()` и удобно для тестов и отладки.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::{BlockType, MemoryOrigin};

impl Serialize for BlockType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for BlockType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        match raw.as_str() {
            "persona" => Ok(BlockType::Persona {}),
            "human" => Ok(BlockType::Human {}),
            "tasks" => Ok(BlockType::Tasks {}),
            "preferences" => Ok(BlockType::Preferences {}),
            other => Err(D::Error::custom(format!("неизвестный BlockType: {other}"))),
        }
    }
}

impl Serialize for MemoryOrigin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MemoryOrigin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        match raw.as_str() {
            "llm" => Ok(MemoryOrigin::Llm {}),
            "user" => Ok(MemoryOrigin::User {}),
            "system" => Ok(MemoryOrigin::System {}),
            other => Err(D::Error::custom(format!(
                "неизвестный MemoryOrigin: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_type_round_trips_as_string() {
        let json = serde_json::to_string(&BlockType::Tasks {}).expect("сериализация");
        assert_eq!(json, "\"tasks\"");
        let parsed: BlockType = serde_json::from_str(&json).expect("разбор");
        assert_eq!(parsed, BlockType::Tasks {});
        assert!(serde_json::from_str::<BlockType>("\"nope\"").is_err());
    }

    #[test]
    fn memory_origin_round_trips_as_string() {
        let json = serde_json::to_string(&MemoryOrigin::Llm {}).expect("сериализация");
        assert_eq!(json, "\"llm\"");
        let parsed: MemoryOrigin = serde_json::from_str(&json).expect("разбор");
        assert_eq!(parsed, MemoryOrigin::Llm {});
    }

    #[test]
    fn block_struct_serializes_with_string_enums() {
        let block = crate::types::CoreMemoryBlock {
            id: "1".to_string(),
            block: BlockType::Persona {},
            value: "v".to_string(),
            size_chars: 1,
            size_tokens: 1,
            created_at: 0,
            updated_at: 0,
            updated_by: MemoryOrigin::System {},
            updated_on_node: "n".to_string(),
        };
        let json = serde_json::to_value(&block).expect("сериализация");
        assert_eq!(json["block"], "persona");
        assert_eq!(json["updated_by"], "system");
    }
}
