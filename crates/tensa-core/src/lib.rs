//! # tensa-core
//!
//! Центральный крейт Tensa (ТЗ tensa-core): общие типы, конфигурация
//! (Контракт 4), заметки и контакты (Контракт 3), координатор подсистем,
//! first-run wizard (ADR-021, Контракт 13).
//!
//! Принципы: координация (не дублирование) — core memory и её хранилище
//! живут в `tensa-memory`, здесь только инициализационные параметры.

pub mod config;
pub mod contacts;
pub mod core;
pub mod error;
pub mod first_run;
pub mod health;
pub mod init;
pub mod models;
pub mod notes;
pub mod paths;
pub mod secrets;

pub use config::{
    ApprovalSection, Config, ConfigStore, FileConfigStore, GeneralConfig, SyncSection,
};
pub use contacts::{ContactsStore, SqliteContactsStore};
pub use core::TensaCore;
pub use error::{CoreError, Result};
pub use first_run::{models_to_download, FirstRunWizard, WizardAnswers, WizardResult, WizardStep};
pub use health::{ComponentHealth, HealthStatus};
pub use init::{default_blocks, validate_params, BlockType, InitParams, MemoryInitializer};
pub use models::{
    Contact, DeviceType, EmailAccount, Language, NewContact, NewNote, NodeRole, Note, VectorClock,
    WhatsAppConfig,
};
pub use notes::{NotesStore, SqliteNotesStore};
pub use paths::TensaPaths;
pub use secrets::{
    EmailAccountSecrets, EmailSecrets, OpenRouterSecrets, Secrets, SyncthingSecrets,
    TelegramSecrets,
};

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
