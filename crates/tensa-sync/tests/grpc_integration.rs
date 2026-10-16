//! Integration-тесты gRPC-слоя tensa-sync (фича `grpc`):
//! реальный сервер на 127.0.0.1 + клиенты по сети.
//! Покрывают: NodeService (регистрация, heartbeat, соединения, конфликт),
//! SyncService (события ApprovalStore, push, идемпотентность, SubscribeEvents),
//! DataService (заметки, очередь одобрений), GrpcSyncService (клиент трейта),
//! VoiceService (UNIMPLEMENTED до tensa-llm), push заметок без адаптера (отказ).

#![cfg(feature = "grpc")]
// Ответы tonic-клиентов: Result<_, tonic::Status> — Status большой по дизайну.
#![allow(clippy::result_large_err)]

use std::sync::Arc;
use tensa_approval::{ActionKind, ActionOrigin, ApprovalStore, NewAction, SqliteApprovalStore};
use tensa_core::{NewNote, Note, NotesStore, SqliteContactsStore, SqliteNotesStore};
use tensa_nodes::{NodeProfiler, NodeRegistry, SqliteNodeRegistry, StubProfiler};
use tensa_sync::grpc::proto::tensa;
use tensa_sync::server::{self, ServerDeps};
use tensa_sync::GrpcSyncService;

struct TestServer {
    addr: String,
    approval: Arc<SqliteApprovalStore>,
    registry: Arc<SqliteNodeRegistry>,
    notes: Arc<SqliteNotesStore>,
}

async fn spawn_server() -> TestServer {
    let approval = Arc::new(
        SqliteApprovalStore::init_with_config(tensa_approval::ApprovalConfig::default())
            .await
            .expect("approval"),
    );
    let registry = Arc::new(SqliteNodeRegistry::new_in_memory().await.expect("registry"));
    let notes = Arc::new(SqliteNotesStore::new_in_memory().await.expect("notes"));
    let contacts = Arc::new(
        SqliteContactsStore::new_in_memory()
            .await
            .expect("contacts"),
    );

    let approval_sync: Arc<dyn tensa_sync::store_ext::ApprovalSync> = approval.clone();
    let deps = ServerDeps::new(
        approval_sync,
        registry.clone(),
        notes.clone(),
        contacts,
        approval.clone(),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = server::serve(listener, deps).await;
    });

    TestServer {
        addr: format!("http://{addr}"),
        approval,
        registry,
        notes,
    }
}

