//! Core memory — SQLite-реализация (Контракт 13).
//! Блоки всегда в контексте LLM; редактируются агентом через tool calls.

use crate::error::{MemoryError, Result};
use crate::models::{
    BlockType, CoreMemoryBlock, CoreMemoryConfig, CoreMemoryHistory, MemoryOperation, MemoryOrigin,
};
use crate::store::CoreMemoryStore;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

const SCHEMA_SQL: &str = include_str!("../schema.sql");

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Оценка числа токенов: ~4 символа на токен (унифицированно, Контракт 13).
pub fn estimate_tokens(text: &str) -> i32 {
    (text.chars().count() as i32 / 4).max(1)
}

/// SQLite-хранилище core memory (~/.tensa/memory.db).
pub struct SqliteCoreMemoryStore {
    pool: SqlitePool,
    config: std::sync::RwLock<CoreMemoryConfig>,
    node_id: String,
}

fn block_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<CoreMemoryBlock> {
    let block_str: String = row.try_get("block")?;
    Ok(CoreMemoryBlock {
        id: Uuid::from_str(&row.try_get::<String, _>("id")?)?,
        block: BlockType::from_db(&block_str)?,
        value: row.try_get("value")?,
        size_chars: row.try_get("size_chars")?,
        size_tokens: row.try_get("size_tokens")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        updated_by: MemoryOrigin::Llm, // восстанавливается ниже по строке
        updated_on_node: row.try_get("updated_on_node")?,
        vector_clock: serde_json::from_str(&row.try_get::<String, _>("vector_clock")?)?,
    })
}

fn origin_str_of(row: &sqlx::sqlite::SqliteRow) -> Result<MemoryOrigin> {
    let s: String = row.try_get("updated_by")?;
    Ok(match s.as_str() {
        "user" => MemoryOrigin::User,
        "system" => MemoryOrigin::System,
        _ => MemoryOrigin::Llm,
    })
}

impl SqliteCoreMemoryStore {
    /// Открыть (или создать) хранилище по пути БД.
    pub async fn new(db_path: &Path, config: CoreMemoryConfig, node_id: String) -> Result<Self> {
        let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self {
            pool,
            config: std::sync::RwLock::new(config),
            node_id,
        };
        store.run_schema().await?;
        Ok(store)
    }

