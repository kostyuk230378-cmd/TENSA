//! `tensa version` — версия и окружение сборки.

use crate::commands::Ctx;
use crate::error::Result;
use crate::VERSION;

/// Выполнить `version`.
pub async fn run(ctx: &Ctx) -> Result<()> {
    let native = tensa_llm::NATIVE_ENGINE;
    let default_config = tensa_core::TensaPaths::new().config_file();

    if ctx.output.json_mode() {
        return ctx.output.json(&serde_json::json!({
            "name": "tensa",
            "version": VERSION,
            "native_llm": native,
            "default_config": default_config.display().to_string(),
        }));
    }

    ctx.output.message(format!("tensa {VERSION}"));
    ctx.output.key_values(&[
        (
            "нативный LLM (llama.cpp)".to_string(),
            if native {
                "включён".to_string()
            } else {
                "нет (фича `native` выключена)".to_string()
            },
        ),
        (
            "конфиг по умолчанию".to_string(),
            default_config.display().to_string(),
        ),
    ]);
    Ok(())
}
