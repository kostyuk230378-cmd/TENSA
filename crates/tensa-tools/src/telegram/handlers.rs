//! Обработчики обновлений Telegram (ТЗ tensa-tools: `telegram/src/handlers.rs`).
//!
//! Правила (Контракт 1 и ТЗ): команды принимаются только из чата владельца;
//! решения из Telegram идут с origin `Telegram` (trusted-источник: без
//! дополнительного одобрения, но с аудитом); действия от LLM всегда проходят
//! через `ApprovalStore`.

use tensa_approval::{ActionOrigin, ApprovalStore};

use super::bot::{CallbackQuery, Update};
use super::callbacks::{self, Decision};
use super::error::TelegramError;
use super::TelegramService;
use crate::audit;
use crate::email::EmailService;

/// Справка бота.
const HELP_TEXT: &str = "Tensa — секретарь на связи.\n\
Команды:\n\
/status — состояние узла и очередь решений\n\
/approvals — ожидающие решения действия (с кнопками)\n\
/approve <id> — подтвердить действие\n\
/reject <id> — отклонить действие\n\
/help — эта справка";

/// Максимум действий в выдаче `/approvals` (защита от флуда).
const APPROVALS_LIMIT: usize = 5;

/// Обработать одно обновление.
pub async fn handle_update(
    service: &TelegramService,
    store: &dyn ApprovalStore,
    update: Update,
) -> Result<(), TelegramError> {
    if let Some(callback) = update.callback_query {
        return handle_callback(service, store, callback).await;
    }
    if let Some(message) = update.message {
        if !service.is_owner(message.chat.id) {
            audit::record(
                service.root(),
                super::NAME,
                &format!("message from foreign chat {}", message.chat.id),
                "denied",
            );
            tracing::warn!(
                chat = message.chat.id,
                "telegram: сообщение из чужого чата — отклонено"
            );
            return Ok(());
        }
        if let Some(text) = message.text {
            return handle_command(service, store, &text).await;
        }
    }
    Ok(())
}

/// Ответ на команду владельца.
async fn handle_command(
    service: &TelegramService,
    store: &dyn ApprovalStore,
    text: &str,
) -> Result<(), TelegramError> {
    let mut parts = text.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let argument = parts.next();

    match command {
        "/start" | "/help" => {
            audit::record(service.root(), super::NAME, "command help", "ok");
            service.send_to_owner(HELP_TEXT).await?;
        }
        "/status" => {
            let pending = store.get_pending().await?;
            audit::record(service.root(), super::NAME, "command status", "ok");
            service
                .send_to_owner(&format!(
                    "{}\nОжидают решения: {}\nПодробно с кнопками: /approvals",
                    service.status_line(),
                    pending.len()
                ))
                .await?;
        }
        "/approvals" => {
            let pending = store.get_pending().await?;
            audit::record(service.root(), super::NAME, "command approvals", "ok");
            if pending.is_empty() {
                service.send_to_owner("Ожидающих решений нет.").await?;
                return Ok(());
            }
            service
                .send_to_owner(&format!("Ожидают решения: {}", pending.len()))
                .await?;
            for action in pending.iter().take(APPROVALS_LIMIT) {
                let payload = action.payload.to_string();
                let short = if payload.chars().count() > 160 {
                    let mut text: String = payload.chars().take(160).collect();
                    text.push('…');
                    text
                } else {
                    payload
                };
                let caption = format!(
                    "{} · {}\n{}\nid: {}",
                    action.kind.as_str(),
                    action.status.as_str(),
                    short,
                    action.id
                );
                service
                    .client()
                    .send_message(
                        service.owner_chat_id(),
                        &caption,
                        Some(callbacks::keyboard(&action.id)),
                    )
                    .await?;
            }
        }
        "/approve" => decide_by_command(service, store, argument, Decision::Approve).await?,
        "/reject" => decide_by_command(service, store, argument, Decision::Reject).await?,
        other => {
            // Естественные формулировки владельца: «Утверждаю»/«Отклоняю»
            // работают без команд (владелец — trusted-источник, Контракт 1).
            if let Some(decision) = approval_intent(other) {
                decide_single_pending(service, store, decision).await?;
            } else {
                // Свободный текст: обработка запросов — с этапом роутера (LLM).
                audit::record(
                    service.root(),
                    super::NAME,
                    &format!("message len={}", other.chars().count()),
                    "accepted",
                );
                service
                    .send_to_owner(
                        "Принял. Обработка запросов появится с этапом роутера (LLM); \
                         сейчас доступны: /status, /approvals, /approve, /reject, /help; \
                         «Утверждаю»/«Отклоняю» работают и без команд.",
                    )
                    .await?;
            }
        }
    }
    Ok(())
}

