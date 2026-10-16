//! Демонстрация цикла одобрения Telegram-инструмента:
//! `propose` создаёт действие (видно в PWA «Одобрения» и в боте `/approvals`),
//! `execute` исполняет одобренное (фактическая отправка через Bot API),
//! `bot` запускает long polling (команды и кнопки владельца).
//!
//! Запуск:
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> propose "текст"
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> execute <uuid>
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> bot
//!
//! Требуется настроенный бот (`secrets.toml` или `TELEGRAM_BOT_TOKEN`) —
//! кроме `propose`: он работает с локальной базой одобрений и сети не трогает.

use std::path::PathBuf;
use std::sync::Arc;

use tensa_approval::{ApprovalConfig, SqliteApprovalStore};
use tensa_tools::telegram::TelegramService;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ошибка: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let config_path = PathBuf::from(
        args.next()
            .ok_or("usage: telegram_flow <config.toml> propose \"текст\" | execute <uuid> | bot")?,
    );
    let command = args.next().unwrap_or_else(|| "help".to_string());

    let root = config_path
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let config = tensa_core::Config::load(&config_path)?;
    let store = SqliteApprovalStore::new(
        root.join("approval.db"),
        ApprovalConfig::from(&config.approval),
        config.general.node_id.clone(),
    )
    .await?;
    let service = Arc::new(TelegramService::from_files(&config_path)?);

    match command.as_str() {
        "propose" => {
            let text = args.collect::<Vec<_>>().join(" ");
            let action = service.propose_send(&text, &store).await?;
            println!(
                "создано действие {} (kind={}, статус={}) — смотри PWA «Одобрения» или /approvals в боте",
                action.id,
                action.kind.as_str(),
                action.status.as_str()
            );
        }
        "execute" => {
            let raw_id = args.next().ok_or("execute требует uuid действия")?;
            let id = uuid::Uuid::parse_str(&raw_id)?;
            let action = service.execute_approved(&store, id).await?;
            println!(
                "отправлено владельцу: действие {} → статус {}",
                action.id,
                action.status.as_str()
            );
        }
        "bot" => {
            service.run_polling(Arc::new(store)).await?;
        }
        _ => {
            println!("usage: telegram_flow <config.toml> propose \"текст\" | execute <uuid> | bot");
        }
    }
    Ok(())
}
