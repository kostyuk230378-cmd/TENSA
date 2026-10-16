//! Degraded-режим (ТЗ tensa-router: «если всё недоступно — только поиск»).

use crate::models::QueryResponse;

/// Ответ деградированного режима: без генерации, с причиной; поиск
/// выполняет вызывающий (память — Lance, вне ответственности роутера).
pub fn degraded_response(reason: &str) -> QueryResponse {
    QueryResponse {
        text: String::new(),
        action: None,
        action_payload: None,
        tool_calls: Vec::new(),
        reasoning: Some(format!("degraded: только поиск ({reason})")),
        degraded: true,
        model_used: "none".to_string(),
        node_used: String::new(),
        tokens_used: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degraded_response_has_no_generation() {
        let response = degraded_response("нет моделей");
        assert!(response.degraded);
        assert!(response.text.is_empty());
        assert_eq!(response.model_used, "none");
        assert!(response.reasoning.expect("причина").contains("нет моделей"));
    }
}
