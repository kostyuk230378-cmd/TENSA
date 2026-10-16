//! `tensa contacts create/list/search/delete` (Контракт 3: контакты).

use tensa_core::{ContactsStore, NewContact};

use crate::cli::{ContactsArgs, ContactsCommand};
use crate::commands::{short, Ctx};
use crate::error::{parse_id, Result};

/// Выполнить `contacts`.
pub async fn run(ctx: &Ctx, args: &ContactsArgs) -> Result<()> {
    let store = ctx.contacts().await?;
    match &args.command {
        ContactsCommand::Create {
            name,
            email,
            phone,
            tags,
        } => {
            let draft = NewContact {
                name: name.clone(),
                email: email.clone(),
                phone: phone.clone(),
                telegram: None,
                whatsapp: None,
                notes: None,
                tags: tags.clone(),
            };
            let contact = store.create_contact(draft).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&contact);
            }
            ctx.output
                .success(format!("контакт создан: {}", contact.id));
            ctx.output
                .key_values(&[("имя".to_string(), contact.name.clone())]);
        }
        ContactsCommand::List { limit, offset } => {
            let contacts = store.list_contacts(*limit, *offset).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&contacts);
            }
            if contacts.is_empty() {
                ctx.output.message("контактов нет");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = contacts.iter().map(contact_row).collect();
            ctx.output
                .table(&["id", "имя", "почта", "телефон", "теги"], &rows);
        }
        ContactsCommand::Search { query, limit } => {
            let contacts = store.search_contacts(query, *limit).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&contacts);
            }
            if contacts.is_empty() {
                ctx.output.message("ничего не найдено");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = contacts.iter().map(contact_row).collect();
            ctx.output
                .table(&["id", "имя", "почта", "телефон", "теги"], &rows);
        }
        ContactsCommand::Delete { id } => {
            let id = parse_id(id)?;
            store.delete_contact(id).await?;
            if ctx.output.json_mode() {
                return ctx
                    .output
                    .json(&serde_json::json!({ "deleted": id.to_string() }));
            }
            ctx.output.success(format!("контакт удалён: {id}"));
        }
    }
    Ok(())
}

/// Строка таблицы контактов.
fn contact_row(contact: &tensa_core::Contact) -> Vec<String> {
    vec![
        contact.id.to_string(),
        short(&contact.name, 32),
        contact.email.clone().unwrap_or_default(),
        contact.phone.clone().unwrap_or_default(),
        contact.tags.join(", "),
    ]
}
