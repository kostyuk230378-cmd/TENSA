//! gRPC-слой: сгенерированный proto-код (Контракт 8) и конверсии
//! между proto-сообщениями и доменными типами крейтов Tensa.

use crate::error::{Result, SyncError};
use crate::service::{ContactEvent, NoteEvent};

/// Сгенерированные типы (tonic/prost). Иерархия модулей повторяет
/// пакеты proto: tensa.common, tensa.sync, tensa.node, tensa.data,
/// tensa.voice, tensa.query.
///
/// `result_large_err` подавлен осознанно: tonic генерирует
/// `Result<_, tonic::Status>`, и Status большой по дизайну фреймворка.
#[allow(clippy::result_large_err)]
pub mod proto {
    pub mod tensa {
        pub mod common {
            tonic::include_proto!("tensa.common");
        }
        pub mod sync {
            tonic::include_proto!("tensa.sync");
        }
        pub mod node {
            tonic::include_proto!("tensa.node");
        }
        pub mod data {
            tonic::include_proto!("tensa.data");
        }
        pub mod voice {
            tonic::include_proto!("tensa.voice");
        }
        pub mod query {
            tonic::include_proto!("tensa.query");
        }
    }
}

pub use proto::tensa;

use tensa_approval::{ActionEvent, ActionKind, ActionStatus, EventType, VectorClock};
use tensa_nodes::{Heartbeat, Node, NodeResources, Platform, ThermalStatus};

fn net(e: tonic::Status) -> SyncError {
    SyncError::Network(e.message().to_string())
}

// === VectorClock ===

pub fn vc_to_proto(vc: &VectorClock) -> tensa::common::VectorClock {
    tensa::common::VectorClock {
        clocks: vc.iter().map(|(k, v)| (k.clone(), *v)).collect(),
    }
}

pub fn vc_from_proto(p: Option<tensa::common::VectorClock>) -> VectorClock {
    match p {
        Some(vc) => VectorClock::from_map(vc.clocks.into_iter().collect()),
        None => VectorClock::new(),
    }
}

// === Approval (Контракт 8: sync.proto) ===

