//! Контроль версий и автообновление с подтверждением (ADR-003, уровень 3).
//!
//! Шаги ADR: обнаружение → уведомление → подтверждение → скачивание →
//! пересборка → smoke-тесты → применение / откат. Здесь — проверяемая
//! машина состояний и сравнение версий; HTTP-проверка — фича `update-http`.

use crate::error::{LlmError, Result};
use serde::{Deserialize, Serialize};

/// Проверяющий версии: текущая версия + репозиторий (владелец/имя).
#[derive(Debug, Clone)]
pub struct UpdateChecker {
    current: semver::Version,
    repo: String,
}

impl UpdateChecker {
    /// `current` — версия/тег текущей сборки (например, "b4000" → не semver;
    /// тогда используйте числовые теги llama.cpp, см. check_tag).
    pub fn new(current: &str, repo: &str) -> Result<Self> {
        let current = parse_version(current)
            .ok_or_else(|| LlmError::Update(format!("bad current version: {current}")))?;
        if repo.is_empty() {
            return Err(LlmError::Update("repo is empty".into()));
        }
        Ok(Self {
            current,
            repo: repo.to_string(),
        })
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn current(&self) -> &semver::Version {
        &self.current
    }

    /// Есть ли новая версия по тегу удалённой стороны.
    /// Возвращает Some(версия) если тег новее текущей.
    pub fn check_tag(&self, tag: &str) -> Result<Option<semver::Version>> {
        let remote =
            parse_version(tag).ok_or_else(|| LlmError::Update(format!("bad remote tag: {tag}")))?;
        Ok((remote > self.current).then_some(remote))
    }

    /// HTTP-проверка последнего релиза через GitHub API (ADR-003: проверка
    /// при запуске). Фича `update-http`; нативный этап обновления (скачивание,
    /// пересборка, откат) — за [`UpdateFlow`].
    #[cfg(feature = "update-http")]
    pub async fn fetch_latest(&self) -> Result<Option<semver::Version>> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);
        let client = reqwest::Client::builder()
            .user_agent("tensa-llm/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| LlmError::Update(e.to_string()))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| LlmError::Update(format!("request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(LlmError::Update(format!("github status {}", resp.status())));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LlmError::Update(format!("bad json: {e}")))?;
        let tag = body
            .get("tag_name")
            .and_then(|t| t.as_str())
            .ok_or_else(|| LlmError::Update("tag_name missing".into()))?;
        self.check_tag(tag)
    }
}

/// Убрать префикс 'v' и разобрать semver ("v1.2.3" → 1.2.3).
fn parse_version(s: &str) -> Option<semver::Version> {
    let s = s.trim().trim_start_matches('v');
    semver::Version::parse(s).ok()
}

/// Стадии обновления (ADR-003: шаги 1–8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "stage")]
pub enum UpdateStage {
    #[default]
    Idle,
    /// Шаг 1–2: новая версия обнаружена, пользователь уведомлён.
    Detected { from: String, to: String },
    /// Шаг 3: пользователь подтвердил.
    Confirmed,
    /// Шаг 4: новая версия скачана.
    Downloaded,
    /// Шаг 5: пересборка выполнена.
    Rebuilt,
    /// Шаги 6–7: smoke-тесты прошли, обновление применено.
    Applied,
    /// Шаг 8: smoke-тесты не прошли — откат.
    RolledBack { reason: String },
}

/// Машина состояний обновления (ADR-003, шаги 1–8).
/// Переходы строго по порядку; неверный порядок — ошибка.
#[derive(Debug, Default)]
pub struct UpdateFlow {
    stage: UpdateStage,
}

impl UpdateFlow {
    pub fn new() -> Self {
        Self {
            stage: UpdateStage::Idle,
        }
    }

    pub fn stage(&self) -> &UpdateStage {
        &self.stage
    }

    /// Шаги 1–2: обнаружено обновление (только из Idle).
    pub fn detect(&mut self, from: &str, to: &str) -> Result<()> {
        if self.stage != UpdateStage::Idle {
            return Err(LlmError::Update(format!(
                "detect allowed only from Idle (now {:?})",
                self.stage
            )));
        }
        let from_v =
            parse_version(from).ok_or_else(|| LlmError::Update(format!("bad version: {from}")))?;
        let to_v =
            parse_version(to).ok_or_else(|| LlmError::Update(format!("bad version: {to}")))?;
        if to_v <= from_v {
            return Err(LlmError::Update("remote version is not newer".into()));
        }
        self.stage = UpdateStage::Detected {
            from: from_v.to_string(),
            to: to_v.to_string(),
        };
        Ok(())
    }

