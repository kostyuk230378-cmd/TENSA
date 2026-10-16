//! Поиск: RRF (Reciprocal Rank Fusion) для гибридного поиска.

use crate::models::SearchResult;
use std::collections::HashMap;

/// Константа RRF (стандартная k=60).
pub const RRF_K: f32 = 60.0;

/// Слияние двух ранжированных списков через RRF.
/// score = sum(1 / (k + rank_i)), rank с 1.
pub fn rrf_merge(semantic: Vec<SearchResult>, keyword: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    let mut scores: HashMap<uuid::Uuid, (f32, SearchResult)> = HashMap::new();
    let mut add_list = |items: Vec<SearchResult>| {
        for (i, item) in items.into_iter().enumerate() {
            let contribution = 1.0 / (RRF_K + i as f32 + 1.0);
            let entry = scores
                .entry(item.document_id)
                .and_modify(|(s, _)| *s += contribution);
            match entry {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    // Чанк тот же — оставляем больший исходный score.
                    let (_, existing) = e.get_mut();
                    if item.score > existing.score {
                        *existing = item;
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert((contribution, item));
                }
            }
        }
    };
    add_list(semantic);
    add_list(keyword);
    let mut merged: Vec<(f32, SearchResult)> = scores.into_values().collect();
    merged.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    merged.into_iter().take(limit).map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn result(id: Uuid, score: f32) -> SearchResult {
        SearchResult {
            document_id: id,
            source_path: "/doc".into(),
            chunk_text: "текст".into(),
            score,
            indexed_at: 0,
        }
    }

    #[test]
    fn rrf_prefers_items_in_both_lists() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let semantic = vec![result(a, 0.9), result(b, 0.5)];
        let keyword = vec![result(b, 0.0), result(c, 0.0)];
        let merged = rrf_merge(semantic, keyword, 10);
        // b — в обоих списках, должен быть первым.
        assert_eq!(merged[0].document_id, b);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn rrf_respects_limit() {
        let ids: Vec<SearchResult> = (0..10)
            .map(|_| result(Uuid::new_v4(), 0.1))
            .collect();
        let merged = rrf_merge(ids.clone(), Vec::new(), 3);
        assert_eq!(merged.len(), 3);
    }
}
