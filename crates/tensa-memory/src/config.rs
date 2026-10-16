//! Секция [memory] конфигурации (Контракт 4) — то, что отображается
//! на MemoryConfig крейта.

use serde::{Deserialize, Serialize};

/// Настройки памяти для config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfigSection {
    /// Размер чанка в символах.
    pub chunk_size: usize,
    /// Перекрытие чанков.
    pub chunk_overlap: usize,
    /// Модель эмбеддингов.
    pub embedding_model: String,
    /// Размерность эмбеддинга.
    pub embedding_dim: usize,
    /// Тип индекса: flat | ivf_pq | hnsw.
    pub index_type: String,
    /// Максимальный размер файла (МБ).
    pub max_file_size_mb: u64,
}

impl Default for MemoryConfigSection {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 200,
            embedding_model: "intfloat/multilingual-e5-small".into(),
            embedding_dim: 384,
            index_type: "flat".into(),
            max_file_size_mb: 100,
        }
    }
}

impl MemoryConfigSection {
    /// Преобразовать в рабочую конфигурацию крейта.
    pub fn into_memory_config(&self, search_cache_ttl_seconds: u64) -> crate::models::MemoryConfig {
        crate::models::MemoryConfig {
            chunk_size: self.chunk_size,
            chunk_overlap: self.chunk_overlap,
            embedding_model: self.embedding_model.clone(),
            embedding_dim: self.embedding_dim,
            index_type: match self.index_type.as_str() {
                "ivf_pq" => crate::models::IndexType::IvfPq,
                "hnsw" => crate::models::IndexType::Hnsw,
                _ => crate::models::IndexType::Flat,
            },
            max_file_size_mb: self.max_file_size_mb,
            search_cache_ttl_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_contract() {
        let s = MemoryConfigSection::default();
        assert_eq!(s.chunk_size, 1000);
        assert_eq!(s.chunk_overlap, 200);
        assert_eq!(s.embedding_dim, 384);
        let cfg = s.into_memory_config(3600);
        assert_eq!(cfg.search_cache_ttl_seconds, 3600);
        assert_eq!(cfg.index_type, crate::models::IndexType::Flat);
    }

    #[test]
    fn index_type_parsing() {
        let s = MemoryConfigSection {
            index_type: "hnsw".into(),
            ..MemoryConfigSection::default()
        };
        assert_eq!(
            s.into_memory_config(60).index_type,
            crate::models::IndexType::Hnsw
        );
    }
}
