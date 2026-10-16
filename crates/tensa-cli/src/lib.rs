//! tensa-cli: CLI для отладки и управления Tensa (ТЗ tensa-cli).
//!
//! Принципы (ТЗ): логика живёт в крейтах, CLI только вызывает; состояние
//! не хранится (каждый запуск — новый процесс, in-process доступ к ядру);
//! секреты не логируются.
//!
//! Точка входа — бинарь `tensa` (`src/main.rs`); модули:
//! [`cli`] (разбор аргументов), [`commands`] (реализация команд),
//! [`output`] (цвет/таблицы/JSON/тихий режим), [`error`] (ошибки).

pub mod cli;
pub mod commands;
pub mod error;
pub mod output;

pub use error::{CliError, Result};

/// Версия крейта (для `tensa version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
