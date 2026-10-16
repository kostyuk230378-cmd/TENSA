//! # tensa-router
//!
//! Маршрутизация запросов к LLM: локальная модель по умолчанию
//! (Gemma 4 E2B), облачная для сложных задач (Gemma 4 26B A4B), авто-fallback,
//! выбор узла (локальный heavy / удалённый heavy по gRPC), бюджет OpenRouter,
//! degraded-режим (ТЗ tensa-router; ADR-001..003, ADR-012, ADR-013).
//!
//! - [`RouterConfig`] — режим, модели, лимиты (значения — по ТЗ);
//! - [`BrainRouter`] + [`DefaultBrainRouter`] — запрос, поток, health, бюджет;
//! - [`local::LocalBackend`], [`cloud::CloudBackend`], [`remote::RemoteBackend`] —
//!   точки внедрения (в продакшене — llama.cpp, OpenRouter, gRPC).
//!
//! Секреты: API-ключ читается из `OPENROUTER_API_KEY` и не логируется.
//!
//! Ограничения ТЗ: не хранит историю, не выполняет действия, не логирует
//! секреты, не превышает бюджет.

pub mod budget;
pub mod cloud;
pub mod config;
pub mod default;
pub mod degraded;
pub mod error;
pub mod fallback;
pub mod local;
pub mod models;
pub mod node_selector;
pub mod remote;
pub mod router;

pub use budget::BudgetTracker;
pub use cloud::{CloudBackend, CloudReply, HttpCloudBackend, StubCloudBackend};
pub use config::{default_cloud_models, CloudModelConfig, LocalModelConfig, RouterConfig};
pub use default::{DefaultBrainRouter, RouterDeps};
pub use error::{Result, RouterError};
pub use local::{EngineLocalBackend, LocalBackend, LocalReply, StubLocalBackend};
pub use models::{
    BrainMode, BudgetStatus, QueryChunk, QueryPriority, QueryRequest, QueryResponse, QueryStream,
    RouterHealth, ToolCall,
};
pub use node_selector::{select_target, RouteTarget};
pub use remote::{GrpcRemoteBackend, RemoteBackend, StubRemoteBackend};
pub use router::BrainRouter;

// Общие типы — единый источник (не дублировать).
pub use tensa_core::Language;
pub use tensa_llm::ModelTier;

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
