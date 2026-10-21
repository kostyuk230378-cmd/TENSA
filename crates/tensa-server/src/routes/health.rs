//! `GET /health`, `GET /version` и `GET /server/info` (Контракт 12).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use tensa_core::{ComponentHealth, HealthStatus};

use crate::state::AppState;

/// Версия крейта (из Cargo.toml): отдаётся в `/version` и `/health`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Снимок состояния ядра — реальные пробы хранилищ (очередь одобрения, заметки).
///
/// Неуспешная проба помечает компонент нездоровым (`degraded`), но сам запрос
/// остаётся успешным: клиент различает «ядро живо, но деградировало» по полям
/// ответа, а не по HTTP-статусу.
pub(crate) async fn snapshot(state: &AppState) -> HealthStatus {
    let mut components = HashMap::new();

    let approvals = match state.approvals.get_pending().await {
        Ok(pending) => ComponentHealth {
            healthy: true,
            detail: Some(format!("pending: {}", pending.len())),
        },
        Err(err) => ComponentHealth {
            healthy: false,
            detail: Some(format!("approval store unavailable: {err}")),
        },
    };
    components.insert("approval_store".to_string(), approvals);

    let notes = match state.notes.count_notes().await {
        Ok(count) => ComponentHealth {
            healthy: true,
            detail: Some(format!("notes: {count}")),
        },
        Err(err) => ComponentHealth {
            healthy: false,
            detail: Some(format!("notes store unavailable: {err}")),
        },
    };
    components.insert("notes_store".to_string(), notes);

    HealthStatus::from_components(VERSION, state.started_at, components)
}

/// `GET /health` — сводный статус ядра (Контракт 12: `HealthStatus`).
pub async fn health(State(state): State<AppState>) -> Json<HealthStatus> {
    Json(snapshot(&state).await)
}

/// `GET /version` — версия ядра. По Контракту 12 тело — JSON-строка.
pub async fn version() -> Json<String> {
    Json(VERSION.to_string())
}

/// Адрес сервера для ввода в мобильном приложении.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    /// LAN-адрес, если сервер слушает не только loopback.
    pub url: Option<String>,
    /// Локальный адрес для этого ПК.
    pub local_url: String,
    /// Порт HTTP.
    pub port: u16,
    /// Сервер слушает все интерфейсы.
    pub bind_all: bool,
}

/// `GET /server/info` — какой адрес показать для телефона в Wi-Fi.
pub async fn server_info() -> Json<ServerInfo> {
    let raw = std::env::var("TENSA_BIND").unwrap_or_else(|_| "127.0.0.1:8765".to_string());
    let addr: SocketAddr = raw
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 8765)));
    let bind_all = addr.ip().is_unspecified();
    let port = addr.port();
    let local_url = format!("http://127.0.0.1:{port}");
    let url = if bind_all {
        lan_ipv4().map(|ip| format!("http://{ip}:{port}"))
    } else {
        None
    };
    Json(ServerInfo {
        url,
        local_url,
        port,
        bind_all,
    })
}

/// Основной IPv4-адрес машины в локальной сети (без отправки пакетов).
fn lan_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}
