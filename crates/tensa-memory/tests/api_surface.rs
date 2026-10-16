//! Компиляционная проверка совместимости API `lance 13.0` / `fastembed 7.1`
//! (апрув версий Архитектором 2026-10-15).
//!
//! Цель — зафиксировать в CI ровно те сигнатуры, которые будут использовать
//! `LanceMemoryStore` и fastembed-эмбеддер. Совпадение поверхностей вызовов
//! проверяется компилятором: если API изменится при обновлении версии,
//! этот файл перестанет собираться.
//!
//! Тяжёлых операций нет: сетевые загрузки моделей и запись датасетов — в
//! модуле `compile_surface`, который никогда не исполняется.

#![cfg(all(feature = "lance", feature = "fastembed"))]

/// Поверхность вызовов (не исполняется — только компиляция).
#[allow(dead_code)]
mod compile_surface {
    use lance::dataset::{Dataset, WriteMode, WriteParams};
    use lance::deps::arrow_array::{Float32Array, RecordBatchReader};
    use lance::deps::arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    // === lance 13 (Контракт 2: датасеты documents / search_cache) ===

    /// Схема датасета: эмбеддинг — FixedSizeList<Float32, 384>.
    pub fn dataset_schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("document_id", DataType::Utf8, false),
            Field::new("chunk_text", DataType::Utf8, false),
            Field::new(
                "embedding",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 384),
                true,
            ),
        ])
    }

    /// Создание/запись датасета (13.0: `dest: impl Into<WriteDestination>`).
    pub async fn write_dataset(
        batches: impl RecordBatchReader + Send + 'static,
        uri: &str,
    ) -> lance::Result<Dataset> {
        let params = WriteParams {
            mode: WriteMode::Create,
            ..Default::default()
        };
        Dataset::write(batches, uri, Some(params)).await
    }

    /// Открытие + векторный поиск + сбор результатов.
    pub async fn search_nearest(uri: &str, query: Float32Array) -> lance::Result<usize> {
        let dataset = Dataset::open(uri).await?;
        let mut scanner = dataset.scan();
        scanner.nearest("embedding", &query, 10)?;
        scanner.limit(Some(10), None)?;
        let batch = scanner.try_into_batch().await?;
        Ok(batch.num_rows())
    }

    /// Стриминговый вариант сбора (для больших выдач).
    pub async fn search_stream(uri: &str, query: Float32Array) -> lance::Result<u64> {
        let dataset = Dataset::open(uri).await?;
        let mut scanner = dataset.scan();
        scanner.nearest("embedding", &query, 10)?;
        let stream = scanner.try_into_stream().await?;
        let _count = scanner.count_rows().await?;
        drop(stream);
        Ok(dataset.count_rows(None).await? as u64)
    }

    /// Удаление по предикату (переиндексация документа).
    pub async fn delete_by_document(uri: &str, document_id: &str) -> lance::Result<()> {
        let mut dataset = Dataset::open(uri).await?;
        let _ = dataset
            .delete(&format!("document_id = '{}'", document_id))
            .await?;
        Ok(())
    }

    // === fastembed 7.1 (Контракт 2: multilingual-e5-small, 384) ===

    use fastembed::{Embedding, EmbeddingModel, TextEmbedding, TextInitOptions};

    /// Опции инициализации (7.1: `TextInitOptions`; `InitOptions` deprecated).
    pub fn init_options() -> TextInitOptions {
        TextInitOptions::new(EmbeddingModel::MultilingualE5Small).with_show_download_progress(false)
    }

    /// Инициализация модели (в 7.1 — синхронная).
    pub fn build_model() -> fastembed::Result<TextEmbedding> {
        TextEmbedding::try_new(init_options())
    }

    /// Эмбеддинг с префиксами E5 (`query:` / `passage:` передаются вызывающим;
    /// fastembed префиксы не добавляет автоматически).
    pub fn embed_prefixed(
        model: &mut TextEmbedding,
        prefix: &str,
        texts: Vec<String>,
    ) -> fastembed::Result<Vec<Embedding>> {
        let prefixed: Vec<String> = texts.into_iter().map(|t| format!("{prefix}{t}")).collect();
        model.embed(prefixed, Some(16))
    }
}

// === Рантайм-проверки (дешёвые, без I/O) ===

#[test]
fn lance_schema_construction() {
    let schema = compile_surface::dataset_schema();
    assert_eq!(schema.fields().len(), 4);
    match schema
        .field_with_name("embedding")
        .expect("embedding field")
        .data_type()
    {
        lance::deps::arrow_schema::DataType::FixedSizeList(_, 384) => {}
        other => panic!("unexpected embedding type: {other:?}"),
    }
}

#[test]
fn lance_write_params_default_is_create_compatible() {
    use lance::dataset::{WriteMode, WriteParams};
    let params = WriteParams {
        mode: WriteMode::Create,
        ..Default::default()
    };
    assert!(matches!(params.mode, WriteMode::Create));
}

#[test]
fn fastembed_init_options_construction() {
    // Конструктор опций не выполняет загрузку модели (I/O не происходит).
    let _options = compile_surface::init_options();
}
