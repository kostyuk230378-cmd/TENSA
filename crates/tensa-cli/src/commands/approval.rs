//! `tensa approval list/show/approve/reject` (Контракт 1: очередь одобрения).

use tensa_approval::{Action, ActionOrigin, ApprovalStore};

use crate::cli::{ApprovalArgs, ApprovalCommand};
use crate::commands::{enum_name, short, ts, Ctx};
use crate::error::{parse_id, CliError, Result};

/// Выполнить `approval`.
pub async fn run(ctx: &Ctx, args: &ApprovalArgs) -> Result<()> {
    let store = ctx.approvals().await?;
    match &args.command {
        ApprovalCommand::List => {
            let pending = store.get_pending().await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&pending);
            }
            if pending.is_empty() {
                ctx.output.message("ожидающих действий нет");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = pending.iter().map(action_row).collect();
            ctx.output
                .table(&["id", "вид", "статус", "создано", "payload"], &rows);
        }
        ApprovalCommand::Show { id } => {
            let action = load(&store, id).await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&action);
            }
            let payload = serde_json::to_string_pretty(&action.payload)
                .unwrap_or_else(|_| action.payload.to_string());
            ctx.output.key_values(&[
                ("id".to_string(), action.id.to_string()),
                ("вид".to_string(), enum_name(&action.kind)),
                ("статус".to_string(), enum_name(&action.status)),
                ("создано".to_string(), ts(action.created_at)),
                ("истекает".to_string(), ts(action.expires_at)),
                ("инициатор".to_string(), enum_name(&action.created_by)),
                ("payload".to_string(), payload),
            ]);
        }
        ApprovalCommand::Approve { id, by } => {
            let action = store.approve(parse_id(id)?, origin(by)).await?;
            report(ctx, "подтверждено", &action)?;
        }
        ApprovalCommand::Reject { id, by } => {
            let action = store.reject(parse_id(id)?, origin(by)).await?;
            report(ctx, "отклонено", &action)?;
        }
    }
    Ok(())
}

/// Загрузить действие по строковому id или вернуть понятную ошибку.
async fn load(store: &dyn ApprovalStore, raw: &str) -> Result<Action> {
    let id = parse_id(raw)?;
    store
        .get_by_id(id)
        .await?
        .ok_or_else(|| CliError::Message(format!("действие {id} не найдено")))
}

/// Строка таблицы списка.
fn action_row(action: &Action) -> Vec<String> {
    vec![
        action.id.to_string(),
        enum_name(&action.kind),
        enum_name(&action.status),
        ts(action.created_at),
        short(&action.payload.to_string(), 48),
    ]
}

/// Инициатор решения из `--by` (неизвестное значение — user: решение из UI).
fn origin(raw: &str) -> ActionOrigin {
    raw.trim().parse().unwrap_or(ActionOrigin::User)
}

/// Отчёт о решении по действию.
fn report(ctx: &Ctx, verb: &str, action: &Action) -> Result<()> {
    if ctx.output.json_mode() {
        return ctx.output.json(action);
    }
    ctx.output.success(format!(
        "{verb}: {} (статус: {})",
        action.id,
        enum_name(&action.status)
    ));
    Ok(())
}
