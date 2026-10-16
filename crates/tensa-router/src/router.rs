//! Трейт маршрутизатора (ТЗ tensa-router: `BrainRouter`, сигнатуры дословно).

use crate::error::Result;
use crate::models::{BudgetStatus, QueryRequest, QueryResponse, QueryStream, RouterHealth};

/// Маршрутизатор запросов к LLM: локальная/облачная модель, выбор узла,
/// бюджет и degraded-режим.
#[async_trait::async_trait]
pub trait BrainRouter: Send + Sync {
    /// Выполнить запрос.
    async fn query(&self, request: QueryRequest) -> Result<QueryResponse>;

    /// Выполнить запрос с потоковым ответом.
    async fn query_stream(&self, request: QueryRequest) -> Result<QueryStream>;

    /// Состояние роутера.
    async fn health(&self) -> Result<RouterHealth>;

    /// Статус бюджета облака.
    async fn get_budget_status(&self) -> Result<BudgetStatus>;
}
