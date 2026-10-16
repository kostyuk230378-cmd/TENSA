//! # tensa-sync
//!
//! Синхронизация между узлами Tensa (ADR-012, Контракт 8, ТЗ tensa-sync).
//!
//! Обеспечивает:
//! - синхронизацию ApprovalStore (постоянно), заметок и контактов (по требованию);
//! - heartbeat узлов (каждые 30 с, офлайн > 90 с);
//! - управление соединениями к разделяемым ресурсам (TTL 15 минут, ADR-015);
//! - управление Syncthing (ADR-011, ADR-018) — фича `syncthing`;
//! - gRPC-транспорт (tonic) — фича `grpc`.
//!
//! Ограничения ТЗ: не хранит данные, не выполняет действия, не принимает
//! решения, не логирует секреты.

pub mod adapters;
pub mod config;
pub mod connections;
pub mod error;
pub mod heartbeat;
pub mod service;
pub mod status;
pub mod store_ext;
pub mod sync_log;

#[cfg(feature = "grpc")]
pub mod client;
#[cfg(feature = "grpc")]
pub mod grpc;
#[cfg(feature = "grpc")]
pub mod server;

pub use adapters::{StoreContactsSync, StoreNotesSync};
pub use config::SyncConfig;
pub use connections::{ConnectionManager, Lease as ConnectionLease};
pub use error::{Result, SyncError};
pub use heartbeat::{HeartbeatState, NodeHeartbeat};
pub use service::{
    ContactEvent, ContactsSyncSource, NoopSyncSource, NoteEvent, NotesSyncSource, SyncService,
};
pub use status::{SyncStatus, SyncthingStatus};

#[cfg(feature = "grpc")]
pub use client::GrpcSyncService;
#[cfg(feature = "grpc")]
pub use server::{serve, EventBus, ServerDeps};

#[cfg(feature = "syncthing")]
pub mod syncthing;

/// Версия крейта (для handshake между узлами).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
