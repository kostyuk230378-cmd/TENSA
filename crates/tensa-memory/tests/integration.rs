//! Integration-тесты tensa-memory (по списку ТЗ):
//! core memory (CRUD + лимиты), recall (FTS5, TTL), archival (семантика, теги,
//! удаление), manager (контекст, search_all), tool calls (5 инструментов),
//! ошибки (превышение блока, неизвестный tool, пустые аргументы).

use std::sync::Arc;
use tensa_memory::*;

fn embedder() -> Arc<dyn Embedder> {
    Arc::new(HashEmbedder::new(384))
}

async fn manager() -> DefaultMemoryManager {
    let core = SqliteCoreMemoryStore::new_in_memory().await.expect("core");
    let recall = SqliteRecallMemoryStore::new_in_memory()
        .await
        .expect("recall");
    let archival = SqliteArchivalMemoryStore::new_in_memory(embedder())
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
    .expect("manager")
}

fn msg(role: Role, content: &str, session: uuid::Uuid) -> RecallMessage {
    RecallMessage {
        id: uuid::Uuid::new_v4(),
        role,
        content: content.into(),
        timestamp: chrono::Utc::now().timestamp(),
        session_id: session,
        node_id: "node-test".into(),
        metadata: serde_json::json!({}),
    }
}

// --- Core memory ---

#[tokio::test]
async fn core_replace_get_all() {
    let m = manager().await;
    let block = m
        .core_replace(BlockType::Persona, "Я — Tensa".into(), MemoryOrigin::System)
        .await
        .expect("replace");
    assert_eq!(block.block, BlockType::Persona);
    assert!(block.size_chars > 0 && block.size_tokens > 0);

    let got = m.core_get(BlockType::Persona).await.expect("get");
    assert_eq!(got.value, "Я — Tensa");
    assert_eq!(got.updated_by, MemoryOrigin::System);

    m.core_replace(
        BlockType::Human,
        "Владелец: Иван".into(),
        MemoryOrigin::User,
    )
    .await
    .expect("human");
    let all = m.core_get_all().await.expect("all");
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn core_append_combines() {
    let m = manager().await;
    m.core_replace(BlockType::Tasks, "Задача 1".into(), MemoryOrigin::User)
        .await
        .expect("replace");
    m.core_append(BlockType::Tasks, "Задача 2".into(), MemoryOrigin::Llm)
        .await
        .expect("append");
    let tasks = m.core_get(BlockType::Tasks).await.expect("get");
    assert!(tasks.value.contains("Задача 1"));
    assert!(tasks.value.contains("Задача 2"));
    assert_eq!(tasks.updated_by, MemoryOrigin::Llm);
}

#[tokio::test]
async fn core_block_size_exceeded() {
    let core = SqliteCoreMemoryStore::new_in_memory().await.expect("core");
    // Лимит по умолчанию 2000 токенов (~8000 символов) — даём с запасом.
    let big = "x".repeat(9000);
    let err = core
        .replace_block(BlockType::Tasks, big, MemoryOrigin::Llm)
        .await
        .expect_err("must fail");
    assert!(
        matches!(err, MemoryError::BlockSizeExceeded { .. }),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn core_get_missing_block() {
    let m = manager().await;
    let err = m
        .core_get(BlockType::Preferences)
        .await
        .expect_err("not created yet");
    assert!(matches!(err, MemoryError::BlockNotFound(_)));
}

// --- Recall memory ---

#[tokio::test]
async fn recall_append_search_recent() {
    let m = manager().await;
    let session = uuid::Uuid::new_v4();
    m.recall_append(msg(Role::User, "Напомни про встречу с Иваном", session))
        .await
        .expect("1");
    m.recall_append(msg(Role::Assistant, "Встреча с Иваном в 15:00", session))
        .await
        .expect("2");
    m.recall_append(msg(Role::User, "Какая погода?", session))
        .await
        .expect("3");

    let hits = m.recall_search("Иван", 10).await.expect("search");
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|r| r.source == MemorySource::Recall));

    let recent = m.recall_recent(2).await.expect("recent");
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[1].content, "Какая погода?"); // хронологический порядок
}

#[tokio::test]
async fn recall_cleanup_old() {
    let recall = SqliteRecallMemoryStore::new_in_memory()
        .await
        .expect("recall");
    let session = uuid::Uuid::new_v4();
    let mut old = msg(Role::User, "старое", session);
    old.timestamp = chrono::Utc::now().timestamp() - 400 * 86400; // 400 дней назад
    recall.append_message(old).await.expect("old");
    recall
        .append_message(msg(Role::User, "свежее", session))
        .await
        .expect("new");

    let removed = recall.cleanup_old(365).await.expect("cleanup");
    assert_eq!(removed, 1);
    assert_eq!(recall.get_recent(10).await.expect("recent").len(), 1);
}

// --- Archival memory ---

