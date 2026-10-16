//! `tensa notes create/list/search/delete` (Контракт 3: заметки).

use tensa_core::{NewNote, NotesStore};

use crate::cli::{NotesArgs, NotesCommand};
use crate::commands::{short, ts, Ctx};
use crate::error::{parse_id, Result};

/// Выполнить `notes`.
pub async fn run(ctx: &Ctx, args: &NotesArgs) -> Result<()> {
    let store = ctx.notes().await?;
    match &args.command {
        NotesCommand::Create { title, body, tags } => {
            let draft = NewNote {
                title: title.clone(),
                body: body.clone(),
                tags: tags.clone(),
            };
            let note = store.create_note(draft).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&note);
            }
            ctx.output.success(format!("заметка создана: {}", note.id));
            ctx.output.key_values(&[
                ("заголовок".to_string(), note.title.clone()),
                ("теги".to_string(), note.tags.join(", ")),
            ]);
        }
        NotesCommand::List { limit, offset } => {
            let notes = store.list_notes(*limit, *offset).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&notes);
            }
            if notes.is_empty() {
                ctx.output.message("заметок нет");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = notes
                .iter()
                .map(|note| {
                    vec![
                        note.id.to_string(),
                        short(&note.title, 40),
                        note.tags.join(", "),
                        ts(note.updated_at),
                    ]
                })
                .collect();
            ctx.output
                .table(&["id", "заголовок", "теги", "обновлено"], &rows);
        }
        NotesCommand::Search { query, limit } => {
            let notes = store.search_notes(query, *limit).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&notes);
            }
            if notes.is_empty() {
                ctx.output.message("ничего не найдено");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = notes
                .iter()
                .map(|note| {
                    vec![
                        note.id.to_string(),
                        short(&note.title, 40),
                        ts(note.updated_at),
                    ]
                })
                .collect();
            ctx.output.table(&["id", "заголовок", "обновлено"], &rows);
        }
        NotesCommand::Delete { id } => {
            let id = parse_id(id)?;
            store.delete_note(id).await?;
            if ctx.output.json_mode() {
                return ctx
                    .output
                    .json(&serde_json::json!({ "deleted": id.to_string() }));
            }
            ctx.output.success(format!("заметка удалена: {id}"));
        }
    }
    Ok(())
}
