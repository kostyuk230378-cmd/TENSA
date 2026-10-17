//! Бюджет облака OpenRouter (ТЗ tensa-router: дневной лимит токенов 100 000,
//! месячный лимит $10, предупреждение при 80 %, блокировка при 100 %).
//!
//! Хранение — JSON-файл в каталоге Tensa (путь задаёт вызывающий;
//! [derived] отдельного файла для бюджета в ТЗ нет).

use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Result, RouterError};
use crate::models::BudgetStatus;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BudgetState {
    /// Дата дневного окна (YYYY-MM-DD).
    date: String,
    /// Месяц месячного окна (YYYY-MM).
    month: String,
    daily_tokens_used: u64,
    monthly_usd_used: f64,
}

/// Учётчик бюджета облака.
pub struct BudgetTracker {
    path: PathBuf,
    daily_tokens_limit: u64,
    monthly_usd_limit: f64,
    warn_at_percent: u8,
    state: Mutex<BudgetState>,
}

impl BudgetTracker {
    /// Загрузить состояние из файла (или начать с нуля).
    pub fn load(
        path: PathBuf,
        daily_tokens_limit: u64,
        monthly_usd_limit: f64,
        warn_at_percent: u8,
    ) -> Result<Self> {
        let state = if path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&path)?)?
        } else {
            BudgetState::default()
        };
        Ok(Self {
            path,
            daily_tokens_limit,
            monthly_usd_limit,
            warn_at_percent,
            state: Mutex::new(state),
        })
    }

    /// Текущий статус (с пересчётом окон по текущей дате).
    pub fn status(&self) -> Result<BudgetStatus> {
        let mut guard = self.lock();
        self.roll_periods(&mut guard, Utc::now().date_naive());
        Ok(self.snapshot(&guard))
    }

    /// Записать расход и вернуть новый статус.
    pub fn record_usage(&self, tokens: u64, usd: f64) -> Result<BudgetStatus> {
        self.record_usage_on(Utc::now().date_naive(), tokens, usd)
    }

    /// Записать расход на указанную дату (тесты и перенос состояния).
    pub fn record_usage_on(&self, date: NaiveDate, tokens: u64, usd: f64) -> Result<BudgetStatus> {
        let mut guard = self.lock();
        self.roll_periods(&mut guard, date);
        guard.daily_tokens_used = guard.daily_tokens_used.saturating_add(tokens);
        guard.monthly_usd_used += usd;
        if guard.monthly_usd_used < 0.0 {
            guard.monthly_usd_used = 0.0;
        }

        let status = self.snapshot(&guard);
        if status.warn && !status.exhausted() {
            tracing::warn!(
                daily = status.daily_tokens_used,
                limit = status.daily_tokens_limit,
                "бюджет облака на пороге предупреждения"
            );
        }
        self.persist(&guard)?;
        Ok(status)
    }

    /// Можно ли расходовать бюджет (Ошибка при достижении лимита).
    pub fn check_available(&self) -> Result<()> {
        let status = self.status()?;
        if status.daily_tokens_used >= status.daily_tokens_limit {
            return Err(RouterError::BudgetExceeded(format!(
                "дневной лимит токенов исчерпан: {}/{}",
                status.daily_tokens_used, status.daily_tokens_limit
            )));
        }
        if status.monthly_usd_used >= status.monthly_usd_limit {
            return Err(RouterError::BudgetExceeded(format!(
                "месячный лимит USD исчерпан: {:.2}/{:.2}",
                status.monthly_usd_used, status.monthly_usd_limit
            )));
        }
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BudgetState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn roll_periods(&self, state: &mut BudgetState, today: NaiveDate) {
        let date_key = today.format("%Y-%m-%d").to_string();
        if state.date != date_key {
            state.date = date_key;
            state.daily_tokens_used = 0;
        }
        let month_key = today.format("%Y-%m").to_string();
        if state.month != month_key {
            state.month = month_key;
            state.monthly_usd_used = 0.0;
        }
    }

    fn snapshot(&self, state: &BudgetState) -> BudgetStatus {
        let tokens_percent = percent(state.daily_tokens_used, self.daily_tokens_limit);
        let usd_percent = if self.monthly_usd_limit > 0.0 {
            (state.monthly_usd_used / self.monthly_usd_limit * 100.0).floor() as u64
        } else {
            100
        };
        BudgetStatus {
            daily_tokens_used: state.daily_tokens_used,
            daily_tokens_limit: self.daily_tokens_limit,
            monthly_usd_used: state.monthly_usd_used,
            monthly_usd_limit: self.monthly_usd_limit,
            warn: tokens_percent >= u64::from(self.warn_at_percent)
                || usd_percent >= u64::from(self.warn_at_percent),
        }
    }

    fn persist(&self, state: &BudgetState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, serde_json::to_string_pretty(state)?)?;
        Ok(())
    }
}

