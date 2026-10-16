//! `NodeRegistry` (Контракт 9): реестр узлов поверх tensa-nodes.

use tensa_nodes::{Node as CoreNode, NodeRegistry as CoreNodeRegistry, SqliteNodeRegistry};

use crate::convert::{data_root, name_of, parse_uuid, uuid_str};
use crate::error::TensaError;
use crate::types::Node;

/// FFI-обёртка реестра узлов (`interface NodeRegistry` в UDL).
pub struct NodeRegistry {
    inner: SqliteNodeRegistry,
}

impl NodeRegistry {
    /// Открыть реестр (база — `<root>/nodes.db`).
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        let registry = SqliteNodeRegistry::new(&data_root(&config_path).join("nodes.db")).await?;
        Ok(Self { inner: registry })
    }

    /// Узлы реестра (`online_only` — только онлайн).
    pub async fn list_nodes(&self, online_only: bool) -> Result<Vec<Node>, TensaError> {
        let nodes = if online_only {
            self.inner.list_online().await?
        } else {
            self.inner.list_all().await?
        };
        Ok(nodes.into_iter().map(to_node).collect())
    }

    /// Узел по id.
    pub async fn get_node(&self, node_id: String) -> Result<Option<Node>, TensaError> {
        let id = parse_uuid("nodes.get", &node_id)?;
        Ok(self.inner.get(id).await?.map(to_node))
    }

    /// Онлайн heavy-узел для инференса.
    pub async fn find_heavy(&self) -> Result<Option<Node>, TensaError> {
        Ok(self.inner.find_heavy().await?.map(to_node))
    }

    /// Занять соединение к узлу (резервирование ресурса).
    pub async fn acquire_connection(
        &self,
        node_id: String,
        resource: String,
        account_id: Option<String>,
    ) -> Result<(), TensaError> {
        let id = parse_uuid("nodes.acquire", &node_id)?;
        self.inner
            .acquire_connection(id, &resource, account_id.as_deref())
            .await?;
        Ok(())
    }

    /// Освободить соединение к узлу.
    pub async fn release_connection(
        &self,
        node_id: String,
        resource: String,
        account_id: Option<String>,
    ) -> Result<(), TensaError> {
        let id = parse_uuid("nodes.release", &node_id)?;
        self.inner
            .release_connection(id, &resource, account_id.as_deref())
            .await?;
        Ok(())
    }

    /// Активное соединение для ресурса.
    pub async fn get_active_connection(
        &self,
        resource: String,
        account_id: Option<String>,
    ) -> Result<Option<String>, TensaError> {
        let active = self
            .inner
            .get_active_connection(&resource, account_id.as_deref())
            .await?;
        Ok(active.map(uuid_str))
    }
}

/// Конвертация узла ядра в UDL-представление.
fn to_node(node: CoreNode) -> Node {
    Node {
        node_id: uuid_str(node.node_id),
        name: node.name,
        device_type: name_of(&node.device_type),
        platform: name_of(&node.platform),
        role: name_of(&node.role),
        tailscale_ip: node.tailscale_ip,
        grpc_port: node.grpc_port,
        last_seen: node.last_seen,
        online: node.online,
        ram_total_mb: node.ram_total_mb,
        ram_available_mb: node.ram_available_mb,
        cpu_cores: node.cpu_cores,
        cpu_usage_percent: node.cpu_usage_percent,
        has_gpu: node.has_gpu,
        has_npu: node.has_npu,
        battery_percent: node.battery_percent,
        battery_charging: node.battery_charging,
        thermal_status: name_of(&node.thermal_status),
    }
}
