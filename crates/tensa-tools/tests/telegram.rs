//! Интеграционные тесты Telegram-инструмента (сеть не трогается).
//!
//! Проверяется цикл одобрения на in-memory хранилище: `propose` создаёт
//! pending-действие `telegram_send`, неодобренное исполнить нельзя,
//! метаданные инструмента соответствуют ТЗ.

use std::path::PathBuf;
use std::sync::Arc;

use tensa_approval::{ApprovalConfig, ApprovalStore, SqliteApprovalStore};
use tensa_tools::telegram::TelegramService;
use tensa_tools::Tool;

/// Сервис на временном корне (сеть не используется: токен фиктивный).
fn test_service() -> TelegramService {
    TelegramService::from_parts(
        std::env::temp_dir().join("tensa-telegram-test"),
        PathBuf::from("config.toml"),
        "test-bot-credential".to_string(),
        "42".to_string(),
        "11111111-2222-3333-4444-555555555555".to_string(),
    )
    .expect("конструктор сервиса")
}

/// Хранилище одобрений в памяти с явно разрешённым telegram-каналом.
async fn test_store() -> SqliteApprovalStore {
    let config = ApprovalConfig {
        allow_telegram_send: true,
        ..ApprovalConfig::default()
    };
    SqliteApprovalStore::init_with_config(config)
        .await
        .expect("хранилище в памяти")
}

#[tokio::test]
async fn propose_creates_pending_telegram_action() {
    let store = test_store().await;
    let service = test_service();

    let action = service
        .propose_send("напоминание: позвонить в банк", &store)
        .await
        .expect("propose");
    assert_eq!(action.kind.as_str(), "telegram_send");
    assert_eq!(action.status.as_str(), "pending");
    assert_eq!(
        action.payload.get("text").and_then(|value| value.as_str()),
        Some("напоминание: позвонить в банк")
    );

    let pending = store.get_pending().await.expect("pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, action.id);
}

#[tokio::test]
async fn propose_rejects_empty_text() {
    let store = test_store().await;
    assert!(test_service().propose_send("   ", &store).await.is_err());
}

#[tokio::test]
async fn execute_pending_is_denied_without_network() {
    let store = test_store().await;
    let service = test_service();

    let action = service
        .propose_send("черновик", &store)
        .await
        .expect("propose");
    // Пока действие pending — исполнение отсекается до сети.
    let error = service
        .execute_approved(&store, action.id)
        .await
        .expect_err("pending нельзя исполнять");
    assert!(error.to_string().contains("approved"), "ошибка: {error}");
}

#[tokio::test]
async fn execute_foreign_kind_is_denied() {
    let store = test_store().await;
    let service = test_service();

    // Действие другого вида через прямую запись в стор.
    let foreign = store
        .request_action(tensa_approval::NewAction {
            kind: tensa_approval::ActionKind::AppLaunch,
            payload: serde_json::json!({ "name": "calc" }),
            created_by: tensa_approval::ActionOrigin::Llm,
            expires_in_hours: None,
        })
        .await
        .expect("request");
    store
        .approve(foreign.id, tensa_approval::ActionOrigin::User)
        .await
        .expect("approve");

    let error = service
        .execute_approved(&store, foreign.id)
        .await
        .expect_err("чужой kind нельзя исполнять телеграмом");
    assert!(
        error.to_string().contains("telegram_send"),
        "ошибка: {error}"
    );
}

#[tokio::test]
async fn tool_metadata_matches_contract() {
    let tool = tensa_tools::telegram::TelegramTool::new(Arc::new(test_service()));
    assert_eq!(tool.name(), "telegram");
    assert!(tool.requires_approval());
    assert!(tool.description().contains("владельц"));
}
