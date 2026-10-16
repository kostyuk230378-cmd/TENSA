//! Fallback облака и повторы (ТЗ tensa-router: «бесплатная → платная →
//! локальная; таймаут 30 сек, 2 попытки»).

use std::time::Duration;

use crate::cloud::{CloudBackend, CloudReply};
use crate::config::RouterConfig;
use crate::error::{Result, RouterError};
use crate::models::QueryRequest;

/// Облачная попытка (модель + признак бесплатности).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudAttempt {
    pub model: String,
    pub free: bool,
}

/// План облачных попыток: бесплатные (priority ↑), затем платные (priority ↑).
pub fn plan(config: &RouterConfig) -> Vec<CloudAttempt> {
    let mut free: Vec<_> = config.cloud_models.iter().filter(|m| m.free).collect();
    let mut paid: Vec<_> = config.cloud_models.iter().filter(|m| !m.free).collect();
    free.sort_by_key(|model| model.priority);
    paid.sort_by_key(|model| model.priority);
    free.into_iter()
        .chain(paid)
        .map(|model| CloudAttempt {
            model: model.model.clone(),
            free: model.free,
        })
        .collect()
}

/// Выполнить облачную попытку: таймаут на вызов; бесплатная модель — до
/// `retries + 1` попыток (ТЗ), платная — одна.
///
/// Неверный API-ключ не повторяется: ключ общий для всех моделей.
pub async fn call_with_retries(
    backend: &dyn CloudBackend,
    attempt: &CloudAttempt,
    request: &QueryRequest,
    timeout: Duration,
    retries: u32,
) -> Result<CloudReply> {
    let max_attempts = if attempt.free { retries + 1 } else { 1 };
    let mut last_error: Option<RouterError> = None;

    for _ in 0..max_attempts {
        match tokio::time::timeout(timeout, backend.chat(&attempt.model, request)).await {
            Ok(Ok(reply)) => return Ok(reply),
            Ok(Err(error)) => {
                let fatal = matches!(error, RouterError::ApiKey(_));
                last_error = Some(error);
                if fatal {
                    break;
                }
            }
            Err(_) => {
                last_error = Some(RouterError::Cloud(format!(
                    "таймаут {} c (модель {})",
                    timeout.as_secs(),
                    attempt.model
                )));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| RouterError::Cloud("облако недоступно".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::StubCloudBackend;

    fn config_with_free_and_paid() -> RouterConfig {
        RouterConfig::default()
    }

    #[test]
    fn plan_orders_free_then_paid_by_priority() {
        let mut config = config_with_free_and_paid();
        config.cloud_models = vec![
            crate::config::CloudModelConfig {
                model: "paid-b".to_string(),
                priority: 2,
                free: false,
            },
            crate::config::CloudModelConfig {
                model: "free-b".to_string(),
                priority: 5,
                free: true,
            },
            crate::config::CloudModelConfig {
                model: "free-a".to_string(),
                priority: 1,
                free: true,
            },
        ];
        let plan = plan(&config);
        let order: Vec<&str> = plan.iter().map(|a| a.model.as_str()).collect();
        assert_eq!(order, vec!["free-a", "free-b", "paid-b"]);
    }

    #[tokio::test]
    async fn free_model_retries_then_succeeds() {
        let backend = StubCloudBackend::new(true);
        backend.push(Err(RouterError::Cloud("сеть".to_string())));
        backend.push(Err(RouterError::Cloud("сеть".to_string())));
        backend.push(Ok(CloudReply::text("ок")));

        let attempt = CloudAttempt {
            model: "free".to_string(),
            free: true,
        };
        let reply = call_with_retries(
            &backend,
            &attempt,
            &QueryRequest::text("x"),
            Duration::from_secs(5),
            2,
        )
        .await
        .expect("ответ после повторов");
        assert_eq!(reply.text, "ок");
        assert_eq!(backend.calls().len(), 3);
    }

    #[tokio::test]
    async fn paid_model_called_once() {
        let backend = StubCloudBackend::new(true);
        backend.push(Err(RouterError::Cloud("сеть".to_string())));
        backend.push(Ok(CloudReply::text("не должно быть вызвано")));

        let attempt = CloudAttempt {
            model: "paid".to_string(),
            free: false,
        };
        let error = call_with_retries(
            &backend,
            &attempt,
            &QueryRequest::text("x"),
            Duration::from_secs(5),
            2,
        )
        .await
        .expect_err("ошибка");
        assert!(matches!(error, RouterError::Cloud(_)));
        assert_eq!(backend.calls().len(), 1);
    }

    #[tokio::test]
    async fn api_key_error_is_not_retried() {
        let backend = StubCloudBackend::new(true);
        backend.push(Err(RouterError::ApiKey("401".to_string())));
        backend.push(Ok(CloudReply::text("не должно быть вызвано")));

        let attempt = CloudAttempt {
            model: "free".to_string(),
            free: true,
        };
        let error = call_with_retries(
            &backend,
            &attempt,
            &QueryRequest::text("x"),
            Duration::from_secs(5),
            2,
        )
        .await
        .expect_err("ошибка ключа");
        assert!(matches!(error, RouterError::ApiKey(_)));
        assert_eq!(backend.calls().len(), 1);
    }
}
