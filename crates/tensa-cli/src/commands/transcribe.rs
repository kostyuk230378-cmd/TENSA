//! `tensa transcribe` — заглушка: распознавание речи — этап tensa-tools/voice.

use crate::cli::TranscribeArgs;
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `transcribe`.
pub async fn run(_ctx: &Ctx, _args: &TranscribeArgs) -> Result<()> {
    Err(CliError::Unimplemented(
        "transcribe: распознавание речи (ggml-модель) — этап tensa-tools/voice; \
         формат вызова зафиксирован (путь к аудиофайлу)"
            .to_string(),
    ))
}