    /// Шаг 3: подтверждение пользователя.
    pub fn confirm(&mut self) -> Result<()> {
        match &self.stage {
            UpdateStage::Detected { .. } => {
                self.stage = UpdateStage::Confirmed;
                Ok(())
            }
            other => Err(LlmError::Update(format!(
                "confirm allowed only from Detected (now {other:?})"
            ))),
        }
    }

    /// Шаг 4: скачано.
    pub fn downloaded(&mut self) -> Result<()> {
        expect(&self.stage, &UpdateStage::Confirmed, "downloaded")?;
        self.stage = UpdateStage::Downloaded;
        Ok(())
    }

    /// Шаг 5: пересобрано.
    pub fn rebuilt(&mut self) -> Result<()> {
        expect(&self.stage, &UpdateStage::Downloaded, "rebuilt")?;
        self.stage = UpdateStage::Rebuilt;
        Ok(())
    }

    /// Шаги 6–8: результат smoke-тестов.
    pub fn smoke_test(&mut self, passed: bool, reason: &str) -> Result<()> {
        expect(&self.stage, &UpdateStage::Rebuilt, "smoke_test")?;
        self.stage = if passed {
            UpdateStage::Applied
        } else {
            UpdateStage::RolledBack {
                reason: reason.to_string(),
            }
        };
        Ok(())
    }

    /// Сброс в Idle (после применения/отката).
    pub fn reset(&mut self) -> Result<()> {
        match &self.stage {
            UpdateStage::Applied | UpdateStage::RolledBack { .. } => {
                self.stage = UpdateStage::Idle;
                Ok(())
            }
            other => Err(LlmError::Update(format!(
                "reset allowed only from terminal state (now {other:?})"
            ))),
        }
    }
}

fn expect(current: &UpdateStage, required: &UpdateStage, action: &str) -> Result<()> {
    if current != required {
        return Err(LlmError::Update(format!(
            "{action} allowed only from {required:?} (now {current:?})"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_tag_semantics() {
        let c = UpdateChecker::new("1.0.0", "ggml-org/llama.cpp").expect("checker");
        assert_eq!(
            c.check_tag("v1.1.0").expect("ok"),
            Some(semver::Version::parse("1.1.0").expect("v"))
        );
        assert_eq!(c.check_tag("1.0.0").expect("ok"), None);
        assert_eq!(c.check_tag("v0.9.0").expect("ok"), None);
        assert!(c.check_tag("not-a-version").is_err());
    }

    #[test]
    fn checker_validates_input() {
        assert!(UpdateChecker::new("bad!", "repo").is_err());
        assert!(UpdateChecker::new("1.0.0", "").is_err());
    }

    #[test]
    fn update_flow_happy_path() {
        let mut flow = UpdateFlow::new();
        assert_eq!(flow.stage(), &UpdateStage::Idle);
        flow.detect("1.0.0", "1.1.0").expect("detect");
        flow.confirm().expect("confirm");
        flow.downloaded().expect("downloaded");
        flow.rebuilt().expect("rebuilt");
        flow.smoke_test(true, "").expect("smoke");
        assert_eq!(flow.stage(), &UpdateStage::Applied);
        flow.reset().expect("reset");
        assert_eq!(flow.stage(), &UpdateStage::Idle);
    }

    #[test]
    fn update_flow_rollback_on_failed_smoke() {
        let mut flow = UpdateFlow::new();
        flow.detect("1.0.0", "1.1.0").expect("detect");
        flow.confirm().expect("confirm");
        flow.downloaded().expect("downloaded");
        flow.rebuilt().expect("rebuilt");
        flow.smoke_test(false, "inference mismatch").expect("smoke");
        match flow.stage() {
            UpdateStage::RolledBack { reason } => assert_eq!(reason, "inference mismatch"),
            other => panic!("expected RolledBack, got {other:?}"),
        }
    }

    #[test]
    fn update_flow_rejects_wrong_order() {
        let mut flow = UpdateFlow::new();
        // Без обнаружения — нельзя подтвердить.
        assert!(flow.confirm().is_err());
        // Нельзя скачать без подтверждения.
        flow.detect("1.0.0", "1.1.0").expect("detect");
        assert!(flow.downloaded().is_err());
        // Нельзя подтвердить дважды.
        flow.confirm().expect("confirm");
        assert!(flow.confirm().is_err());
        // Нельзя detect, пока не сброшено.
        assert!(flow.detect("1.0.0", "1.2.0").is_err());
    }

    #[test]
    fn detect_rejects_non_newer_and_bad_versions() {
        let mut flow = UpdateFlow::new();
        assert!(flow.detect("1.0.0", "1.0.0").is_err());
        assert!(flow.detect("bad", "1.0.0").is_err());
        assert_eq!(flow.stage(), &UpdateStage::Idle);
    }
}
