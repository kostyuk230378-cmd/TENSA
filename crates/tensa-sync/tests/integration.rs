//! Integration-тесты tensa-sync (по списку ТЗ, без сети):
//! heartbeat-цикл, соединения + офлайн-освобождение, поток событий между узлами.

use std::sync::{Arc, Mutex};
use tensa_sync::sync_log::SyncLedger;
use tensa_sync::{ConnectionManager, HeartbeatState, NodeHeartbeat};

fn hb(node: &str, ts: i64) -> NodeHeartbeat {
    NodeHeartbeat {
        node_id: node.into(),
        role: "heavy".into(),
        version: "0.1.0".into(),
        timestamp: ts,
        load: Some(0.1),
    }
}

#[test]
fn heartbeat_lifecycle_with_offline_transition() {
    let mut state = HeartbeatState::new();
    // Узлы отправляют heartbeat'ы каждые 30 с.
    state.observe(hb("pc", 0));
    state.observe(hb("phone", 0));

    assert_eq!(state.online_nodes(90, 60).len(), 2);
    state.observe(hb("pc", 60));
    // phone молчит с t=0: на 90 ещё на границе, на 100 — офлайн.
    assert!(state.is_online("phone", 90, 90));
    assert!(!state.is_online("phone", 90, 100));

    // ТЗ: освобождение соединений узла при офлайн.
    let mut mgr = ConnectionManager::new();
    mgr.acquire("email", "phone", Some("acc"), 0)
        .expect("acquire");
    state.report_stale(90, 200); // должен залогировать "phone"
    let freed = mgr.release_node("phone");
    assert_eq!(freed, vec!["email".to_string()]);
}

#[test]
fn connection_lease_full_conflict_scenario() {
    let mut mgr = ConnectionManager::new();
    // PC держит почту.
    mgr.acquire("email", "pc", None, 0).expect("pc");
    // Телефон просит — отклонён, в очереди.
    assert!(mgr.acquire("email", "phone", None, 1).is_err());
    // PC освобождает — телефон следующий.
    mgr.release("email", "pc", 2).expect("release");
    let next = mgr.next_in_queue("email");
    assert_eq!(next, Some("phone".into()));
    mgr.acquire("email", "phone", None, 3).expect("phone now");
    // TTL: через 15+ мин аренда телефона вытесняется лениво.
    mgr.acquire("email", "pc", None, 3 + 15 * 60 + 1)
        .expect("pc after ttl");
    assert_eq!(mgr.lease("email").expect("lease").holder, "pc");
}

#[test]
fn ledger_tracks_rounds_and_status() {
    let mut ledger = SyncLedger::default();
    ledger.mark_started();
    ledger.mark_round(true, 10);
    ledger.mark_round(true, 70);
    let status = ledger.snapshot(1, 1);
    assert!(status.running);
    assert_eq!(status.last_sync_at, Some(70));
}

/// Проверка одного раунда: SyncRound реализуется поверх ApprovalSync.
#[tokio::test]
async fn approval_sync_round_between_two_nodes() {
    use tensa_approval::{ActionKind, ApprovalConfig, NewAction, SqliteApprovalStore};
    use tensa_sync::store_ext::ApprovalSync;

    let a = SqliteApprovalStore::init_with_config(ApprovalConfig::default())
        .await
        .expect("a");
    let b = SqliteApprovalStore::init_with_config(ApprovalConfig::default())
        .await
        .expect("b");

    for i in 0..3 {
        a.request_action(NewAction {
            kind: ActionKind::TelegramSend,
            payload: serde_json::json!({ "chat_id": i }),
            created_by: tensa_approval::ActionOrigin::Llm,
            expires_in_hours: None,
        })
        .await
        .expect("request");
    }

    // Раунд a -> b.
    let events = a.events_since(0).await.expect("events");
    assert_eq!(events.len(), 3);
    b.apply_remote(events).await.expect("apply");

    // Все 3 действия присутствуют на b (трейт ApprovalStore в области видимости).
    use tensa_approval::ApprovalStore as _;
    let pending = b.get_pending().await.expect("pending");
    assert_eq!(pending.len(), 3);
}

/// Проверка, что общий HeartbeatState потокобезопасен под мьютексом (шаринг Arc).
#[test]
fn shared_state_compiles_and_works() {
    let state = Arc::new(Mutex::new(HeartbeatState::new()));
    let s2 = Arc::clone(&state);
    s2.lock().expect("lock").observe(hb("n", 1));
    assert!(state.lock().expect("lock").is_online("n", 90, 30));
}
