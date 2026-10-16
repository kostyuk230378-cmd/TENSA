//! `tensa stop` — заглушка CLI MVP (см. `start`).

use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `stop`.
pub async fn run(_ctx: &Ctx) -> Result<()> {
    Err(CliError::Unimplemented(
        "stop: демона ядра в CLI MVP нет — останавливать нечего (см. `tensa start`)".to_string(),
    ))
}
