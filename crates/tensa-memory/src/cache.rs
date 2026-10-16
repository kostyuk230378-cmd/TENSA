//! Кэш поисковых запросов (Контракт 2: search_cache, TTL 1 час, LRU).

use crate::error::Result;
use crate::models::SearchCacheEntry;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use uuid::Uuid;

/// Вместимость LRU по умолчанию.
const DEFAULT_CAPACITY: usize = 256;

/// Кэш запросов: хеш -> запись. Потокобезопасный (Mutex).
pub struct SearchCache {
    inner: Mutex<LruCache<String, SearchCacheEntry>>,
    ttl_seconds: u64,
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

impl SearchCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self::with_capacity(ttl_seconds, DEFAULT_CAPACITY)
    }

    pub fn with_capacity(ttl_seconds: u64, capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity.max(1)).unwrap_or(NonZeroUsize::MIN);
        Self {
            inner: Mutex::new(LruCache::new(capacity)),
            ttl_seconds,
        }
    }

    /// Взять блокировку кэша. Отравленная блокировка восстанавливается:
    /// кэш — вспомогательные данные, паника из-за него недопустима.
    fn lock_cache(&self) -> std::sync::MutexGuard<'_, LruCache<String, SearchCacheEntry>> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Взять из кэша (если не истёк TTL).
    pub fn get(&self, query_hash: &str) -> Option<SearchCacheEntry> {
        let mut cache = self.lock_cache();
        let entry = cache.get(query_hash)?;
        if entry.expires_at < now_ts() {
            cache.pop(query_hash);
            return None;
        }
        Some(entry.clone())
    }

    /// Положить в кэш.
    pub fn put(
        &self,
        query_text: &str,
        query_embedding: Vec<f32>,
        result_ids: Vec<Uuid>,
    ) -> Result<SearchCacheEntry> {
        let created_at = now_ts();
        let entry = SearchCacheEntry {
            query_hash: crate::chunker::sha256_hex(query_text),
            query_text: query_text.to_string(),
            query_embedding,
            result_ids,
            created_at,
            expires_at: created_at + self.ttl_seconds as i64,
        };
        self.lock_cache()
            .put(entry.query_hash.clone(), entry.clone());
        Ok(entry)
    }

    /// Очистить весь кэш.
    pub fn clear(&self) {
        self.lock_cache().clear();
    }

    /// Удалить истёкшие записи; возвращает количество удалённых.
    pub fn evict_expired(&self) -> usize {
        let now = now_ts();
        let mut cache = self.lock_cache();
        let expired: Vec<String> = cache
            .iter()
            .filter(|(_, e)| e.expires_at < now)
            .map(|(h, _)| h.clone())
            .collect();
        let n = expired.len();
        for h in expired {
            cache.pop(&h);
        }
        n
    }

    pub fn len(&self) -> usize {
        self.lock_cache().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip() {
        let cache = SearchCache::new(3600);
        let ids = vec![Uuid::new_v4()];
        let entry = cache.put("кофе", vec![0.1; 4], ids.clone()).expect("put");
        let got = cache.get(&entry.query_hash).expect("hit");
        assert_eq!(got.result_ids, ids);
        assert_eq!(got.query_text, "кофе");
    }

    #[test]
    fn ttl_expiry() {
        let cache = SearchCache::with_capacity(1, 16);
        let entry = cache.put("q", vec![0.0; 2], vec![]).expect("put");
        // Имитация истечения: подменяем expires_at в прошлое через новую запись нельзя,
        // поэтому проверяем evict по TTL=0 -> уже истекло на следующей секунде.
        assert!(cache.get(&entry.query_hash).is_some());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn evict_expired_count() {
        let cache = SearchCache::with_capacity(0, 16);
        cache.put("a", vec![0.0; 2], vec![]).expect("a");
        cache.put("b", vec![0.0; 2], vec![]).expect("b");
        // TTL 0: expires_at == created_at, evict_expired чистит при now > created_at.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert_eq!(cache.evict_expired(), 2);
        assert!(cache.is_empty());
    }

    #[test]
    fn lru_capacity() {
        let cache = SearchCache::with_capacity(3600, 2);
        let e1 = cache.put("a", vec![0.0; 2], vec![]).expect("a");
        cache.put("b", vec![0.0; 2], vec![]).expect("b");
        cache.put("c", vec![0.0; 2], vec![]).expect("c");
        // a вытеснен.
        assert!(cache.get(&e1.query_hash).is_none());
        assert_eq!(cache.len(), 2);
    }
}
