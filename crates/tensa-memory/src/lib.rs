//! # tensa-memory
//!
//! Память Tensa (ТЗ tensa-memory + dop_tz, ADR-021, Контракты 2/13):
//! - **core memory** (SQLite) — блоки persona/human/tasks/preferences, всегда
//!   в контексте LLM;
//! - **recall memory** (SQLite + FTS5) — история разговоров;
//! - **archival memory** (SQLite-векторный fallback / Lance за фичей) — факты;
//! - **MemoryManager** — координатор трёх уровней + tool calls агента;
//! - чанкинг документов (1000/200, SHA-256), кэш поиска (TTL 1 ч, LRU),
//!   трейт `Embedder` (реальная модель multilingual-e5-small — за фичей
//!   `fastembed`).
//!
//! Ограничения ТЗ: не извлекает текст, не выполняет OCR, не синхронизирует,
//! не логирует секреты.

pub mod archival_memory;
pub mod cache;
pub mod chunker;
pub mod config;
pub mod core_memory;
pub mod embedder;
pub mod error;
#[cfg(feature = "fastembed")]
pub mod fastembed_embedder;
pub mod manager;
pub mod models;
pub mod recall_memory;
pub mod store;
pub mod tools;

pub use archival_memory::SqliteArchivalMemoryStore;
pub use cache::SearchCache;
pub use chunker::{chunk_hash, chunk_text, sha256_hex};
pub use config::MemoryConfigSection;
pub use core_memory::{estimate_tokens, init_default_blocks, SqliteCoreMemoryStore};
pub use embedder::{cosine_similarity, Embedder, HashEmbedder};
pub use error::{MemoryError, Result};
#[cfg(feature = "fastembed")]
pub use fastembed_embedder::FastEmbedder;
pub use manager::{DefaultMemoryManager, MemoryManager};
pub use models::{
    ArchivalEntry, BlockType, CoreMemoryBlock, CoreMemoryConfig, CoreMemoryHistory, Document,
    DocumentChunk, DocumentMeta, IndexType, MemoryConfig, MemoryOperation, MemoryOrigin,
    MemorySearchResult, MemorySource, RecallMessage, Role, SearchResult, VectorClock,
};
pub use recall_memory::SqliteRecallMemoryStore;
pub use store::{
    ArchivalMemoryStore, CoreMemoryStore, MemoryContext, MemoryManagerConfig, MemoryStore,
    RecallMemoryStore,
};
pub use tools::{dispatch as dispatch_tool, MEMORY_TOOLS};

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Исполнение SQL-схемы постранично: statement'ы заканчиваются ';' в конце
/// строки; строки комментариев пропускаются. Триггеры FTS5 записаны
/// однострочно (см. schema.sql), поэтому ';' внутри строки безопасен.
pub(crate) async fn run_schema_statements(pool: &sqlx::SqlitePool, schema: &str) -> Result<()> {
    use sqlx::Executor;
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
