//! Демонстрация цикла одобрения для почты: propose → approve (PWA/бот) → execute.
//!
//! Запуск:
//!   cargo run -p tensa-tools --example email_flow -- <config.toml> accounts
//!   cargo run -p tensa-tools --example email_flow -- <config.toml> propose <to> [subject] [body...]
//!   cargo run -p tensa-tools --example email_flow -- <config.toml> execute <uuid>
//!
//! `propose` работает офлайн (только база одобрений); `execute` реально
//! отправляет письмо через SMTP — только для действия в статусе `approved`
//! и при наличии креденшелов (secrets.toml / SMTP_PASSWORD).

use std::path::PathBuf;

use tensa_approval::{ApprovalConfig, SqliteApprovalStore};
use tensa_tools::email::{EmailService, OutgoingEmail};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("ошибка: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let config_path = PathBuf::from(args.next().ok_or(
        "usage: email_flow <config.toml> accounts | propose <to> [subject] [body...] | execute <uuid>",
    )?);
    let command = args.next().unwrap_or_else(|| "accounts".to_string());

    let root = config_path
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let config = if config_path.exists() {
        tensa_core::Config::load(&config_path)?
    } else {
        tensa_core::Config::default()
    };
    let store = SqliteApprovalStore::new(
        root.join("approval.db"),
        ApprovalConfig::from(&config.approval),
        config.general.node_id.clone(),
    )
    .await?;
    let service = EmailService::from_files(&config_path)?;

    match command.as_str() {
        "accounts" => {
            let accounts = service.accounts();
            if accounts.is_empty() {
                println!(
                    "в config.toml нет [email.accounts] (или домены не распознаны — задайте хосты явно)"
                );
            }
            for account in accounts {
                println!(
                    "{}: {} → SMTP {}:{} (imap {}:{})",
                    account.id,
                    account.email,
                    account.smtp_host,
                    account.smtp_port,
                    account.imap_host,
                    account.imap_port
                );
            }
        }
        "propose" => {
            let to = args.next().ok_or("нужен адрес получателя")?;
            let subject = args.next().unwrap_or_default();
            let body = args.collect::<Vec<_>>().join(" ");
            let action = service
                .propose_send(
                    OutgoingEmail {
                        account_id: None,
                        to,
                        cc: None,
                        bcc: None,
                        subject,
                        body,
                        attachment: None,
                    },
                    &store,
                )
                .await?;
            println!(
                "создано действие {} (kind={}, статус={}) — одобрить в PWA «Одобрения» или в боте",
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
                "письмо отправлено: действие {} → статус {}",
                action.id,
                action.status.as_str()
            );
        }
        _ => {
            println!("usage: email_flow <config.toml> accounts | propose <to> [subject] [body...] | execute <uuid>");
        }
    }
    Ok(())
}
