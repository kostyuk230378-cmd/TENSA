//! # tensa-tools
//!
//! Мета-крейт инструментов секретаря (ТЗ tensa-tools): общий трейт [`Tool`],
//! реестр [`ToolRegistry`] и сами инструменты.
//!
//! Реализовано: [`WolTool`] — пробуждение устройства по сети (Wake-on-LAN);
//! [`TelegramTool`]/[`TelegramService`] — бот-секретарь: уведомления владельцу,
//! решения из чата (inline-кнопки), цикл одобрения (propose → PWA/бот → execute);
//! [`EmailTool`]/[`EmailService`] — отправка почты через одобрение (SMTP/lettre,
//! автоопределение провайдеров; чтение IMAP — следующая итерация).
//! По плану ТЗ далее — documents, ocr, calendar, apps, whatsapp.
//!
//! Принципы ТЗ: сетевые действия — только через `ApprovalStore`
//! (централизованно, вне инструмента), поэтому инструмент лишь объявляет
//! [`Tool::requires_approval`]; аудит — в `~/.tensa/logs/tools.log`.
//!
//! [derived] WoL оформлен модулем `wol/` внутри крейта (ТЗ предполагает
//! отдельный подкрейт `crates/tensa-tools/wol`): выделение подкрейтов требует
//! правки корневого `Cargo.toml` воркспейса и делается отдельной задачей.

pub mod audit;
pub mod email;
pub mod error;
pub mod telegram;
pub mod wol;

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

pub use email::{EmailError, EmailService, EmailTool};
pub use error::{Result, ToolsError};
pub use telegram::{TelegramError, TelegramService, TelegramTool};
pub use wol::{MacAddress, WolError, WolParams, WolTool};

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Общий трейт инструмента секретаря (ТЗ tensa-tools).
///
/// Реализации обязаны быть потокобезопасными ([`Send`] + [`Sync`]): реестр
/// хранит их как `Arc<dyn Tool>`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Уникальное имя инструмента — ключ в [`ToolRegistry`].
    fn name(&self) -> &str;

    /// Описание для LLM и интерфейса: что делает инструмент и какие принимает
    /// параметры.
    fn description(&self) -> &str;

    /// Выполнить инструмент с параметрами вызова.
    ///
    /// Реализация сама проверяет параметры и возвращает понятную ошибку
    /// ([`ToolsError`]).
    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value>;

    /// Требуется ли подтверждение владельца перед выполнением.
    fn requires_approval(&self) -> bool;
}

/// Реестр инструментов (ТЗ tensa-tools): имя инструмента → инструмент.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Создать пустой реестр.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Зарегистрировать инструмент. Повторная регистрация под тем же именем
    /// заменяет предыдущий инструмент.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        tracing::debug!(tool = %name, "tools: регистрация инструмента");
        self.tools.insert(name, tool);
    }

    /// Получить инструмент по имени.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Имена всех инструментов (по алфавиту — порядок для UI и тестов).
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Тестовый инструмент с заданным именем.
    struct StubTool {
        name: String,
    }

    #[async_trait]
    impl Tool for StubTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "заглушка для тестов реестра"
        }

        async fn execute(&self, _params: serde_json::Value) -> Result<serde_json::Value> {
            Ok(json!({ "stub": true }))
        }

        fn requires_approval(&self) -> bool {
            false
        }
    }

    fn stub(name: &str) -> Arc<dyn Tool> {
        Arc::new(StubTool {
            name: name.to_string(),
        })
    }

    #[test]
    fn registry_register_get_list() {
        let mut registry = ToolRegistry::new();
        assert!(registry.list().is_empty());

        registry.register(stub("beta"));
        registry.register(stub("alpha"));

        assert_eq!(
            registry.list(),
            vec!["alpha".to_string(), "beta".to_string()]
        );
        assert_eq!(
            registry.get("alpha").map(|tool| tool.name().to_string()),
            Some("alpha".to_string())
        );
        assert_eq!(
            registry.get("beta").map(|tool| tool.name().to_string()),
            Some("beta".to_string())
        );
        assert!(registry.get("gamma").is_none());
    }

    #[test]
    fn registry_register_replaces_same_name() {
        let mut registry = ToolRegistry::new();
        registry.register(stub("same"));
        registry.register(stub("same"));

        assert_eq!(registry.list(), vec!["same".to_string()]);
    }

    #[test]
    fn registry_holds_wol_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(WolTool::new()));

        let tool = registry.get("wol").expect("wol зарегистрирован");
        assert_eq!(tool.name(), "wol");
        assert!(tool.description().contains("Wake-on-LAN"));
        assert!(tool.requires_approval());
    }

    #[tokio::test]
    async fn execute_with_invalid_params_is_err() {
        let tool = WolTool::new();

        // Параметры не объект JSON.
        assert!(tool.execute(json!([1, 2, 3])).await.is_err());
        // Нет обязательного mac.
        assert!(tool.execute(json!({})).await.is_err());
        // Битый MAC, адрес и порт — ошибка до отправки пакета.
        assert!(tool
            .execute(json!({ "mac": "не-mac", "broadcast": "127.0.0.1", "port": 9 }))
            .await
            .is_err());
        assert!(tool
            .execute(json!({ "mac": "AA:BB:CC:DD:EE:FF", "broadcast": "300.1.1.1" }))
            .await
            .is_err());
        assert!(tool
            .execute(json!({ "mac": "AA:BB:CC:DD:EE:FF", "port": 0 }))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn stub_execute_returns_value() {
        let tool = stub("stub");
        let result = tool.execute(json!({})).await.expect("успешное выполнение");
        assert_eq!(result["stub"], json!(true));
        assert!(!tool.requires_approval());
    }
}
