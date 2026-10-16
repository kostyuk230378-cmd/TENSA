//! Recall memory — история разговоров (SQLite + FTS5, dop_tz).

use crate::error::Result;
use crate::models::{MemorySearchResult, MemorySource, RecallMessage, Role};
use crate::store::RecallMemoryStore;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../schema.sql");

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn message_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<RecallMessage> {
    let role: String = row.try_get("role")?;
    Ok(RecallMessage {
        id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
        role: match role.as_str() {
            "assistant" => Role::Assistant,
            "system" => Role::System,
            _ => Role::User,
        },
        content: row.try_get("content")?,
        timestamp: row.try_get("timestamp")?,
        session_id: Uuid::from_str(&row.try_get::<String, _>("session_id")?)?,
        node_id: row.try_get("node_id")?,
        metadata: serde_json::from_str(&row.try_get::<String, _>("metadata")?)?,
    })
}

/// SQLite-хранилище истории разговоров.
pub struct SqliteRecallMemoryStore {
    pool: SqlitePool,
}

impl SqliteRecallMemoryStore {
    pub async fn new(db_path: &std::path::Path) -> Result<Self> {
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self { pool };
        store.run_schema().await?;
        Ok(store)
    }

    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self { pool };
        store.run_schema().await?;
        Ok(store)
    }

    async fn run_schema(&self) -> Result<()> {
        crate::run_schema_statements(&self.pool, SCHEMA_SQL).await
    }
}

#[async_trait]
impl RecallMemoryStore for SqliteRecallMemoryStore {
    async fn append_message(&self, message: RecallMessage) -> Result<()> {
        sqlx::query(
            "INSERT INTO recall_messages (id, role, content, timestamp, session_id, node_id, metadata) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(message.id.to_string())
        .bind(message.role.as_str())
        .bind(&message.content)
        .bind(message.timestamp)
        .bind(message.session_id.to_string())
        .bind(&message.node_id)
        .bind(message.metadata.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        // FTS5: экранируем кавычки, каждое слово — префиксный запрос (word*),
        // объединённый AND: находит «Иваном» по запросу «Иван».
        let safe = query
            .split_whitespace()
            .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ");
        let rows = sqlx::query(
            "SELECT m.id, m.content, m.timestamp, bm25(recall_fts) AS rank \
             FROM recall_fts f JOIN recall_messages m ON m.rowid = f.rowid \
             WHERE recall_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(&safe)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let rank: f64 = row.try_get("rank").unwrap_or(0.0);
            out.push(MemorySearchResult {
                id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
                content: row.try_get("content")?,
                // bm25 — отрицательный (меньше = лучше); нормируем в score.
                score: (-rank).max(0.0) as f32,
                source: MemorySource::Recall,
                timestamp: row.try_get("timestamp")?,
            });
        }
        Ok(out)
    }

    async fn get_session(&self, session_id: Uuid, limit: usize) -> Result<Vec<RecallMessage>> {
        let rows = sqlx::query(
            "SELECT * FROM recall_messages WHERE session_id = ? ORDER BY timestamp ASC LIMIT ?",
        )
        .bind(session_id.to_string())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(message_from_row).collect()
    }

    async fn get_recent(&self, limit: usize) -> Result<Vec<RecallMessage>> {
        let rows = sqlx::query("SELECT * FROM recall_messages ORDER BY timestamp DESC LIMIT ?")
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;
        let mut out: Vec<RecallMessage> =
            rows.iter().map(message_from_row).collect::<Result<_>>()?;
        out.reverse(); // хронологический порядок
        Ok(out)
    }

    async fn cleanup_old(&self, older_than_days: u32) -> Result<usize> {
        let cutoff = now_ts() - (older_than_days as i64) * 86400;
        let result = sqlx::query("DELETE FROM recall_messages WHERE timestamp < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() as usize)
    }
}