    /// Хранилище в памяти (для тестов).
    pub async fn new_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self {
            pool,
            config: std::sync::RwLock::new(CoreMemoryConfig::default()),
            node_id: "local".into(),
        };
        store.run_schema().await?;
        Ok(store)
    }

    async fn run_schema(&self) -> Result<()> {
        crate::run_schema_statements(&self.pool, SCHEMA_SQL).await
    }

    /// Записать событие в историю.
    async fn push_history(
        &self,
        block: BlockType,
        old_value: Option<&str>,
        new_value: &str,
        origin: MemoryOrigin,
        operation: MemoryOperation,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO core_memory_history (block, old_value, new_value, changed_at, changed_by, changed_on_node, operation) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(block.as_str())
        .bind(old_value)
        .bind(new_value)
        .bind(now_ts())
        .bind(origin.as_str())
        .bind(&self.node_id)
        .bind(operation.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl CoreMemoryStore for SqliteCoreMemoryStore {
    async fn get_block(&self, block: BlockType) -> Result<CoreMemoryBlock> {
        let row = sqlx::query("SELECT * FROM core_memory_blocks WHERE block = ?")
            .bind(block.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| MemoryError::BlockNotFound(block.as_str().to_string()))?;
        let mut b = block_from_row(&row)?;
        b.updated_by = origin_str_of(&row)?;
        Ok(b)
    }

    async fn get_all_blocks(&self) -> Result<Vec<CoreMemoryBlock>> {
        let rows = sqlx::query("SELECT * FROM core_memory_blocks ORDER BY created_at ASC")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut b = block_from_row(row)?;
            b.updated_by = origin_str_of(row)?;
            out.push(b);
        }
        Ok(out)
    }

    async fn replace_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock> {
        self.validate_size(block, &value).await?;
        let old = match self.get_block(block).await {
            Ok(b) => Some(b),
            Err(MemoryError::BlockNotFound(_)) => None,
            Err(e) => return Err(e),
        };
        let now = now_ts();
        let mut vc = old
            .as_ref()
            .map(|b| b.vector_clock.clone())
            .unwrap_or_default();
        vc.tick(&self.node_id);
        let (chars, tokens) = (value.chars().count() as i32, estimate_tokens(&value));

        if let Some(old_block) = &old {
            sqlx::query(
                "UPDATE core_memory_blocks SET value = ?, size_chars = ?, size_tokens = ?, updated_at = ?, updated_by = ?, updated_on_node = ?, vector_clock = ? WHERE block = ?",
            )
            .bind(&value)
            .bind(chars)
            .bind(tokens)
            .bind(now)
            .bind(origin.as_str())
            .bind(&self.node_id)
            .bind(serde_json::to_string(&vc)?)
            .bind(block.as_str())
            .execute(&self.pool)
            .await?;
            self.push_history(
                block,
                Some(&old_block.value),
                &value,
                origin,
                MemoryOperation::Replace,
            )
            .await?;
            let mut updated = old_block.clone();
            updated.value = value;
            updated.size_chars = chars;
            updated.size_tokens = tokens;
            updated.updated_at = now;
            updated.updated_by = origin;
            updated.updated_on_node = self.node_id.clone();
            updated.vector_clock = vc;
            info!(block = block.as_str(), "core memory replaced");
            Ok(updated)
        } else {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO core_memory_blocks (id, block, value, size_chars, size_tokens, created_at, updated_at, updated_by, updated_on_node, vector_clock) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id.to_string())
            .bind(block.as_str())
            .bind(&value)
            .bind(chars)
            .bind(tokens)
            .bind(now)
            .bind(now)
            .bind(origin.as_str())
            .bind(&self.node_id)
            .bind(serde_json::to_string(&vc)?)
            .execute(&self.pool)
            .await?;
            self.push_history(block, None, &value, origin, MemoryOperation::Replace)
                .await?;
            info!(block = block.as_str(), "core memory created");
            Ok(CoreMemoryBlock {
                id,
                block,
                value,
                size_chars: chars,
                size_tokens: tokens,
                created_at: now,
                updated_at: now,
                updated_by: origin,
                updated_on_node: self.node_id.clone(),
                vector_clock: vc,
            })
        }
    }

    async fn append_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock> {
        let existing = match self.get_block(block).await {
            Ok(b) => b,
            Err(MemoryError::BlockNotFound(_)) => {
                return self.replace_block(block, value, origin).await;
            }
            Err(e) => return Err(e),
        };
        let combined = format!("{}\n{}", existing.value, value);
        self.replace_block(block, combined, origin).await?;
        self.push_history(
            block,
            Some(&existing.value),
            &value,
            origin,
            MemoryOperation::Append,
        )
        .await?;
        // replace_block вернул уже объединённый блок; перечитываем для консистентности.
        self.get_block(block).await
    }

    async fn get_history(&self, block: BlockType, limit: usize) -> Result<Vec<CoreMemoryHistory>> {
        let rows = sqlx::query(
            "SELECT * FROM core_memory_history WHERE block = ? ORDER BY id DESC LIMIT ?",
        )
        .bind(block.as_str())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let block_str: String = row.try_get("block")?;
            let changed_by: String = row.try_get("changed_by")?;
            let operation: String = row.try_get("operation")?;
            out.push(CoreMemoryHistory {
                id: row.try_get("id")?,
                block: BlockType::from_db(&block_str)?,
                old_value: row.try_get("old_value")?,
                new_value: row.try_get("new_value")?,
                changed_at: row.try_get("changed_at")?,
                changed_by: match changed_by.as_str() {
                    "user" => MemoryOrigin::User,
                    "system" => MemoryOrigin::System,
                    _ => MemoryOrigin::Llm,
                },
                changed_on_node: row.try_get("changed_on_node")?,
                operation: match operation.as_str() {
                    "append" => MemoryOperation::Append,
                    _ => MemoryOperation::Replace,
                },
            });
        }
        Ok(out)
    }

    async fn get_config(&self) -> Result<CoreMemoryConfig> {
        let config = self
            .config
            .read()
            .map_err(|_| MemoryError::Config("блокировка конфигурации отравлена".to_string()))?;
        Ok(config.clone())
    }

    async fn update_config(&self, config: CoreMemoryConfig) -> Result<()> {
        *self
            .config
            .write()
            .map_err(|_| MemoryError::Config("блокировка конфигурации отравлена".to_string()))? =
            config;
        Ok(())
    }

    async fn validate_size(&self, block: BlockType, value: &str) -> Result<()> {
        let tokens = estimate_tokens(value);
        let limit = self
            .config
            .read()
            .map_err(|_| MemoryError::Config("блокировка конфигурации отравлена".to_string()))?
            .max_tokens_per_block;
        if tokens > limit as i32 {
            return Err(MemoryError::BlockSizeExceeded {
                chars: value.chars().count() as i32,
                tokens,
                limit_tokens: limit,
            });
        }
        tracing::debug!(block = block.as_str(), tokens, "block size ok");
        Ok(())
    }
}

/// Публичный helper: инициализация дефолтных блоков (init_default, dop_tz).
pub async fn init_default_blocks(
    store: &SqliteCoreMemoryStore,
    blocks: Vec<(BlockType, String)>,
) -> Result<()> {
    for (block, value) in blocks {
        if matches!(
            store.get_block(block).await,
            Err(MemoryError::BlockNotFound(_))
        ) {
            store
                .replace_block(block, value, MemoryOrigin::System)
                .await?;
        }
    }
    Ok(())
}