/// Решение по действию из текстовой команды.
async fn decide_by_command(
    service: &TelegramService,
    store: &dyn ApprovalStore,
    argument: Option<&str>,
    decision: Decision,
) -> Result<(), TelegramError> {
    let Some(raw_id) = argument else {
        service
            .send_to_owner("Укажите id действия: /approve <id> или /reject <id>")
            .await?;
        return Ok(());
    };
    let Some((_, id)) = callbacks::decode(&format!(
        "{}{raw_id}",
        match decision {
            Decision::Approve => callbacks::APPROVE_PREFIX,
            Decision::Reject => callbacks::REJECT_PREFIX,
        }
    )) else {
        service
            .send_to_owner("Не разобрал id действия (нужен UUID).")
            .await?;
        return Ok(());
    };
    apply_decision(service, store, decision, id, None).await
}

/// Естественно-языковое намерение решения владельца (без команд).
///
/// Принимаем только явные формулировки: одно неосторожное слово не должно
/// подтверждать LLM-действия (простое «ок» намеренно не считается).
fn approval_intent(text: &str) -> Option<Decision> {
    let lower = text.to_lowercase();
    if lower.contains("утвержд") || lower.contains("подтвержд") || lower.contains("approve")
    {
        return Some(Decision::Approve);
    }
    if lower.contains("отклон") || lower.contains("отказ") || lower.contains("reject") {
        return Some(Decision::Reject);
    }
    None
}

/// Решить единственное ожидающее действие; при 0/нескольких — подсказать.
async fn decide_single_pending(
    service: &TelegramService,
    store: &dyn ApprovalStore,
    decision: Decision,
) -> Result<(), TelegramError> {
    let pending = store.get_pending().await?;
    match pending.len() {
        0 => {
            service
                .send_to_owner("Очередь решений пуста — подтверждать нечего.")
                .await?;
        }
        1 => {
            let id = pending[0].id;
            apply_decision(service, store, decision, id, None).await?;
        }
        count => {
            service
                .send_to_owner(&format!(
                    "Ожидают решения: {count}. Уточните: /approvals (кнопки) или /approve <id>"
                ))
                .await?;
        }
    }
    Ok(())
}

/// Ответ на нажатие inline-кнопки.
async fn handle_callback(
    service: &TelegramService,
    store: &dyn ApprovalStore,
    callback: CallbackQuery,
) -> Result<(), TelegramError> {
    if let Some(message) = &callback.message {
        if !service.is_owner(message.chat.id) {
            audit::record(
                service.root(),
                super::NAME,
                "callback from foreign chat",
                "denied",
            );
            service
                .client()
                .answer_callback_query(&callback.id, Some("Нет доступа"))
                .await?;
            return Ok(());
        }
    }

    let Some(data) = callback.data.as_deref() else {
        service
            .client()
            .answer_callback_query(&callback.id, Some("Пустые данные кнопки"))
            .await?;
        return Ok(());
    };
    let Some((decision, id)) = callbacks::decode(data) else {
        service
            .client()
            .answer_callback_query(&callback.id, Some("Неизвестная схема кнопки"))
            .await?;
        return Ok(());
    };

    // message_id нужен, чтобы заменить сообщение результатом решения.
    let message_id = callback.message.as_ref().map(|message| message.message_id);
    apply_decision(service, store, decision, id, message_id).await?;
    service
        .client()
        .answer_callback_query(
            &callback.id,
            Some(match decision {
                Decision::Approve => "Подтверждено",
                Decision::Reject => "Отклонено",
            }),
        )
        .await?;
    Ok(())
}

