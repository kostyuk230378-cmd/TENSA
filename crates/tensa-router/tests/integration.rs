//! Интеграционные тесты tensa-router (ТЗ: локальный инференс, облако,
//! fallback, удалённый gRPC, бюджет, degraded, ошибки).

use std::sync::Arc;

use tensa_core::paths::TensaPaths;
use tensa_nodes::{NodeRegistry, SqliteNodeRegistry};
use tensa_router::{
    BrainMode, BrainRouter, CloudModelConfig, CloudReply, DefaultBrainRouter, QueryPriority,
    QueryRequest, RouterConfig, RouterDeps, RouterError, StubCloudBackend, StubLocalBackend,
    StubRemoteBackend,
};

struct Fixture {
    router: DefaultBrainRouter,
    cloud: Arc<StubCloudBackend>,
    remote: Arc<StubRemoteBackend>,
    registry: Arc<SqliteNodeRegistry>,
    root: std::path::PathBuf,
}

async fn fixture(config: RouterConfig, local_loaded: bool, cloud_available: bool) -> Fixture {
    let root = std::env::temp_dir().join(format!("tensa-router-it-{}", uuid::Uuid::new_v4()));
    let paths = TensaPaths::with_root(root.clone());

    let local: Arc<dyn tensa_router::LocalBackend> = if local_loaded {
        Arc::new(StubLocalBackend::loaded("локальный ответ"))
    } else {
        Arc::new(StubLocalBackend::unavailable())
    };
    let cloud = Arc::new(StubCloudBackend::new(cloud_available));
    let remote = Arc::new(StubRemoteBackend::new("удалённый ответ"));
    let registry = Arc::new(SqliteNodeRegistry::new_in_memory().await.expect("реестр"));

    let deps = RouterDeps {
        local,
        cloud: cloud.clone(),
        remote: remote.clone(),
        nodes: registry.clone(),
        profiler: None,
        self_node_id: None,
    };
    let router = DefaultBrainRouter::with_paths(config, deps, &paths).expect("роутер");

    Fixture {
        router,
        cloud,
        remote,
        registry,
        root,
    }
}

fn auto_config() -> RouterConfig {
    RouterConfig {
        mode: BrainMode::Auto,
        ..RouterConfig::default()
    }
}

fn cloud_config() -> RouterConfig {
    RouterConfig {
        mode: BrainMode::Cloud,
        ..RouterConfig::default()
    }
}

