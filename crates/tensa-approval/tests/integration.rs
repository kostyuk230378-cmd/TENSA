//! Integration-тесты полного жизненного цикла ApprovalStore
//! (ТЗ tensa-approval, раздел «Тесты»).

use tensa_approval::*;

async fn store() -> SqliteApprovalStore {
    SqliteApprovalStore::new_in_memory().await.expect("store")
}

fn email_action() -> NewAction {
    NewAction {
        kind: ActionKind::EmailSend,
        payload: serde_json::json!({ "to": "test@example.com", "subject": "hi" }),
        created_by: ActionOrigin::Llm,
        expires_in_hours: None,
    }
}

// --- Полный цикл ---

#[tokio::test]
async fn full_cycle_request_approve_execute() {
    let s = store().await;
    let a = s.request_action(email_action()).await.expect("request");
    assert_eq!(a.status, ActionStatus::Pending);
    assert_ne!(a.id, a.operation_id); // оба UUID, но разные

    let approved = s.approve(a.id, ActionOrigin::User).await.expect("approve");
    assert_eq!(approved.status, ActionStatus::Approved);
    assert!(approved.approved_at.is_some());
    assert_eq!(approved.approved_by, Some(ActionOrigin::User));

    let executed = s.execute(a.id).await.expect("execute");
    assert_eq!(executed.status, ActionStatus::Executed);
    assert!(executed.executed_at.is_some());
}

#[tokio::test]
async fn reject_cycle() {
    let s = store().await;
    let a = s.request_action(email_action()).await.expect("request");
    let rejected = s
        .reject(a.id, ActionOrigin::Telegram)
        .await
        .expect("reject");
    assert_eq!(rejected.status, ActionStatus::Rejected);

    // После reject одобрить/исполнить нельзя.
    assert!(s.approve(a.id, ActionOrigin::User).await.is_err());
    assert!(s.execute(a.id).await.is_err());
}

#[tokio::test]
async fn expiry_via_expire_old() {
    let s = store().await;
    let a = s
        .request_action(NewAction {
            expires_in_hours: Some(0), // истекает немедленно
            ..email_action()
        })
        .await
        .expect("request");
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await; // timestamp в секундах
    let n = s.expire_old().await.expect("expire_old");
    assert_eq!(n, 1);
    let a = s.get_by_id(a.id).await.expect("get").expect("exists");
    assert_eq!(a.status, ActionStatus::Expired);
    // Истёкшее нельзя одобрить.
    assert!(s.approve(a.id, ActionOrigin::User).await.is_err());
}

#[tokio::test]
async fn get_pending_ordered() {
    let s = store().await;
    let a1 = s.request_action(email_action()).await.expect("1");
    let a2 = s
        .request_action(NewAction {
            kind: ActionKind::TelegramSend,
            payload: serde_json::json!({ "chat_id": 1 }),
            created_by: ActionOrigin::Llm,
            expires_in_hours: None,
        })
        .await
        .expect("2");
    let pending = s.get_pending().await.expect("pending");
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].id, a1.id);
    assert_eq!(pending[1].id, a2.id);
}

// --- Идемпотентность и синхронизация ---

#[tokio::test]
async fn events_since_and_apply_idempotent() {
    let s = store().await;
    let a = s.request_action(email_action()).await.expect("request");
    let _ = s.approve(a.id, ActionOrigin::User).await.expect("approve");

    let events = s.get_events_since(0).await.expect("events");
    assert!(events.len() >= 2);
    assert_eq!(events[0].event_type, EventType::Created);
    assert_eq!(events[1].event_type, EventType::Approved);

    // Повторное применение последнего события не должно дублировать эффект.
    let last = events.last().expect("last").clone();
    s.apply_event(last).await.expect("apply");
    let all = s.get_events_since(0).await.expect("events2");
    // Дубликат события отсечён: количество не выросло.
    assert_eq!(all.len(), events.len());
}

#[tokio::test]
async fn apply_event_creates_missing_action_from_snapshot() {
    let s1 = store().await;
    let a = s1.request_action(email_action()).await.expect("request");
    let ev = s1
        .get_events_since(0)
        .await
        .expect("events")
        .into_iter()
        .next()
        .expect("created event");

    let s2 = SqliteApprovalStore::new_in_memory().await.expect("s2");
    s2.apply_event(ev).await.expect("apply");
    let copy = s2.get_by_id(a.id).await.expect("get").expect("replicated");
    assert_eq!(copy.status, a.status);
    assert_eq!(copy.operation_id, a.operation_id);
}

// --- Конфигурация: выключатели и лимиты ---

#[tokio::test]
async fn disabled_kind_rejected() {
    let s = SqliteApprovalStore::init_with_config(ApprovalConfig {
        allow_email_send: false,
        ..ApprovalConfig::default()
    })
    .await
    .expect("store");
    let err = s
        .request_action(email_action())
        .await
        .expect_err("must fail");
    assert!(
        matches!(err, ApprovalError::KindDisabled { .. }),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn whatsapp_disabled_by_default() {
    let s = store().await;
    let err = s
        .request_action(NewAction {
            kind: ActionKind::WhatsAppSend,
            payload: serde_json::json!({}),
            created_by: ActionOrigin::Llm,
            expires_in_hours: None,
        })
        .await
        .expect_err("whatsapp must be disabled by default");
    assert!(matches!(err, ApprovalError::KindDisabled { .. }));
}

#[tokio::test]
async fn max_pending_enforced() {
    let s = SqliteApprovalStore::init_with_config(ApprovalConfig {
        max_pending_actions: 2,
        ..ApprovalConfig::default()
    })
    .await
    .expect("store");
    let _ = s.request_action(email_action()).await.expect("1");
    let _ = s.request_action(email_action()).await.expect("2");
    let err = s
        .request_action(email_action())
        .await
        .expect_err("3rd must fail");
    assert!(
        matches!(err, ApprovalError::MaxPendingReached { limit: 2 }),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn auto_approve_trusted_llm_still_goes_through_queue() {
    // auto_approve_trusted=true одобряет только trusted origin; LLM — всегда очередь.
    let s = SqliteApprovalStore::init_with_config(ApprovalConfig {
        auto_approve_trusted: true,
        ..ApprovalConfig::default()
    })
    .await
    .expect("store");
    let a = s.request_action(email_action()).await.expect("request");
    assert_eq!(
        a.status,
        ActionStatus::Pending,
        "LLM никогда не авто-одобряется"
    );
}

#[tokio::test]
async fn auto_approve_trusted_user() {
    let s = SqliteApprovalStore::init_with_config(ApprovalConfig {
        auto_approve_trusted: true,
        ..ApprovalConfig::default()
    })
    .await
    .expect("store");
    let a = s
        .request_action(NewAction {
            created_by: ActionOrigin::User,
            ..email_action()
        })
        .await
        .expect("request");
    assert_eq!(a.status, ActionStatus::Approved);
}
