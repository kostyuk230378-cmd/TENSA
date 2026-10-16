//! SQLite-реализация ApprovalStore (Контракт 1: `~/.tensa/approval.db`).

use crate::config::ApprovalConfig;
use crate::error::{ApprovalError, Result};
use crate::events::{ActionEvent, EventType};
use crate::models::{Action, ActionKind, ActionOrigin, ActionStatus, NewAction};
use crate::store::ApprovalStore;
use crate::vector_clock::VectorClock;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{Executor, Row, Sqlite};
use std::path::Path;
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../schema.sql");

/// Хранилище на SQLite. Все операции — в транзакциях, события пишутся
/// в `action_events` атомарно с изменением `actions`.
pub struct SqliteApprovalStore {
    pool: SqlitePool,
    config: ApprovalConfig,
    node_id: String,
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn row_to_action(row: &sqlx::sqlite::SqliteRow) -> Result<Action> {
    let kind_str: String = row.try_get("kind")?;
    let status_str: String = row.try_get("status")?;
    let created_by_str: String = row.try_get("created_by")?;
    let approved_by_str: Option<String> = row.try_get("approved_by")?;
    let vc_str: String = row.try_get("vector_clock")?;
    Ok(Action {
        id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
        kind: ActionKind::from_db(&kind_str)?,
        payload: serde_json::from_str(&row.try_get::<String, _>("payload")?)?,
        status: ActionStatus::from_db(&status_str)?,
        created_at: row.try_get("created_at")?,
        created_by: ActionOrigin::from_db(&created_by_str)?,
        created_on_node: row.try_get("created_on_node")?,
        expires_at: row.try_get("expires_at")?,
        approved_at: row.try_get("approved_at")?,
        approved_by: approved_by_str
            .map(|s| ActionOrigin::from_db(&s))
            .transpose()?,
        approved_on_node: row.try_get("approved_on_node")?,
        executed_at: row.try_get("executed_at")?,
        executed_on_node: row.try_get("executed_on_node")?,
        result: row
            .try_get::<Option<String>, _>("result")?
            .map(|s| serde_json::from_str(&s))
            .transpose()?,
        operation_id: Uuid::from_str(&row.try_get::<String, _>("operation_id")?)?,
        vector_clock: serde_json::from_str(&vc_str)?,
    })
}

fn event_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<ActionEvent> {
    let event_type_str: String = row.try_get("event_type")?;
    Ok(ActionEvent {
        event_id: row.try_get::<i64, _>("id")? as u64,
        action_id: Uuid::from_str(&row.try_get::<String, _>("action_id")?)?,
        event_type: EventType::from_db(&event_type_str)?,
        event_data: serde_json::from_str(&row.try_get::<String, _>("event_data")?)?,
        timestamp: row.try_get("timestamp")?,
        node_id: row.try_get("node_id")?,
        vector_clock: serde_json::from_str(&row.try_get::<String, _>("vector_clock")?)?,
    })
}

impl SqliteApprovalStore {
    /// Открыть (или создать) хранилище по пути к файлу БД.
    pub async fn new(
        db_path: impl AsRef<Path>,
        config: ApprovalConfig,
        node_id: String,
    ) -> Result<Self> {
        let url = format!(
            "sqlite://{}",
            db_path.as_ref().to_string_lossy().replace('\\', "/")
        );
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        Self::init(pool, config, node_id).await
    }

    /// Хранилище в памяти (для тестов), конфигурация по умолчанию.
    pub async fn new_in_memory() -> Result<Self> {
        Self::init_with_config(ApprovalConfig::default()).await
    }

    /// Хранилище в памяти с произвольной конфигурацией (для тестов).
    /// [derived] Помощник для integration-тестов выключателей и лимитов.
    pub async fn init_with_config(config: ApprovalConfig) -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Self::init(pool, config, "local".to_string()).await
    }