fn heavy_node() -> tensa_nodes::Node {
    use tensa_core::{DeviceType, NodeRole};
    use tensa_nodes::{NodeResources, Platform, ThermalStatus};
    tensa_nodes::Node {
        node_id: uuid::Uuid::new_v4(),
        name: "heavy-1".to_string(),
        device_type: DeviceType::Desktop,
        platform: Platform::Windows,
        role: NodeRole::Heavy,
        tailscale_ip: "100.64.0.10".to_string(),
        grpc_port: 50051,
        last_seen: 0,
        online: true,
        ram_total_mb: 32768,
        ram_available_mb: 16384,
        cpu_cores: 16,
        cpu_usage_percent: 10.0,
        has_gpu: true,
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
async fn normal_priority_uses_local() {
    let f = fixture(auto_config(), true, true).await;
    let response = f
        .router
        .query(QueryRequest::text("вопрос"))
        .await
        .expect("ответ");

    assert_eq!(response.text, "локальный ответ");
    assert!(!response.degraded);
    assert_eq!(response.model_used, "gemma-4-E2B-it");
    assert!(f.cloud.calls().is_empty(), "облако не должно вызываться");
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn high_priority_uses_cloud_and_records_budget() {
    let f = fixture(auto_config(), true, true).await;
    f.cloud.push(Ok(CloudReply {
        text: "облачный ответ".to_string(),
        tokens_used: Some(42),
        tool_calls: Vec::new(),
        reasoning: None,
    }));

    let mut request = QueryRequest::text("сложный вопрос");
    request.priority = QueryPriority::High;
    let response = f.router.query(request).await.expect("ответ");

    assert_eq!(response.text, "облачный ответ");
    assert_eq!(response.node_used, "cloud");
    assert_eq!(response.tokens_used, Some(42));

    let status = f.router.get_budget_status().await.expect("бюджет");
    assert_eq!(status.daily_tokens_used, 42);
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn mode_local_ignores_use_cloud_and_priority() {
    let config = RouterConfig {
        mode: BrainMode::Local,
        ..RouterConfig::default()
    };
    let f = fixture(config, true, true).await;

    let mut request = QueryRequest::text("вопрос");
    request.use_cloud = true;
    request.priority = QueryPriority::High;
    let response = f.router.query(request).await.expect("ответ");

    assert_eq!(response.text, "локальный ответ");
    assert!(f.cloud.calls().is_empty());
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn use_cloud_flag_forces_cloud_in_auto_mode() {
    let f = fixture(auto_config(), true, true).await;
    f.cloud.push(Ok(CloudReply::text("облако по флагу")));

    let mut request = QueryRequest::text("вопрос");
    request.use_cloud = true;
    let response = f.router.query(request).await.expect("ответ");

    assert_eq!(response.text, "облако по флагу");
    assert_eq!(f.cloud.calls().len(), 1);
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn cloud_free_fails_paid_succeeds() {
    let f = fixture(cloud_config(), false, true).await;
    // Бесплатная: 3 попытки (retries = 2) — все с ошибкой; платная: успех.
    f.cloud.push(Err(RouterError::Cloud("сеть".to_string())));
    f.cloud.push(Err(RouterError::Cloud("сеть".to_string())));
    f.cloud.push(Err(RouterError::Cloud("сеть".to_string())));
    f.cloud.push(Ok(CloudReply::text("платный ответ")));

    let response = f
        .router
        .query(QueryRequest::text("вопрос"))
        .await
        .expect("ответ");

    assert_eq!(response.text, "платный ответ");
    let calls = f.cloud.calls();
    assert_eq!(calls.len(), 4, "3 попытки free + 1 paid: {calls:?}");
    assert!(calls[0].ends_with(":free"));
    let paid_model = RouterConfig::default()
        .cloud_models
        .iter()
        .find(|model| !model.free)
        .map(|model| model.model.clone())
        .expect("платная модель");
    assert_eq!(calls[3], paid_model);
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn cloud_failure_falls_back_to_local() {
    let f = fixture(auto_config(), true, true).await;
    for _ in 0..4 {
        f.cloud.push(Err(RouterError::Cloud("сеть".to_string())));
    }

    let mut request = QueryRequest::text("вопрос");
    request.priority = QueryPriority::High;
    let response = f.router.query(request).await.expect("ответ");

    assert_eq!(response.text, "локальный ответ");
    assert!(!response.degraded);
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn api_key_error_falls_back_to_local() {
    let f = fixture(auto_config(), true, true).await;
    f.cloud
        .push(Err(RouterError::ApiKey("HTTP 401".to_string())));

    let mut request = QueryRequest::text("вопрос");
    request.use_cloud = true;
    let response = f.router.query(request).await.expect("ответ");

    assert_eq!(response.text, "локальный ответ");
    assert_eq!(f.cloud.calls().len(), 1, "ключ не повторяется");
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn monthly_limit_is_reported_in_status() {
    let config = RouterConfig {
        mode: BrainMode::Cloud,
        cloud_models: vec![CloudModelConfig {
            model: "paid-only".to_string(),
            priority: 1,
            free: false,
        }],
        ..RouterConfig::default()
    };
    let f = fixture(config, false, true).await;

    let status = f
        .router
        .budget()
        .record_usage(0, 10.0)
        .expect("запись бюджета");
    assert_eq!(status.monthly_usd_used, 10.0);
    assert!(status.exhausted());

    let response = f
        .router
        .query(QueryRequest::text("вопрос"))
        .await
        .expect("ответ");
    assert!(response.degraded);
    assert!(
        f.cloud.calls().is_empty(),
        "при исчерпанном бюджете не звоним"
    );
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn remote_heavy_is_used_when_local_unavailable() {
    let f = fixture(auto_config(), false, true).await;
    let node = heavy_node();
    f.registry
        .register(node.clone())
        .await
        .expect("регистрация");

    let response = f
        .router
        .query(QueryRequest::text("вопрос"))
        .await
        .expect("ответ");

    assert_eq!(response.text, "удалённый ответ");
    assert_eq!(response.node_used, node.node_id.to_string());
    assert_eq!(f.remote.calls(), vec![node.node_id]);
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn degraded_when_nothing_available() {
    let f = fixture(auto_config(), false, false).await;

    let response = f
        .router
        .query(QueryRequest::text("вопрос"))
        .await
        .expect("ответ");

    assert!(response.degraded);
    assert!(response.text.is_empty());
    assert_eq!(response.model_used, "none");

    let health = f.router.health().await.expect("health");
    assert!(health.degraded);
    assert!(health.degraded_reason.is_some());
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn health_reports_local_availability() {
    let f = fixture(auto_config(), true, false).await;
    let health = f.router.health().await.expect("health");
    assert!(health.local_available);
    assert!(!health.cloud_available);
    assert!(!health.degraded, "локальная модель есть — не degraded");
    let _ = std::fs::remove_dir_all(f.root);
}

#[tokio::test]
async fn stream_yields_text_then_done() {
    use futures::StreamExt;

    let f = fixture(auto_config(), true, true).await;
    let mut stream = f
        .router
        .query_stream(QueryRequest::text("вопрос"))
        .await
        .expect("поток");

    let first = stream.next().await.expect("чанк 1").expect("ок");
    let second = stream.next().await.expect("чанк 2").expect("ок");

    assert_eq!(first.text, "локальный ответ");
    assert!(!first.done);
    assert!(second.done);
    assert!(stream.next().await.is_none());
    let _ = std::fs::remove_dir_all(f.root);
}