/// Общее применение решения: очередь + правка сообщения + аудит.
async fn apply_decision(
    service: &TelegramService,
    store: &dyn ApprovalStore,
    decision: Decision,
    id: uuid::Uuid,
    message_id: Option<i64>,
) -> Result<(), TelegramError> {
    let result = match decision {
        Decision::Approve => store.approve(id, ActionOrigin::Telegram).await,
        Decision::Reject => store.reject(id, ActionOrigin::Telegram).await,
    };

    match result {
        Ok(action) => {
            audit::record(
                service.root(),
                super::NAME,
                &format!(
                    "{} {}",
                    match decision {
                        Decision::Approve => "approve",
                        Decision::Reject => "reject",
                    },
                    action.id
                ),
                "ok",
            );
            let verb = match decision {
                Decision::Approve => "✔ Подтверждено",
                Decision::Reject => "✖ Отклонено",
            };
            let mut text = format!(
                "{verb} владельцем (telegram)\nстатус: {}",
                action.status.as_str()
            );

            // Одобренную telegram-отправку исполняем сразу — цикл закрывается
            // в один клик: propose → ✔ → сообщение ушло.
            if matches!(decision, Decision::Approve) && action.kind.as_str() == "telegram_send" {
                match service.execute_approved(store, action.id).await {
                    Ok(executed) => {
                        text = format!(
                            "{verb} и исполнено (telegram)\nстатус: {}",
                            executed.status.as_str()
                        );
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "telegram: исполнение одобренного не удалось");
                    }
                }
            }

            // Одобренную отправку почты тоже пробуем исполнить сразу:
            // без креденшелов действие останется approved и исполнится позже.
            if matches!(decision, Decision::Approve) && action.kind.as_str() == "email_send" {
                match EmailService::from_files(service.config_path()) {
                    Ok(email_service) => {
                        match email_service.execute_approved(store, action.id).await {
                            Ok(executed) => {
                                text = format!(
                                    "{verb} и письмо отправлено\nстатус: {}",
                                    executed.status.as_str()
                                );
                            }
                            Err(error) => {
                                tracing::warn!(error = %error, "telegram: отправка письма не удалась");
                                text = format!(
                                    "{verb}: действие approved, но отправка не удалась ({error})"
                                );
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "telegram: сервис почты недоступен");
                        text = format!("{verb}: approved; сервис почты недоступен ({error})");
                    }
                }
            }
            match message_id {
                Some(message_id) => {
                    let _ = service
                        .client()
                        .edit_message_text(service.owner_chat_id(), message_id, &text)
                        .await;
                }
                None => service.send_to_owner(&text).await?,
            }
        }
        Err(error) => {
            audit::record(
                service.root(),
                super::NAME,
                &format!("decision {id}"),
                "denied",
            );
            let text = format!("Не получилось: {error}");
            match message_id {
                Some(message_id) => {
                    let _ = service
                        .client()
                        .edit_message_text(service.owner_chat_id(), message_id, &text)
                        .await;
                }
                None => service.send_to_owner(&text).await?,
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_approve_keywords() {
        for text in ["Утверждаю", "подтверждаю!", "APPROVE this", "да, утверждаю"]
        {
            assert_eq!(
                approval_intent(text),
                Some(Decision::Approve),
                "текст: {text}"
            );
        }
    }

    #[test]
    fn intent_reject_keywords() {
        for text in ["Отклоняю", "отказ", "please reject"] {
            assert_eq!(
                approval_intent(text),
                Some(Decision::Reject),
                "текст: {text}"
            );
        }
    }

    #[test]
    fn plain_ok_is_not_an_intent() {
        // Одно неосторожное «ок» не должно подтверждать LLM-действия.
        assert_eq!(approval_intent("ок"), None);
        assert_eq!(approval_intent("как дела?"), None);
    }
}
