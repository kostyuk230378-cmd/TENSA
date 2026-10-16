//! `tensa sync ...` — заглушка: обмен между узлами — этап tensa-sync.

use crate::cli::{SyncArgs, SyncCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `sync`.
pub async fn run(_ctx: &Ctx, args: &SyncArgs) -> Result<()> {
    let name = match &args.command {
        SyncCommand::Start => "start",
        SyncCommand::Stop => "stop",
        SyncCommand::Status => "status",
        SyncCommand::Push => "push",
        SyncCommand::Pull => "pull",
    };
    Err(CliError::Unimplemented(format!(
        "sync {name}: синхронизация узлов (gRPC-обмен, векторные часы) — этап tensa-sync; \
         CLI-проводка после него"
    )))
}