pub fn approval_event_to_proto(ev: &ActionEvent) -> tensa::sync::ApprovalEvent {
    // operation_id доступен из снимка действия в event_data. [derived]
    let operation_id = ev
        .event_data
        .get("action")
        .and_then(|a| a.get("operation_id"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    tensa::sync::ApprovalEvent {
        event_id: ev.event_id,
        action_id: ev.action_id.to_string(),
        event_type: ev.event_type.as_str().to_string(),
        event_data: ev.event_data.to_string(),
        timestamp: ev.timestamp,
        node_id: ev.node_id.clone(),
        vector_clock: Some(vc_to_proto(&ev.vector_clock)),
        operation_id,
    }
}

pub fn approval_event_from_proto(msg: &tensa::sync::ApprovalEvent) -> Result<ActionEvent> {
    let event_type: EventType = msg
        .event_type
        .parse()
        .map_err(|e: tensa_approval::ApprovalError| SyncError::Config(e.to_string()))?;
    Ok(ActionEvent {
        event_id: msg.event_id,
        action_id: msg
            .action_id
            .parse()
            .map_err(|e| SyncError::Config(format!("bad action_id: {e}")))?,
        event_type,
        event_data: serde_json::from_str(&msg.event_data)?,
        timestamp: msg.timestamp,
        node_id: msg.node_id.clone(),
        vector_clock: vc_from_proto(msg.vector_clock.clone()),
    })
}

// === Notes / Contacts (Контракт 8: sync.proto) ===

pub fn note_event_to_proto(ev: &NoteEvent) -> tensa::sync::NoteEvent {
    tensa::sync::NoteEvent {
        note_id: ev.note_id.to_string(),
        event_type: ev.event_type.clone(),
        note_data: ev.note_data.to_string(),
        timestamp: ev.timestamp,
        node_id: ev.node_id.clone(),
        vector_clock: Some(vc_to_proto(&ev.vector_clock)),
    }
}

pub fn note_event_from_proto(msg: &tensa::sync::NoteEvent) -> Result<NoteEvent> {
    Ok(NoteEvent {
        note_id: msg
            .note_id
            .parse()
            .map_err(|e| SyncError::Config(format!("bad note_id: {e}")))?,
        event_type: msg.event_type.clone(),
        note_data: serde_json::from_str(&msg.note_data)?,
        timestamp: msg.timestamp,
        node_id: msg.node_id.clone(),
        vector_clock: vc_from_proto(msg.vector_clock.clone()),
    })
}

pub fn contact_event_to_proto(ev: &ContactEvent) -> tensa::sync::ContactEvent {
    tensa::sync::ContactEvent {
        contact_id: ev.contact_id.to_string(),
        event_type: ev.event_type.clone(),
        contact_data: ev.contact_data.to_string(),
        timestamp: ev.timestamp,
        node_id: ev.node_id.clone(),
        vector_clock: Some(vc_to_proto(&ev.vector_clock)),
    }
}

pub fn contact_event_from_proto(msg: &tensa::sync::ContactEvent) -> Result<ContactEvent> {
    Ok(ContactEvent {
        contact_id: msg
            .contact_id
            .parse()
            .map_err(|e| SyncError::Config(format!("bad contact_id: {e}")))?,
        event_type: msg.event_type.clone(),
        contact_data: serde_json::from_str(&msg.contact_data)?,
        timestamp: msg.timestamp,
        node_id: msg.node_id.clone(),
        vector_clock: vc_from_proto(msg.vector_clock.clone()),
    })
}

// === Node (Контракт 8: node.proto) ===

fn platform_to_proto(p: &Platform) -> i32 {
    match p {
        Platform::Windows => tensa::common::Platform::Windows as i32,
        Platform::Linux => tensa::common::Platform::Linux as i32,
        Platform::Android => tensa::common::Platform::Android as i32,
    }
}

fn platform_from_proto(v: i32) -> Result<Platform> {
    match tensa::common::Platform::try_from(v) {
        Ok(tensa::common::Platform::Windows) => Ok(Platform::Windows),
        Ok(tensa::common::Platform::Linux) => Ok(Platform::Linux),
        Ok(tensa::common::Platform::Android) => Ok(Platform::Android),
        _ => Err(SyncError::Config(format!("bad platform: {v}"))),
    }
}

fn role_to_proto(r: &tensa_core::NodeRole) -> i32 {
    match r {
        tensa_core::NodeRole::Heavy => tensa::common::NodeRole::Heavy as i32,
        tensa_core::NodeRole::Thin => tensa::common::NodeRole::Thin as i32,
        tensa_core::NodeRole::Auto => tensa::common::NodeRole::Auto as i32,
    }
}

fn role_from_proto(v: i32) -> Result<tensa_core::NodeRole> {
    match tensa::common::NodeRole::try_from(v) {
        Ok(tensa::common::NodeRole::Heavy) => Ok(tensa_core::NodeRole::Heavy),
        Ok(tensa::common::NodeRole::Thin) => Ok(tensa_core::NodeRole::Thin),
        Ok(tensa::common::NodeRole::Auto) => Ok(tensa_core::NodeRole::Auto),
        _ => Err(SyncError::Config(format!("bad role: {v}"))),
    }
}

fn thermal_from_str(s: &str) -> Result<ThermalStatus> {
    Ok(match s {
        "nominal" => ThermalStatus::Nominal,
        "fair" => ThermalStatus::Fair,
        "serious" => ThermalStatus::Serious,
        "critical" => ThermalStatus::Critical,
        other => return Err(SyncError::Config(format!("bad thermal_status: {other}"))),
    })
}

fn device_type_from_str(s: &str) -> Result<tensa_core::DeviceType> {
    Ok(match s {
        "desktop" => tensa_core::DeviceType::Desktop,
        "android" => tensa_core::DeviceType::Android,
        other => return Err(SyncError::Config(format!("bad device_type: {other}"))),
    })
}

fn device_type_to_str(d: &tensa_core::DeviceType) -> String {
    match d {
        tensa_core::DeviceType::Desktop => "desktop".into(),
        tensa_core::DeviceType::Android => "android".into(),
    }
}

pub fn node_to_proto(n: &Node) -> tensa::node::Node {
    tensa::node::Node {
        node_id: n.node_id.to_string(),
        name: n.name.clone(),
        device_type: device_type_to_str(&n.device_type),
        platform: platform_to_proto(&n.platform),
        role: role_to_proto(&n.role),
        tailscale_ip: n.tailscale_ip.clone(),
        grpc_port: n.grpc_port as u32,
        last_seen: n.last_seen,
        online: n.online,
        ram_total_mb: n.ram_total_mb,
        ram_available_mb: n.ram_available_mb,
        cpu_cores: n.cpu_cores,
        cpu_usage_percent: n.cpu_usage_percent,
        has_gpu: n.has_gpu,
        has_npu: n.has_npu,
        battery_percent: n.battery_percent.map(|b| b as u32),
        battery_charging: n.battery_charging,
        thermal_status: n.thermal_status.as_str().to_string(),
        resources: Some(tensa::node::NodeResources {
            telegram: n.resources.telegram,
            email: n.resources.email,
            whatsapp: n.resources.whatsapp,
            llm_local: n.resources.llm_local,
            llm_cloud: n.resources.llm_cloud,
            memory_store: n.resources.memory_store,
            approval_store: n.resources.approval_store,
        }),
        created_at: n.created_at,
        updated_at: n.updated_at,
    }
}

pub fn node_from_proto(msg: &tensa::node::Node) -> Result<Node> {
    let resources = msg
        .resources
        .as_ref()
        .map(|r| NodeResources {
            telegram: r.telegram,
            email: r.email,
            whatsapp: r.whatsapp,
            llm_local: r.llm_local,
            llm_cloud: r.llm_cloud,
            memory_store: r.memory_store,
            approval_store: r.approval_store,
        })
        .unwrap_or_default();
    Ok(Node {
        node_id: msg
            .node_id
            .parse()
            .map_err(|e| SyncError::Config(format!("bad node_id: {e}")))?,
        name: msg.name.clone(),
        device_type: device_type_from_str(&msg.device_type)?,
        platform: platform_from_proto(msg.platform)?,
        role: role_from_proto(msg.role)?,
        tailscale_ip: msg.tailscale_ip.clone(),
        grpc_port: msg.grpc_port as u16,
        last_seen: msg.last_seen,
        online: msg.online,
        ram_total_mb: msg.ram_total_mb,
        ram_available_mb: msg.ram_available_mb,
        cpu_cores: msg.cpu_cores,
        cpu_usage_percent: msg.cpu_usage_percent,
        has_gpu: msg.has_gpu,
        has_npu: msg.has_npu,
        battery_percent: msg.battery_percent.map(|b| b.min(100) as u8),
        battery_charging: msg.battery_charging,
        thermal_status: thermal_from_str(&msg.thermal_status)?,
        resources,
        created_at: msg.created_at,
        updated_at: msg.updated_at,
    })
}

pub fn heartbeat_to_proto(node_id: &uuid::Uuid, hb: &Heartbeat) -> tensa::node::HeartbeatRequest {
    tensa::node::HeartbeatRequest {
        node_id: node_id.to_string(),
        ram_available_mb: hb.ram_available_mb,
        cpu_usage_percent: hb.cpu_usage_percent,
        battery_percent: hb.battery_percent.map(|b| b as u32),
        battery_charging: hb.battery_charging,
        thermal_status: hb.thermal_status.as_str().to_string(),
        timestamp: hb.timestamp,
    }
}

pub fn heartbeat_from_proto(msg: &tensa::node::HeartbeatRequest) -> Result<Heartbeat> {
    Ok(Heartbeat {
        node_id: msg
            .node_id
            .parse()
            .map_err(|e| SyncError::Config(format!("bad node_id: {e}")))?,
        ram_available_mb: msg.ram_available_mb,
        cpu_usage_percent: msg.cpu_usage_percent,
        battery_percent: msg.battery_percent.map(|b| b.min(100) as u8),
        battery_charging: msg.battery_charging,
        thermal_status: thermal_from_str(&msg.thermal_status)?,
        timestamp: msg.timestamp,
    })
}

// === Data (Контракт 8: data.proto) ===

fn action_kind_to_proto(k: &ActionKind) -> i32 {
    match k {
        ActionKind::EmailSend => tensa::common::ActionKind::EmailSend as i32,
        ActionKind::TelegramSend => tensa::common::ActionKind::TelegramSend as i32,
        ActionKind::WhatsAppSend => tensa::common::ActionKind::WhatsappSend as i32,
        ActionKind::CalendarCreate => tensa::common::ActionKind::CalendarCreate as i32,
        ActionKind::AppLaunch => tensa::common::ActionKind::AppLaunch as i32,
        ActionKind::FileOpen => tensa::common::ActionKind::FileOpen as i32,
    }
}

fn action_status_to_proto(s: &ActionStatus) -> i32 {
    match s {
        ActionStatus::Pending => tensa::common::ActionStatus::Pending as i32,
        ActionStatus::Approved => tensa::common::ActionStatus::Approved as i32,
        ActionStatus::Rejected => tensa::common::ActionStatus::Rejected as i32,
        ActionStatus::Executed => tensa::common::ActionStatus::Executed as i32,
        ActionStatus::Failed => tensa::common::ActionStatus::Failed as i32,
        ActionStatus::Expired => tensa::common::ActionStatus::Expired as i32,
    }
}

pub fn action_to_proto_approval(a: &tensa_approval::Action) -> tensa::data::ApprovalAction {
    tensa::data::ApprovalAction {
        action_id: a.id.to_string(),
        kind: action_kind_to_proto(&a.kind),
        payload: a.payload.to_string(),
        status: action_status_to_proto(&a.status),
        created_at: a.created_at,
        expires_at: a.expires_at,
    }
}

pub fn core_note_to_proto(n: &tensa_core::Note) -> tensa::data::Note {
    tensa::data::Note {
        note_id: n.id.to_string(),
        title: n.title.clone(),
        body: n.body.clone(),
        tags: n.tags.clone(),
        created_at: n.created_at,
        updated_at: n.updated_at,
    }
}

pub fn core_contact_to_proto(c: &tensa_core::Contact) -> tensa::data::Contact {
    tensa::data::Contact {
        contact_id: c.id.to_string(),
        name: c.name.clone(),
        email: c.email.clone(),
        phone: c.phone.clone(),
        telegram: c.telegram.clone(),
        whatsapp: c.whatsapp.clone(),
    }
}

pub fn memory_search_to_proto(r: &tensa_memory::SearchResult) -> tensa::data::SearchResult {
    tensa::data::SearchResult {
        document_id: r.document_id.to_string(),
        source_path: r.source_path.clone(),
        chunk_text: r.chunk_text.clone(),
        score: r.score,
        indexed_at: r.indexed_at,
    }
}

/// Общий helper: ответ common.Result об успехе.
pub fn ok_result(operation_id: impl Into<String>) -> tensa::common::Result {
    tensa::common::Result {
        success: true,
        error: None,
        operation_id: operation_id.into(),
    }
}

/// Общий helper: ответ common.Result об ошибке.
pub fn err_result(code: &str, message: impl Into<String>) -> tensa::common::Result {
    tensa::common::Result {
        success: false,
        error: Some(tensa::common::Error {
            code: code.to_string(),
            message: message.into(),
            details: String::new(),
        }),
        operation_id: String::new(),
    }
}

/// Преобразование tonic::Status в SyncError для клиента.
pub fn status_to_error(st: tonic::Status) -> SyncError {
    net(st)
}
