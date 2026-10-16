//! gRPC-клиент: GrpcSyncService (ТЗ tensa-sync: «Реализация: GrpcSyncService»).

use crate::error::{Result, SyncError};
use crate::grpc::{self, proto::tensa};
use crate::service::{ContactEvent, NoteEvent, SyncService};
use crate::status::SyncStatus;
use crate::sync_log::SyncLedger;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tensa_approval::ActionEvent;
use tensa_nodes::{Node, NodeProfiler};
use tonic::transport::{Channel, Endpoint};

fn net(e: tonic::Status) -> SyncError {
    SyncError::Network(e.message().to_string())
}

/// Синхронизация узла через gRPC (клиент к удалённому серверу Tensa).
pub struct GrpcSyncService {
    endpoint: Endpoint,
    node_id: String,
    profiler: Arc<dyn NodeProfiler>,
    ledger: Mutex<SyncLedger>,
}

impl GrpcSyncService {
    /// Создать клиент. `node_id` — идентификатор локального узла (UUID-строка).
    pub fn new(
        endpoint: Endpoint,
        node_id: impl Into<String>,
        profiler: Arc<dyn NodeProfiler>,
    ) -> Self {
        Self {
            endpoint,
            node_id: node_id.into(),
            profiler,
            ledger: Mutex::new(SyncLedger::default()),
        }
    }

    async fn sync_client(
        &self,
    ) -> Result<tensa::sync::sync_service_client::SyncServiceClient<Channel>> {
        tensa::sync::sync_service_client::SyncServiceClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| SyncError::Network(e.to_string()))
    }

    async fn node_client(
        &self,
    ) -> Result<tensa::node::node_service_client::NodeServiceClient<Channel>> {
        tensa::node::node_service_client::NodeServiceClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| SyncError::Network(e.to_string()))
    }

    fn check(resp: tensa::common::Result) -> Result<String> {
        if resp.success {
            Ok(resp.operation_id)
        } else {
            let message = resp
                .error
                .map(|e| format!("{}: {}", e.code, e.message))
                .unwrap_or_else(|| "unknown remote error".into());
            Err(SyncError::Network(message))
        }
    }
}

#[async_trait]
impl SyncService for GrpcSyncService {
    async fn start(&self) -> Result<()> {
        self.ledger.lock().expect("ledger").mark_started();
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.ledger.lock().expect("ledger").mark_stopped();
        Ok(())
    }

    async fn status(&self) -> Result<SyncStatus> {
        // Best-effort: peers узнаём у сервера; при недоступности — нули. [derived]
        let (running, last_sync_at) = {
            let ledger = self.ledger.lock().expect("ledger");
            (ledger.is_running(), ledger.snapshot(0, 0).last_sync_at)
        };
        let (peers_online, peers_total) = match self.node_client().await {
            Ok(mut client) => {
                let online = client
                    .list_nodes(tensa::node::ListNodesRequest { online_only: true })
                    .await
                    .map(|r| r.into_inner().nodes.len())
                    .unwrap_or(0);
                let total = client
                    .list_nodes(tensa::node::ListNodesRequest { online_only: false })
                    .await
                    .map(|r| r.into_inner().nodes.len())
                    .unwrap_or(0);
                (online, total)
            }
            Err(_) => (0, 0),
        };
        let mut s = self
            .ledger
            .lock()
            .expect("ledger")
            .snapshot(peers_online, peers_total);
        s.running = running;
        s.last_sync_at = last_sync_at;
        Ok(s)
    }

    async fn sync_approval_events(&self) -> Result<Vec<ActionEvent>> {
        let mut client = self.sync_client().await?;
        let resp = client
            .sync_approval_events(tensa::sync::SyncApprovalRequest {
                node_id: self.node_id.clone(),
                last_event_id: 0,
                vector_clock: None,
            })
            .await
            .map_err(net)?;
        let mut stream = resp.into_inner();
        let mut out = Vec::new();
        while let Some(item) = stream.message().await.map_err(net)? {
            out.push(grpc::approval_event_from_proto(&item)?);
        }
        Ok(out)
    }