fn percent(used: u64, limit: u64) -> u64 {
    used.saturating_mul(100).checked_div(limit).unwrap_or(100)
}

/// Состояние бюджета: превышен ли лимит.
impl BudgetStatus {
    /// Исчерпан ли бюджет (100 % и более).
    pub fn exhausted(&self) -> bool {
        self.daily_tokens_used >= self.daily_tokens_limit
            || self.monthly_usd_used >= self.monthly_usd_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> (BudgetTracker, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("tensa-router-budget-{}", uuid::Uuid::new_v4()));
        let path = dir.join("router_budget.json");
        (
            BudgetTracker::load(path.clone(), 100_000, 10.0, 80).expect("бюджет"),
            dir,
        )
    }

    /// Сегодняшняя дата (UTC): тесты не должны зависеть от календаря прогона
    /// (фиксированные даты ломали CI при смене суток UTC).
    fn today() -> NaiveDate {
        chrono::Utc::now().date_naive()
    }

    #[test]
    fn fresh_budget_is_zero() {
        let (tracker, dir) = tracker();
        let status = tracker.status().expect("статус");
        assert_eq!(status.daily_tokens_used, 0);
        assert_eq!(status.monthly_usd_used, 0.0);
        assert!(!status.warn);
        assert!(tracker.check_available().is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn warn_at_threshold() {
        let (tracker, dir) = tracker();
        let status = tracker
            .record_usage_on(today(), 80_000, 0.0)
            .expect("запись");
        assert!(status.warn);
        assert!(!status.exhausted());

        let status = tracker
            .record_usage_on(today(), 100_000, 0.0)
            .expect("запись");
        assert!(status.exhausted());
        assert!(tracker.check_available().is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn periods_roll_over() {
        let (tracker, dir) = tracker();
        tracker
            .record_usage_on(today(), 50_000, 5.0)
            .expect("запись");

        // Следующий день: дневной счётчик сбрасывается, месячный — нет.
        let tomorrow = today().succ_opt().expect("завтра");
        let status = tracker
            .record_usage_on(tomorrow, 0, 0.0)
            .expect("новый день");
        assert_eq!(status.daily_tokens_used, 0);
        assert_eq!(status.monthly_usd_used, 5.0);

        // Следующий месяц: сбрасывается и месячный счётчик.
        let next_month = today()
            .checked_add_months(chrono::Months::new(1))
            .expect("следующий месяц");
        let status = tracker
            .record_usage_on(next_month, 0, 0.0)
            .expect("новый месяц");
        assert_eq!(status.monthly_usd_used, 0.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn state_persists_between_instances() {
        let (tracker, dir) = tracker();
        tracker
            .record_usage_on(today(), 12_345, 1.25)
            .expect("запись");
        drop(tracker);

        let path = dir.join("router_budget.json");
        let reloaded = BudgetTracker::load(path, 100_000, 10.0, 80).expect("перечитывание");
        let status = reloaded.status().expect("статус");
        assert_eq!(status.daily_tokens_used, 12_345);
        assert_eq!(status.monthly_usd_used, 1.25);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn monthly_limit_blocks() {
        let (tracker, dir) = tracker();
        tracker.record_usage_on(today(), 0, 10.0).expect("запись");
        assert!(tracker.check_available().is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
