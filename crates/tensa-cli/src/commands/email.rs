//! `tensa email ...` — заглушка: почтовый клиент — этап tensa-tools (Контракт 6).

use crate::cli::{EmailArgs, EmailCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `email`.
pub async fn run(_ctx: &Ctx, args: &EmailArgs) -> Result<()> {
    let name = match &args.command {
        EmailCommand::Connect { .. } => "connect",
        EmailCommand::Disconnect { .. } => "disconnect",
        EmailCommand::Folders { .. } => "folders",
        EmailCommand::Fetch { .. } => "fetch",
        EmailCommand::Search { .. } => "search",
        EmailCommand::Send { .. } => "send",
    };
    Err(CliError::Unimplemented(format!(
        "email {name}: почтовый клиент — этап tensa-tools (Контракт 6); \
         отправка пойдёт через ApprovalStore (подтверждение владельца)"
    )))
}
