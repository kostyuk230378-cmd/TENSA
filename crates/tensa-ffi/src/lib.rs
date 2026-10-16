//! # tensa-ffi
//!
//! uniffi-обёртка Tensa (Контракт 9): UDL-биндинги для мобильных и настольных
//! клиентов. Источник истины поверхности — `teh_doc/contracts/ffi/tensa.udl`;
//! scaffolding генерируется в `build.rs` и подключается ниже.
//!
//! Реально подключены к ядру: [`ApprovalStore`], [`NotesStore`],
//! [`ContactsStore`], [`ConfigStore`], [`NodeRegistry`], [`CoreMemoryStore`].
//! Остальные интерфейсы объявлены и отвечают «ещё не реализовано»
//! (см. [`stubs`]); namespace-функции — [`init`], [`health`], [`version`].
//!
//! [derived] Особенности UDL-режима uniffi: типы объявляются обычными
//! Rust-типами без derive-макросов uniffi (конверсию генерирует `udl_derive`
//! из scaffolding), конструкторы — `pub async fn new(...)`, unit-варианты
//! enum — с пустыми скобками (`Persona {}`), как требует кодоген.

pub mod approval;
pub mod config;
pub mod contacts;
pub mod convert;
pub mod core_memory;
pub mod error;
pub mod nodes;
pub mod notes;
pub mod serde_impl;
pub mod stubs;
pub mod types;

pub use approval::ApprovalStore;
pub use config::ConfigStore;
pub use contacts::ContactsStore;
pub use core_memory::CoreMemoryStore;
pub use error::TensaError;
pub use nodes::NodeRegistry;
pub use notes::NotesStore;
pub use stubs::{
    AppLauncher, CalendarClient, EmailClient, FirstRunWizard, MemoryStore, QueryEngine, VoiceEngine,
};
pub use types::*;

use std::path::Path;

use tensa_core::TensaCore;

/// Версия ядра (UDL: `string version();`).
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Инициализация ядра по пути к `config.toml`
/// (UDL: `[Async, Throws=TensaError] void init(string config_path);`).
pub async fn init(config_path: String) -> Result<(), TensaError> {
    let path = Path::new(&config_path);
    let core = TensaCore::new(path).await?;
    if path.exists() {
        core.reload_config().await?;
    }
    Ok(())
}

/// Сводный статус ядра
/// (UDL: `[Async, Throws=TensaError] HealthStatus health();`).
///
/// Используется конфигурация по умолчанию `~/.tensa/config.toml`.
pub async fn health() -> Result<HealthStatus, TensaError> {
    let config_path = tensa_core::TensaPaths::new().config_file();
    let core = TensaCore::new(&config_path).await?;
    let status = core.health(&version()).await?;
    Ok(HealthStatus {
        healthy: status.healthy,
        version: status.version,
        uptime: status.uptime,
        degraded: status.degraded,
        degraded_reason: status.degraded_reason,
    })
}

// Генерация FFI-glue из UDL (см. build.rs): должен быть последним включением.
// Линт `large_const_arrays`, который свежий clippy находит в СГЕНЕРИРОВАННОМ
// коде uniffi, погашен в манифесте крейта ([lints.clippy]).
uniffi::include_scaffolding!("tensa");
