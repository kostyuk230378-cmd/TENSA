//! gRPC-сервер: реализация пяти сервисов Контракта 8.
//!
//! - SyncService — события ApprovalStore (реальные) + события заметок и
//!   контактов (через NotesSyncSource/ContactsSyncSource; до подключения
//!   адаптеров работает NoopSyncSource) + шина событий SubscribeEvents;
//! - NodeService — реестр узлов tensa-nodes (регистрация, heartbeat,
//!   соединения) + поток heartbeat'ов по подписке;
//! - DataService — заметки/контакты (tensa-core), очередь одобрений
//!   (tensa-approval), поиск по файлам (tensa-memory, если подключён);
//! - VoiceService, QueryService — заглушки UNIMPLEMENTED до появления
//!   крейта tensa-llm (ТЗ: маршрутизация и инференс там).
//!
// Реализации tonic-сервисов обязаны возвращать Result<_, tonic::Status>
// (Status ~176 байт по дизайну фреймворка) — подавляем lint осознанно.
#![allow(clippy::result_large_err)]

use crate::adapters::{StoreContactsSync, StoreNotesSync};
use crate::error::{Result, SyncError};
use crate::grpc::{self, proto::tensa};
use crate::service::{ContactsSyncSource, NotesSyncSource};
use crate::store_ext::ApprovalSync;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tonic::{Request, Response, Status};
use tracing::info;

type Stream<T> = Pin<Box<dyn futures::Stream<Item = std::result::Result<T, Status>> + Send>>;

/// Шина событий для SubscribeEvents.
pub struct EventBus {
    tx: broadcast::Sender<tensa::sync::Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Опубликовать событие всем подписчикам.
    pub fn publish(&self, event: tensa::sync::Event) {
        let _ = self.tx.send(event);
    }

    fn subscribe(&self) -> broadcast::Receiver<tensa::sync::Event> {
        self.tx.subscribe()
    }
}

// === SyncService ===

pub struct SyncServiceImpl {
    pub approval: Arc<dyn ApprovalSync>,
    pub notes: Arc<dyn NotesSyncSource>,
    pub contacts: Arc<dyn ContactsSyncSource>,
    pub bus: Arc<EventBus>,
}

#[tonic::async_trait]
impl tensa::sync::sync_service_server::SyncService for SyncServiceImpl {
    type SyncApprovalEventsStream = Stream<tensa::sync::ApprovalEvent>;

