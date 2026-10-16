//! SQLite-реализация NodeRegistry (Контракт 7: ~/.tensa/nodes.db).

use crate::error::{NodesError, Result};
use crate::models::{
    DeviceType, Heartbeat, Node, NodeRole, Platform, ThermalStatus, LEASE_TTL_SECONDS,
};
use crate::registry::NodeRegistry;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../schema.sql");

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_device_type(s: &str) -> Result<DeviceType> {
    Ok(match s {
        "desktop" => DeviceType::Desktop,
        "android" => DeviceType::Android,
        other => return Err(NodesError::Config(format!("bad device_type: {other}"))),
    })
}

fn parse_platform(s: &str) -> Result<Platform> {
    Ok(match s {
        "windows" => Platform::Windows,
        "linux" => Platform::Linux,
        "android" => Platform::Android,
        other => return Err(NodesError::Config(format!("bad platform: {other}"))),
    })
}

fn parse_role(s: &str) -> Result<NodeRole> {
    Ok(match s {
        "heavy" => NodeRole::Heavy,
        "thin" => NodeRole::Thin,
        "auto" => NodeRole::Auto,
        other => return Err(NodesError::Config(format!("bad role: {other}"))),
    })
}

fn parse_thermal(s: &str) -> Result<ThermalStatus> {
    Ok(match s {
        "nominal" => ThermalStatus::Nominal,
        "fair" => ThermalStatus::Fair,
        "serious" => ThermalStatus::Serious,
        "critical" => ThermalStatus::Critical,
        other => return Err(NodesError::Config(format!("bad thermal_status: {other}"))),
    })
}

fn role_as_str(role: &NodeRole) -> &'static str {
    match role {
        NodeRole::Heavy => "heavy",
        NodeRole::Thin => "thin",
        NodeRole::Auto => "auto",
    }
}

fn device_as_str(d: &DeviceType) -> &'static str {
    match d {
        DeviceType::Desktop => "desktop",
        DeviceType::Android => "android",
    }
}

fn platform_as_str(p: &Platform) -> &'static str {
    match p {
        Platform::Windows => "windows",
        Platform::Linux => "linux",
        Platform::Android => "android",
    }
}

fn thermal_as_str(t: &ThermalStatus) -> &'static str {
    match t {
        ThermalStatus::Nominal => "nominal",
        ThermalStatus::Fair => "fair",
        ThermalStatus::Serious => "serious",
        ThermalStatus::Critical => "critical",
    }
}

