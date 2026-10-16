//! Archival memory — долговременное хранилище фактов.
//!
//! ТЗ: Lance (векторный поиск). Lance подключается фичей `lance` (тяжёлый
//! ML-стек Arrow); по умолчанию — SQLite-реализация с эмбеддингами в JSON:
//! полный контракт (insert/search/search_by_tags/delete) и семантический
//! поиск по косинусной близости. [derived] Миграция на Lance не меняет трейт.

use crate::error::Result;
use crate::models::{ArchivalEntry, MemorySearchResult, MemorySource};
use crate::store::ArchivalMemoryStore;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../schema.sql");

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// SQLite-реализация archival memory (fallback до включения фичи lance).
pub struct SqliteArchivalMemoryStore {
    pool: SqlitePool,
    embedder: std::sync::Arc<dyn crate::embedder::Embedder>,
}

fn entry_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<ArchivalEntry> {
    Ok(ArchivalEntry {
        id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
        content: row.try_get("content")?,
        embedding: serde_json::from_str(&row.try_get::<String, _>("embedding")?)?,
        tags: serde_json::from_str(&row.try_get::<String, _>("tags")?)?,
        source: row.try_get("source")?,
        created_at: row.try_get("created_at")?,
        node_id: row.try_get("node_id")?,
        metadata: serde_json::from_str(&row.try_get::<String, _>("metadata")?)?,
    })
}

impl SqliteArchivalMemoryStore {
    pub async fn new(
        db_path: &std::path::Path,
        embedder: std::sync::Arc<dyn crate::embedder::Embedder>,
    ) -> Result<Self> {
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self { pool, embedder };
        store.run_schema().await?;
        Ok(store)
    }

    pub async fn new_in_memory(
        embedder: std::sync::Arc<dyn crate::embedder::Embedder>,
    ) -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self { pool, embedder };
        store.run_schema().await?;
        Ok(store)
    }

    async fn run_schema(&self) -> Result<()> {
        crate::run_schema_statements(&self.pool, SCHEMA_SQL).await
    }

    async fn insert_entry(&self, mut entry: ArchivalEntry) -> Result<Uuid> {
        if entry.embedding.is_empty() {
            entry.embedding = self.embedder.embed_passage(&entry.content).await?;
        }
        if entry.created_at == 0 {
            entry.created_at = now_ts();
        }
        sqlx::query(
            "INSERT INTO archival_entries (id, content, embedding, tags, source, created_at, node_id, metadata) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(entry.id.to_string())
        .bind(&entry.content)
        .bind(serde_json::to_string(&entry.embedding)?)
        .bind(serde_json::to_string(&entry.tags)?)
        .bind(&entry.source)
        .bind(entry.created_at)
        .bind(&entry.node_id)
        .bind(entry.metadata.to_string())
        .execute(&self.pool)
        .await?;
        Ok(entry.id)
    }

    async fn all_entries(&self) -> Result<Vec<ArchivalEntry>> {
        let rows = sqlx::query("SELECT * FROM archival_entries")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(entry_from_row).collect()
    }
}

/// Таблица archival_entries (Контракт 2 — archival на SQLite до фичи lance). [derived]
const ARCHIVAL_SQL: &str = "
CREATE TABLE IF NOT EXISTS archival_entries (
    id         TEXT PRIMARY KEY,
    content    TEXT NOT NULL,
    embedding  TEXT NOT NULL,
    tags       TEXT NOT NULL,
    source     TEXT,
    created_at INTEGER NOT NULL,
    node_id    TEXT NOT NULL,
    metadata   TEXT NOT NULL
);
";

impl SqliteArchivalMemoryStore {
    async fn run_schema_all(&self) -> Result<()> {
        self.run_schema().await?;
        for stmt in ARCHIVAL_SQL.split(';') {
            let s = stmt.trim();
            if !s.is_empty() {
                sqlx::query(s).execute(&self.pool).await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ArchivalMemoryStore for SqliteArchivalMemoryStore {
    async fn insert(&self, entry: ArchivalEntry) -> Result<Uuid> {
        self.run_schema_all().await?;
        self.insert_entry(entry).await
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>> {
        self.run_schema_all().await?;
        let qvec = self.embedder.embed_query(query).await?;
        let mut scored: Vec<MemorySearchResult> = self
            .all_entries()
            .await?
            .into_iter()
            .map(|e| {
                let score = crate::embedder::cosine_similarity(&qvec, &e.embedding).unwrap_or(0.0);
                MemorySearchResult {
                    id: e.id,
                    content: e.content,
                    score,
                    source: MemorySource::Archival,
                    timestamp: e.created_at,
                }
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored)
    }

    async fn search_by_tags(
        &self,
        tags: &[String],
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>> {
        self.run_schema_all().await?;
        let mut out = Vec::new();
        for entry in self.all_entries().await? {
            if tags.iter().any(|t| entry.tags.contains(t)) {
                out.push(MemorySearchResult {
                    id: entry.id,
                    content: entry.content,
                    score: 1.0,
                    source: MemorySource::Archival,
                    timestamp: entry.created_at,
                });
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        self.run_schema_all().await?;
        sqlx::query("DELETE FROM archival_entries WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
