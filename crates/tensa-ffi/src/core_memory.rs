//! `CoreMemoryStore` (Контракт 9, ADR-021): блоки core memory поверх tensa-memory.

use tensa_memory::{
    BlockType as CoreBlockType, CoreMemoryConfig, CoreMemoryStore as CoreMemoryStoreTrait,
    MemoryOrigin as CoreMemoryOrigin, SqliteCoreMemoryStore,
};

use crate::convert::{data_root, load_core_config, uuid_str};
use crate::error::TensaError;
use crate::types::{BlockType, CoreMemoryBlock as FfiBlock, MemoryOrigin};

/// FFI-обёртка core memory (`interface CoreMemoryStore` в UDL).
pub struct CoreMemoryStore {
    inner: SqliteCoreMemoryStore,
}

impl CoreMemoryStore {
    /// Открыть хранилище (база — `<root>/memory.db`).
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        let config = load_core_config(&config_path)?;
        let store = SqliteCoreMemoryStore::new(
            &data_root(&config_path).join("memory.db"),
            CoreMemoryConfig::default(),
            config.general.node_id.clone(),
        )
        .await?;
        Ok(Self { inner: store })
    }

    /// Блок по типу.
    pub async fn get_block(&self, block: BlockType) -> Result<FfiBlock, TensaError> {
        let found = self.inner.get_block(block.to_core()).await?;
        Ok(to_block(found))
    }

    /// Все блоки памяти.
    pub async fn get_all_blocks(&self) -> Result<Vec<FfiBlock>, TensaError> {
        let blocks = self.inner.get_all_blocks().await?;
        Ok(blocks.into_iter().map(to_block).collect())
    }

    /// Заменить значение блока.
    pub async fn replace_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<FfiBlock, TensaError> {
        let updated = self
            .inner
            .replace_block(block.to_core(), value, origin.to_core())
            .await?;
        Ok(to_block(updated))
    }

    /// Дописать значение к блоку.
    pub async fn append_block(
        &self,
        block: BlockType,
        value: String,
        origin: MemoryOrigin,
    ) -> Result<FfiBlock, TensaError> {
        let updated = self
            .inner
            .append_block(block.to_core(), value, origin.to_core())
            .await?;
        Ok(to_block(updated))
    }
}

impl BlockType {
    /// UDL-представление из core-типа.
    pub fn from_core(block: CoreBlockType) -> Self {
        match block {
            CoreBlockType::Persona => BlockType::Persona {},
            CoreBlockType::Human => BlockType::Human {},
            CoreBlockType::Tasks => BlockType::Tasks {},
            CoreBlockType::Preferences => BlockType::Preferences {},
        }
    }

    /// Core-представление типа блока.
    pub fn to_core(self) -> CoreBlockType {
        match self {
            BlockType::Persona {} => CoreBlockType::Persona,
            BlockType::Human {} => CoreBlockType::Human,
            BlockType::Tasks {} => CoreBlockType::Tasks,
            BlockType::Preferences {} => CoreBlockType::Preferences,
        }
    }
}

impl MemoryOrigin {
    /// UDL-представление из core-типа.
    pub fn from_core(origin: CoreMemoryOrigin) -> Self {
        match origin {
            CoreMemoryOrigin::Llm => MemoryOrigin::Llm {},
            CoreMemoryOrigin::User => MemoryOrigin::User {},
            CoreMemoryOrigin::System => MemoryOrigin::System {},
        }
    }

    /// Core-представление источника изменения.
    pub fn to_core(self) -> CoreMemoryOrigin {
        match self {
            MemoryOrigin::Llm {} => CoreMemoryOrigin::Llm,
            MemoryOrigin::User {} => CoreMemoryOrigin::User,
            MemoryOrigin::System {} => CoreMemoryOrigin::System,
        }
    }
}

/// Конвертация блока ядра в UDL-представление.
fn to_block(block: tensa_memory::CoreMemoryBlock) -> FfiBlock {
    FfiBlock {
        id: uuid_str(block.id),
        block: BlockType::from_core(block.block),
        value: block.value,
        size_chars: block.size_chars,
        size_tokens: block.size_tokens,
        created_at: block.created_at,
        updated_at: block.updated_at,
        updated_by: MemoryOrigin::from_core(block.updated_by),
        updated_on_node: block.updated_on_node,
    }
}
