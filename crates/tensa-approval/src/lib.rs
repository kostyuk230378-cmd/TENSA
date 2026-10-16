//! # tensa-approval
//!
//! ApprovalStore — центральный элемент безопасности Tensa.
//! Хранит действия, требующие одобрения владельца, до их выполнения.
//!
//! Принципы (Контракт 1, ТЗ tensa-approval):
//! - авто-ответы запрещены;
//! - прямое исполнение сетевых действий из LLM запрещено;
//! - все действия проходят через очередь одобрения;
//! - действия от trusted origin исполняются без одобрения, но с логированием;
//! - event-sourced для синхронизации между узлами.
//!
//! Крейт не выполняет действия и не делает сетевых запросов — только хранит
//! и ведёт жизненный цикл: `created → pending → approved → executed`,
//! ответвления `rejected` / `expired` (TTL) / `failed`.

pub mod config;
pub mod error;
pub mod events;
pub mod models;
pub mod sqlite;
pub mod store;
pub mod vector_clock;

pub use config::ApprovalConfig;
pub use error::{ApprovalError, Result};
pub use events::{ActionEvent, EventType};
pub use models::{Action, ActionKind, ActionOrigin, ActionStatus, NewAction};
pub use sqlite::SqliteApprovalStore;
pub use store::ApprovalStore;
pub use vector_clock::VectorClock;
