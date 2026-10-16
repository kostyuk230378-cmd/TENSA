//! # tensa-nodes
//!
//! Реестр узлов Tensa (ТЗ tensa-nodes, ADR-015, Контракт 7):
//! - регистрация/удаление узлов сети (SQLite nodes.db);
//! - heartbeat (30 с интервал, офлайн > 90 с) с телеметрией RAM/CPU/GPU/NPU/
//!   батарея/thermal;
//! - роли heavy/thin/auto, поиск heavy-узла для инференса;
//! - ресурсы узлов (telegram/email/whatsapp/llm_local/llm_cloud/
//!   memory_store/approval_store) и поиск по ресурсу;
//! - соединения к разделяемым ресурсам (TTL 15 минут, конфликт — Err,
//!   освобождение при офлайн).
//!
//! Ограничения ТЗ: не синхронизирует, не выполняет действия, не логирует
//! секреты.

pub mod config;
pub mod error;
pub mod heartbeat;
pub mod models;
pub mod profile;
pub mod registry;
pub mod sqlite;

pub use config::NodesConfig;
pub use error::{NodesError, Result};
pub use heartbeat::{HEARTBEAT_INTERVAL_SECONDS, HEARTBEAT_TIMEOUT_SECONDS};
pub use models::{
    DeviceType, Heartbeat, Node, NodeConnection, NodeResources, NodeRole, Platform, ThermalStatus,
    LEASE_TTL_SECONDS,
};
pub use profile::{NodeProfile, NodeProfiler, StubProfiler};
pub use registry::NodeRegistry;
pub use sqlite::SqliteNodeRegistry;

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Исполнение SQL-схемы постранично (строка = statement до ';' в конце;
/// комментарии пропускаются; FTS-триггеры записаны однострочно).
pub(crate) async fn run_schema_statements(pool: &sqlx::SqlitePool, schema: &str) -> Result<()> {
    use sqlx::Executor;
    let mut current = String::new();
    for line in schema.lines() {
        if line.trim_start().starts_with("--") {
            continue;
        }
        current.push_str(line);
        current.push('\n');
        if line.trim_end().ends_with(';') {
            let stmt = current.trim();
            if !stmt.is_empty() {
                pool.execute(stmt).await?;
            }
            current.clear();
        }
    }
    let stmt = current.trim();
    if !stmt.is_empty() {
        pool.execute(stmt).await?;
    }
    Ok(())
}
