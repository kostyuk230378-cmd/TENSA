//! REST API клиент Syncthing (ADR-011, ADR-018; фича `syncthing`).
//!
//! Только мониторинг и базовые операции; конфигурация при первом запуске —
//! отдельная задача. API-ключ не логируется (ограничение ТЗ).

use crate::error::{Result, SyncError};
use crate::status::SyncthingStatus;

/// Клиент REST API Syncthing.
pub struct SyncthingClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl SyncthingClient {
    /// Создать клиент. `api_key` не логируется.
    pub fn new(host: &str, port: u16, api_key: &str) -> Result<Self> {
        if api_key.is_empty() {
            return Err(SyncError::Config("syncthing api key is empty".into()));
        }
        Ok(Self {
            base_url: format!("http://{host}:{port}"),
            api_key: api_key.to_string(),
            http: reqwest::Client::new(),
        })
    }

    /// GET /rest/system/status — жив ли Syncthing.
    pub async fn system_status(&self) -> Result<bool> {
        let url = format!("{}/rest/system/status", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SyncError::Syncthing(format!("request failed: {e}")))?;
        Ok(resp.status().is_success())
    }

    /// GET /rest/system/connections — число подключённых устройств.
    pub async fn connections_count(&self) -> Result<usize> {
        let url = format!("{}/rest/system/connections", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SyncError::Syncthing(format!("request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(SyncError::Syncthing(format!("status {}", resp.status())));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyncError::Syncthing(format!("bad json: {e}")))?;
        let count = body
            .get("connections")
            .and_then(|c| c.as_object())
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(count)
    }

    /// Собрать статус для SyncStatus.
    pub async fn status(&self) -> SyncthingStatus {
        match self.system_status().await {
            Ok(true) => {
                let connections = self.connections_count().await.unwrap_or(0);
                SyncthingStatus {
                    running: true,
                    connections,
                    last_error: None,
                }
            }
            Ok(false) => SyncthingStatus {
                running: false,
                connections: 0,
                last_error: Some("system/status returned failure".into()),
            },
            Err(e) => SyncthingStatus {
                running: false,
                connections: 0,
                last_error: Some(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_key_rejected() {
        assert!(SyncthingClient::new("127.0.0.1", 8384, "").is_err());
    }

    #[test]
    fn client_builds() {
        let c = SyncthingClient::new("127.0.0.1", 8384, "k").expect("client");
        assert_eq!(c.base_url, "http://127.0.0.1:8384");
    }
}