    async fn init(pool: SqlitePool, config: ApprovalConfig, node_id: String) -> Result<Self> {
        // schema.sql — простые statement'ы через ';'; выполняем построчно,
        // пропуская строки комментариев (внутри statement'ов их нет).
        let mut current = String::new();
        for line in SCHEMA_SQL.lines() {
            if line.trim_start().starts_with("--") {
                continue;
            }
            current.push_str(line);
            current.push('\n');
            if line.trim_end().ends_with(';') {
                let stmt = current.trim();
                if !stmt.is_empty() {
                    pool.execute(stmt).await?;
                }
                current.clear();
            }
        }
        let stmt = current.trim();
        if !stmt.is_empty() {
            pool.execute(stmt).await?;
        }
        info!(node_id = %node_id, "ApprovalStore initialized");
        Ok(Self {
            pool,
            config,
            node_id,
        })
    }

    /// Вставить событие в журнал (внутри транзакции); возвращает event_id.
    async fn insert_event<'e, E>(
        executor: E,
        action_id: &Uuid,
        event_type: EventType,
        event_data: &serde_json::Value,
        node_id: &str,
        vector_clock: &VectorClock,
    ) -> Result<u64>
    where
        E: Executor<'e, Database = Sqlite>,
    {
        let result = sqlx::query(
            "INSERT INTO action_events (action_id, event_type, event_data, timestamp, node_id, vector_clock) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(action_id.to_string())
        .bind(event_type.as_str())
        .bind(event_data.to_string())
        .bind(now_ts())
        .bind(node_id)
        .bind(serde_json::to_string(vector_clock)?)
        .execute(executor)
        .await?;
        Ok(result.last_insert_rowid() as u64)
    }

    /// Снимок действия для event_data: { action: ..., action_id: ... }.
    fn snapshot(action: &Action) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "action": serde_json::to_value(action)?,
            "action_id": action.id,
        }))
    }
}

