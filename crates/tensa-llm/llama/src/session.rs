//! Сессия чата: окно сообщений и подгонка под бюджет контекста.
//! Движок не хранит историю (ограничение ТЗ: «не хранить историю») —
//! сессия лишь нарезает переданное окно под бюджет промпта.

use crate::engine::ChatMessage;

/// Окно чата с бюджетом по символам (≈ токены*4).
#[derive(Debug, Clone)]
pub struct ChatSession {
    /// Бюджет окна в символах.
    pub window_chars: usize,
}

impl Default for ChatSession {
    fn default() -> Self {
        Self {
            // 4096 токенов контекста * ~4 символа (оставляем запас логике).
            window_chars: 12_000,
        }
    }
}

impl ChatSession {
    pub fn new(window_chars: usize) -> Self {
        Self {
            window_chars: window_chars.max(1),
        }
    }

    /// Обрезать окно сообщений до бюджета, сохраняя самые свежие.
    /// Порядок исходных сообщений сохраняется; system-сообщения
    /// (первое с Role::System) сохраняются приоритетно. [derived]
    pub fn trim<'a>(&self, messages: &'a [ChatMessage]) -> Vec<&'a ChatMessage> {
        if messages.is_empty() {
            return Vec::new();
        }
        let mut budget = self.window_chars;
        // System-сообщение (если первое) — приоритетно.
        let has_system = matches!(messages[0].role, crate::engine::Role::System);
        if has_system {
            budget = budget.saturating_sub(messages[0].content.chars().count());
        }
        let mut tail: Vec<&ChatMessage> = Vec::new();
        for (idx, msg) in messages.iter().enumerate().rev() {
            if has_system && idx == 0 {
                continue;
            }
            let len = msg.content.chars().count();
            if len > budget && !tail.is_empty() {
                break;
            }
            budget = budget.saturating_sub(len);
            tail.push(msg);
            if budget == 0 {
                break;
            }
        }
        tail.reverse();
        let mut out = Vec::with_capacity(tail.len() + has_system as usize);
        if has_system {
            out.push(&messages[0]);
        }
        out.extend(tail);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Role;

    fn msg(role: Role, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.into(),
        }
    }

    #[test]
    fn keeps_all_within_budget() {
        let s = ChatSession::new(100);
        let msgs = vec![msg(Role::User, "привет"), msg(Role::Assistant, "привет!")];
        let kept = s.trim(&msgs);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn drops_oldest_first() {
        let s = ChatSession::new(10);
        let msgs = vec![
            msg(Role::User, "старое длинное сообщение"),
            msg(Role::Assistant, "среднее"),
            msg(Role::User, "новое"),
        ];
        let kept = s.trim(&msgs);
        // Последнее («новое») обязано остаться.
        assert!(kept.iter().any(|m| m.content == "новое"));
        assert!(kept.len() < msgs.len());
    }

    #[test]
    fn system_message_kept_prioritized() {
        let s = ChatSession::new(12);
        let msgs = vec![
            msg(Role::System, "система"),
            msg(Role::User, "очень длинное сообщение пользователя"),
            msg(Role::User, "к"),
        ];
        let kept = s.trim(&msgs);
        assert_eq!(kept[0].role, Role::System);
        assert!(kept.iter().any(|m| m.content == "к"));
    }

    #[test]
    fn empty_input() {
        let s = ChatSession::default();
        assert!(s.trim(&[]).is_empty());
    }

    #[test]
    fn single_huge_message_is_kept() {
        // Даже превышающее бюджет последнее сообщение сохраняется
        // (движок сам вернёт Length при генерации).
        let s = ChatSession::new(5);
        let msgs = vec![msg(Role::User, "очень-очень длинное сообщение")];
        let kept = s.trim(&msgs);
        assert_eq!(kept.len(), 1);
    }
}