    async fn sync_approval_events(
        &self,
        request: Request<tensa::sync::SyncApprovalRequest>,
    ) -> std::result::Result<Response<Self::SyncApprovalEventsStream>, Status> {
        let req = request.into_inner();
        let events = self
            .approval
            .events_since(req.last_event_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let items: Vec<std::result::Result<tensa::sync::ApprovalEvent, Status>> = events
            .iter()
            .map(|e| Ok(grpc::approval_event_to_proto(e)))
            .collect();
        Ok(Response::new(Box::pin(futures::stream::iter(items))))
    }

    async fn push_approval_event(
        &self,
        request: Request<tensa::sync::ApprovalEvent>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let msg = request.into_inner();
        let event = grpc::approval_event_from_proto(&msg)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let operation_id = msg.operation_id.clone();
        self.approval
            .apply_remote(vec![event.clone()])
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        self.bus.publish(tensa::sync::Event {
            event_type: "approval".into(),
            data: msg.event_data.clone(),
            timestamp: msg.timestamp,
            node_id: msg.node_id.clone(),
        });
        Ok(Response::new(grpc::ok_result(operation_id)))
    }

    type SyncNotesStream = Stream<tensa::sync::NoteEvent>;

    async fn sync_notes(
        &self,
        request: Request<tensa::sync::SyncNotesRequest>,
    ) -> std::result::Result<Response<Self::SyncNotesStream>, Status> {
        let req = request.into_inner();
        let events = self
            .notes
            .events_since(req.last_updated_at, 1000)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let items: Vec<std::result::Result<tensa::sync::NoteEvent, Status>> = events
            .iter()
            .map(|e| Ok(grpc::note_event_to_proto(e)))
            .collect();
        Ok(Response::new(Box::pin(futures::stream::iter(items))))
    }

    async fn push_note_event(
        &self,
        request: Request<tensa::sync::NoteEvent>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let msg = request.into_inner();
        let event = grpc::note_event_from_proto(&msg)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.notes
            .apply_event(event)
            .await
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        Ok(Response::new(grpc::ok_result("")))
    }

    type SyncContactsStream = Stream<tensa::sync::ContactEvent>;

    async fn sync_contacts(
        &self,
        request: Request<tensa::sync::SyncContactsRequest>,
    ) -> std::result::Result<Response<Self::SyncContactsStream>, Status> {
        let req = request.into_inner();
        let events = self
            .contacts
            .events_since(req.last_updated_at, 1000)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let items: Vec<std::result::Result<tensa::sync::ContactEvent, Status>> = events
            .iter()
            .map(|e| Ok(grpc::contact_event_to_proto(e)))
            .collect();
        Ok(Response::new(Box::pin(futures::stream::iter(items))))
    }

    async fn push_contact_event(
        &self,
        request: Request<tensa::sync::ContactEvent>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let msg = request.into_inner();
        let event = grpc::contact_event_from_proto(&msg)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.contacts
            .apply_event(event)
            .await
            .map_err(|e| Status::failed_precondition(e.to_string()))?;
        Ok(Response::new(grpc::ok_result("")))
    }

    type SubscribeEventsStream = Stream<tensa::sync::Event>;

    async fn subscribe_events(
        &self,
        request: Request<tensa::sync::SubscribeRequest>,
    ) -> std::result::Result<Response<Self::SubscribeEventsStream>, Status> {
        let req = request.into_inner();
        let wanted = req.event_types.clone();
        let rx = self.bus.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(move |item| {
            let wanted = wanted.clone();
            async move {
                match item {
                    Ok(ev) => {
                        if wanted.is_empty() || wanted.contains(&ev.event_type) {
                            Some(Ok(ev))
                        } else {
                            None
                        }
                    }
                    // Лаг подписчика не роняет поток.
                    Err(BroadcastStreamRecvError::Lagged(_)) => None,
                }
            }
        });
        Ok(Response::new(Box::pin(stream)))
    }
}

// === NodeService ===

pub struct NodeServiceImpl {
    pub registry: Arc<dyn tensa_nodes::NodeRegistry>,
    pub heartbeats: broadcast::Sender<tensa::node::HeartbeatRequest>,
}

#[tonic::async_trait]
impl tensa::node::node_service_server::NodeService for NodeServiceImpl {
    async fn register(
        &self,
        request: Request<tensa::node::RegisterRequest>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let msg = request.into_inner();
        let node_proto = msg
            .node
            .ok_or_else(|| Status::invalid_argument("node is required"))?;
        let node = grpc::node_from_proto(&node_proto)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.registry
            .register(node)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(grpc::ok_result("")))
    }

