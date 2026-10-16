//! `tensa start` — заглушка CLI MVP: постоянный процесс ядра/демон.
//!
//! [derived] Демон появится вместе с apps/desktop и tensa-server
//! (PWA-бэкенд, Контракт 12); CLI работает in-process и состояния между
//! вызовами не хранит.

use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `start`.
pub async fn run(_ctx: &Ctx) -> Result<()> {
    Err(CliError::Unimplemented(
        "start: постоянного процесса ядра в CLI MVP нет — используйте tensa-server (PWA) \
         или apps/desktop; разовые проверки: `tensa health`, `tensa status`"
            .to_string(),
    ))
}
