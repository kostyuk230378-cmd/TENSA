//! Трейты иерархической памяти (dop_tz, Контракт 13, сигнатуры дословно)
//! + трейт MemoryStore для документов (Контракт 2).

use crate::error::Result;
use crate::models::{
    ArchivalEntry, BlockType, CoreMemoryBlock, CoreMemoryConfig, CoreMemoryHistory, Document,
    DocumentMeta, MemoryOrigin, MemorySearchResult, RecallMessage,
};
use async_trait::async_trait;
use uuid::Uuid;

/// Core memory — всегда в контексте LLM.
#[async_trait]
pub trait CoreMemoryStore: Send + Sync {
    async fn get_block(&self, block: BlockType) -> Result<CoreMemoryBlock>;
    async fn get_all_blocks(&self) -> Result<Vec<CoreMemoryBlock>>;
    async fn replace_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;
    async fn append_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;
    async fn get_history(&self, block: BlockType, limit: usize) -> Result<Vec<CoreMemoryHistory>>;
    async fn get_config(&self) -> Result<CoreMemoryConfig>;
    async fn update_config(&self, config: CoreMemoryConfig) -> Result<()>;
    async fn validate_size(&self, block: BlockType, value: &str) -> Result<()>;
}

/// Recall memory — история разговоров.
#[async_trait]
pub trait RecallMemoryStore: Send + Sync {
    /// Добавить сообщение в историю.
    async fn append_message(&self, message: RecallMessage) -> Result<()>;

    /// Поиск по истории (FTS5).
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>>;

    /// Получить историю сессии.
    async fn get_session(&self, session_id: Uuid, limit: usize) -> Result<Vec<RecallMessage>>;

    /// Получить последние N сообщений.
    async fn get_recent(&self, limit: usize) -> Result<Vec<RecallMessage>>;

    /// Удалить старые сообщения (TTL).
    async fn cleanup_old(&self, older_than_days: u32) -> Result<usize>;
}

/// Archival memory — долговременное хранилище.
#[async_trait]
pub trait ArchivalMemoryStore: Send + Sync {
    /// Добавить запись.
    async fn insert(&self, entry: ArchivalEntry) -> Result<Uuid>;

    /// Поиск по архиву (семантический).
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>>;

    /// Поиск по тегам.
    async fn search_by_tags(
        &self,
        tags: &[String],
        limit: usize,
    ) -> Result<Vec<MemorySearchResult>>;

    /// Удалить запись.
    async fn delete(&self, id: Uuid) -> Result<()>;
}

/// Хранилище документов (Контракт 2, сигнатуры дословно).
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn index_document(&self, doc: Document) -> Result<Vec<crate::models::DocumentChunk>>;
    async fn reindex_document(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<crate::models::DocumentChunk>>;
    async fn delete_document(&self, document_id: Uuid) -> Result<()>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<crate::models::SearchResult>>;
    async fn search_hybrid(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::models::SearchResult>>;
    async fn get_document(&self, document_id: Uuid) -> Result<Option<DocumentMeta>>;
    async fn list_documents(&self, limit: usize, offset: usize) -> Result<Vec<DocumentMeta>>;
    async fn clear_cache(&self) -> Result<()>;
}

/// Конфигурация MemoryManager (dop_tz).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryManagerConfig {
    /// Максимум токенов в core memory (8000).
    pub max_core_tokens: u32,
    /// Максимум токенов на блок core memory (2000).
    pub max_block_tokens: u32,
    /// Размер message buffer (20).
    pub message_buffer_size: usize,
    /// Максимум токенов в контексте (16000).
    pub max_context_tokens: u32,
    /// TTL recall memory, дней (365).
    pub recall_ttl_days: u32,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            max_core_tokens: 8000,
            max_block_tokens: 2000,
            message_buffer_size: 20,
            max_context_tokens: 16000,
            recall_ttl_days: 365,
        }
    }
}

/// Контекст памяти для LLM (dop_tz).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryContext {
    pub core_blocks: Vec<CoreMemoryBlock>,
    pub recent_messages: Vec<RecallMessage>,
    pub total_tokens: u32,
}
