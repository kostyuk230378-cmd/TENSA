//! `tensa calendar ...` — заглушка: календарь — этап tensa-tools (CalDAV).

use crate::cli::{CalendarArgs, CalendarCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `calendar`.
pub async fn run(_ctx: &Ctx, args: &CalendarArgs) -> Result<()> {
    let name = match &args.command {
        CalendarCommand::List { .. } => "list",
        CalendarCommand::Conflicts => "conflicts",
        CalendarCommand::Create { .. } => "create",
        CalendarCommand::Delete { .. } => "delete",
    };
    Err(CliError::Unimplemented(format!(
        "calendar {name}: календарь (CalDAV-клиент) — этап tensa-tools; \
         изменения пойдут через ApprovalStore"
    )))
}
