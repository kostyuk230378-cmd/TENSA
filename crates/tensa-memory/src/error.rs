//! Ошибки крейта (thiserror).

use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, MemoryError>;

/// Ошибки tensa-memory.
#[derive(Debug, Error)]
pub enum MemoryError {
    /// Документ слишком большой (max_file_size_mb).
    #[error("document too large: {size_bytes} bytes (limit {limit_bytes})")]
    DocumentTooLarge { size_bytes: i64, limit_bytes: i64 },

    /// Ошибка эмбеддинга.
    #[error("embedding error: {0}")]
    Embedding(String),

    /// Ошибка векторного хранилища (Lance).
    #[error("vector store error: {0}")]
    VectorStore(String),

    /// Ошибка кэша.
    #[error("cache error: {0}")]
    Cache(String),

    /// Конфликт версий (векторные часы).
    #[error("version conflict on {0}")]
    VersionConflict(String),

    /// Превышен размер блока core memory (max_block_tokens).
    #[error("block size exceeded: {chars} chars / ~{tokens} tokens (limit {limit_tokens})")]
    BlockSizeExceeded {
        chars: i32,
        tokens: i32,
        limit_tokens: u32,
    },

    /// Блок не найден.
    #[error("block not found: {0}")]
    BlockNotFound(String),

    /// Ошибка базы данных.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Ошибка сериализации.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Некорректная конфигурация.
    #[error("config error: {0}")]
    Config(String),

    /// Некорректный UUID.
    #[error("invalid uuid: {0}")]
    InvalidUuid(#[from] uuid::Error),

    /// Ошибка ввода-вывода.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
