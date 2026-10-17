//! Локальный эмбеддер multilingual-e5-small (ONNX) через fastembed.
//!
//! Модель лежит в каталоге (`<root>/models/multilingual-e5-small`), файлы
//! берутся с диска — сеть не нужна. Это тот же каталог, который установщик
//! кладёт в `%USERPROFILE%\.tensa\models\multilingual-e5-small`.
//!
//! Префиксы E5 (Контракт 2): `query:` для запросов, `passage:` для документов.

use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::embedder::Embedder;
use crate::error::{MemoryError, Result};

/// Имена ONNX-файла, которые принимает загрузчик (первый найденный).
///
/// `model_O4.onnx` — оптимизированный fp32 (235 МБ, кладёт установщик),
/// `model.onnx` — оригинальный экспорт (470 МБ), квантованные варианты —
/// для совместимости с чужими сборками.
const ONNX_CANDIDATES: [&str; 4] = [
    "model_O4.onnx",
    "model.onnx",
    "model_quantized.onnx",
    "model_qint8_avx512_vnni.onnx",
];

/// Файлы токенизатора (XLM-R / sentencepiece), обязательные для fastembed.
const TOKENIZER_FILES: [&str; 4] = [
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

/// Эмбеддер e5-small на ONNX Runtime (fastembed, локальные файлы).
///
/// `TextEmbedding::embed` требует `&mut self` — модель под мьютексом.
pub struct FastEmbedder {
    model: Mutex<fastembed::TextEmbedding>,
    dim: usize,
}

impl FastEmbedder {
    /// Загрузить модель из каталога (без сети).
    ///
    /// Ожидает `model*.onnx` (см. [`ONNX_CANDIDATES`]) и файлы токенизатора
    /// (см. [`TOKENIZER_FILES`]). Пул-метод — mean pooling (как у e5).
    pub fn from_directory(dir: &Path, dim: usize) -> Result<Self> {
        let onnx = ONNX_CANDIDATES
            .iter()
            .map(|name| dir.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| {
                MemoryError::Embedding(format!(
                    "ONNX-модель не найдена в {} (ожидались: {})",
                    dir.display(),
                    ONNX_CANDIDATES.join(", ")
                ))
            })?;

        let onnx_bytes = std::fs::read(&onnx)
            .map_err(|e| MemoryError::Embedding(format!("чтение {}: {e}", onnx.display())))?;

        let tokenizer_file = read_file(dir, TOKENIZER_FILES[0])?;
        let config_file = read_file(dir, TOKENIZER_FILES[1])?;
        let special_tokens_map_file = read_file(dir, TOKENIZER_FILES[2])?;
        let tokenizer_config_file = read_file(dir, TOKENIZER_FILES[3])?;

        let mut user_model = fastembed::UserDefinedEmbeddingModel::new(
            onnx_bytes,
            fastembed::TokenizerFiles {
                tokenizer_file,
                config_file,
                special_tokens_map_file,
                tokenizer_config_file,
            },
        );
        user_model.pooling = Some(fastembed::Pooling::Mean);

        let model = fastembed::TextEmbedding::try_new_from_user_defined(
            user_model,
            fastembed::InitOptionsUserDefined::new(),
        )
        .map_err(|e| MemoryError::Embedding(format!("fastembed: {e}")))?;

        Ok(Self {
            model: Mutex::new(model),
            dim,
        })
    }

    /// Эмбеддинг одной строки с префиксом E5.
    fn embed_one(&self, prefix: &str, text: &str) -> Result<Vec<f32>> {
        if text.trim().is_empty() {
            return Err(MemoryError::Embedding("empty text".into()));
        }
        let mut guard = self
            .model
            .lock()
            .map_err(|e| MemoryError::Embedding(format!("модель заблокирована: {e}")))?;
        let vectors = guard
            .embed(vec![format!("{prefix}{text}")], None)
            .map_err(|e| MemoryError::Embedding(format!("fastembed: {e}")))?;
        vectors
            .into_iter()
            .next()
            .ok_or_else(|| MemoryError::Embedding("fastembed: пустой результат".into()))
    }
}

/// Прочитать файл модели как байты.
fn read_file(dir: &Path, name: &str) -> Result<Vec<u8>> {
    let path = dir.join(name);
    std::fs::read(&path)
        .map_err(|e| MemoryError::Embedding(format!("чтение {}: {e}", path.display())))
}

#[async_trait]
impl Embedder for FastEmbedder {
    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embed_one("query: ", query)
    }

    async fn embed_passage(&self, passage: &str) -> Result<Vec<f32>> {
        self.embed_one("passage: ", passage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_directory_is_reported() {
        let error = match FastEmbedder::from_directory(Path::new("Z:\\нет-такого-каталога"), 384)
        {
            Ok(_) => panic!("должна быть ошибка"),
            Err(error) => error,
        };
        let text = format!("{error}");
        assert!(text.contains("не найдена"), "текст ошибки: {text}");
    }

    /// Живая проверка на реальной модели (каталог из TENSA_E5_DIR).
    #[tokio::test]
    #[ignore = "требует файлы e5-small (TENSA_E5_DIR); запускается вручную"]
    async fn real_model_embeds_and_matches() {
        let Ok(dir) = std::env::var("TENSA_E5_DIR") else {
            return;
        };
        let embedder = FastEmbedder::from_directory(Path::new(&dir), 384).expect("модель");
        let a = embedder
            .embed_passage("Иван Петров — мой поставщик бумаги")
            .await
            .expect("a");
        let b = embedder
            .embed_passage("поставка бумаги Иваном")
            .await
            .expect("b");
        let c = embedder.embed_passage("рецепт борща").await.expect("c");
        assert_eq!(a.len(), 384);
        let sim_ab = crate::embedder::cosine_similarity(&a, &b).expect("ab");
        let sim_ac = crate::embedder::cosine_similarity(&a, &c).expect("ac");
        assert!(
            sim_ab > sim_ac,
            "близкий текст должен быть ближе: {sim_ab} vs {sim_ac}"
        );
    }
}