    async fn unregister(
        &self,
        request: Request<tensa::node::UnregisterRequest>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let req = request.into_inner();
        let node_id = req
            .node_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("bad node_id: {e}")))?;
        self.registry
            .unregister(node_id)
            .await
            .map_err(|e| match e {
                tensa_nodes::NodesError::NodeNotFound(_) => Status::not_found(e.to_string()),
                other => Status::internal(other.to_string()),
            })?;
        Ok(Response::new(grpc::ok_result("")))
    }

    async fn heartbeat(
        &self,
        request: Request<tensa::node::HeartbeatRequest>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let msg = request.into_inner();
        let hb = grpc::heartbeat_from_proto(&msg)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        self.registry
            .update_heartbeat(hb)
            .await
            .map_err(|e| match e {
                tensa_nodes::NodesError::NodeNotFound(_) => Status::not_found(e.to_string()),
                other => Status::internal(other.to_string()),
            })?;
        // Раздаём подписчикам StreamHeartbeats.
        let _ = self.heartbeats.send(msg);
        Ok(Response::new(grpc::ok_result("")))
    }

    type StreamHeartbeatsStream = Stream<tensa::node::HeartbeatRequest>;

    async fn stream_heartbeats(
        &self,
        _request: Request<tensa::node::StreamHeartbeatsRequest>,
    ) -> std::result::Result<Response<Self::StreamHeartbeatsStream>, Status> {
        let rx = self.heartbeats.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|item| async move {
            match item {
                Ok(hb) => Some(Ok(hb)),
                Err(BroadcastStreamRecvError::Lagged(_)) => None,
            }
        });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_nodes(
        &self,
        request: Request<tensa::node::ListNodesRequest>,
    ) -> std::result::Result<Response<tensa::node::ListNodesResponse>, Status> {
        let req = request.into_inner();
        let nodes = if req.online_only {
            self.registry.list_online().await
        } else {
            self.registry.list_all().await
        }
        .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(tensa::node::ListNodesResponse {
            nodes: nodes.iter().map(grpc::node_to_proto).collect(),
        }))
    }

    async fn get_node(
        &self,
        request: Request<tensa::node::GetNodeRequest>,
    ) -> std::result::Result<Response<tensa::node::Node>, Status> {
        let req = request.into_inner();
        let node_id = req
            .node_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("bad node_id: {e}")))?;
        match self
            .registry
            .get(node_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            Some(node) => Ok(Response::new(grpc::node_to_proto(&node))),
            None => Err(Status::not_found(format!("node {node_id} not found"))),
        }
    }

    async fn acquire_connection(
        &self,
        request: Request<tensa::node::AcquireConnectionRequest>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let req = request.into_inner();
        let node_id = req
            .node_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("bad node_id: {e}")))?;
        self.registry
            .acquire_connection(node_id, &req.resource, req.account_id.as_deref())
            .await
            .map_err(|e| match e {
                tensa_nodes::NodesError::ResourceBusy { .. } => {
                    Status::failed_precondition(e.to_string())
                }
                tensa_nodes::NodesError::NodeNotFound(_) => Status::not_found(e.to_string()),
                tensa_nodes::NodesError::NodeOffline(_) => Status::unavailable(e.to_string()),
                other => Status::internal(other.to_string()),
            })?;
        Ok(Response::new(grpc::ok_result("")))
    }

    async fn release_connection(
        &self,
        request: Request<tensa::node::ReleaseConnectionRequest>,
    ) -> std::result::Result<Response<tensa::common::Result>, Status> {
        let req = request.into_inner();
        let node_id = req
            .node_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("bad node_id: {e}")))?;
        self.registry
            .release_connection(node_id, &req.resource, req.account_id.as_deref())
            .await
            .map_err(|e| match e {
                tensa_nodes::NodesError::Config(_) => Status::failed_precondition(e.to_string()),
                other => Status::internal(other.to_string()),
            })?;
        Ok(Response::new(grpc::ok_result("")))
    }

    async fn get_active_connection(
        &self,
        request: Request<tensa::node::GetActiveConnectionRequest>,
    ) -> std::result::Result<Response<tensa::node::GetActiveConnectionResponse>, Status> {
        let req = request.into_inner();
        let holder = self
            .registry
            .get_active_connection(&req.resource, req.account_id.as_deref())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(tensa::node::GetActiveConnectionResponse {
            node_id: holder.map(|id| id.to_string()),
        }))
    }
}

// === DataService ===

pub struct DataServiceImpl {
    pub notes: Arc<dyn tensa_core::NotesStore>,
    pub contacts: Arc<dyn tensa_core::ContactsStore>,
    pub approval: Arc<dyn tensa_approval::ApprovalStore>,
    pub memory: Option<Arc<dyn tensa_memory::MemoryStore>>,
}

#[tonic::async_trait]
impl tensa::data::data_service_server::DataService for DataServiceImpl {
    async fn get_notes(
        &self,
        request: Request<tensa::data::GetNotesRequest>,
    ) -> std::result::Result<Response<tensa::data::GetNotesResponse>, Status> {
        let req = request.into_inner();
        let notes = self
            .notes
            .list_notes(req.limit as usize, req.offset as usize)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        // Общий count (Контракт 3, одобрено 2026-10-15), не размер страницы.
        let total = self
            .notes
            .count_notes()
            .await
            .map_err(|e| Status::internal(e.to_string()))? as u32;
        Ok(Response::new(tensa::data::GetNotesResponse {
            notes: notes.iter().map(grpc::core_note_to_proto).collect(),
            total,
        }))
    }

