//! Tool calls памяти (dop_tz): core_memory_replace, core_memory_append,
//! recall_memory_search, archival_memory_search, archival_memory_insert.
//!
//! Инструменты — тонкие обёртки над MemoryManager для агента (LLM):
//! вход — JSON-аргументы, выход — JSON-результат.

use crate::error::{MemoryError, Result};
use crate::manager::MemoryManager;
use crate::models::{ArchivalEntry, BlockType, MemoryOrigin};
use serde_json::json;
use uuid::Uuid;

/// Описание инструмента для реестра агента.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: &'static str,
    pub description: &'static str,
}

/// Все инструменты памяти.
pub const MEMORY_TOOLS: &[ToolInfo] = &[
    ToolInfo {
        name: "core_memory_replace",
        description: "Заменить содержимое блока core memory (persona/human/tasks/preferences)",
    },
    ToolInfo {
        name: "core_memory_append",
        description: "Добавить текст в конец блока core memory",
    },
    ToolInfo {
        name: "recall_memory_search",
        description: "Полнотекстовый поиск по истории разговоров",
    },
    ToolInfo {
        name: "archival_memory_search",
        description: "Семантический поиск по долговременной памяти",
    },
    ToolInfo {
        name: "archival_memory_insert",
        description: "Сохранить факт в долговременную память",
    },
];

fn parse_block(value: &serde_json::Value) -> Result<BlockType> {
    let s = value
        .get("block")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MemoryError::Config("missing 'block'".into()))?;
    BlockType::from_db(s)
}

/// Диспетчер tool call: name + JSON-аргументы -> JSON-результат.
pub async fn dispatch(
    manager: &dyn MemoryManager,
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    match name {
        "core_memory_replace" => {
            let block = parse_block(args)?;
            let value = args
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MemoryError::Config("missing 'value'".into()))?;
            let origin = origin_from(args);
            let block_out = manager
                .core_replace(block, value.to_string(), origin)
                .await?;
            Ok(
                json!({ "ok": true, "block": block_out.block.as_str(), "size_tokens": block_out.size_tokens }),
            )
        }
        "core_memory_append" => {
            let block = parse_block(args)?;
            let value = args
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MemoryError::Config("missing 'value'".into()))?;
            let origin = origin_from(args);
            let block_out = manager
                .core_append(block, value.to_string(), origin)
                .await?;
            Ok(
                json!({ "ok": true, "block": block_out.block.as_str(), "size_tokens": block_out.size_tokens }),
            )
        }
        "recall_memory_search" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MemoryError::Config("missing 'query'".into()))?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let results = manager.recall_search(query, limit).await?;
            Ok(json!({ "results": results }))
        }
        "archival_memory_search" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MemoryError::Config("missing 'query'".into()))?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let results = manager.archival_search(query, limit).await?;
            Ok(json!({ "results": results }))
        }
        "archival_memory_insert" => {
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| MemoryError::Config("missing 'content'".into()))?;
            let tags: Vec<String> = args
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let entry = ArchivalEntry {
                id: Uuid::new_v4(),
                content: content.to_string(),
                embedding: Vec::new(), // эмбеддинг посчитает хранилище
                tags,
                source: args
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                created_at: 0,
                node_id: "local".into(),
                metadata: json!({}),
            };
            let id = manager.archival_insert(entry).await?;
            Ok(json!({ "ok": true, "id": id }))
        }
        other => Err(MemoryError::Config(format!("unknown memory tool: {other}"))),
    }
}

fn origin_from(args: &serde_json::Value) -> MemoryOrigin {
    match args.get("origin").and_then(|v| v.as_str()) {
        Some("user") => MemoryOrigin::User,
        Some("system") => MemoryOrigin::System,
        _ => MemoryOrigin::Llm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_registered() {
        let names: Vec<&str> = MEMORY_TOOLS.iter().map(|t| t.name).collect();
        // Ровно 5 инструментов из dop_tz.
        assert_eq!(
            names,
            vec![
                "core_memory_replace",
                "core_memory_append",
                "recall_memory_search",
                "archival_memory_search",
                "archival_memory_insert",
            ]
        );
    }

    #[test]
    fn block_parse() {
        assert_eq!(
            parse_block(&json!({ "block": "persona" })).expect("p"),
            BlockType::Persona
        );
        assert!(parse_block(&json!({})).is_err());
        assert!(parse_block(&json!({ "block": "unknown" })).is_err());
    }
}
