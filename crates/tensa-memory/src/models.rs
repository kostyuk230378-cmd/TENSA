//! Модели крейта (ТЗ tensa-memory + dop_tz: типы дословно).
//! VectorClock переиспользуется из tensa-approval.

pub use tensa_approval::VectorClock;

use crate::error::MemoryError;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// === Документы и поиск (Контракт 2) ===

/// Конфигурация памяти.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Размер чанка в символах (1000).
    pub chunk_size: usize,
    /// Перекрытие чанков (200).
    pub chunk_overlap: usize,
    /// Модель эмбеддингов.
    pub embedding_model: String,
    /// Размерность эмбеддинга (384).
    pub embedding_dim: usize,
    /// Тип индекса (Flat по ADR-004).
    pub index_type: IndexType,
    /// Максимум размера файла (100 МБ).
    pub max_file_size_mb: u64,
    /// TTL кэша поиска, секунд (3600).
    pub search_cache_ttl_seconds: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 200,
            embedding_model: "intfloat/multilingual-e5-small".into(),
            embedding_dim: 384,
            index_type: IndexType::Flat,
            max_file_size_mb: 100,
            search_cache_ttl_seconds: 3600,
        }
    }
}

/// Тип векторного индекса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexType {
    Flat,
    IvfPq,
    Hnsw,
}

/// Индексированный документ.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub source_path: String,
    pub source_hash: String,
    pub title: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub created_at: i64,
    pub modified_at: i64,
    pub chunks: Vec<DocumentChunk>,
    pub metadata: serde_json::Value,
}

/// Чанк документа.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub chunk_index: i32,
    pub source_path: String,
    pub source_hash: String,
    pub chunk_text: String,
    pub chunk_hash: String,
    pub embedding: Vec<f32>,
    pub metadata: serde_json::Value,
    pub account_id: Option<String>,
    pub indexed_at: i64,
    pub updated_at: i64,
}

/// Метаданные документа (без чанков).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub document_id: Uuid,
    pub source_path: String,
    pub source_hash: String,
    pub title: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub created_at: i64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub chunk_count: i32,
    pub metadata: serde_json::Value,
}

/// Результат поиска по документам.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub document_id: Uuid,
    pub source_path: String,
    pub chunk_text: String,
    pub score: f32,
    pub indexed_at: i64,
}

/// Запись кэша поиска.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCacheEntry {
    pub query_hash: String,
    pub query_text: String,
    pub query_embedding: Vec<f32>,
    pub result_ids: Vec<Uuid>,
    pub created_at: i64,
    pub expires_at: i64,
}

// === Иерархическая память (dop_tz, ADR-021, Контракт 13) ===

/// Тип блока core memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Persona,
    Human,
    Tasks,
    Preferences,
}

impl BlockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockType::Persona => "persona",
            BlockType::Human => "human",
            BlockType::Tasks => "tasks",
            BlockType::Preferences => "preferences",
        }
    }

    pub fn from_db(s: &str) -> Result<Self> {
        s.parse()
    }
}

impl std::str::FromStr for BlockType {
    type Err = MemoryError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "persona" => BlockType::Persona,
            "human" => BlockType::Human,
            "tasks" => BlockType::Tasks,
            "preferences" => BlockType::Preferences,
            other => return Err(MemoryError::Config(format!("unknown block type: {other}"))),
        })
    }
}

/// Источник изменения памяти.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOrigin {
    Llm,
    User,
    System,
}

impl MemoryOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryOrigin::Llm => "llm",
            MemoryOrigin::User => "user",
            MemoryOrigin::System => "system",
        }
    }
}

/// Операция с памятью.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOperation {
    Replace,
    Append,
}

impl MemoryOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryOperation::Replace => "replace",
            MemoryOperation::Append => "append",
        }
    }
}

/// Блок core memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreMemoryBlock {
    pub id: Uuid,
    pub block: BlockType,
    pub value: String,
    pub size_chars: i32,
    pub size_tokens: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub updated_by: MemoryOrigin,
    pub updated_on_node: String,
    pub vector_clock: VectorClock,
}

/// История изменения блока (Контракт 13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreMemoryHistory {
    pub id: i64,
    pub block: BlockType,
    pub old_value: Option<String>,
    pub new_value: String,
    pub changed_at: i64,
    pub changed_by: MemoryOrigin,
    pub changed_on_node: String,
    pub operation: MemoryOperation,
}

/// Конфигурация core memory (Контракт 13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreMemoryConfig {
    pub max_tokens_per_block: u32,
    pub max_total_tokens: u32,
    pub blocks: Vec<BlockType>,
}

impl Default for CoreMemoryConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_block: 2000,
            max_total_tokens: 8000,
            blocks: vec![
                BlockType::Persona,
                BlockType::Human,
                BlockType::Tasks,
                BlockType::Preferences,
            ],
        }
    }
}

/// Роль сообщения в истории.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }
}

/// Запись в recall memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallMessage {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub timestamp: i64,
    pub session_id: Uuid,
    pub node_id: String,
    pub metadata: serde_json::Value,
}

/// Запись в archival memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivalEntry {
    pub id: Uuid,
    pub content: String,
    pub embedding: Vec<f32>,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub node_id: String,
    pub metadata: serde_json::Value,
}

/// Результат поиска в памяти.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub id: Uuid,
    pub content: String,
    pub score: f32,
    pub source: MemorySource,
    pub timestamp: i64,
}

/// Источник результата поиска.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    Core,
    Recall,
    Archival,
}
