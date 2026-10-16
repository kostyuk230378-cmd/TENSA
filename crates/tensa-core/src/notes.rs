//! Трейт NotesStore (Контракт 3 / ТЗ tensa-core, сигнатуры дословно)
//! и SQLite-реализация с FTS5.

use crate::error::{CoreError, Result};
use crate::models::{NewNote, Note, VectorClock};
use async_trait::async_trait;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Executor, Row};
use std::str::FromStr;
use uuid::Uuid;

#[async_trait]
pub trait NotesStore: Send + Sync {
    async fn create_note(&self, note: NewNote) -> Result<Note>;
    async fn update_note(&self, note: Note) -> Result<Note>;
    async fn delete_note(&self, id: Uuid) -> Result<()>;
    async fn get_note(&self, id: Uuid) -> Result<Option<Note>>;
    async fn search_notes(&self, query: &str, limit: usize) -> Result<Vec<Note>>;
    async fn list_notes(&self, limit: usize, offset: usize) -> Result<Vec<Note>>;

    /// Заметки, изменённые позже timestamp (для sync-адаптера; одобрено 2026-10-15).
    async fn get_notes_since(&self, timestamp: i64) -> Result<Vec<Note>>;
    /// Количество неудалённых заметок (для DataService.total).
    async fn count_notes(&self) -> Result<u64>;
}

/// SQLite-реализация NotesStore (notes.db, FTS5, soft delete, LWW).
pub struct SqliteNotesStore {
    pool: SqlitePool,
    node_id: String,
}

const SCHEMA_SQL: &str = include_str!("../schema.sql");

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn note_from_row(row: &SqliteRow) -> Result<Note> {
    let tags_str: String = row.try_get("tags")?;
    Ok(Note {
        id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        tags: serde_json::from_str(&tags_str)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        created_on_node: row.try_get("created_on_node")?,
        updated_on_node: row.try_get("updated_on_node")?,
        deleted: row.try_get::<i64, _>("deleted")? != 0,
        vector_clock: serde_json::from_str(&row.try_get::<String, _>("vector_clock")?)?,
    })
}

/// Запуск схемы по statement'ам (строки-комментарии пропускаются).
pub(crate) async fn run_schema(pool: &SqlitePool, schema: &str) -> Result<()> {
    let mut current = String::new();
    for line in schema.lines() {
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
    Ok(())
}

impl SqliteNotesStore {
    /// Открыть (или создать) хранилище заметок.
    pub async fn new(db_path: &std::path::Path, node_id: String) -> Result<Self> {
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        run_schema(&pool, SCHEMA_SQL).await?;
        tracing::info!(node_id = %node_id, "NotesStore initialized");
        Ok(Self { pool, node_id })
    }

    /// Хранилище в памяти (для тестов).
    pub async fn new_in_memory() -> Result<Self> {
        let options = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        run_schema(&pool, SCHEMA_SQL).await?;
        Ok(Self {
            pool,
            node_id: "local".into(),
        })
    }
}

#[async_trait]
impl NotesStore for SqliteNotesStore {
    async fn create_note(&self, note: NewNote) -> Result<Note> {
        let now = now_ts();
        let mut vc = VectorClock::new();
        vc.tick(&self.node_id);
        let note = Note {
            id: Uuid::new_v4(),
            title: note.title,
            body: note.body,
            tags: note.tags,
            created_at: now,
            updated_at: now,
            created_on_node: self.node_id.clone(),
            updated_on_node: self.node_id.clone(),
            deleted: false,
            vector_clock: vc,
        };
        sqlx::query(
            "INSERT INTO notes (id, title, body, tags, created_at, updated_at, created_on_node, updated_on_node, deleted, vector_clock) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(note.id.to_string())
        .bind(&note.title)
        .bind(&note.body)
        .bind(serde_json::to_string(&note.tags)?)
        .bind(note.created_at)
        .bind(note.updated_at)
        .bind(&note.created_on_node)
        .bind(&note.updated_on_node)
        .bind(serde_json::to_string(&note.vector_clock)?)
        .execute(&self.pool)
        .await?;
        tracing::info!(note_id = %note.id, "note created");
        Ok(note)
    }

    async fn update_note(&self, note: Note) -> Result<Note> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM notes WHERE id = ? AND deleted = 0")
            .bind(note.id.to_string())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(note.id.to_string()))?;
        let local = note_from_row(&row)?;
        // LWW: чужая версия старше локальной — оставляем локальную (упрощение до gRPC-слоя).
        if note.updated_at < local.updated_at && note.updated_on_node != self.node_id {
            return Ok(local);
        }
        let mut updated = note;
        updated.updated_at = now_ts();
        updated.updated_on_node = self.node_id.clone();
        updated.vector_clock.tick(&self.node_id);
        sqlx::query(
            "UPDATE notes SET title = ?, body = ?, tags = ?, updated_at = ?, updated_on_node = ?, vector_clock = ? WHERE id = ?",
        )
        .bind(&updated.title)
        .bind(&updated.body)
        .bind(serde_json::to_string(&updated.tags)?)
        .bind(updated.updated_at)
        .bind(&updated.updated_on_node)
        .bind(serde_json::to_string(&updated.vector_clock)?)
        .bind(updated.id.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(note_id = %updated.id, "note updated");
        Ok(updated)
    }

    async fn delete_note(&self, id: Uuid) -> Result<()> {
        // Soft delete (Контракт 3).
        let result = sqlx::query(
            "UPDATE notes SET deleted = 1, updated_at = ?, updated_on_node = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now_ts())
        .bind(&self.node_id)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(id.to_string()));
        }
        tracing::info!(note_id = %id, "note soft-deleted");
        Ok(())
    }

    async fn get_note(&self, id: Uuid) -> Result<Option<Note>> {
        let row = sqlx::query("SELECT * FROM notes WHERE id = ? AND deleted = 0")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(note_from_row).transpose()
    }

    async fn get_notes_since(&self, timestamp: i64) -> Result<Vec<Note>> {
        // Одобрено 2026-10-15: только неудалённые, по времени изменения.
        let rows = sqlx::query(
            "SELECT * FROM notes WHERE updated_at > ? AND deleted = 0 ORDER BY updated_at ASC",
        )
        .bind(timestamp)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(note_from_row).collect()
    }

    async fn count_notes(&self) -> Result<u64> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM notes WHERE deleted = 0")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("count")? as u64)
    }

    async fn search_notes(&self, query: &str, limit: usize) -> Result<Vec<Note>> {
        // FTS5 (unicode61). Пустой запрос — пустой результат.
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT n.* FROM notes_fts f JOIN notes n ON n.rowid = f.rowid \
             WHERE notes_fts MATCH ? AND n.deleted = 0 \
             ORDER BY bm25(notes_fts) LIMIT ?",
        )
        .bind(q)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(note_from_row).collect()
    }

    async fn list_notes(&self, limit: usize, offset: usize) -> Result<Vec<Note>> {
        let rows = sqlx::query(
            "SELECT * FROM notes WHERE deleted = 0 ORDER BY updated_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(note_from_row).collect()
    }
}
