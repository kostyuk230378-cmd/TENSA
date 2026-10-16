//! `GET /health` и `GET /version` (Контракт 12).

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
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
