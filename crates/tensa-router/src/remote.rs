//! Удалённый инференс heavy-узла по gRPC (ТЗ tensa-router: «удалённый heavy →
//! gRPC»; Контракт 8, query.proto).
//!
//! [derived] В структуре ТЗ отдельный файл для удалённого бэкенда не выделен;
//! вынесен в `remote.rs`, чтобы точка внедрения (для тестов без сети) была
//! явной. Реальный транспорт — сгенерированный `QueryService` из tensa-sync.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tensa_nodes::Node;
use tensa_sync::grpc::proto::tensa;

use crate::error::{Result, RouterError};
use crate::models::{Language, QueryRequest, QueryResponse, ToolCall};

/// Бэкенд удалённого инференса.
#[async_trait]
pub trait RemoteBackend: Send + Sync {
    /// Выполнить запрос на heavy-узле.
    async fn query(&self, node: &Node, request: &QueryRequest) -> Result<QueryResponse>;
}

/// gRPC-клиент к `QueryService` удалённого узла.
pub struct GrpcRemoteBackend {
    timeout: Duration,
}

impl GrpcRemoteBackend {
    /// Создать бэкенд с таймаутом вызова.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl RemoteBackend for GrpcRemoteBackend {
    async fn query(&self, node: &Node, request: &QueryRequest) -> Result<QueryResponse> {
        let endpoint = tonic::transport::Endpoint::from_shared(format!(
            "http://{}:{}",
            node.tailscale_ip, node.grpc_port
        ))
        .map_err(|error| {
            RouterError::NodeUnavailable(format!("{}: некорректный адрес: {error}", node.name))
        })?;

        let call = async {
            let mut client =
                tensa::query::query_service_client::QueryServiceClient::connect(endpoint)
                    .await
                    .map_err(|error| {
                        RouterError::NodeUnavailable(format!("{}: {error}", node.name))
                    })?;

            let proto_request = tensa::query::QueryRequest {
                node_id: node.node_id.to_string(),
                text: request.text.clone(),
                language: match request.language {
                    Language::Ru => tensa::common::Language::Ru,
                    Language::En => tensa::common::Language::En,
                } as i32,
                context: request.context.clone(),
                use_cloud: request.use_cloud,
            };

            let response = client
                .query(proto_request)
                .await
                .map_err(|status| RouterError::NodeUnavailable(status.message().to_string()))?;
            Ok::<_, RouterError>(response.into_inner())
        };

        let response = tokio::time::timeout(self.timeout, call)
            .await
            .map_err(|_| {
                RouterError::NodeUnavailable(format!(
                    "{}: таймаут {} c",
                    node.name,
                    self.timeout.as_secs()
                ))
            })??;

        Ok(map_response(node, response))
    }
}

fn parse_json_field(raw: &str) -> Option<serde_json::Value> {
    if raw.is_empty() {
        return None;
    }
    Some(serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string())))
}

fn map_response(node: &Node, response: tensa::query::QueryResponse) -> QueryResponse {
    let action = if response.action.is_empty() {
        None
    } else {
        Some(response.action)
    };
    let reasoning = if response.reasoning.is_empty() {
        None
    } else {
        Some(response.reasoning)
    };

    QueryResponse {
        text: response.text,
        action,
        action_payload: parse_json_field(&response.action_payload),
        tool_calls: response
            .tool_calls
            .into_iter()
            .map(|call| ToolCall {
                tool: call.tool,
                params: parse_json_field(&call.params).unwrap_or(serde_json::Value::Null),
            })
            .collect(),
        reasoning,
        degraded: response.degraded,
        model_used: response.model_used,
        node_used: node.node_id.to_string(),
        tokens_used: None,
    }
}

/// Заглушка удалённого бэкенда (тесты и CI).
pub struct StubRemoteBackend {
    /// Текст ответа.
    pub text: String,
    calls: Mutex<Vec<uuid::Uuid>>,
}

impl StubRemoteBackend {
    /// Создать заглушку.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Узлы, к которым обращались (в порядке вызовов).
    pub fn calls(&self) -> Vec<uuid::Uuid> {
        match self.calls.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[async_trait]
impl RemoteBackend for StubRemoteBackend {
    async fn query(&self, node: &Node, _request: &QueryRequest) -> Result<QueryResponse> {
        match self.calls.lock() {
            Ok(mut guard) => guard.push(node.node_id),
            Err(poisoned) => poisoned.into_inner().push(node.node_id),
        }
        Ok(QueryResponse {
            text: self.text.clone(),
            action: None,
            action_payload: None,
            tool_calls: Vec::new(),
            reasoning: None,
            degraded: false,
            model_used: "remote-stub".to_string(),
            node_used: node.node_id.to_string(),
            tokens_used: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_field_handles_broken_json() {
        assert!(parse_json_field("").is_none());
        assert_eq!(
            parse_json_field("{\"a\":1}"),
            Some(serde_json::json!({"a": 1}))
        );
        assert_eq!(
            parse_json_field("не-json"),
            Some(serde_json::Value::String("не-json".to_string()))
        );
    }
}
