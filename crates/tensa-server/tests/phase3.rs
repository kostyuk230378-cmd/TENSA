//! Фаза 3: протокол локальной модели (system_prompt_tensa.md) — сквозной
//! цикл «LLM вернула JSON → tool call применён к памяти → изменение видно
//! в get_context», плюс fallback-ветки парсера.

use std::sync::Arc;

use serde_json::json;
use tensa_memory::{
    dispatch_tool, BlockType, DefaultMemoryManager, HashEmbedder, MemoryManager,
    MemoryManagerConfig, SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};
use tensa_router::protocol::{parse_tool_reply, SECRETARY_TOOLS};

/// Менеджер in-memory (как в тестах tensa-memory).
async fn manager() -> DefaultMemoryManager {
    let core = SqliteCoreMemoryStore::new_in_memory().await.expect("core");
    let recall = SqliteRecallMemoryStore::new_in_memory()
        .await
        .expect("recall");
    let archival = SqliteArchivalMemoryStore::new_in_memory(Arc::new(HashEmbedder::new(384)))
        .await
        .expect("archival");
    DefaultMemoryManager::new(
        Arc::new(core),
        Arc::new(recall),
        Arc::new(archival),
        MemoryManagerConfig::default(),
        "node-test".into(),
    )
    .await
    .expect("менеджер")
}

/// Имена инструментов из протокола (как их передаёт NativeLocal).
fn known_tools() -> Vec<&'static str> {
    SECRETARY_TOOLS.iter().map(|(name, _)| *name).collect()
}

#[tokio::test]
async fn llm_json_applied_to_memory_and_visible_in_context() {
    let manager = manager().await;

    // Локальная модель отвечает JSON в Markdown-ограждении с болтовнёй вокруг.
    let raw = "Хорошо, записываю:\n```json\n{\"action\": \"core_memory_append\", \
               \"params\": {\"block\": \"tasks\", \"value\": \"\\nКупить хлеб\"}, \
               \"reasoning\": \"новая задача\"}\n```";
    let parsed = parse_tool_reply(raw, &known_tools());
    assert_eq!(parsed.tool_calls.len(), 1, "один tool call из JSON");
    assert_eq!(parsed.tool_calls[0].tool, "core_memory_append");

    // Применение через диспетчер (как в /query для memory-инструментов).
    let call = &parsed.tool_calls[0];
    let out = dispatch_tool(&manager, &call.tool, &call.params)
        .await
        .expect("tool call применён");
    assert_eq!(out["ok"], json!(true));

    // Изменение видно в контексте.
    let context = manager.get_context().await.expect("контекст");
    let tasks = context
        .core_blocks
        .iter()
        .find(|block| block.block == BlockType::Tasks)
        .expect("блок tasks");
    assert!(tasks.value.contains("Купить хлеб"));
    assert!(context.total_tokens > 0);
}

#[tokio::test]
async fn archival_insert_via_protocol_is_searchable() {
    let manager = manager().await;
    let raw = r#"{"action": "archival_memory_insert", "params": {"content": "Иван предпочитает сметы в PDF", "tags": ["иван", "почта"]}}"#;
    let parsed = parse_tool_reply(raw, &known_tools());
    assert_eq!(parsed.tool_calls.len(), 1);
    let call = &parsed.tool_calls[0];
    let id = dispatch_tool(&manager, &call.tool, &call.params)
        .await
        .expect("вставка в архив");
    assert!(id["id"].as_str().expect("uuid").len() >= 32);

    // Семантический поиск находит сохранённый факт.
    let found = manager
        .archival_search("сметы в PDF", 5)
        .await
        .expect("поиск");
    assert!(
        found.iter().any(|hit| hit.content.contains("сметы в PDF")),
        "факт найден в архиве"
    );
}

#[tokio::test]
async fn unparseable_reply_falls_back_to_ask_user() {
    let parsed = parse_tool_reply(
        "Кажется, вы хотите что-то найти. Уточните запрос.",
        &known_tools(),
    );
    assert!(parsed.tool_calls.is_empty());
    assert_eq!(parsed.action.as_deref(), Some("ask_user"));
    assert!(parsed
        .action_payload
        .as_deref()
        .unwrap()
        .contains("Уточните"));
}

#[tokio::test]
async fn unknown_tool_in_reply_falls_back_to_ask_user() {
    let parsed = parse_tool_reply(
        r#"{"action": "wipe_disk", "params": {"target": "C:"}}"#,
        &known_tools(),
    );
    assert!(
        parsed.tool_calls.is_empty(),
        "неизвестный инструмент не выполняется"
    );
    assert_eq!(parsed.action.as_deref(), Some("ask_user"));
}

#[tokio::test]
async fn invalid_memory_args_are_rejected_by_dispatcher() {
    let manager = manager().await;
    let raw =
        r#"{"action": "core_memory_append", "params": {"block": "unknown-block", "value": "x"}}"#;
    let parsed = parse_tool_reply(raw, &known_tools());
    assert_eq!(parsed.tool_calls.len(), 1, "имя инструмента известно");
    let call = &parsed.tool_calls[0];
    assert!(
        dispatch_tool(&manager, &call.tool, &call.params)
            .await
            .is_err(),
        "неизвестный блок отклонён диспетчером"
    );
}
