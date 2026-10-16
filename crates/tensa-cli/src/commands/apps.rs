//! `tensa apps ...` — заглушка: инструменты ОС — этап tensa-tools (apps).

use crate::cli::{AppsArgs, AppsCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `apps`.
pub async fn run(_ctx: &Ctx, args: &AppsArgs) -> Result<()> {
    let name = match &args.command {
        AppsCommand::List => "list",
        AppsCommand::Scan => "scan",
        AppsCommand::Launch { .. } => "launch",
        AppsCommand::Open { .. } => "open",
    };
    Err(CliError::Unimplemented(format!(
        "apps {name}: инструменты ОС — этап tensa-tools (apps); \
         запуск приложений/файлов пойдёт через ApprovalStore"
    )))
}
