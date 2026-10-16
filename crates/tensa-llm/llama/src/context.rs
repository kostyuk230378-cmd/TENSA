//! Контекст инференса: оценка памяти и предварительные проверки.
//! Чистая логика — позволяет честно вернуть `OutOfMemory` до загрузки
//! нативной модели (ТЗ: ошибка «недостаточно RAM»).

use crate::error::{LlmError, Result};

/// Оценка памяти KV-кэша llama.cpp.
///
/// Формула: `2 (K+V) * layers * kv_heads * head_dim * context * bytes_per_elem`.
/// Приблизительна (без учёта выравнивания и служебных буферов) — для
/// предварительной проверки «хватит ли RAM» до загрузки. [derived]
pub fn estimate_kv_cache_bytes(
    context_size: u32,
    n_layers: u32,
    n_kv_heads: u32,
    head_dim: u32,
    bytes_per_elem: u32,
) -> u64 {
    2u64 * n_layers as u64
        * n_kv_heads as u64
        * head_dim as u64
        * context_size as u64
        * bytes_per_elem.max(1) as u64
}

/// Проверить, что требуемая память помещается в доступную.
pub fn ensure_fits(required_mb: u64, available_mb: u64) -> Result<()> {
    if required_mb > available_mb {
        return Err(LlmError::OutOfMemory {
            required_mb,
            available_mb,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_cache_formula() {
        // Небольшая модель: 4096 контекст, 16 слоёв, 8 kv-голов, 64 head_dim, f16.
        let bytes = estimate_kv_cache_bytes(4096, 16, 8, 64, 2);
        // 2 * 16 * 8 * 64 * 4096 * 2 = 134_217_728 байт (128 MiB)
        assert_eq!(bytes, 134_217_728);
        assert_eq!(bytes / (1024 * 1024), 128);
    }

    #[test]
    fn zero_bytes_per_elem_clamped() {
        // Защита от деления/нуля: минимум 1 байт.
        let bytes = estimate_kv_cache_bytes(1, 1, 1, 1, 0);
        assert_eq!(bytes, 2);
    }

    #[test]
    fn fits_and_not_fits() {
        assert!(ensure_fits(500, 1000).is_ok());
        assert!(ensure_fits(1000, 1000).is_ok());
        let err = ensure_fits(2000, 512).expect_err("no fit");
        match err {
            LlmError::OutOfMemory {
                required_mb,
                available_mb,
            } => {
                assert_eq!(required_mb, 2000);
                assert_eq!(available_mb, 512);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
