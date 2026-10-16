//! Конфигурация ApprovalStore (ТЗ tensa-approval, раздел «Конфигурация»).

/// Настройки очереди одобрения. Значения по умолчанию — из ТЗ.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApprovalConfig {
    /// TTL действия в часах (по умолчанию 24).
    pub action_ttl_hours: u64,
    /// Автоматически одобрять trusted origin (по умолчанию false;
    /// принцип Контракта 1: авто-ответы запрещены).
    pub auto_approve_trusted: bool,
    /// Разрешить отправку почты.
    pub allow_email_send: bool,
    /// Разрешить отправку в Telegram.
    pub allow_telegram_send: bool,
    /// Разрешить отправку в WhatsApp (по умолчанию false).
    pub allow_whatsapp_send: bool,
    /// Требовать подтверждение незнакомых получателей.
    pub confirm_unknown_recipients: bool,
    /// Максимум одновременно ожидающих действий.
    pub max_pending_actions: usize,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            action_ttl_hours: 24,
            auto_approve_trusted: false,
            allow_email_send: true,
            allow_telegram_send: true,
            allow_whatsapp_send: false,
            confirm_unknown_recipients: true,
            max_pending_actions: 1000,
        }
    }
}
