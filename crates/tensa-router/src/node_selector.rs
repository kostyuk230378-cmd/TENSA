//! Выбор узла для инференса (ТЗ tensa-router: «локальный heavy → локальная,
//! удалённый heavy → gRPC, только облако → облако»).

use tensa_nodes::{Node, NodeRegistry};
use uuid::Uuid;

use crate::error::Result;

/// Цель маршрутизации.
#[derive(Debug, Clone)]
pub enum RouteTarget {
    /// Локальная модель этого узла.
    Local,
    /// Удалённый heavy-узел (gRPC).
    Remote(Box<Node>),
    /// Облачная модель.
    Cloud,
    /// Degraded-режим (только поиск), с причиной.
    Degraded(String),
}

/// Выбрать цель: облако (если предпочтено) → локально (если доступно и
/// heavy) → удалённый heavy → облако → degraded.
pub async fn select_target(
    registry: &dyn NodeRegistry,
    local_node_id: Option<Uuid>,
    local_available: bool,
    cloud_available: bool,
    prefer_cloud: bool,
) -> Result<RouteTarget> {
    if prefer_cloud && cloud_available {
        return Ok(RouteTarget::Cloud);
    }

    if local_available {
        return Ok(RouteTarget::Local);
    }

    if let Some(node) = registry.find_heavy().await? {
        if Some(node.node_id) != local_node_id {
            return Ok(RouteTarget::Remote(Box::new(node)));
        }
    }

    if cloud_available {
        return Ok(RouteTarget::Cloud);
    }

    Ok(RouteTarget::Degraded(
        "нет локальной модели, heavy-узлов и облака".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tensa_core::{DeviceType, NodeRole};
    use tensa_nodes::{NodeResources, Platform, SqliteNodeRegistry, ThermalStatus};

    fn node(role: NodeRole, online: bool, ram_mb: u64) -> Node {
        Node {
            node_id: Uuid::new_v4(),
            name: format!("node-{role:?}"),
            device_type: DeviceType::Desktop,
            platform: Platform::Windows,
            role,
            tailscale_ip: "100.64.0.10".to_string(),
            grpc_port: 50051,
            last_seen: 0,
            online,
            ram_total_mb: ram_mb,
            ram_available_mb: ram_mb / 2,
            cpu_cores: 8,
            cpu_usage_percent: 10.0,
            has_gpu: false,
            has_npu: false,
            battery_percent: None,
            battery_charging: None,
            thermal_status: ThermalStatus::Nominal,
            resources: NodeResources::default(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[tokio::test]
    async fn prefer_cloud_wins_when_available() {
        let registry = SqliteNodeRegistry::new_in_memory().await.expect("реестр");
        let target = select_target(&registry, None, true, true, true)
            .await
            .expect("выбор");
        assert!(matches!(target, RouteTarget::Cloud));
    }

    #[tokio::test]
    async fn local_wins_when_available() {
        let registry = SqliteNodeRegistry::new_in_memory().await.expect("реестр");
        let target = select_target(&registry, None, true, true, false)
            .await
            .expect("выбор");
        assert!(matches!(target, RouteTarget::Local));
    }

    #[tokio::test]
    async fn remote_heavy_is_selected_when_local_unavailable() {
        let registry = SqliteNodeRegistry::new_in_memory().await.expect("реестр");
        let heavy = node(NodeRole::Heavy, true, 32768);
        registry.register(heavy.clone()).await.expect("регистрация");

        let target = select_target(&registry, None, false, true, false)
            .await
            .expect("выбор");
        match target {
            RouteTarget::Remote(node) => assert_eq!(node.node_id, heavy.node_id),
            other => panic!("ожидался Remote, получено {other:?}"),
        }
    }

    #[tokio::test]
    async fn own_node_is_not_selected_remotely() {
        let registry = SqliteNodeRegistry::new_in_memory().await.expect("реестр");
        let own = node(NodeRole::Heavy, true, 32768);
        registry.register(own.clone()).await.expect("регистрация");

        let target = select_target(&registry, Some(own.node_id), false, true, false)
            .await
            .expect("выбор");
        assert!(matches!(target, RouteTarget::Cloud));
    }

    #[tokio::test]
    async fn degraded_when_nothing_available() {
        let registry = SqliteNodeRegistry::new_in_memory().await.expect("реестр");
        let target = select_target(&registry, None, false, false, false)
            .await
            .expect("выбор");
        assert!(matches!(target, RouteTarget::Degraded(_)));
    }
}
