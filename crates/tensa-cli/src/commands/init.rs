//! `tensa init` — первый запуск: `config.toml` + `secrets.toml`
//! (доп. ТЗ tensa-core, Контракт 13: мастер из 8 шагов).
//!
//! [derived] CLI проходит мастер неинтерактивно: обязательные шаги
//! (язык, знакомство) заполняются флагами; опциональные шаги
//! (telegram/email/whatsapp/tailscale) — задача приложений, здесь пропущены.

use tensa_core::FirstRunWizard;

use crate::cli::InitArgs;
use crate::commands::Ctx;
use crate::error::Result;

/// Выполнить `init`.
pub async fn run(ctx: &Ctx, args: &InitArgs) -> Result<()> {
    if ctx.config_path.exists() {
        ctx.output.warning(format!(
            "уже инициализировано: {} (init пропущен)",
            ctx.config_path.display()
        ));
        return Ok(());
    }

    ctx.paths.ensure_dirs()?;

    let node_id = uuid::Uuid::new_v4().to_string();
    let wizard = FirstRunWizard::new(
        ctx.config_path.clone(),
        ctx.paths.secrets_file(),
        None, // core memory подключается приложением (обратный трейт, tensa-memory)
        node_id.clone(),
    )
    .await?;

    wizard.step_language(args.language.into()).await?;
    let owner_title = if args.owner_title.trim().is_empty() {
        args.owner_name.clone()
    } else {
        args.owner_title.clone()
    };
    wizard
        .step_introduction(
            args.owner_name.clone(),
            owner_title,
            args.secretary_name.clone(),
            args.timezone.clone(),
        )
        .await?;
    if args.download_models {
        wizard.step_models().await?;
    }
    let result = wizard.step_finish().await?;

    if ctx.output.json_mode() {
        return ctx.output.json(&serde_json::json!({
            "config": ctx.config_path.display().to_string(),
            "secrets": ctx.paths.secrets_file().display().to_string(),
            "node_id": node_id,
            "core_memory_initialized": result.core_memory_initialized,
        }));
    }

    ctx.output.success("ядро инициализировано");
    ctx.output.key_values(&[
        ("config".to_string(), ctx.config_path.display().to_string()),
        (
            "secrets".to_string(),
            ctx.paths.secrets_file().display().to_string(),
        ),
        ("node_id".to_string(), node_id),
        (
            "language".to_string(),
            format!("{:?}", result.config.general.language).to_lowercase(),
        ),
    ]);
    if !result.core_memory_initialized {
        ctx.output
            .warning("core memory не инициализирована: инициализатор подключается приложением");
    }
    Ok(())
}