#[tokio::test]
async fn archival_insert_search_delete_by_tags() {
    // Архив тестируем напрямую: у manager только insert/search.
    let archival = SqliteArchivalMemoryStore::new_in_memory(embedder())
        .await
        .expect("archival");

    let id = archival
        .insert(ArchivalEntry {
            id: uuid::Uuid::new_v4(),
            content: "Пароль от Wi-Fi: тенза-2026".into(),
            embedding: Vec::new(), // посчитает хранилище
            tags: vec!["wifi".into(), "пароль".into()],
            source: Some("заметка".into()),
            created_at: 0,
            node_id: "node-test".into(),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("insert");
    assert!(!id.is_nil());

    // Семантический поиск.
    let hits = archival.search("пароль Wi-Fi", 5).await.expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source, MemorySource::Archival);

    // Поиск по тегам.
    let by_tags = archival
        .search_by_tags(&["wifi".to_string()], 5)
        .await
        .expect("by tags");
    assert_eq!(by_tags.len(), 1);

    // Удаление — больше не находим.
    archival.delete(id).await.expect("delete");
    assert!(archival
        .search("пароль", 5)
        .await
        .expect("search2")
        .is_empty());
    assert!(archival
        .search_by_tags(&["wifi".to_string()], 5)
        .await
        .expect("tags2")
        .is_empty());
}

// --- Manager: контекст и сквозной поиск ---

#[tokio::test]
async fn manager_get_context_and_search_all() {
    let m = manager().await;
    m.core_replace(
        BlockType::Persona,
        "Я — Tensa, секретарь. Люблю порядок в отпуск записях".into(),
        MemoryOrigin::System,
    )
    .await
    .expect("persona");
    let session = uuid::Uuid::new_v4();
    m.recall_append(msg(Role::User, "Обсудили отпуск в Сочи", session))
        .await
        .expect("recall");
    m.archival_insert(ArchivalEntry {
        id: uuid::Uuid::new_v4(),
        content: "Отпуск забронирован на июль".into(),
        embedding: Vec::new(),
        tags: vec!["отпуск".into()],
        source: None,
        created_at: 0,
        node_id: "node-test".into(),
        metadata: serde_json::json!({}),
    })
    .await
    .expect("archival");

    // Контекст: core + recent recall.
    let ctx = m.get_context().await.expect("context");
    assert_eq!(ctx.core_blocks.len(), 1);
    assert_eq!(ctx.recent_messages.len(), 1);
    assert!(ctx.total_tokens > 0);

    // Сквозной поиск находит во всех трёх источниках.
    let found = m.search_all("отпуск", 10).await.expect("search_all");
    let sources: Vec<MemorySource> = found.iter().map(|r| r.source).collect();
    assert!(sources.contains(&MemorySource::Core));
    assert!(sources.contains(&MemorySource::Recall));
    assert!(sources.contains(&MemorySource::Archival));
}

#[tokio::test]
async fn manager_init_default_idempotent() {
    let m = manager().await;
    let params = tensa_core::InitParams::default();
    m.init_default(&params).await.expect("init 1");
    m.init_default(&params)
        .await
        .expect("init 2 (идемпотентно)");
    let blocks = m.core_get_all().await.expect("all");
    assert_eq!(blocks.len(), 4); // persona, human, tasks, preferences
}

// --- Tool calls ---

#[tokio::test]
async fn tools_core_replace_and_append() {
    let m = manager().await;
    let out = dispatch_tool(
        &m,
        "core_memory_replace",
        &serde_json::json!({ "block": "persona", "value": "Я — Tensa", "origin": "system" }),
    )
    .await
    .expect("replace");
    assert_eq!(out["ok"], true);

    let out = dispatch_tool(
        &m,
        "core_memory_append",
        &serde_json::json!({ "block": "tasks", "value": "Купить хлеб" }),
    )
    .await
    .expect("append");
    assert_eq!(out["ok"], true);

    let tasks = m.core_get(BlockType::Tasks).await.expect("get");
    assert!(tasks.value.contains("Купить хлеб"));
}

#[tokio::test]
async fn tools_recall_and_archival_search_insert() {
    let m = manager().await;
    let session = uuid::Uuid::new_v4();
    m.recall_append(msg(Role::User, "Встреча с командой в понедельник", session))
        .await
        .expect("recall");

    let out = dispatch_tool(
        &m,
        "recall_memory_search",
        &serde_json::json!({ "query": "встреча", "limit": 5 }),
    )
    .await
    .expect("recall search");
    assert_eq!(out["results"].as_array().expect("arr").len(), 1);

    let out = dispatch_tool(
        &m,
        "archival_memory_insert",
        &serde_json::json!({ "content": "Пин-код сейфа 1234", "tags": ["сейф"] }),
    )
    .await
    .expect("insert");
    assert_eq!(out["ok"], true);

    let out = dispatch_tool(
        &m,
        "archival_memory_search",
        &serde_json::json!({ "query": "пин-код сейфа" }),
    )
    .await
    .expect("archival search");
    assert!(!out["results"].as_array().expect("arr").is_empty());
}

#[tokio::test]
async fn tool_errors_missing_args_and_unknown() {
    let m = manager().await;
    // Неизвестный инструмент.
    let err = dispatch_tool(&m, "memory_nope", &serde_json::json!({}))
        .await
        .expect_err("unknown tool");
    assert!(matches!(err, MemoryError::Config(_)), "got: {err:?}");

    // Отсутствует обязательный аргумент.
    let err = dispatch_tool(
        &m,
        "core_memory_replace",
        &serde_json::json!({ "block": "tasks" }),
    )
    .await
    .expect_err("missing value");
    assert!(matches!(err, MemoryError::Config(_)));

    // Некорректный блок.
    let err = dispatch_tool(
        &m,
        "core_memory_replace",
        &serde_json::json!({ "block": "hack", "value": "x" }),
    )
    .await
    .expect_err("bad block");
    assert!(matches!(err, MemoryError::Config(_)));
}
