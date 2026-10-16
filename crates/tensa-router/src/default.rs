//! Реализация по умолчанию: `DefaultBrainRouter` (ТЗ tensa-router).
//!
//! Логика маршрутизации (ТЗ):
//! 1) приоритет (Low/Normal → локальная, High → облако);
//! 2) доступность (локальный heavy → локальная, удалённый heavy → gRPC,
//!    только облако → облако);
//! 3) облачный fallback (бесплатная → платная → локальная);
//! 4) degraded mode (если всё недоступно — только поиск).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use tensa_core::paths::TensaPaths;
use tensa_llm::LlmEngine;
use tensa_nodes::NodeRegistry;
use tensa_perf::PerfProfiler;
use uuid::Uuid;

use crate::cloud::{CloudBackend, HttpCloudBackend};
use crate::config::RouterConfig;
use crate::degraded;
use crate::error::{Result, RouterError};
use crate::fallback;
use crate::local::{EngineLocalBackend, LocalBackend};
use crate::models::{
    BrainMode, BudgetStatus, QueryChunk, QueryPriority, QueryRequest, QueryResponse, QueryStream,
    RouterHealth,
};
use crate::node_selector::{self, RouteTarget};
use crate::remote::{GrpcRemoteBackend, RemoteBackend};
use crate::router::BrainRouter;

/// Зависимости роутера (точки внедрения для тестов и продакшена).
pub struct RouterDeps {
    pub local: Arc<dyn LocalBackend>,
    pub cloud: Arc<dyn CloudBackend>,
    pub remote: Arc<dyn RemoteBackend>,
    pub nodes: Arc<dyn NodeRegistry>,
    /// Профилировщик устройства (подтверждает, что узел heavy).
    pub profiler: Option<Arc<dyn PerfProfiler>>,
    /// Идентификатор этого узла (если известен).
    pub self_node_id: Option<Uuid>,
}

impl RouterDeps {
    /// Продовая сборка: движок llama.cpp + OpenRouter + gRPC-клиент.
    pub fn production(
        config: &RouterConfig,
        engine: Arc<dyn LlmEngine>,
        nodes: Arc<dyn NodeRegistry>,
        profiler: Option<Arc<dyn PerfProfiler>>,
    ) -> Result<Self> {
        let timeout = Duration::from_secs(config.cloud_free_timeout_seconds.max(1));
        Ok(Self {
            local: Arc::new(EngineLocalBackend::new(
                engine,
                config.local_model.model.clone(),
            )),
            cloud: Arc::new(HttpCloudBackend::from_env(timeout)?),
            remote: Arc::new(GrpcRemoteBackend::new(timeout)),
            nodes,
            profiler,
            self_node_id: None,
        })
    }
}

/// Роутер по умолчанию.
pub struct DefaultBrainRouter {
    config: RouterConfig,
    deps: RouterDeps,
    budget: crate::budget::BudgetTracker,
    last_degraded_reason: RwLock<Option<String>>,
}

impl DefaultBrainRouter {
    /// Создать роутер (валидирует конфиг, загружает бюджет из файла).
    pub fn new(config: RouterConfig, deps: RouterDeps, budget_path: PathBuf) -> Result<Self> {
        config.validate()?;
        let budget = crate::budget::BudgetTracker::load(
            budget_path,
            config.openrouter_daily_limit_tokens,
            config.openrouter_monthly_limit_usd,
            config.openrouter_warn_at_percent,
        )?;
        Ok(Self {
            config,
            deps,
            budget,
            last_degraded_reason: RwLock::new(None),
        })
    }

    /// Создать роутер с путём бюджета по умолчанию
    /// (`<root>/router_budget.json`) — [derived], ТЗ путь не задаёт.
    pub fn with_paths(config: RouterConfig, deps: RouterDeps, paths: &TensaPaths) -> Result<Self> {
        Self::new(config, deps, paths.root().join("router_budget.json"))
    }

    /// Конфигурация роутера.
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Учётчик бюджета облака.
    pub fn budget(&self) -> &crate::budget::BudgetTracker {
        &self.budget
    }

