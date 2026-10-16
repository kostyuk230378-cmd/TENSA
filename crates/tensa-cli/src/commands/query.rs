//! `tensa query` — заглушка: диалог с моделями — этап проводки router/llm в CLI.

use crate::cli::QueryArgs;
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `query`.
pub async fn run(_ctx: &Ctx, _args: &QueryArgs) -> Result<()> {
    Err(CliError::Unimplemented(
        "query: роутер и локальный инференс реализованы в крейтах (tensa-router/llm), \
         но CLI-проводка (нативная сборка с --features native и загрузка модели) — в очереди"
            .to_string(),
    ))
}