    async fn get_note(
        &self,
        request: Request<tensa::data::GetNoteRequest>,
    ) -> std::result::Result<Response<tensa::data::Note>, Status> {
        let req = request.into_inner();
        let note_id = req
            .note_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("bad note_id: {e}")))?;
        match self
            .notes
            .get_note(note_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            Some(note) => Ok(Response::new(grpc::core_note_to_proto(&note))),
            None => Err(Status::not_found(format!("note {note_id} not found"))),
        }
    }

    async fn get_contacts(
        &self,
        request: Request<tensa::data::GetContactsRequest>,
    ) -> std::result::Result<Response<tensa::data::GetContactsResponse>, Status> {
        let req = request.into_inner();
        let contacts = self
            .contacts
            .list_contacts(req.limit as usize, req.offset as usize)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        // Общий count (Контракт 3, одобрено 2026-10-15), не размер страницы.
        let total = self
            .contacts
            .count_contacts()
            .await
            .map_err(|e| Status::internal(e.to_string()))? as u32;
        Ok(Response::new(tensa::data::GetContactsResponse {
            contacts: contacts.iter().map(grpc::core_contact_to_proto).collect(),
            total,
        }))
    }

    async fn get_contact(
        &self,
        request: Request<tensa::data::GetContactRequest>,
    ) -> std::result::Result<Response<tensa::data::Contact>, Status> {
        let req = request.into_inner();
        let contact_id = req
            .contact_id
            .parse()
            .map_err(|e| Status::invalid_argument(format!("bad contact_id: {e}")))?;
        match self
            .contacts
            .get_contact(contact_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            Some(contact) => Ok(Response::new(grpc::core_contact_to_proto(&contact))),
            None => Err(Status::not_found(format!("contact {contact_id} not found"))),
        }
    }

    type SearchFilesStream = Stream<tensa::data::SearchResult>;

    async fn search_files(
        &self,
        request: Request<tensa::data::SearchFilesRequest>,
    ) -> std::result::Result<Response<Self::SearchFilesStream>, Status> {
        let req = request.into_inner();
        let memory = self
            .memory
            .as_ref()
            .ok_or_else(|| Status::unimplemented("memory store is not attached"))?;
        let results = memory
            .search(&req.query, req.limit as usize)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let items: Vec<std::result::Result<tensa::data::SearchResult, Status>> = results
            .iter()
            .map(|r| Ok(grpc::memory_search_to_proto(r)))
            .collect();
        Ok(Response::new(Box::pin(futures::stream::iter(items))))
    }

    async fn get_approval_queue(
        &self,
        _request: Request<tensa::data::GetApprovalQueueRequest>,
    ) -> std::result::Result<Response<tensa::data::GetApprovalQueueResponse>, Status> {
        let actions = self
            .approval
            .get_pending()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(tensa::data::GetApprovalQueueResponse {
            actions: actions.iter().map(grpc::action_to_proto_approval).collect(),
        }))
    }
}

// === VoiceService / QueryService (до tensa-llm) ===

#[derive(Default)]
pub struct VoiceServiceImpl;

#[tonic::async_trait]
impl tensa::voice::voice_service_server::VoiceService for VoiceServiceImpl {
    async fn transcribe(
        &self,
        _request: Request<tonic::Streaming<tensa::voice::AudioChunk>>,
    ) -> std::result::Result<Response<tensa::voice::TranscribeResponse>, Status> {
        Err(Status::unimplemented(
            "tensa-llm (ASR/Whisper) is not implemented yet",
        ))
    }

    async fn transcribe_file(
        &self,
        _request: Request<tensa::voice::TranscribeFileRequest>,
    ) -> std::result::Result<Response<tensa::voice::TranscribeResponse>, Status> {
        Err(Status::unimplemented(
            "tensa-llm (ASR/Whisper) is not implemented yet",
        ))
    }

    type DetectVoiceActivityStream = Stream<tensa::voice::VadResult>;

    async fn detect_voice_activity(
        &self,
        _request: Request<tonic::Streaming<tensa::voice::AudioChunk>>,
    ) -> std::result::Result<Response<Self::DetectVoiceActivityStream>, Status> {
        Err(Status::unimplemented(
            "tensa-llm (VAD) is not implemented yet",
        ))
    }
}

#[derive(Default)]
pub struct QueryServiceImpl;