    fn node_label(&self) -> String {
        self.deps
            .self_node_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "local".to_string())
    }

    /// Облако пригодно к вызову: настроено и бюджет не исчерпан.
    fn cloud_available(&self) -> bool {
        self.deps.cloud.available() && self.budget.check_available().is_ok()
    }

    /// Предпочесть облако? (ТЗ: High → облако; GUI-переключатель — `use_cloud`;
    /// режим `Local` — мастер-выключатель.)
    fn prefer_cloud(&self, request: &QueryRequest) -> bool {
        match self.config.mode {
            BrainMode::Local => false,
            BrainMode::Cloud => true,
            BrainMode::Auto => request.use_cloud || request.priority == QueryPriority::High,
        }
    }

    /// Локальный узел готов к инференсу (ТЗ: «локальный heavy → локальная»).
    async fn local_ready(&self) -> bool {
        let engine_ok = self.deps.local.available().await;
        if !engine_ok {
            return false;
        }
        match &self.deps.profiler {
            Some(profiler) => match profiler.profile().await {
                Ok(profile) => match profiler.decide_role(&profile).await {
                    Ok(decision) => decision.can_run_llm,
                    Err(error) => {
                        tracing::debug!(
                            %error,
                            "роль узла недоступна — опираемся на готовность движка"
                        );
                        engine_ok
                    }
                },
                Err(error) => {
                    tracing::debug!(%error, "профиль недоступен — опираемся на готовность движка");
                    engine_ok
                }
            },
            None => engine_ok,
        }
    }

    async fn query_local(&self, request: &QueryRequest) -> Result<QueryResponse> {
        let reply = self.deps.local.generate(request).await?;
        Ok(QueryResponse {
            text: reply.text,
            action: None,
            action_payload: None,
            tool_calls: reply.tool_calls,
            reasoning: reply.reasoning,
            degraded: false,
            model_used: self.config.local_model.model.clone(),
            node_used: self.node_label(),
            tokens_used: Some(u64::from(reply.tokens_generated)),
        })
    }

    async fn query_cloud(&self, request: &QueryRequest) -> Result<QueryResponse> {
        self.budget.check_available()?;

        let timeout = Duration::from_secs(self.config.cloud_free_timeout_seconds.max(1));
        let mut last_error: Option<RouterError> = None;

        for attempt in fallback::plan(&self.config) {
            match fallback::call_with_retries(
                self.deps.cloud.as_ref(),
                &attempt,
                request,
                timeout,
                self.config.cloud_free_max_retries,
            )
            .await
            {
                Ok(reply) => {
                    // [derived] USD-стоимость подключится вместе с прайс-листом
                    // OpenRouter; дневной лимит токенов учитывается уже сейчас.
                    let tokens = reply.tokens_used.unwrap_or(0);
                    self.budget.record_usage(tokens, 0.0)?;
                    return Ok(QueryResponse {
                        text: reply.text,
                        action: None,
                        action_payload: None,
                        tool_calls: reply.tool_calls,
                        reasoning: reply.reasoning,
                        degraded: false,
                        model_used: attempt.model.clone(),
                        node_used: "cloud".to_string(),
                        tokens_used: reply.tokens_used,
                    });
                }
                Err(error) => {
                    let fatal = matches!(
                        error,
                        RouterError::ApiKey(_) | RouterError::BudgetExceeded(_)
                    );
                    last_error = Some(error);
                    if fatal {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| RouterError::Cloud("облако недоступно".to_string())))
    }

    /// Авто-fallback после отказа маршрута: локальная → облако → degraded.
    async fn fallback_after_error(
        &self,
        request: &QueryRequest,
        error: RouterError,
    ) -> Result<QueryResponse> {
        tracing::warn!(%error, "маршрут недоступен — авто-fallback");

        if self.config.auto_fallback {
            if self.config.cloud_fallback_to_local && self.deps.local.available().await {
                return self.query_local(request).await;
            }
            if matches!(error, RouterError::NodeUnavailable(_)) && self.cloud_available() {
                if let Ok(response) = self.query_cloud(request).await {
                    return Ok(response);
                }
            }
        }

        Ok(self.degrade(&format!("{error}")))
    }

    fn degrade(&self, reason: &str) -> QueryResponse {
        if let Ok(mut guard) = self.last_degraded_reason.write() {
            *guard = Some(reason.to_string());
        }
        degraded::degraded_response(reason)
    }
}

#[async_trait]
impl BrainRouter for DefaultBrainRouter {
    async fn query(&self, request: QueryRequest) -> Result<QueryResponse> {
        let cloud_available = self.cloud_available();
        let prefer_cloud = self.prefer_cloud(&request);
        let local_available = if prefer_cloud && cloud_available {
            false
        } else {
            self.local_ready().await
        };

        let target = node_selector::select_target(
            self.deps.nodes.as_ref(),
            self.deps.self_node_id,
            local_available,
            cloud_available,
            prefer_cloud,
        )
        .await?;
        tracing::debug!(?target, "цель маршрутизации");

        match target {
            RouteTarget::Local => self.query_local(&request).await,
            RouteTarget::Remote(node) => match self.deps.remote.query(&node, &request).await {
                Ok(response) => Ok(response),
                Err(error) => self.fallback_after_error(&request, error).await,
            },
            RouteTarget::Cloud => match self.query_cloud(&request).await {
                Ok(response) => Ok(response),
                Err(error) => self.fallback_after_error(&request, error).await,
            },
            RouteTarget::Degraded(reason) => Ok(self.degrade(&reason)),
        }
    }

    async fn query_stream(&self, request: QueryRequest) -> Result<QueryStream> {
        // [derived] Поштучный стриминг появится с нативным слоем (TASK-ENV-001);
        // контракт потока сохранён: текст → done.
        let response = self.query(request).await?;
        let chunks: Vec<Result<QueryChunk>> = vec![
            Ok(QueryChunk {
                text: response.text,
                done: false,
            }),
            Ok(QueryChunk {
                text: String::new(),
                done: true,
            }),
        ];
        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn health(&self) -> Result<RouterHealth> {
        let local_available = self.deps.local.available().await;
        let cloud_available = self.cloud_available();
        let degraded = !local_available && !cloud_available;
        let degraded_reason = if degraded {
            Some(
                self.last_degraded_reason
                    .read()
                    .ok()
                    .and_then(|guard| guard.clone())
                    .unwrap_or_else(|| "нет локальной модели и облака".to_string()),
            )
        } else {
            None
        };

        Ok(RouterHealth {
            local_available,
            cloud_available,
            degraded,
            degraded_reason,
            current_node: self.node_label(),
        })
    }

    async fn get_budget_status(&self) -> Result<BudgetStatus> {
        self.budget.status()
    }
}
