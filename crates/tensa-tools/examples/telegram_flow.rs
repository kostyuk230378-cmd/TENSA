//! Демонстрация цикла одобрения Telegram-инструмента:
//! `propose` создаёт действие (видно в PWA «Одобрения» и в боте `/approvals`),
//! `execute` исполняет одобренное (фактическая отправка через Bot API),
//! `bind` — само-привязка владельца (ждёт первый `/start`, пишет chat id в
//! `secrets.toml`) и сразу запуск бота; `bot` — long polling без привязки.
//!
//! Запуск:
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> propose "текст"
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> execute <uuid>
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> bind
//!   cargo run -p tensa-tools --example telegram_flow -- <config.toml> bot
//!
//! Требуется настроенный бот (`secrets.toml` или `TELEGRAM_BOT_TOKEN`) —
//! кроме `propose`: он работает с локальной базой одобрений и сети не трогает.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tensa_approval::{ApprovalConfig, SqliteApprovalStore};
use tensa_tools::telegram::{TelegramClient, TelegramService};

/// Записать `owner_chat_id` в `secrets.toml` (секция `[telegram]`).
///
/// Простая текстовая правка: файл секретов маленький и стабильной структуры;
/// значение не логируется.
fn save_owner_chat_id(secrets_path: &Path, chat_id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(secrets_path).unwrap_or_default();
    let new_line = format!("owner_chat_id = \"{chat_id}\"");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();

    let mut replaced = false;
    for line in lines.iter_mut() {
        if line.trim_start().starts_with("owner_chat_id") {
            *line = new_line.clone();
            replaced = true;
        }
    }
    if !replaced {
        match lines.iter().position(|line| line.trim() == "[telegram]") {
            Some(index) => lines.insert(index + 1, new_line),
            None => {
                lines.push(String::new());
                lines.push("[telegram]".to_string());
                lines.push(new_line);
            }
        }
    }

    let mut body = lines.join("\n");
    body.push('\n');
    std::fs::write(secrets_path, body)?;
    Ok(())
}

/// Ждать первый `/start` владельца и привязать chat id (само-привязка).
async fn bind_owner(
    client: &TelegramClient,
    secrets_path: &Path,
) -> Result<i64, Box<dyn std::error::Error>> {
    println!("ждём первого сообщения боту (открой t.me и нажми Start)…");
    let mut offset: Option<i64> = None;
    loop {
        // Сетевые сбои не должны убивать привязку — повторяем.
        let updates = match client.get_updates(offset, 25).await {
            Ok(updates) => updates,
            Err(error) => {
                eprintln!("telegram: {error} — повтор через 3 c");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        };
        for update in updates {
            offset = Some(update.update_id + 1);
            if let Some(message) = update.message {
                let chat_id = message.chat.id;
                save_owner_chat_id(secrets_path, chat_id)?;
                client
                    .send_message(
                        &chat_id.to_string(),
                        "Владелец привязан: Tensa на связи.\n/help — команды, /approvals — очередь решений.",
                        None,
                    )
                    .await?;
                println!("владелец привязан: chat_id={chat_id}");
                return Ok(chat_id);
            }
        }
    }
}

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

    match command.as_str() {
        "propose" => {
            let service = TelegramService::from_files(&config_path)?;
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
            let service = TelegramService::from_files(&config_path)?;
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
            let service = Arc::new(TelegramService::from_files(&config_path)?);
            service.run_polling(Arc::new(store)).await?;
        }
        "bind" => {
            // Само-привязка владельца: credential читаем из secrets/окружения,
            // ждём первый /start, пишем chat id, затем — обычный long polling.
            let secrets_path = root.join("secrets.toml");
            let secrets = if secrets_path.exists() {
                tensa_core::Secrets::load(&secrets_path)?
            } else {
                tensa_core::Secrets::default()
            };
            let credential = std::env::var("TELEGRAM_BOT_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| secrets.telegram.credential.clone())
                .ok_or("не задан токен бота")?;
            let client = TelegramClient::new(credential)?;
            bind_owner(&client, &secrets_path).await?;

            // Секреты перечитаются внутри from_files — владелец уже известен.
            let service = Arc::new(TelegramService::from_files(&config_path)?);
            service.run_polling(Arc::new(store)).await?;
        }
        _ => {
            println!("usage: telegram_flow <config.toml> propose \"текст\" | execute <uuid> | bind | bot");
        }
    }
    Ok(())
}