#[async_trait]
impl ApprovalStore for SqliteApprovalStore {
    async fn request_action(&self, action: NewAction) -> Result<Action> {
        if !action.kind.config_flag(&self.config) {
            return Err(ApprovalError::KindDisabled {
                kind: action.kind.as_str().to_string(),
            });
        }
        let mut tx = self.pool.begin().await?;

        let pending: i64 =
            sqlx::query("SELECT COUNT(*) AS count FROM actions WHERE status = 'pending'")
                .fetch_one(&mut *tx)
                .await?
                .try_get("count")?;
        if pending as usize >= self.config.max_pending_actions {
            return Err(ApprovalError::MaxPendingReached {
                limit: self.config.max_pending_actions,
            });
        }

        let created_at = now_ts();
        let ttl_hours = action
            .expires_in_hours
            .unwrap_or(self.config.action_ttl_hours);
        let expires_at = created_at + (ttl_hours as i64) * 3600;
        let mut vector_clock = VectorClock::new();
        vector_clock.tick(&self.node_id);

        let action = Action {
            id: Uuid::new_v4(),
            kind: action.kind,
            payload: action.payload,
            status: ActionStatus::Pending,
            created_at,
            created_by: action.created_by,
            created_on_node: self.node_id.clone(),
            expires_at,
            approved_at: None,
            approved_by: None,
            approved_on_node: None,
            executed_at: None,
            executed_on_node: None,
            result: None,
            operation_id: Uuid::new_v4(),
            vector_clock,
        };

        sqlx::query(
            "INSERT INTO actions (id, kind, payload, status, created_at, created_by, created_on_node, \
             expires_at, approved_at, approved_by, approved_on_node, executed_at, executed_on_node, \
             result, operation_id, vector_clock) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(action.id.to_string())
        .bind(action.kind.as_str())
        .bind(action.payload.to_string())
        .bind(action.status.as_str())
        .bind(action.created_at)
        .bind(action.created_by.as_str())
        .bind(&action.created_on_node)
        .bind(action.expires_at)
        .bind(action.approved_at)
        .bind(action.approved_by.map(|o| o.as_str()))
        .bind(&action.approved_on_node)
        .bind(action.executed_at)
        .bind(&action.executed_on_node)
        .bind(action.result.as_ref().map(|v| v.to_string()))
        .bind(action.operation_id.to_string())
        .bind(serde_json::to_string(&action.vector_clock)?)
        .execute(&mut *tx)
        .await?;

        // [derived] auto_approve_trusted: trusted origin (user/telegram)
        // проходит без отдельного одобрения, но с событием в журнале.
        let auto = self.config.auto_approve_trusted
            && matches!(
                action.created_by,
                ActionOrigin::User | ActionOrigin::Telegram
            );
        let mut approved = action.clone();

        let event_id = if auto {
            approved.status = ActionStatus::Approved;
            approved.approved_at = Some(now_ts());
            approved.approved_by = Some(action.created_by);
            approved.approved_on_node = Some(self.node_id.clone());
            approved.vector_clock.tick(&self.node_id);
            sqlx::query(
                "UPDATE actions SET status = ?, approved_at = ?, approved_by = ?, approved_on_node = ?, vector_clock = ? WHERE id = ?",
            )
            .bind(approved.status.as_str())
            .bind(approved.approved_at)
            .bind(approved.approved_by.map(|o| o.as_str()))
            .bind(&approved.approved_on_node)
            .bind(serde_json::to_string(&approved.vector_clock)?)
            .bind(approved.id.to_string())
            .execute(&mut *tx)
            .await?;
            Self::insert_event(
                &mut *tx,
                &approved.id,
                EventType::Approved,
                &Self::snapshot(&approved)?,
                &self.node_id,
                &approved.vector_clock,
            )
            .await?
        } else {
            Self::insert_event(
                &mut *tx,
                &action.id,
                EventType::Created,
                &Self::snapshot(&action)?,
                &self.node_id,
                &action.vector_clock,
            )
            .await?
        };

        tx.commit().await?;
        info!(action_id = %action.id, event_id, kind = action.kind.as_str(), "action requested");
        // При авто-одобрении trusted origin возвращаем уже одобренное действие.
        Ok(if auto { approved } else { action })
    }

    async fn approve(&self, id: Uuid, approved_by: ActionOrigin) -> Result<Action> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM actions WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;
        let action = row_to_action(&row)?;

        if action.status != ActionStatus::Pending {
            return Err(ApprovalError::InvalidTransition {
                action_id: id.to_string(),
                from: action.status,
                expected: "pending".to_string(),
            });
        }
        if action.expires_at < now_ts() {
            return Err(ApprovalError::Expired(id.to_string()));
        }

        let mut updated = action;
        updated.status = ActionStatus::Approved;
        updated.approved_at = Some(now_ts());
        updated.approved_by = Some(approved_by);
        updated.approved_on_node = Some(self.node_id.clone());
        updated.vector_clock.tick(&self.node_id);

        sqlx::query(
            "UPDATE actions SET status = ?, approved_at = ?, approved_by = ?, approved_on_node = ?, vector_clock = ? WHERE id = ?",
        )
        .bind(updated.status.as_str())
        .bind(updated.approved_at)
        .bind(updated.approved_by.map(|o| o.as_str()))
        .bind(&updated.approved_on_node)
        .bind(serde_json::to_string(&updated.vector_clock)?)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
        Self::insert_event(
            &mut *tx,
            &id,
            EventType::Approved,
            &Self::snapshot(&updated)?,
            &self.node_id,
            &updated.vector_clock,
        )
        .await?;
        tx.commit().await?;
        info!(action_id = %id, "action approved");
        Ok(updated)
    }

    async fn reject(&self, id: Uuid, rejected_by: ActionOrigin) -> Result<Action> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM actions WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;
        let action = row_to_action(&row)?;

        if action.status != ActionStatus::Pending {
            return Err(ApprovalError::InvalidTransition {
                action_id: id.to_string(),
                from: action.status,
                expected: "pending".to_string(),
            });
        }

