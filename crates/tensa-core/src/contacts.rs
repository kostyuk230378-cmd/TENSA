//! Хранилище контактов (Контракт 3, ТЗ tensa-core): SQLite + FTS5,
//! soft delete, LWW по updated_at + vector_clock.

use crate::error::{CoreError, Result};
use crate::models::{Contact, NewContact, VectorClock};
use crate::notes::run_schema;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

/// Трейт хранилища контактов (сигнатуры дословно из ТЗ).
#[async_trait]
pub trait ContactsStore: Send + Sync {
    async fn create_contact(&self, contact: NewContact) -> Result<Contact>;
    async fn update_contact(&self, contact: Contact) -> Result<Contact>;
    async fn delete_contact(&self, id: Uuid) -> Result<()>;
    async fn get_contact(&self, id: Uuid) -> Result<Option<Contact>>;
    async fn find_by_email(&self, email: &str) -> Result<Option<Contact>>;
    async fn find_by_name(&self, name: &str) -> Result<Vec<Contact>>;
    async fn search_contacts(&self, query: &str, limit: usize) -> Result<Vec<Contact>>;
    async fn list_contacts(&self, limit: usize, offset: usize) -> Result<Vec<Contact>>;

    /// Контакты, изменённые позже timestamp (для sync-адаптера; одобрено 2026-10-15).
    async fn get_contacts_since(&self, timestamp: i64) -> Result<Vec<Contact>>;
    /// Количество неудалённых контактов (для DataService.total).
    async fn count_contacts(&self) -> Result<u64>;
}

/// SQLite-реализация ContactsStore (notes.db, contacts_fts, soft delete).
pub struct SqliteContactsStore {
    pool: SqlitePool,
    node_id: String,
}

const SCHEMA_SQL: &str = include_str!("../schema.sql");

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn contact_from_row(row: &SqliteRow) -> Result<Contact> {
    let tags_str: String = row.try_get("tags")?;
    Ok(Contact {
        id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
        name: row.try_get("name")?,
        email: row.try_get("email")?,
        phone: row.try_get("phone")?,
        telegram: row.try_get("telegram")?,
        whatsapp: row.try_get("whatsapp")?,
        notes: row.try_get("notes")?,
        tags: serde_json::from_str(&tags_str)?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        created_on_node: row.try_get("created_on_node")?,
        updated_on_node: row.try_get("updated_on_node")?,
        deleted: row.try_get::<i64, _>("deleted")? != 0,
        vector_clock: serde_json::from_str(&row.try_get::<String, _>("vector_clock")?)?,
    })
}

impl SqliteContactsStore {
    /// Открыть (или создать) хранилище контактов (notes.db — общая БД).
    pub async fn new(db_path: &std::path::Path, node_id: String) -> Result<Self> {
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        run_schema(&pool, SCHEMA_SQL).await?;
        tracing::info!(node_id = %node_id, "ContactsStore initialized");
        Ok(Self { pool, node_id })
    }

