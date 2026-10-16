//! Трейт эмбеддера + тривиальная тестовая реализация.
//!
//! Префиксы E5 (Контракт 2): `query:` для запросов, `passage:` для документов.
//! Реальная модель (multilingual-e5-small, 384) — за фичей `fastembed`.

use crate::error::{MemoryError, Result};
use async_trait::async_trait;

/// Поставщик эмбеддингов.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Размерность вектора.
    fn dim(&self) -> usize;

    /// Эмбеддинг запроса (префикс query:).
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;

    /// Эмбеддинг документа (префикс passage:).
    async fn embed_passage(&self, passage: &str) -> Result<Vec<f32>>;
}

/// Детерминированный эмбеддер для тестов и офлайн-работы:
/// хеш текста проецируется в вектор заданной размерности.
/// НЕ семантический — только для контрактов и тестов. [derived]
pub struct HashEmbedder {
    dim: usize,
}

impl HashEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim: dim.max(1) }
    }
}

#[async_trait]
impl Embedder for HashEmbedder {
    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embed_with_prefix("query: ", query)
    }

    async fn embed_passage(&self, passage: &str) -> Result<Vec<f32>> {
        self.embed_with_prefix("passage: ", passage)
    }
}

impl HashEmbedder {
    fn embed_with_prefix(&self, prefix: &str, text: &str) -> Result<Vec<f32>> {
        if text.trim().is_empty() {
            return Err(MemoryError::Embedding("empty text".into()));
        }
        let mut vec = vec![0f32; self.dim];
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Простая проекция: каждый тритет текста влияет на одну координату.
        let chars: Vec<char> = format!("{prefix}{text}").chars().collect();
        for w in chars.windows(3) {
            use std::hash::Hasher;
            hasher.write_u32(u32::from_le_bytes([w[0] as u8, w[1] as u8, w[2] as u8, 0]));
            let h = hasher.finish();
            let idx = (h as usize) % self.dim;
            vec[idx] += ((h >> 32) as i64 % 1000) as f32 / 1000.0;
        }
        // Нормализация L2, чтобы вектор был ненулевым и сравнимым.
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for x in &mut vec {
                *x /= norm;
            }
        } else {
            return Err(MemoryError::Embedding("empty text".into()));
        }
        Ok(vec)
    }
}

/// Косинусная близость (для поиска top-k).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        return Err(MemoryError::Embedding(format!(
            "dim mismatch: {} vs {}",
            a.len(),
            b.len()
        )));
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        return Ok(0.0);
    }
    Ok(dot / (na * nb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hash_embedder_dims_and_determinism() {
        let e = HashEmbedder::new(384);
        assert_eq!(e.dim(), 384);
        let v1 = e.embed_passage("текст документа").await.expect("v1");
        let v2 = e.embed_passage("текст документа").await.expect("v2");
        assert_eq!(v1.len(), 384);
        assert_eq!(v1, v2);
    }

    #[tokio::test]
    async fn query_and_passage_prefixes_differ() {
        let e = HashEmbedder::new(384);
        let q = e.embed_query("кофе").await.expect("q");
        let p = e.embed_passage("кофе").await.expect("p");
        assert_ne!(q, p, "префиксы query:/passage: должны влиять");
    }

    #[tokio::test]
    async fn empty_text_error() {
        let e = HashEmbedder::new(384);
        assert!(e.embed_query("").await.is_err());
    }

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![0.6, 0.8, 0.0];
        let sim = cosine_similarity(&v, &v).expect("sim");
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_dim_mismatch() {
        assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).is_err());
    }
}
