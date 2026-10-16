//! Inline-кнопки решений в Telegram (ТЗ tensa-tools: `telegram/src/callbacks.rs`).
//!
//! Владелец может подтверждать/отклонять действия прямо из чата: под списком
//! ожидающих действий бот прикладывает клавиатуру с `callback_data` вида
//! `approve:<uuid>` / `reject:<uuid>` — здесь живёт кодирование и разбор этой
//! схемы (единственный источник истины для handlers).

use serde_json::{json, Value};
use uuid::Uuid;

/// Префикс подтверждения.
pub const APPROVE_PREFIX: &str = "approve:";

/// Префикс отклонения.
pub const REJECT_PREFIX: &str = "reject:";

/// Решение владельца из inline-кнопки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Подтвердить действие.
    Approve,
    /// Отклонить действие.
    Reject,
}

/// Собрать `callback_data` для решения по действию.
pub fn encode(decision: Decision, action_id: &Uuid) -> String {
    let prefix = match decision {
        Decision::Approve => APPROVE_PREFIX,
        Decision::Reject => REJECT_PREFIX,
    };
    format!("{prefix}{action_id}")
}

/// Разобрать `callback_data`; `None` — чужая или повреждённая схема.
pub fn decode(data: &str) -> Option<(Decision, Uuid)> {
    let (decision, raw_id) = if let Some(raw_id) = data.strip_prefix(APPROVE_PREFIX) {
        (Decision::Approve, raw_id)
    } else {
        (Decision::Reject, data.strip_prefix(REJECT_PREFIX)?)
    };
    Uuid::parse_str(raw_id).ok().map(|id| (decision, id))
}

/// Клавиатура «Подтвердить/Отклонить» под сообщением о действии.
pub fn keyboard(action_id: &Uuid) -> Value {
    json!({
        "inline_keyboard": [[
            { "text": "✔ Подтвердить", "callback_data": encode(Decision::Approve, action_id) },
            { "text": "✖ Отклонить", "callback_data": encode(Decision::Reject, action_id) },
        ]]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_both_decisions() {
        let id = Uuid::new_v4();
        let approve = encode(Decision::Approve, &id);
        assert!(approve.starts_with(APPROVE_PREFIX));
        assert_eq!(decode(&approve), Some((Decision::Approve, id)));

        let reject = encode(Decision::Reject, &id);
        assert_eq!(decode(&reject), Some((Decision::Reject, id)));
    }

    #[test]
    fn decode_rejects_foreign_and_broken_data() {
        assert_eq!(decode("hello"), None);
        assert_eq!(decode("approve:"), None);
        assert_eq!(decode("approve:not-a-uuid"), None);
        assert_eq!(decode("delete:550e8400-e29b-41d4-a716-446655440000"), None);
    }

    #[test]
    fn keyboard_contains_both_buttons() {
        let id = Uuid::new_v4();
        let keyboard = keyboard(&id);
        assert_eq!(
            keyboard["inline_keyboard"][0].as_array().map(Vec::len),
            Some(2)
        );
        assert!(keyboard["inline_keyboard"][0][0]["callback_data"]
            .as_str()
            .is_some_and(|data| data.starts_with(APPROVE_PREFIX)));
    }
}