    /// Хранилище в памяти (для тестов).
    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
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
impl ContactsStore for SqliteContactsStore {
    async fn create_contact(&self, contact: NewContact) -> Result<Contact> {
        let now = now_ts();
        let mut vc = VectorClock::new();
        vc.tick(&self.node_id);
        let contact = Contact {
            id: Uuid::new_v4(),
            name: contact.name,
            email: contact.email,
            phone: contact.phone,
            telegram: contact.telegram,
            whatsapp: contact.whatsapp,
            notes: contact.notes,
            tags: contact.tags,
            created_at: now,
            updated_at: now,
            created_on_node: self.node_id.clone(),
            updated_on_node: self.node_id.clone(),
            deleted: false,
            vector_clock: vc,
        };
        sqlx::query(
            "INSERT INTO contacts (id, name, email, phone, telegram, whatsapp, notes, tags, \
             created_at, updated_at, created_on_node, updated_on_node, deleted, vector_clock) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(contact.id.to_string())
        .bind(&contact.name)
        .bind(&contact.email)
        .bind(&contact.phone)
        .bind(&contact.telegram)
        .bind(&contact.whatsapp)
        .bind(&contact.notes)
        .bind(serde_json::to_string(&contact.tags)?)
        .bind(contact.created_at)
        .bind(contact.updated_at)
        .bind(&contact.created_on_node)
        .bind(&contact.updated_on_node)
        .bind(serde_json::to_string(&contact.vector_clock)?)
        .execute(&self.pool)
        .await?;
        tracing::info!(contact_id = %contact.id, "contact created");
        Ok(contact)
    }

    async fn update_contact(&self, contact: Contact) -> Result<Contact> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT * FROM contacts WHERE id = ? AND deleted = 0")
            .bind(contact.id.to_string())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| CoreError::NotFound(contact.id.to_string()))?;
        let local = contact_from_row(&row)?;
        // LWW: чужая версия старше локальной — оставляем локальную.
        if contact.updated_at < local.updated_at && contact.updated_on_node != self.node_id {
            return Ok(local);
        }
        let mut updated = contact;
        updated.updated_at = now_ts();
        updated.updated_on_node = self.node_id.clone();
        updated.vector_clock.tick(&self.node_id);
        sqlx::query(
            "UPDATE contacts SET name = ?, email = ?, phone = ?, telegram = ?, whatsapp = ?, \
             notes = ?, tags = ?, updated_at = ?, updated_on_node = ?, vector_clock = ? WHERE id = ?",
        )
        .bind(&updated.name)
        .bind(&updated.email)
        .bind(&updated.phone)
        .bind(&updated.telegram)
        .bind(&updated.whatsapp)
        .bind(&updated.notes)
        .bind(serde_json::to_string(&updated.tags)?)
        .bind(updated.updated_at)
        .bind(&updated.updated_on_node)
        .bind(serde_json::to_string(&updated.vector_clock)?)
        .bind(updated.id.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(contact_id = %updated.id, "contact updated");
        Ok(updated)
    }

    async fn delete_contact(&self, id: Uuid) -> Result<()> {
        // Soft delete (Контракт 3).
        let result = sqlx::query(
            "UPDATE contacts SET deleted = 1, updated_at = ?, updated_on_node = ? WHERE id = ? AND deleted = 0",
        )
        .bind(now_ts())
        .bind(&self.node_id)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(id.to_string()));
        }
        tracing::info!(contact_id = %id, "contact soft-deleted");
        Ok(())
    }

    async fn get_contact(&self, id: Uuid) -> Result<Option<Contact>> {
        let row = sqlx::query("SELECT * FROM contacts WHERE id = ? AND deleted = 0")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(contact_from_row).transpose()
    }

    async fn get_contacts_since(&self, timestamp: i64) -> Result<Vec<Contact>> {
        // Одобрено 2026-10-15: только неудалённые, по времени изменения.
        let rows = sqlx::query(
            "SELECT * FROM contacts WHERE updated_at > ? AND deleted = 0 ORDER BY updated_at ASC",
        )
        .bind(timestamp)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(contact_from_row).collect()
    }

    async fn count_contacts(&self) -> Result<u64> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM contacts WHERE deleted = 0")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get::<i64, _>("count")? as u64)
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<Contact>> {
        let row = sqlx::query("SELECT * FROM contacts WHERE email = ? AND deleted = 0 LIMIT 1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(contact_from_row).transpose()
    }

    async fn find_by_name(&self, name: &str) -> Result<Vec<Contact>> {
        let rows = sqlx::query("SELECT * FROM contacts WHERE name = ? AND deleted = 0")
            .bind(name)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(contact_from_row).collect()
    }

    async fn search_contacts(&self, query: &str, limit: usize) -> Result<Vec<Contact>> {
        // FTS5 (unicode61) по name/notes. Пустой запрос — пустой результат.
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT c.* FROM contacts_fts f JOIN contacts c ON c.rowid = f.rowid \
             WHERE contacts_fts MATCH ? AND c.deleted = 0 \
             ORDER BY bm25(contacts_fts) LIMIT ?",
        )
        .bind(q)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(contact_from_row).collect()
    }

    async fn list_contacts(&self, limit: usize, offset: usize) -> Result<Vec<Contact>> {
        let rows = sqlx::query(
            "SELECT * FROM contacts WHERE deleted = 0 ORDER BY updated_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(contact_from_row).collect()
    }
}