#[tonic::async_trait]
impl tensa::query::query_service_server::QueryService for QueryServiceImpl {
    async fn query(
        &self,
        _request: Request<tensa::query::QueryRequest>,
    ) -> std::result::Result<Response<tensa::query::QueryResponse>, Status> {
        Err(Status::unimplemented(
            "tensa-llm (brain router) is not implemented yet",
        ))
    }

    type StreamQueryStream = Stream<tensa::query::QueryChunk>;

    async fn stream_query(
        &self,
        _request: Request<tensa::query::QueryRequest>,
    ) -> std::result::Result<Response<Self::StreamQueryStream>, Status> {
        Err(Status::unimplemented(
            "tensa-llm (brain router) is not implemented yet",
        ))
    }

    async fn voice_query(
        &self,
        _request: Request<tonic::Streaming<tensa::voice::AudioChunk>>,
    ) -> std::result::Result<Response<tensa::query::QueryResponse>, Status> {
        Err(Status::unimplemented(
            "tensa-llm (brain router) is not implemented yet",
        ))
    }
}

// === Сборка сервера ===

/// Зависимости сервера (всё — трейты, подставляются интегратором).
pub struct ServerDeps {
    pub approval: Arc<dyn ApprovalSync>,
    pub notes_sync: Arc<dyn NotesSyncSource>,
    pub contacts_sync: Arc<dyn ContactsSyncSource>,
    pub registry: Arc<dyn tensa_nodes::NodeRegistry>,
    pub notes: Arc<dyn tensa_core::NotesStore>,
    pub contacts: Arc<dyn tensa_core::ContactsStore>,
    pub approval_store: Arc<dyn tensa_approval::ApprovalStore>,
    pub memory: Option<Arc<dyn tensa_memory::MemoryStore>>,
}

impl ServerDeps {
    /// Минимальный набор: sync-адаптеры заметок/контактов строятся из Stores
    /// (одобрено 2026-10-15), без памяти.
    pub fn new(
        approval: Arc<dyn ApprovalSync>,
        registry: Arc<dyn tensa_nodes::NodeRegistry>,
        notes: Arc<dyn tensa_core::NotesStore>,
        contacts: Arc<dyn tensa_core::ContactsStore>,
        approval_store: Arc<dyn tensa_approval::ApprovalStore>,
    ) -> Self {
        let notes_sync: Arc<dyn NotesSyncSource> = Arc::new(StoreNotesSync::new(notes.clone()));
        let contacts_sync: Arc<dyn ContactsSyncSource> =
            Arc::new(StoreContactsSync::new(contacts.clone()));
        Self {
            approval,
            notes_sync,
            contacts_sync,
            registry,
            notes,
            contacts,
            approval_store,
            memory: None,
        }
    }
}

/// Запустить gRPC-сервер на готовом listener (TCP, h2c).
pub async fn serve(listener: tokio::net::TcpListener, deps: ServerDeps) -> Result<()> {
    let bus = Arc::new(EventBus::new(1024));
    let (hb_tx, _) = broadcast::channel(1024);

    let sync_svc = tensa::sync::sync_service_server::SyncServiceServer::new(SyncServiceImpl {
        approval: deps.approval,
        notes: deps.notes_sync,
        contacts: deps.contacts_sync,
        bus,
    });
    let node_svc = tensa::node::node_service_server::NodeServiceServer::new(NodeServiceImpl {
        registry: deps.registry,
        heartbeats: hb_tx,
    });
    let data_svc = tensa::data::data_service_server::DataServiceServer::new(DataServiceImpl {
        notes: deps.notes,
        contacts: deps.contacts,
        approval: deps.approval_store,
        memory: deps.memory,
    });

    let incoming = tonic::transport::server::TcpIncoming::from_listener(listener, true, None)
        .map_err(|e| SyncError::Network(e.to_string()))?;

    info!("tensa-sync gRPC server listening");
    tonic::transport::Server::builder()
        .add_service(sync_svc)
        .add_service(node_svc)
        .add_service(data_svc)
        .add_service(tensa::voice::voice_service_server::VoiceServiceServer::new(
            VoiceServiceImpl,
        ))
        .add_service(tensa::query::query_service_server::QueryServiceServer::new(
            QueryServiceImpl,
        ))
        .serve_with_incoming(incoming)
        .await
        .map_err(|e| SyncError::Network(e.to_string()))
}
