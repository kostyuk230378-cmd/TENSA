//! `tensa health` — проверка здоровья ядра (ТЗ tensa-core: `HealthStatus`).

use tensa_core::TensaCore;

use crate::commands::Ctx;
use crate::error::Result;
use crate::VERSION;

/// Выполнить `health`.
pub async fn run(ctx: &Ctx) -> Result<()> {
    let core = TensaCore::new(&ctx.config_path).await?;
    let status = core.health(VERSION).await?;

    if ctx.output.json_mode() {
        return ctx.output.json(&status);
    }

    let head = if status.healthy {
        "здоров"
    } else {
        "деградация/не запущено"
    };
    ctx.output.message(format!(
        "состояние: {head} | версия {} | uptime {}",
        status.version, status.uptime
    ));
    if let Some(reason) = &status.degraded_reason {
        ctx.output.warning(format!("причина: {reason}"));
    }

    let mut names: Vec<&String> = status.components.keys().collect();
    names.sort();
    let rows: Vec<Vec<String>> = names
        .iter()
        .filter_map(|name| {
            status.components.get(*name).map(|component| {
                vec![
                    name.to_string(),
                    if component.healthy { "ok" } else { "fail" }.to_string(),
                    component.detail.clone().unwrap_or_default(),
                ]
            })
        })
        .collect();
    ctx.output.table(&["компонент", "статус", "детали"], &rows);
    Ok(())
}