    async fn push_approval_event(&self, event: ActionEvent) -> Result<()> {
        let mut client = self.sync_client().await?;
        let resp = client
            .push_approval_event(grpc::approval_event_to_proto(&event))
            .await
            .map_err(net)?
            .into_inner();
        Self::check(resp)?;
        Ok(())
    }

    async fn sync_notes(&self) -> Result<Vec<NoteEvent>> {
        let mut client = self.sync_client().await?;
        let resp = client
            .sync_notes(tensa::sync::SyncNotesRequest {
                node_id: self.node_id.clone(),
                last_updated_at: 0,
                vector_clock: None,
            })
            .await
            .map_err(net)?;
        let mut stream = resp.into_inner();
        let mut out = Vec::new();
        while let Some(item) = stream.message().await.map_err(net)? {
            out.push(grpc::note_event_from_proto(&item)?);
        }
        Ok(out)
    }

    async fn push_note_event(&self, event: NoteEvent) -> Result<()> {
        let mut client = self.sync_client().await?;
        let resp = client
            .push_note_event(grpc::note_event_to_proto(&event))
            .await
            .map_err(net)?
            .into_inner();
        Self::check(resp)?;
        Ok(())
    }

    async fn sync_contacts(&self) -> Result<Vec<ContactEvent>> {
        let mut client = self.sync_client().await?;
        let resp = client
            .sync_contacts(tensa::sync::SyncContactsRequest {
                node_id: self.node_id.clone(),
                last_updated_at: 0,
                vector_clock: None,
            })
            .await
            .map_err(net)?;
        let mut stream = resp.into_inner();
        let mut out = Vec::new();
        while let Some(item) = stream.message().await.map_err(net)? {
            out.push(grpc::contact_event_from_proto(&item)?);
        }
        Ok(out)
    }

    async fn push_contact_event(&self, event: ContactEvent) -> Result<()> {
        let mut client = self.sync_client().await?;
        let resp = client
            .push_contact_event(grpc::contact_event_to_proto(&event))
            .await
            .map_err(net)?
            .into_inner();
        Self::check(resp)?;
        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<()> {
        let profile = self.profiler.profile();
        let node_id: uuid::Uuid = self
            .node_id
            .parse()
            .map_err(|e| SyncError::Config(format!("bad node_id: {e}")))?;
        let hb = tensa_nodes::Heartbeat {
            node_id,
            ram_available_mb: profile.ram_available_mb,
            cpu_usage_percent: profile.cpu_usage_percent,
            battery_percent: profile.battery_percent,
            battery_charging: profile.battery_charging,
            thermal_status: profile.thermal_status,
            timestamp: chrono::Utc::now().timestamp(),
        };
        let msg = grpc::heartbeat_to_proto(&node_id, &hb);
        let mut client = self.node_client().await?;
        let resp = client.heartbeat(msg).await.map_err(net)?.into_inner();
        Self::check(resp)?;
        Ok(())
    }

    async fn list_online_nodes(&self) -> Result<Vec<Node>> {
        let mut client = self.node_client().await?;
        let resp = client
            .list_nodes(tensa::node::ListNodesRequest { online_only: true })
            .await
            .map_err(net)?
            .into_inner();
        resp.nodes
            .iter()
            .map(grpc::node_from_proto)
            .collect::<Result<Vec<_>>>()
    }

    async fn acquire_connection(&self, resource: &str, account_id: Option<&str>) -> Result<()> {
        let mut client = self.node_client().await?;
        let resp = client
            .acquire_connection(tensa::node::AcquireConnectionRequest {
                node_id: self.node_id.clone(),
                resource: resource.to_string(),
                account_id: account_id.map(String::from),
            })
            .await
            .map_err(net)?
            .into_inner();
        Self::check(resp)?;
        Ok(())
    }

    async fn release_connection(&self, resource: &str, account_id: Option<&str>) -> Result<()> {
        let mut client = self.node_client().await?;
        let resp = client
            .release_connection(tensa::node::ReleaseConnectionRequest {
                node_id: self.node_id.clone(),
                resource: resource.to_string(),
                account_id: account_id.map(String::from),
            })
            .await
            .map_err(net)?
            .into_inner();
        Self::check(resp)?;
        Ok(())
    }
}
