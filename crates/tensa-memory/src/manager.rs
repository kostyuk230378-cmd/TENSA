//! MemoryManager — координатор трёх уровней памяти (dop_tz, ADR-021).

use crate::error::Result;
use crate::models::{
    BlockType, CoreMemoryBlock, MemoryOrigin, MemorySearchResult, MemorySource, RecallMessage,
};
use crate::store::{
    ArchivalMemoryStore, CoreMemoryStore, MemoryContext, MemoryManagerConfig, RecallMemoryStore,
};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

/// Координатор трёх уровней памяти (dop_tz, сигнатуры дословно).
#[async_trait]
pub trait MemoryManager: Send + Sync {
    // Core memory
    async fn core_get(&self, block: BlockType) -> Result<CoreMemoryBlock>;
    async fn core_get_all(&self) -> Result<Vec<CoreMemoryBlock>>;
    async fn core_replace(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;
    async fn core_append(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock>;

    // Recall memory
    async fn recall_append(&self, message: RecallMessage) -> Result<()>;
    async fn recall_search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>>;
    async fn recall_recent(&self, limit: usize) -> Result<Vec<RecallMessage>>;

    // Archival memory
    async fn archival_insert(&self, entry: crate::models::ArchivalEntry) -> Result<uuid::Uuid>;
    async fn archival_search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>>;

    // Общее
    async fn search_all(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>>;
    /// Получить контекст для LLM (core + recent recall).
    async fn get_context(&self) -> Result<MemoryContext>;
}

/// Реализация по умолчанию (dop_tz).
pub struct DefaultMemoryManager {
    core: Arc<dyn CoreMemoryStore>,
    recall: Arc<dyn RecallMemoryStore>,
    archival: Arc<dyn ArchivalMemoryStore>,
    config: MemoryManagerConfig,
    /// Идентификатор узла — используется при синхронизации (появится с gRPC-слоем).
    #[allow(dead_code)]
    node_id: String,
}

impl DefaultMemoryManager {
    pub async fn new(
        core: Arc<dyn CoreMemoryStore>,
        recall: Arc<dyn RecallMemoryStore>,
        archival: Arc<dyn ArchivalMemoryStore>,
        config: MemoryManagerConfig,
        node_id: String,
    ) -> Result<Self> {
        Ok(Self {
            core,
            recall,
            archival,
            config,
            node_id,
        })
    }

    /// Инициализация при первом запуске: дефолтные блоки core memory
    /// (значения — из Контракта 13; параметры — из tensa-core::init).
    pub async fn init_default(&self, params: &tensa_core::InitParams) -> Result<()> {
        if let Ok(existing) = self.core_get(BlockType::Persona).await {
            if !existing.value.is_empty() {
                info!("core memory already initialized");
                return Ok(());
            }
        }
        for (block, value) in tensa_core::default_blocks(params) {
            use tensa_core::BlockType as CoreBlock;
            let block = match block {
                CoreBlock::Persona => BlockType::Persona,
                CoreBlock::Human => BlockType::Human,
                CoreBlock::Tasks => BlockType::Tasks,
                CoreBlock::Preferences => BlockType::Preferences,
            };
            self.core_replace(block, value, MemoryOrigin::System)
                .await?;
        }
        info!("core memory initialized with default blocks");
        Ok(())
    }
}

#[async_trait]
impl MemoryManager for DefaultMemoryManager {
    async fn core_get(&self, block: BlockType) -> Result<CoreMemoryBlock> {
        self.core.get_block(block).await
    }

    async fn core_get_all(&self) -> Result<Vec<CoreMemoryBlock>> {
        self.core.get_all_blocks().await
    }

    async fn core_replace(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock> {
        self.core.replace_block(block, value, origin).await
    }

    async fn core_append(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<CoreMemoryBlock> {
        self.core.append_block(block, value, origin).await
    }

    async fn recall_append(&self, message: RecallMessage) -> Result<()> {
        self.recall.append_message(message).await
    }

    async fn recall_search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>> {
        self.recall.search(query, limit).await
    }

    async fn recall_recent(&self, limit: usize) -> Result<Vec<RecallMessage>> {
        self.recall.get_recent(limit).await
    }

    async fn archival_insert(&self, entry: crate::models::ArchivalEntry) -> Result<uuid::Uuid> {
        self.archival.insert(entry).await
    }

    async fn archival_search(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>> {
        self.archival.search(query, limit).await
    }

    async fn search_all(&self, query: &str, limit: usize) -> Result<Vec<MemorySearchResult>> {
        // Объединяем core (по подстроке) + recall (FTS) + archival (семантика).
        let mut results: Vec<MemorySearchResult> = Vec::new();
        // Core: блоки короткие — ищем подстрокой (без регистра).
        let q = query.to_lowercase();
        for block in self.core.get_all_blocks().await? {
            if !q.is_empty() && block.value.to_lowercase().contains(&q) {
                results.push(MemorySearchResult {
                    id: block.id,
                    content: block.value,
                    score: 1.0,
                    source: MemorySource::Core,
                    timestamp: block.updated_at,
                });
            }
        }
        let half = limit / 2;
        results.extend(self.recall.search(query, half).await?);
        results.extend(self.archival.search(query, half).await?);
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    async fn get_context(&self) -> Result<MemoryContext> {
        let core_blocks = self.core.get_all_blocks().await?;
        let recent_messages = self
            .recall
            .get_recent(self.config.message_buffer_size)
            .await?;
        let total_tokens: u32 = core_blocks
            .iter()
            .map(|b| b.size_tokens.max(0) as u32)
            .sum::<u32>()
            + recent_messages
                .iter()
                .map(|m| (m.content.chars().count() as u32 / 4).max(1))
                .sum::<u32>();
        Ok(MemoryContext {
            core_blocks,
            recent_messages,
            total_tokens,
        })
    }
}

#[async_trait]
impl tensa_core::MemoryInitializer for DefaultMemoryManager {
    /// Адаптер к обратному трейту tensa-core (без цикла зависимостей).
    async fn init_default(&self, params: &tensa_core::InitParams) -> tensa_core::Result<()> {
        DefaultMemoryManager::init_default(self, params)
            .await
            .map_err(|error| tensa_core::CoreError::Memory(error.to_string()))
    }
}
