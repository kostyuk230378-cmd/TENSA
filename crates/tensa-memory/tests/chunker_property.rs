//! Property-тесты чанкера (Фаза 5, 02-testing.md): инварианты на случайных
//! текстах — чанки не теряют данные (каждый чанк находим в тексте,
//! покрытие достигает конца), ограничены размером, без пустых чанков.

use proptest::prelude::*;
use tensa_memory::{chunk_hash, chunk_text, MemoryConfig};

fn config() -> MemoryConfig {
    MemoryConfig {
        chunk_size: 120,
        chunk_overlap: 20,
        ..MemoryConfig::default()
    }
}

/// Стратегия: текст из случайных слов через пробел (0..60 слов).
fn words_strategy() -> impl Strategy<Value = String> {
    proptest::collection::vec("[а-яa-z0-9]{1,12}", 0..60).prop_map(|words| words.join(" "))
}

proptest! {
    /// Каждый чанк — подстрока исходного текста (без искажений).
    #[test]
    fn chunks_are_substrings_of_source(words in words_strategy()) {
        let config = config();
        let text = words.trim().to_string();
        let chunks = chunk_text(&text, &config);
        for chunk in &chunks {
            prop_assert!(text.contains(chunk.as_str()), "чанк не найден в тексте: {chunk:?}");
        }
    }

    /// Чанки ограничены размером и не пусты (для непустого текста).
    #[test]
    fn chunks_are_bounded_and_non_empty(words in words_strategy()) {
        let config = config();
        let text = words.trim().to_string();
        let chunks = chunk_text(&text, &config);
        if text.is_empty() {
            prop_assert!(chunks.is_empty());
        } else {
            prop_assert!(!chunks.is_empty(), "непустой текст без чанков");
            for chunk in &chunks {
                prop_assert!(!chunk.is_empty());
                prop_assert!(
                    chunk.chars().count() <= config.chunk_size,
                    "чанк {} длиннее лимита {}",
                    chunk.chars().count(),
                    config.chunk_size
                );
            }
        }
    }

    /// Голова и хвост текста не теряются: первый чанк — префикс,
    /// последний — суффикс.
    #[test]
    fn first_chunk_is_prefix_and_last_is_suffix(words in words_strategy()) {
        let config = config();
        let text = words.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }
        let chunks = chunk_text(&text, &config);
        prop_assert!(!chunks.is_empty());
        prop_assert!(
            text.starts_with(chunks[0].as_str()),
            "первый чанк не префикс"
        );
        prop_assert!(
            text.ends_with(chunks[chunks.len() - 1].as_str()),
            "последний чанк не суффикс"
        );
    }

    /// Покрытие: объединение всех вхождений чанков покрывает все
    /// непробельные символы текста — контент не теряется между чанками.
    #[test]
    fn chunks_cover_text_without_gaps(words in words_strategy()) {
        let config = config();
        let text = words.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }
        let chunks = chunk_text(&text, &config);
        prop_assert!(!chunks.is_empty());

        // Отметить все вхождения всех чанков (включая перекрытия).
        let chars: Vec<char> = text.chars().collect();
        let mut covered = vec![false; chars.len()];
        for chunk in &chunks {
            let needle: Vec<char> = chunk.chars().collect();
            if needle.is_empty() {
                continue;
            }
            for start in 0..=(chars.len() - needle.len()) {
                if chars[start..start + needle.len()] == needle[..] {
                    for covered in &mut covered[start..start + needle.len()] {
                        *covered = true;
                    }
                }
            }
        }
        // Каждый непробельный символ должен быть покрыт хотя бы одним чанком.
        for (index, ch) in chars.iter().enumerate() {
            if !ch.is_whitespace() {
                prop_assert!(
                    covered[index],
                    "символ {index} ({ch:?}) не покрыт ни одним чанком"
                );
            }
        }
    }

    /// Хеш чанка детерминирован и различает разные чанки.
    #[test]
    fn chunk_hash_is_deterministic(a in "[а-яa-z0-9 ]{1,200}", b in "[а-яa-z0-9 ]{1,200}") {
        prop_assume!(a != b, "ожидались разные строки");
        prop_assert_eq!(chunk_hash(&a), chunk_hash(&a));
        prop_assert_ne!(chunk_hash(&a), chunk_hash(&b));
    }
}