        let mut updated = action;
        updated.status = ActionStatus::Rejected;
        updated.vector_clock.tick(&self.node_id);

        sqlx::query("UPDATE actions SET status = ?, vector_clock = ? WHERE id = ?")
            .bind(updated.status.as_str())
            .bind(serde_json::to_string(&updated.vector_clock)?)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        Self::insert_event(
            &mut *tx,
            &id,
            EventType::Rejected,
            &Self::snapshot(&updated)?,
            &self.node_id,
            &updated.vector_clock,
        )
        .await?;
        tx.commit().await?;
        info!(action_id = %id, rejected_by = rejected_by.as_str(), "action rejected");
        Ok(updated)
    }

    async fn execute(&self, id: Uuid) -> Result<Action> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM actions WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;
        let action = row_to_action(&row)?;

        if action.status != ActionStatus::Approved {
            return Err(ApprovalError::InvalidTransition {
                action_id: id.to_string(),
                from: action.status,
                expected: "approved".to_string(),
            });
        }

        let mut updated = action;
        updated.status = ActionStatus::Executed;
        updated.executed_at = Some(now_ts());
        updated.executed_on_node = Some(self.node_id.clone());
        updated.vector_clock.tick(&self.node_id);

        sqlx::query(
            "UPDATE actions SET status = ?, executed_at = ?, executed_on_node = ?, vector_clock = ? WHERE id = ?",
        )
        .bind(updated.status.as_str())
        .bind(updated.executed_at)
        .bind(&updated.executed_on_node)
        .bind(serde_json::to_string(&updated.vector_clock)?)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
        Self::insert_event(
            &mut *tx,
            &id,
            EventType::Executed,
            &Self::snapshot(&updated)?,
            &self.node_id,
            &updated.vector_clock,
        )
        .await?;
        tx.commit().await?;
        info!(action_id = %id, "action executed");
        Ok(updated)
    }

    async fn get_pending(&self) -> Result<Vec<Action>> {
        let rows =
            sqlx::query("SELECT * FROM actions WHERE status = 'pending' ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await?;
        rows.iter().map(row_to_action).collect()
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Action>> {
        let row = sqlx::query("SELECT * FROM actions WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_action).transpose()
    }

    async fn expire_old(&self) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        let now = now_ts();
        let rows = sqlx::query(
            "SELECT id, vector_clock FROM actions WHERE status = 'pending' AND expires_at < ?",
        )
        .bind(now)
        .fetch_all(&mut *tx)
        .await?;

        for row in &rows {
            let id: String = row.try_get("id")?;
            let mut vc: VectorClock =
                serde_json::from_str(&row.try_get::<String, _>("vector_clock")?)?;
            vc.tick(&self.node_id);
            sqlx::query("UPDATE actions SET status = 'expired', vector_clock = ? WHERE id = ?")
                .bind(serde_json::to_string(&vc)?)
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            let uuid = Uuid::from_str(&id)?;
            Self::insert_event(
                &mut *tx,
                &uuid,
                EventType::Expired,
                &serde_json::json!({ "id": id }),
                &self.node_id,
                &vc,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(rows.len())
    }

    async fn apply_event(&self, event: ActionEvent) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Идемпотентность: такое же событие (узел+тип+время+действие) уже есть?
        let duplicate: i64 = sqlx::query(
            "SELECT COUNT(*) AS count FROM action_events \
             WHERE action_id = ? AND event_type = ? AND node_id = ? AND timestamp = ?",
        )
        .bind(event.action_id.to_string())
        .bind(event.event_type.as_str())
        .bind(&event.node_id)
        .bind(event.timestamp)
        .fetch_one(&mut *tx)
        .await?
        .try_get("count")?;
        if duplicate > 0 {
            tx.commit().await?;
            return Ok(());
        }

        // Состояние действия: применяем событие только если его часы новее (LWW).
        let local = sqlx::query("SELECT * FROM actions WHERE id = ?")
            .bind(event.action_id.to_string())
            .fetch_optional(&mut *tx)
            .await?;
        let snapshot = event.event_data.get("action").cloned();
        match (local, snapshot) {
            (None, Some(snapshot)) => {
                let mut action: Action = serde_json::from_value(snapshot)?;
                action.vector_clock = event.vector_clock.clone();
                sqlx::query(
                    "INSERT INTO actions (id, kind, payload, status, created_at, created_by, created_on_node, \
                     expires_at, approved_at, approved_by, approved_on_node, executed_at, executed_on_node, \
                     result, operation_id, vector_clock) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(action.id.to_string())
                .bind(action.kind.as_str())
                .bind(action.payload.to_string())
                .bind(action.status.as_str())
                .bind(action.created_at)
                .bind(action.created_by.as_str())
                .bind(&action.created_on_node)
                .bind(action.expires_at)
                .bind(action.approved_at)
                .bind(action.approved_by.map(|o| o.as_str()))
                .bind(&action.approved_on_node)
                .bind(action.executed_at)
                .bind(&action.executed_on_node)
                .bind(action.result.as_ref().map(|v| v.to_string()))
                .bind(action.operation_id.to_string())
                .bind(serde_json::to_string(&action.vector_clock)?)
                .execute(&mut *tx)
                .await?;
            }
            (Some(row), Some(snapshot)) => {
                let local_action = row_to_action(&row)?;
                let remote: Action = serde_json::from_value(snapshot)?;
                let remote_newer = remote.timestamp_hint() >= local_action.timestamp_hint();
                if remote_newer {
                    sqlx::query(
                        "UPDATE actions SET status = ?, approved_at = ?, approved_by = ?, approved_on_node = ?, \
                         executed_at = ?, executed_on_node = ?, result = ?, vector_clock = ? WHERE id = ?",
                    )
                    .bind(remote.status.as_str())
                    .bind(remote.approved_at)
                    .bind(remote.approved_by.map(|o| o.as_str()))
                    .bind(&remote.approved_on_node)
                    .bind(remote.executed_at)
                    .bind(&remote.executed_on_node)
                    .bind(remote.result.as_ref().map(|v| v.to_string()))
                    .bind(serde_json::to_string(&event.vector_clock)?)
                    .bind(event.action_id.to_string())
                    .execute(&mut *tx)
                    .await?;
                }
            }
            (None, None) => {
                return Err(ApprovalError::Config(format!(
                    "event without action snapshot: {}",
                    event.action_id
                )));
            }
            (Some(_), None) => {
                // Локальное действие есть, снимка нет — применяем только часы.
                sqlx::query("UPDATE actions SET vector_clock = ? WHERE id = ?")
                    .bind(serde_json::to_string(&event.vector_clock)?)
                    .bind(event.action_id.to_string())
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Событие пишем в журнал с локальным автоинкрементным id.
        sqlx::query(
            "INSERT INTO action_events (action_id, event_type, event_data, timestamp, node_id, vector_clock) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(event.action_id.to_string())
        .bind(event.event_type.as_str())
        .bind(event.event_data.to_string())
        .bind(event.timestamp)
        .bind(&event.node_id)
        .bind(serde_json::to_string(&event.vector_clock)?)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn get_events_since(&self, last_event_id: u64) -> Result<Vec<ActionEvent>> {
        let rows = sqlx::query("SELECT * FROM action_events WHERE id > ? ORDER BY id ASC")
            .bind(last_event_id as i64)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(event_from_row).collect()
    }
}

impl Action {
    /// Подсказка для LWW: максимум из временных меток изменения.
    /// [derived] В Контракте 1 нет единого updated_at; берём позднейший из
    /// известных моментов (executed/approved/created).
    fn timestamp_hint(&self) -> i64 {
        self.executed_at
            .or(self.approved_at)
            .unwrap_or(self.created_at)
    }
}
