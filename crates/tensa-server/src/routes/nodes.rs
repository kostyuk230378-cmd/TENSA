//! Узлы сети (Контракт 12 / Контракт 7): read-only вид реестра
//! `tensa-nodes`. Регистрация и heartbeat — по gRPC (NodeService, ADR-012),
//! HTTP отдаёт состояние владельцу.

use std::str::FromStr;

use axum::extract::{Path as AxumPath, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Параметры списка узлов.
#[derive(Debug, Deserialize)]
pub struct NodesQuery {
    /// Только онлайн-узлы.
    pub online_only: Option<bool>,
}

/// `GET /nodes?online_only=` — список узлов реестра.
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<NodesQuery>,
) -> Result<Json<Vec<tensa_nodes::Node>>> {
    let nodes = if query.online_only.unwrap_or(false) {
        state.nodes.list_online().await?
    } else {
        state.nodes.list_all().await?
    };
    Ok(Json(nodes))
}

/// `GET /nodes/heavy` — онлайн heavy-узел для инференса.
pub async fn heavy(State(state): State<AppState>) -> Result<Json<tensa_nodes::Node>> {
    state
        .nodes
        .find_heavy()
        .await?
        .ok_or_else(|| ServerError::NotFound("онлайн heavy-узел".to_string()))
        .map(Json)
}

/// `GET /nodes/:id` — карточка узла.
pub async fn get(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<tensa_nodes::Node>> {
    let uuid = Uuid::from_str(&id)
        .map_err(|error| ServerError::BadRequest(format!("id узла: {error}")))?;
    state
        .nodes
        .get(uuid)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("узел {id}")))
        .map(Json)
}
