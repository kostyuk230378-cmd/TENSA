//! Clamp контекста (dop_tz): `get_context` укладывается в
//! `max_context_tokens` — старейшие recall-сообщения вытесняются,
//! core memory сохраняется всегда.

use std::sync::Arc;

use tensa_memory::{
    DefaultMemoryManager, MemoryManager, MemoryManagerConfig, RecallMessage, Role,
    SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};

fn message(content: &str, session: uuid::Uuid) -> RecallMessage {
    RecallMessage {
        id: uuid::Uuid::new_v4(),
        role: Role::User,
        content: content.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        session_id: session,
        node_id: "node-test".into(),
        metadata: serde_json::json!({}),
    }
}

/// Менеджер с жёстким лимитом контекста.
async fn manager(max_context: u32) -> DefaultMemoryManager {
    let core = SqliteCoreMemoryStore::new_in_memory().await.expect("core");
    let recall = SqliteRecallMemoryStore::new_in_memory()
        .await
        .expect("recall");
    let archival =
        SqliteArchivalMemoryStore::new_in_memory(Arc::new(tensa_memory::HashEmbedder::new(384)))
            .await
            .expect("archival");
    DefaultMemoryManager::new(
        Arc::new(core),
        Arc::new(recall),
        Arc::new(archival),
        MemoryManagerConfig {
            max_context_tokens: max_context,
            ..MemoryManagerConfig::default()
        },
        "node-test".into(),
    )
    .await
    .expect("менеджер")
}

#[tokio::test]
async fn get_context_trims_oldest_messages_to_limit() {
    let manager = manager(200).await;

    // Инициализация: 4 блока core (~107 токенов суммарно).
    manager
        .init_default(&tensa_core::InitParams::default())
        .await
        .expect("init");

    // 5 длинных сообщений (~25 токенов каждое: 100 символов / 4).
    let session = uuid::Uuid::new_v4();
    for index in 0..5 {
        let content = format!("сообщение номер {index} {}", "х".repeat(90));
        manager
            .recall_append(message(&content, session))
            .await
            .expect("recall");
    }

    let context = manager.get_context().await.expect("контекст");
    assert!(
        context.total_tokens <= 200,
        "контекст {} превысил лимит 200",
        context.total_tokens
    );
    assert!(
        context.recent_messages.len() < 5,
        "ожидали вытеснение старых сообщений, осталось {}",
        context.recent_messages.len()
    );
    // Вытеснение частичное: свежие сообщения удержаны (2..=4 из 5).
    assert!(
        (2..=4).contains(&context.recent_messages.len()),
        "неожиданное число оставшихся: {}",
        context.recent_messages.len()
    );
    // Core memory не тронута: 4 блока на месте.
    assert_eq!(context.core_blocks.len(), 4);
}

#[tokio::test]
async fn get_context_keeps_everything_within_limit() {
    let manager = manager(16000).await;
    let session = uuid::Uuid::new_v4();
    for index in 0..3 {
        manager
            .recall_append(message(&format!("короткое {index}"), session))
            .await
            .expect("recall");
    }
    let context = manager.get_context().await.expect("контекст");
    assert_eq!(context.recent_messages.len(), 3, "лимита не достигли");
    assert!(context.total_tokens > 0);
    // Свежайшее сообщение в контексте (хронологический порядок сохранён).
    assert!(context
        .recent_messages
        .last()
        .expect("последнее")
        .content
        .contains("2"));
}

#[tokio::test]
async fn empty_memory_gives_zero_context() {
    let manager = manager(16000).await;
    let context = manager.get_context().await.expect("контекст");
    assert_eq!(context.total_tokens, 0);
    assert!(context.core_blocks.is_empty());
    assert!(context.recent_messages.is_empty());
}