fn node_from_row(row: &SqliteRow) -> Result<Node> {
    Ok(Node {
        node_id: Uuid::from_str(&row.try_get::<String, _>("node_id")?)?,
        name: row.try_get("name")?,
        device_type: parse_device_type(&row.try_get::<String, _>("device_type")?)?,
        platform: parse_platform(&row.try_get::<String, _>("platform")?)?,
        role: parse_role(&row.try_get::<String, _>("role")?)?,
        tailscale_ip: row.try_get("tailscale_ip")?,
        grpc_port: row.try_get::<i64, _>("grpc_port")? as u16,
        last_seen: row.try_get("last_seen")?,
        online: row.try_get::<i64, _>("online")? != 0,
        ram_total_mb: row.try_get::<i64, _>("ram_total_mb")? as u64,
        ram_available_mb: row.try_get::<i64, _>("ram_available_mb")? as u64,
        cpu_cores: row.try_get::<i64, _>("cpu_cores")? as u32,
        cpu_usage_percent: row.try_get("cpu_usage_percent")?,
        has_gpu: row.try_get::<i64, _>("has_gpu")? != 0,
        has_npu: row.try_get::<i64, _>("has_npu")? != 0,
        battery_percent: row
            .try_get::<Option<i64>, _>("battery_percent")?
            .map(|v| v as u8),
        battery_charging: row
            .try_get::<Option<i64>, _>("battery_charging")?
            .map(|v| v != 0),
        thermal_status: parse_thermal(&row.try_get::<String, _>("thermal_status")?)?,
        resources: serde_json::from_str(&row.try_get::<String, _>("resources")?)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// SqliteNodeRegistry — хранилище реестра узлов.
pub struct SqliteNodeRegistry {
    pool: SqlitePool,
}

impl SqliteNodeRegistry {
    pub async fn new(db_path: &Path) -> Result<Self> {
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        let registry = Self { pool };
        registry.run_schema().await?;
        info!("NodeRegistry initialized");
        Ok(registry)
    }

    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let registry = Self { pool };
        registry.run_schema().await?;
        Ok(registry)
    }

    async fn run_schema(&self) -> Result<()> {
        crate::run_schema_statements(&self.pool, SCHEMA_SQL).await
    }
}

#[async_trait]
impl NodeRegistry for SqliteNodeRegistry {
    async fn register(&self, node: Node) -> Result<()> {
        // Upsert: повторная регистрация обновляет данные узла.
        let exists = sqlx::query("SELECT 1 FROM nodes WHERE node_id = ?")
            .bind(node.node_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        let now = now_ts();
        if exists.is_some() {
            sqlx::query(
                "UPDATE nodes SET name = ?, device_type = ?, platform = ?, role = ?, tailscale_ip = ?, \
                 grpc_port = ?, last_seen = ?, online = 1, ram_total_mb = ?, ram_available_mb = ?, \
                 cpu_cores = ?, cpu_usage_percent = ?, has_gpu = ?, has_npu = ?, battery_percent = ?, \
                 battery_charging = ?, thermal_status = ?, resources = ?, updated_at = ? \
                 WHERE node_id = ?",
            )
            .bind(&node.name)
            .bind(device_as_str(&node.device_type))
            .bind(platform_as_str(&node.platform))
            .bind(role_as_str(&node.role))
            .bind(&node.tailscale_ip)
            .bind(node.grpc_port as i64)
            .bind(now)
            .bind(node.ram_total_mb as i64)
            .bind(node.ram_available_mb as i64)
            .bind(node.cpu_cores as i64)
            .bind(node.cpu_usage_percent)
            .bind(node.has_gpu as i64)
            .bind(node.has_npu as i64)
            .bind(node.battery_percent.map(|v| v as i64))
            .bind(node.battery_charging.map(|v| v as i64))
            .bind(thermal_as_str(&node.thermal_status))
            .bind(serde_json::to_string(&node.resources)?)
            .bind(now)
            .bind(node.node_id.to_string())
            .execute(&self.pool)
            .await?;
            info!(node_id = %node.node_id, "node re-registered");
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO nodes (node_id, name, device_type, platform, role, tailscale_ip, grpc_port, \
             last_seen, online, ram_total_mb, ram_available_mb, cpu_cores, cpu_usage_percent, \
             has_gpu, has_npu, battery_percent, battery_charging, thermal_status, resources, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(node.node_id.to_string())
        .bind(&node.name)
        .bind(device_as_str(&node.device_type))
        .bind(platform_as_str(&node.platform))
        .bind(role_as_str(&node.role))
        .bind(&node.tailscale_ip)
        .bind(node.grpc_port as i64)
        .bind(now)
        .bind(node.ram_total_mb as i64)
        .bind(node.ram_available_mb as i64)
        .bind(node.cpu_cores as i64)
        .bind(node.cpu_usage_percent)
        .bind(node.has_gpu as i64)
        .bind(node.has_npu as i64)
        .bind(node.battery_percent.map(|v| v as i64))
        .bind(node.battery_charging.map(|v| v as i64))
        .bind(thermal_as_str(&node.thermal_status))
        .bind(serde_json::to_string(&node.resources)?)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        info!(node_id = %node.node_id, "node registered");
        Ok(())
    }

    async fn unregister(&self, node_id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM nodes WHERE node_id = ?")
            .bind(node_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(NodesError::NodeNotFound(node_id.to_string()));
        }
        // Соединения узла освобождаются.
        sqlx::query("UPDATE node_connections SET active = 0 WHERE node_id = ? AND active = 1")
            .bind(node_id.to_string())
            .execute(&self.pool)
            .await?;
        info!(node_id = %node_id, "node unregistered");
        Ok(())
    }

    async fn update_heartbeat(&self, heartbeat: Heartbeat) -> Result<()> {
        let node_id = heartbeat.node_id.to_string();
        let exists = sqlx::query("SELECT 1 FROM nodes WHERE node_id = ?")
            .bind(&node_id)
            .fetch_optional(&self.pool)
            .await?
            .is_some();
        if !exists {
            return Err(NodesError::NodeNotFound(node_id));
        }
        let now = now_ts();
        sqlx::query(
            "INSERT INTO node_heartbeats (node_id, ram_available_mb, cpu_usage_percent, battery_percent, battery_charging, thermal_status, timestamp) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&node_id)
        .bind(heartbeat.ram_available_mb as i64)
        .bind(heartbeat.cpu_usage_percent)
        .bind(heartbeat.battery_percent.map(|v| v as i64))
        .bind(heartbeat.battery_charging.map(|v| v as i64))
        .bind(thermal_as_str(&heartbeat.thermal_status))
        .bind(heartbeat.timestamp)
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "UPDATE nodes SET last_seen = ?, online = 1, ram_available_mb = ?, cpu_usage_percent = ?, \
             battery_percent = ?, battery_charging = ?, thermal_status = ?, updated_at = ? WHERE node_id = ?",
        )
        .bind(heartbeat.timestamp)
        .bind(heartbeat.ram_available_mb as i64)
        .bind(heartbeat.cpu_usage_percent)
        .bind(heartbeat.battery_percent.map(|v| v as i64))
        .bind(heartbeat.battery_charging.map(|v| v as i64))
        .bind(thermal_as_str(&heartbeat.thermal_status))
        .bind(now)
        .bind(&node_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, node_id: Uuid) -> Result<Option<Node>> {
        let row = sqlx::query("SELECT * FROM nodes WHERE node_id = ?")
            .bind(node_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(node_from_row).transpose()
    }

    async fn list_online(&self) -> Result<Vec<Node>> {
        let rows = sqlx::query("SELECT * FROM nodes WHERE online = 1 ORDER BY last_seen DESC")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(node_from_row).collect()
    }

    async fn list_all(&self) -> Result<Vec<Node>> {
        let rows = sqlx::query("SELECT * FROM nodes ORDER BY created_at ASC")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(node_from_row).collect()
    }

    async fn find_heavy(&self) -> Result<Option<Node>> {
        // Онлайн heavy-узел с максимальной доступной RAM; auto с GPU тоже кандидат. [derived]
        let row = sqlx::query(
            "SELECT * FROM nodes WHERE online = 1 AND (role = 'heavy' OR (role = 'auto' AND has_gpu = 1)) \
             ORDER BY ram_available_mb DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(node_from_row).transpose()
    }

    async fn find_with_resource(&self, resource: &str) -> Result<Vec<Node>> {
        let rows = sqlx::query("SELECT * FROM nodes WHERE online = 1 ORDER BY last_seen DESC")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for row in rows {
            let node = node_from_row(&row)?;
            if node.resources.has(resource) {
                out.push(node);
            }
        }
        Ok(out)
    }

    async fn acquire_connection(
        &self,
        node_id: Uuid,
        resource: &str,
        account_id: Option<&str>,
    ) -> Result<()> {
        let now = now_ts();
        // Активная аренда на этот ресурс/аккаунт?
        let active = sqlx::query(
            "SELECT node_id FROM node_connections WHERE resource = ? AND account_id IS ? AND active = 1 AND expires_at > ? LIMIT 1",
        )
        .bind(resource)
        .bind(account_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = active {
            let holder: String = row.try_get("node_id")?;
            if holder != node_id.to_string() {
                return Err(NodesError::ResourceBusy {
                    resource: resource.to_string(),
                    holder,
                });
            }
            // Повторный захват тем же узлом — продлеваем аренду.
            sqlx::query(
                "UPDATE node_connections SET expires_at = ? WHERE node_id = ? AND resource = ? AND account_id IS ? AND active = 1",
            )
            .bind(now + LEASE_TTL_SECONDS)
            .bind(node_id.to_string())
            .bind(resource)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }
        // Узел существует и онлайн?
        let node = sqlx::query("SELECT online FROM nodes WHERE node_id = ?")
            .bind(node_id.to_string())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| NodesError::NodeNotFound(node_id.to_string()))?;
        if node.try_get::<i64, _>("online")? == 0 {
            return Err(NodesError::NodeOffline(node_id.to_string()));
        }
        sqlx::query(
            "INSERT INTO node_connections (node_id, resource, account_id, active, acquired_at, expires_at) \
             VALUES (?, ?, ?, 1, ?, ?)",
        )
        .bind(node_id.to_string())
        .bind(resource)
        .bind(account_id)
        .bind(now)
        .bind(now + LEASE_TTL_SECONDS)
        .execute(&self.pool)
        .await?;
        info!(node_id = %node_id, resource, "connection acquired");
        Ok(())
    }

    async fn release_connection(
        &self,
        node_id: Uuid,
        resource: &str,
        account_id: Option<&str>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE node_connections SET active = 0 WHERE node_id = ? AND resource = ? AND account_id IS ? AND active = 1",
        )
        .bind(node_id.to_string())
        .bind(resource)
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(NodesError::Config(format!(
                "no active connection: node {node_id}, resource {resource}"
            )));
        }
        info!(node_id = %node_id, resource, "connection released");
        Ok(())
    }

    async fn get_active_connection(
        &self,
        resource: &str,
        account_id: Option<&str>,
    ) -> Result<Option<Uuid>> {
        let now = now_ts();
        let row = sqlx::query(
            "SELECT node_id FROM node_connections WHERE resource = ? AND account_id IS ? AND active = 1 AND expires_at > ? LIMIT 1",
        )
        .bind(resource)
        .bind(account_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(r) => Ok(Some(Uuid::from_str(&r.try_get::<String, _>("node_id")?)?)),
            None => Ok(None),
        }
    }

    async fn cleanup_stale(&self, timeout_seconds: u64) -> Result<usize> {
        let cutoff = now_ts() - timeout_seconds as i64;
        // Кто молчит — офлайн.
        let result = sqlx::query(
            "UPDATE nodes SET online = 0, updated_at = ? WHERE online = 1 AND last_seen < ?",
        )
        .bind(now_ts())
        .bind(cutoff)
        .execute(&self.pool)
        .await?;
        // Соединения офлайн-узлов освобождаем (ТЗ: освобождение при офлайн).
        sqlx::query(
            "UPDATE node_connections SET active = 0 WHERE active = 1 AND node_id IN (SELECT node_id FROM nodes WHERE online = 0)",
        )
        .execute(&self.pool)
        .await?;
        // Просроченные аренды тоже гасим.
        sqlx::query("UPDATE node_connections SET active = 0 WHERE active = 1 AND expires_at < ?")
            .bind(now_ts())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() as usize)
    }
}
