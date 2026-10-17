//! Интеграционные тесты email-инструмента (сеть не трогается).
//!
//! Проверяется цикл одобрения на in-memory хранилище: `propose` создаёт
//! pending-действие `email_send`, неодобренное исполнить нельзя, инструмент
//! реестра не умеет отправить письмо мимо очереди.

use std::sync::Arc;

use tensa_approval::{ApprovalConfig, ApprovalStore, SqliteApprovalStore};
use tensa_tools::email::{EmailService, EmailTool, OutgoingEmail};
use tensa_tools::Tool;

/// Конфиг с одним gmail-аккаунтом (автоопределение хостов).
fn config_with_gmail() -> tensa_core::Config {
    let mut config = tensa_core::Config::default();
    let mut table = toml::Table::new();
    table.insert(
        "email".to_string(),
        toml::Value::String("owner@gmail.com".to_string()),
    );
    table.insert(
        "label".to_string(),
        toml::Value::String("Работа".to_string()),
    );
    let mut accounts = toml::Table::new();
    accounts.insert("work".to_string(), toml::Value::Table(table));
    config
        .email
        .insert("accounts".to_string(), toml::Value::Table(accounts));
    config
}

/// Сервис на временном корне (сеть не используется).
fn test_service() -> EmailService {
    EmailService::from_parts(
        std::env::temp_dir().join("tensa-email-test"),
        config_with_gmail(),
        tensa_core::Secrets::default(),
    )
}

/// Хранилище одобрений в памяти с явно разрешённой отправкой почты.
async fn test_store() -> SqliteApprovalStore {
    let config = ApprovalConfig {
        allow_email_send: true,
        ..ApprovalConfig::default()
    };
    SqliteApprovalStore::init_with_config(config)
        .await
        .expect("хранилище в памяти")
}

fn sample_email() -> OutgoingEmail {
    OutgoingEmail {
        account_id: Some("work".to_string()),
        to: "friend@example.com".to_string(),
        cc: None,
        bcc: None,
        subject: "тема письма".to_string(),
        body: "текст письма".to_string(),
        attachment: None,
    }
}

#[tokio::test]
async fn accounts_resolve_from_config() {
    let service = test_service();
    let accounts = service.accounts();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].smtp_host, "smtp.gmail.com");
    assert_eq!(accounts[0].smtp_port, 587);
}

#[tokio::test]
async fn propose_creates_pending_email_action() {
    let store = test_store().await;
    let service = test_service();

    let action = service
        .propose_send(sample_email(), &store)
        .await
        .expect("propose");
    assert_eq!(action.kind.as_str(), "email_send");
    assert_eq!(action.status.as_str(), "pending");
    assert_eq!(
        action.payload.get("to").and_then(|value| value.as_str()),
        Some("friend@example.com")
    );

    let pending = store.get_pending().await.expect("pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, action.id);
}

#[tokio::test]
async fn propose_requires_recipient() {
    let store = test_store().await;
    let mut email = sample_email();
    email.to = "   ".to_string();
    assert!(test_service().propose_send(email, &store).await.is_err());
}

#[tokio::test]
async fn execute_pending_is_denied_without_network() {
    let store = test_store().await;
    let service = test_service();

    let action = service
        .propose_send(sample_email(), &store)
        .await
        .expect("propose");
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
        .expect_err("чужой kind нельзя исполнять почтой");
    assert!(error.to_string().contains("email_send"), "ошибка: {error}");
}

#[tokio::test]
async fn tool_queues_instead_of_sending() {
    let store = Arc::new(test_store().await);
    let tool = EmailTool::new(Arc::new(test_service()), store.clone());

    assert_eq!(tool.name(), "email");
    assert!(tool.requires_approval());

    let result = tool
        .execute(serde_json::json!({
            "to": "friend@example.com",
            "subject": "из инструмента",
            "body": "текст",
        }))
        .await
        .expect("execute");
    assert_eq!(result["queued"], true);
    let action_id = result["action_id"]
        .as_str()
        .expect("id действия")
        .to_string();

    let pending = store.get_pending().await.expect("pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id.to_string(), action_id);
    assert_eq!(pending[0].kind.as_str(), "email_send");
}

#[tokio::test]
async fn tool_rejects_missing_recipient() {
    let store = Arc::new(test_store().await);
    let tool = EmailTool::new(Arc::new(test_service()), store);
    assert!(tool
        .execute(serde_json::json!({ "subject": "s", "body": "b" }))
        .await
        .is_err());
}
