//! `tensa memory index/reindex/delete/search/list` — заглушки CLI MVP.
//!
//! [derived] Векторный поиск и эмбеддер реализованы в tensa-memory; проводка
//! в CLI (открытие DefaultMemoryManager с эмбеддером) — отдельная задача
//! этапа status/apps. Команды объявлены по ТЗ, чтобы клиенты видели
//! полное дерево и получали честное «ещё не готово».

use crate::cli::{MemoryArgs, MemoryCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `memory`.
pub async fn run(_ctx: &Ctx, args: &MemoryArgs) -> Result<()> {
    let name = match &args.command {
        MemoryCommand::Index { .. } => "index",
        MemoryCommand::Reindex => "reindex",
        MemoryCommand::Delete { .. } => "delete",
        MemoryCommand::Search { .. } => "search",
        MemoryCommand::List => "list",
    };
    Err(CliError::Unimplemented(format!(
        "memory {name}: проводка tensa-memory в CLI — в очереди; сейчас в CLI доступны \
         notes/contacts/approval/nodes/config/benchmark"
    )))
}
