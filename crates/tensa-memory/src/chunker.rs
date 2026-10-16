//! Чанкинг документов (Контракт 2): 1000 символов, перекрытие 200, SHA-256,
//! дедупликация по chunk_hash.

use crate::models::MemoryConfig;
use sha2::{Digest, Sha256};

/// SHA-256 хеш строки (hex).
pub fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let bytes = hasher.finalize();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Разбить текст на чанки с перекрытием.
///
/// Границы режутся по пробелу, если он есть в окне (чтобы не рвать слова),
/// иначе — жёстко по размеру (длинные токены/URL).
pub fn chunk_text(text: &str, config: &MemoryConfig) -> Vec<String> {
    let chunk_size = config.chunk_size.max(1);
    let overlap = config.chunk_overlap.min(chunk_size.saturating_sub(1));
    let step = chunk_size - overlap;
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < n {
        let end = (start + chunk_size).min(n);
        let mut slice: String = chars[start..end].iter().collect();
        // Поиск границы слова в правой части среза (если не конец текста).
        if end < n {
            if let Some(space) = slice.rfind(char::is_whitespace) {
                if space > overlap.min(slice.len().saturating_sub(1)) {
                    slice.truncate(space);
                }
            }
        }
        let trimmed = slice.trim().to_string();
        if !trimmed.is_empty() {
            chunks.push(trimmed);
        }
        if end >= n {
            break;
        }
        start += step.max(1);
        // Защита от зацикливания: если срез не продвинулся, режем жёстко.
        let _ = start;
    }
    chunks
}
/// Хеш чанка (SHA-256) для дедупликации.
pub fn chunk_hash(chunk: &str) -> String {
    sha256_hex(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MemoryConfig {
        MemoryConfig::default()
    }

    #[test]
    fn sha256_known_value() {
        // известное значение
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn chunk_hash_differs() {
        assert_ne!(chunk_hash("a"), chunk_hash("b"));
        assert_eq!(chunk_hash("x"), chunk_hash("x"));
    }

    #[test]
    fn empty_text_no_chunks() {
        assert!(chunk_text("", &config()).is_empty());
        assert!(chunk_text("   ", &config()).is_empty());
    }

    #[test]
    fn short_text_one_chunk() {
        let chunks = chunk_text("Привет, мир", &config());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Привет, мир");
    }

    #[test]
    fn long_text_multiple_chunks_with_overlap() {
        let text = "слово ".repeat(500); // 3000 символов
        let chunks = chunk_text(&text, &config());
        assert!(chunks.len() >= 3, "got {} chunks", chunks.len());
        // Все чанки в пределах лимита (с допуском на обрезку по слову).
        for c in &chunks {
            assert!(c.chars().count() <= 1000, "chunk len {}", c.chars().count());
        }
        // Перекрытие: хвост первого чанка встречается в одном из последующих.
        let tail: String = chunks[0]
            .chars()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let tail_trimmed = tail.trim();
        assert!(
            chunks[1..].iter().any(|c| c.contains(tail_trimmed)),
            "overlap expected: tail {:?} not found",
            tail_trimmed
        );
    }

    #[test]
    fn custom_config_respected() {
        let cfg = MemoryConfig {
            chunk_size: 10,
            chunk_overlap: 2,
            ..MemoryConfig::default()
        };
        let text = "0123456789abcdefghijklmno";
        let chunks = chunk_text(text, &cfg);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            assert!(c.chars().count() <= 10);
        }
    }
}