async fn sync_client(
    addr: &str,
) -> tensa::sync::sync_service_client::SyncServiceClient<tonic::transport::Channel> {
    for _ in 0..50 {
        if let Ok(c) =
            tensa::sync::sync_service_client::SyncServiceClient::connect(addr.to_string()).await
        {
            return c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("server did not start");
}

async fn node_client(
    addr: &str,
) -> tensa::node::node_service_client::NodeServiceClient<tonic::transport::Channel> {
    for _ in 0..50 {
        if let Ok(c) =
            tensa::node::node_service_client::NodeServiceClient::connect(addr.to_string()).await
        {
            return c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("server did not start");
}

fn proto_node(node_id: uuid::Uuid, name: &str) -> tensa::node::Node {
    let now = chrono::Utc::now().timestamp();
    tensa::node::Node {
        node_id: node_id.to_string(),
        name: name.into(),
        device_type: "desktop".into(),
        platform: tensa::common::Platform::Windows as i32,
        role: tensa::common::NodeRole::Auto as i32,
        tailscale_ip: "100.64.0.1".into(),
        grpc_port: 8766,
        last_seen: now,
        online: true,
        ram_total_mb: 16384,
        ram_available_mb: 8192,
        cpu_cores: 8,
        cpu_usage_percent: 5.0,
        has_gpu: true,
        has_npu: false,
        battery_percent: None,
        battery_charging: None,
        thermal_status: "nominal".into(),
        resources: Some(tensa::node::NodeResources {
            telegram: true,
            email: true,
            whatsapp: false,
            llm_local: true,
            llm_cloud: true,
            memory_store: true,
            approval_store: true,
        }),
        created_at: now,
        updated_at: now,
    }
}

async fn new_action(store: &SqliteApprovalStore, kind: ActionKind) -> uuid::Uuid {
    store
        .request_action(NewAction {
            kind,
            payload: serde_json::json!({ "to": "x@y.z" }),
            created_by: ActionOrigin::Llm,
            expires_in_hours: None,
        })
        .await
        .expect("request_action")
        .id
}

// --- NodeService ---

#[tokio::test]
async fn node_register_heartbeat_list_get() {
    let srv = spawn_server().await;
    let mut client = node_client(&srv.addr).await;
    let node_id = uuid::Uuid::new_v4();

    let resp = client
        .register(tensa::node::RegisterRequest {
            node: Some(proto_node(node_id, "pc")),
        })
        .await
        .expect("register rpc")
        .into_inner();
    assert!(resp.success, "register: {:?}", resp.error);

    let hb = client
        .heartbeat(tensa::node::HeartbeatRequest {
            node_id: node_id.to_string(),
            ram_available_mb: 7000,
            cpu_usage_percent: 21.5,
            battery_percent: Some(90),
            battery_charging: Some(true),
            thermal_status: "fair".into(),
            timestamp: chrono::Utc::now().timestamp(),
        })
        .await
        .expect("heartbeat rpc")
        .into_inner();
    assert!(hb.success, "heartbeat: {:?}", hb.error);

    let list = client
        .list_nodes(tensa::node::ListNodesRequest { online_only: true })
        .await
        .expect("list rpc")
        .into_inner();
    assert_eq!(list.nodes.len(), 1);
    assert_eq!(list.nodes[0].ram_available_mb, 7000);
    assert_eq!(list.nodes[0].battery_percent, Some(90));

    let got = client
        .get_node(tensa::node::GetNodeRequest {
            node_id: node_id.to_string(),
        })
        .await
        .expect("get rpc")
        .into_inner();
    assert_eq!(got.name, "pc");
    assert_eq!(got.thermal_status, "fair");

    // Несуществующий узел — NOT_FOUND.
    let err = client
        .get_node(tensa::node::GetNodeRequest {
            node_id: uuid::Uuid::new_v4().to_string(),
        })
        .await
        .expect_err("not found");
    assert_eq!(err.code(), tonic::Code::NotFound);
}

async fn acquire_email(
    client: &mut tensa::node::node_service_client::NodeServiceClient<tonic::transport::Channel>,
    id: uuid::Uuid,
) -> Result<tonic::Response<tensa::common::Result>, tonic::Status> {
    client
        .acquire_connection(tensa::node::AcquireConnectionRequest {
            node_id: id.to_string(),
            resource: "email".into(),
            account_id: Some("acc1".into()),
        })
        .await
}

#[tokio::test]
async fn connections_conflict_and_release_over_grpc() {
    let srv = spawn_server().await;
    let mut client = node_client(&srv.addr).await;
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    for (id, name) in [(a, "a"), (b, "b")] {
        client
            .register(tensa::node::RegisterRequest {
                node: Some(proto_node(id, name)),
            })
            .await
            .expect("register");
    }

    let r = acquire_email(&mut client, a)
        .await
        .expect("a acquires")
        .into_inner();
    assert!(r.success);

    // Конфликт: b получает FAILED_PRECONDITION.
    let err = acquire_email(&mut client, b).await.expect_err("busy");
    assert_eq!(err.code(), tonic::Code::FailedPrecondition);

    // Владелец ресурса — a.
    let holder = client
        .get_active_connection(tensa::node::GetActiveConnectionRequest {
            resource: "email".into(),
            account_id: Some("acc1".into()),
        })
        .await
        .expect("active rpc")
        .into_inner();
    assert_eq!(holder.node_id.as_deref(), Some(a.to_string().as_str()));

    // a освобождает — b может взять.
    let r = client
        .release_connection(tensa::node::ReleaseConnectionRequest {
            node_id: a.to_string(),
            resource: "email".into(),
            account_id: Some("acc1".into()),
        })
        .await
        .expect("release")
        .into_inner();
    assert!(r.success);
    let r = acquire_email(&mut client, b)
        .await
        .expect("b acquires")
        .into_inner();
    assert!(r.success);
}

// --- SyncService: события ApprovalStore ---

#[tokio::test]
async fn approval_events_sync_and_push_idempotent() {
    let srv = spawn_server().await;
    let action_id = new_action(&srv.approval, ActionKind::EmailSend).await;

    let mut client = sync_client(&srv.addr).await;
    let resp = client
        .sync_approval_events(tensa::sync::SyncApprovalRequest {
            node_id: "test-node".into(),
            last_event_id: 0,
            vector_clock: None,
        })
        .await
        .expect("sync rpc");
    let mut stream = resp.into_inner();
    let first = stream
        .message()
        .await
        .expect("stream")
        .expect("created event");
    assert_eq!(first.action_id, action_id.to_string());
    assert_eq!(first.event_type, "created");

    // Push того же события на сервер — идемпотентно (success, без дублей).
    let push = client
        .push_approval_event(first.clone())
        .await
        .expect("push rpc")
        .into_inner();
    assert!(push.success, "push: {:?}", push.error);
    let push2 = client
        .push_approval_event(first)
        .await
        .expect("push2 rpc")
        .into_inner();
    assert!(push2.success);
    let events = srv.approval.get_events_since(0).await.expect("events");
    assert_eq!(events.len(), 1, "дубликат не записан");
}

#[tokio::test]
async fn subscribe_events_receives_approval_push() {
    let srv = spawn_server().await;
    let mut client = sync_client(&srv.addr).await;

    // Подписка на события типа approval.
    let resp = client
        .subscribe_events(tensa::sync::SubscribeRequest {
            node_id: "watcher".into(),
            event_types: vec!["approval".into()],
        })
        .await
        .expect("subscribe rpc");
    let mut stream = resp.into_inner();

    // Небольшая пауза, чтобы подписка закрепилась на шине.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Публикуем событие через push_approval_event.
    let action_id = new_action(&srv.approval, ActionKind::TelegramSend).await;
    let events = srv.approval.get_events_since(0).await.expect("events");
    let ev = events
        .iter()
        .find(|e| e.action_id == action_id)
        .expect("created event");
    let pushed = client
        .push_approval_event(tensa_sync::grpc::approval_event_to_proto(ev))
        .await
        .expect("push")
        .into_inner();
    assert!(pushed.success);

    let received = tokio::time::timeout(std::time::Duration::from_secs(5), stream.message())
        .await
        .expect("timeout waiting for event")
        .expect("stream alive")
        .expect("event");
    assert_eq!(received.event_type, "approval");
    assert!(received.data.contains(&action_id.to_string()));
}

// --- Заметки: реальный адаптер (одобрено 2026-10-15) ---

#[tokio::test]
async fn notes_sync_reads_store_events() {
    let srv = spawn_server().await;
    let note = srv
        .notes
        .create_note(NewNote {
            title: "заметка".into(),
            body: "тело".into(),
            tags: vec![],
        })
        .await
        .expect("create");

    let mut client = sync_client(&srv.addr).await;
    let resp = client
        .sync_notes(tensa::sync::SyncNotesRequest {
            node_id: "remote".into(),
            last_updated_at: 0,
            vector_clock: None,
        })
        .await
        .expect("sync notes rpc");
    let mut stream = resp.into_inner();
    let ev = stream.message().await.expect("stream").expect("event");
    assert_eq!(ev.note_id, note.id.to_string());
    assert_eq!(ev.event_type, "created");
    assert!(ev.note_data.contains("заметка"));
    assert!(stream.message().await.expect("end").is_none());

    // watermark: повторный запрос от updated_at — пусто.
    let resp = client
        .sync_notes(tensa::sync::SyncNotesRequest {
            node_id: "remote".into(),
            last_updated_at: note.updated_at,
            vector_clock: None,
        })
        .await
        .expect("sync2");
    let mut stream = resp.into_inner();
    assert!(stream.message().await.expect("stream2").is_none());
}

#[tokio::test]
async fn notes_push_applies_lww_and_rejects_missing() {
    let srv = spawn_server().await;
    let note = srv
        .notes
        .create_note(NewNote {
            title: "локальная".into(),
            body: "b".into(),
            tags: vec![],
        })
        .await
        .expect("create");
    let mut client = sync_client(&srv.addr).await;

    // Свежий снимок с чужого узла — применяется (LWW).
    let mut remote = note.clone();
    remote.title = "remote".into();
    remote.updated_at = note.updated_at + 10;
    remote.updated_on_node = "other-node".into();
    let msg = tensa::sync::NoteEvent {
        note_id: remote.id.to_string(),
        event_type: "updated".into(),
        note_data: serde_json::to_value(&remote).expect("snapshot").to_string(),
        timestamp: remote.updated_at,
        node_id: "other-node".into(),
        vector_clock: None,
    };
    let r = client
        .push_note_event(msg)
        .await
        .expect("push")
        .into_inner();
    assert!(r.success, "push: {:?}", r.error);
    let local = srv
        .notes
        .get_note(note.id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(local.title, "remote");

    // Устаревший снимок — LWW пропускает без ошибки.
    let mut stale = note.clone();
    stale.title = "старое".into();
    stale.updated_at = note.updated_at - 5; // строго старше текущей локальной версии
    stale.updated_on_node = "other-node".into();
    let msg = tensa::sync::NoteEvent {
        note_id: stale.id.to_string(),
        event_type: "updated".into(),
        note_data: serde_json::to_value(&stale).expect("snapshot").to_string(),
        timestamp: stale.updated_at,
        node_id: "other-node".into(),
        vector_clock: None,
    };
    let r = client
        .push_note_event(msg)
        .await
        .expect("push stale")
        .into_inner();
    assert!(r.success);
    let local = srv
        .notes
        .get_note(note.id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(local.title, "remote", "устаревшее событие не применено");

    // Несуществующая локально заметка — явный отказ
    // (insert-with-id не входит в NotesStore, решение ожидает апрува).
    let ghost = Note {
        id: uuid::Uuid::new_v4(),
        title: "ghost".into(),
        body: String::new(),
        tags: vec![],
        created_at: 0,
        updated_at: 1,
        created_on_node: "remote".into(),
        updated_on_node: "remote".into(),
        deleted: false,
        vector_clock: tensa_approval::VectorClock::new(),
    };
    let msg = tensa::sync::NoteEvent {
        note_id: ghost.id.to_string(),
        event_type: "created".into(),
        note_data: serde_json::to_value(&ghost).expect("snapshot").to_string(),
        timestamp: 1,
        node_id: "remote".into(),
        vector_clock: None,
    };
    let err = client.push_note_event(msg).await.expect_err("missing");
    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    assert!(
        err.message().contains("insert-with-id"),
        "msg: {}",
        err.message()
    );
}

// --- DataService ---

#[tokio::test]
async fn data_service_notes_and_approval_queue() {
    let srv = spawn_server().await;
    let note = srv
        .notes
        .create_note(NewNote {
            title: "план".into(),
            body: "текст заметки".into(),
            tags: vec!["работа".into()],
        })
        .await
        .expect("create note");
    // Вторая заметка — для проверки total (общий count, не размер страницы).
    srv.notes
        .create_note(NewNote {
            title: "вторая".into(),
            body: "b".into(),
            tags: vec![],
        })
        .await
        .expect("create note 2");
    let _ = new_action(&srv.approval, ActionKind::CalendarCreate).await;

    let mut client = tensa::data::data_service_client::DataServiceClient::connect(srv.addr.clone())
        .await
        .expect("connect");

    // Запрашиваем 1 запись — в total всё равно 2 (Контракт 3, count_notes).
    let notes = client
        .get_notes(tensa::data::GetNotesRequest {
            node_id: "n".into(),
            limit: 1,
            offset: 0,
        })
        .await
        .expect("get notes rpc")
        .into_inner();
    assert_eq!(notes.notes.len(), 1);
    assert_eq!(notes.total, 2, "total — общий count, не размер страницы");

    let got = client
        .get_note(tensa::data::GetNoteRequest {
            note_id: note.id.to_string(),
        })
        .await
        .expect("get note rpc")
        .into_inner();
    assert_eq!(got.body, "текст заметки");
    assert_eq!(got.tags, vec!["работа".to_string()]);

    let queue = client
        .get_approval_queue(tensa::data::GetApprovalQueueRequest {
            node_id: "n".into(),
        })
        .await
        .expect("queue rpc")
        .into_inner();
    assert_eq!(queue.actions.len(), 1);
    assert_eq!(
        queue.actions[0].status,
        tensa::common::ActionStatus::Pending as i32
    );
    assert_eq!(
        queue.actions[0].kind,
        tensa::common::ActionKind::CalendarCreate as i32
    );
}

// --- GrpcSyncService (клиент трейта SyncService) ---

#[tokio::test]
async fn grpc_sync_service_full_flow() {
    let srv = spawn_server().await;
    let node_id = uuid::Uuid::new_v4();
    // Узел-клиент регистрируем на сервере.
    srv.registry
        .register(tensa_nodes::Node {
            node_id,
            name: "client".into(),
            device_type: tensa_core::DeviceType::Desktop,
            platform: tensa_nodes::Platform::Windows,
            role: tensa_core::NodeRole::Auto,
            tailscale_ip: "100.64.0.2".into(),
            grpc_port: 8766,
            last_seen: chrono::Utc::now().timestamp(),
            online: true,
            ram_total_mb: 8192,
            ram_available_mb: 4096,
            cpu_cores: 4,
            cpu_usage_percent: 10.0,
            has_gpu: false,
            has_npu: false,
            battery_percent: None,
            battery_charging: None,
            thermal_status: tensa_nodes::ThermalStatus::Nominal,
            resources: tensa_nodes::NodeResources::default(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        })
        .await
        .expect("register node");

    let endpoint = tonic::transport::Endpoint::from_shared(srv.addr.clone()).expect("endpoint");
    let profiler: Arc<dyn NodeProfiler> = Arc::new(StubProfiler::desktop());
    let svc = GrpcSyncService::new(endpoint, node_id.to_string(), profiler);

    tensa_sync::service::SyncService::start(&svc)
        .await
        .expect("start");

    // Heartbeat отправляется от лица узла.
    tensa_sync::service::SyncService::send_heartbeat(&svc)
        .await
        .expect("heartbeat");
    let node = srv.registry.get(node_id).await.expect("get").expect("node");
    assert_eq!(node.ram_available_mb, 8192, "телеметрия из StubProfiler");

    // Онлайн-узлы видны через трейт.
    let online = tensa_sync::service::SyncService::list_online_nodes(&svc)
        .await
        .expect("list");
    assert_eq!(online.len(), 1);

    // Соединение: захват и освобождение.
    tensa_sync::service::SyncService::acquire_connection(&svc, "email", Some("acc"))
        .await
        .expect("acquire");
    let holder = srv
        .registry
        .get_active_connection("email", Some("acc"))
        .await
        .expect("active");
    assert_eq!(holder, Some(node_id));
    tensa_sync::service::SyncService::release_connection(&svc, "email", Some("acc"))
        .await
        .expect("release");

    let status = tensa_sync::service::SyncService::status(&svc)
        .await
        .expect("status");
    assert!(status.running);
    assert_eq!(status.peers_online, 1);

    tensa_sync::service::SyncService::stop(&svc)
        .await
        .expect("stop");
    let status = tensa_sync::service::SyncService::status(&svc)
        .await
        .expect("status2");
    assert!(!status.running);
}

#[tokio::test]
async fn grpc_sync_service_fetches_approval_events() {
    let srv = spawn_server().await;
    let action_id = new_action(&srv.approval, ActionKind::AppLaunch).await;

    let endpoint = tonic::transport::Endpoint::from_shared(srv.addr.clone()).expect("endpoint");
    let profiler: Arc<dyn NodeProfiler> = Arc::new(StubProfiler::desktop());
    let svc = GrpcSyncService::new(endpoint, uuid::Uuid::new_v4().to_string(), profiler);

    let events = tensa_sync::service::SyncService::sync_approval_events(&svc)
        .await
        .expect("sync");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action_id, action_id);
    assert_eq!(events[0].event_type, tensa_approval::EventType::Created);
}

// --- VoiceService: заглушка до tensa-llm ---

#[tokio::test]
async fn voice_service_is_unimplemented() {
    let srv = spawn_server().await;
    let mut client =
        tensa::voice::voice_service_client::VoiceServiceClient::connect(srv.addr.clone())
            .await
            .expect("connect");
    let err = client
        .transcribe_file(tensa::voice::TranscribeFileRequest {
            node_id: "n".into(),
            audio_data: vec![1, 2, 3],
            format: "pcm".into(),
            language: "ru".into(),
        })
        .await
        .expect_err("unimplemented");
    assert_eq!(err.code(), tonic::Code::Unimplemented);
    assert!(err.message().contains("tensa-llm"));
}
