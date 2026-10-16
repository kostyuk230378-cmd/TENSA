//! Модели документов (Контракт 2) — отдельный модуль, чтобы трейты
//! иерархической памяти (models.rs) не тащили модели документов.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Документ для индексации (Контракт 2).
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

/// Метаданные документа (результат get/list).
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
