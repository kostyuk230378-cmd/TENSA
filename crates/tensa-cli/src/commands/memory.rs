//! `tensa memory index/search/list/delete` — семантическая память
//! (Контракт 2/13): archival-записи в SQLite + эмбеддинги e5-small.
//!
//! Модель берётся из `<root>/models/multilingual-e5-small` (её кладёт
//! установщик или DownloadModels.cmd). Без модели команды index/search
//! честно отказывают — подменять семантику хэшем нельзя.

use std::io::Read;
use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use crate::cli::{MemoryArgs, MemoryCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

use tensa_memory::{
    chunk_text, ArchivalEntry, ArchivalMemoryStore, Embedder, FastEmbedder, HashEmbedder,
    MemoryConfigSection, SqliteArchivalMemoryStore,
};

/// Каталог модели внутри корня данных.
const MODEL_DIR: &str = "multilingual-e5-small";

/// Выполнить `memory`.
pub async fn run(ctx: &Ctx, args: &MemoryArgs) -> Result<()> {
    match &args.command {
        MemoryCommand::Index { path } => index(ctx, path).await,
        MemoryCommand::Search { query, limit } => search(ctx, query, *limit).await,
        MemoryCommand::List => list(ctx).await,
        MemoryCommand::Delete { id } => delete(ctx, id).await,
        MemoryCommand::Reindex => Err(CliError::Unimplemented(
            "memory reindex: переиндексация документов появится вместе с documents-этапом; \
             сейчас переиндексируйте файл через `memory index`"
                .into(),
        )),
    }
}

/// Секция [memory] конфига (с дефолтами Контракта 4).
fn memory_section(ctx: &Ctx) -> Result<MemoryConfigSection> {
    let config = ctx.config()?;
    let defaults = MemoryConfigSection::default();
    let table = &config.memory;
    Ok(MemoryConfigSection {
        chunk_size: table
            .get("chunk_size")
            .and_then(|v| v.as_integer())
            .map(|v| v.max(1) as usize)
            .unwrap_or(defaults.chunk_size),
        chunk_overlap: table
            .get("chunk_overlap")
            .and_then(|v| v.as_integer())
            .map(|v| v.max(0) as usize)
            .unwrap_or(defaults.chunk_overlap),
        embedding_model: table
            .get("embedding_model")
            .and_then(|v| v.as_str())
            .unwrap_or(defaults.embedding_model.as_str())
            .to_string(),
        embedding_dim: table
            .get("embedding_dim")
            .and_then(|v| v.as_integer())
            .map(|v| v.max(1) as usize)
            .unwrap_or(defaults.embedding_dim),
        index_type: table
            .get("index_type")
            .and_then(|v| v.as_str())
            .unwrap_or(defaults.index_type.as_str())
            .to_string(),
        max_file_size_mb: table
            .get("max_file_size_mb")
            .and_then(|v| v.as_integer())
            .map(|v| v.max(1) as u64)
            .unwrap_or(defaults.max_file_size_mb),
    })
}

/// Эмбеддер: e5-small из корня данных; без модели — отказ с подсказкой.
fn embedder(ctx: &Ctx) -> Result<Arc<dyn Embedder>> {
    let dir = ctx.paths.models_dir().join(MODEL_DIR);
    match FastEmbedder::from_directory(&dir, 384) {
        Ok(fast) => Ok(Arc::new(fast)),
        Err(error) => {
            if std::env::var("TENSA_HASH_EMBEDDER").as_deref() == Ok("1") {
                ctx.output.warning(
                    "модель не найдена — TENSA_HASH_EMBEDDER=1: лексический хэш (только отладка)",
                );
                return Ok(Arc::new(HashEmbedder::new(384)));
            }
            Err(CliError::Message(format!(
                "модель эмбеддингов не загружена ({error}); установите Tensa с моделью \
                 или запустите DownloadModels.cmd — файлы должны лежать в {}",
                dir.display()
            )))
        }
    }
}

/// Открыть архивный стор.
async fn store(ctx: &Ctx) -> Result<(SqliteArchivalMemoryStore, MemoryConfigSection)> {
    let section = memory_section(ctx)?;
    let db = ctx.paths.memory_dir().join("archival.db");
    let store = SqliteArchivalMemoryStore::new(&db, embedder(ctx)?)
        .await
        .map_err(|e| CliError::Message(format!("открытие памяти: {e}")))?;
    Ok((store, section))
}

/// `memory index <path>` — прочитать UTF-8 файл, нарезать на чанки, заэмбеддить.
async fn index(ctx: &Ctx, path: &std::path::Path) -> Result<()> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| CliError::Message(format!("открытие {}: {e}", path.display())))?;
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| {
        CliError::Message(format!(
            "{}: только текстовые UTF-8 файлы ({e})",
            path.display()
        ))
    })?;

    let config = ctx.config()?;
    let node_id = config.general.node_id.clone();
    let (store, section) = store(ctx).await?;
    let memory_config = section.into_memory_config(3600);
    let chunks = chunk_text(&text, &memory_config);
    if chunks.is_empty() {
        return Err(CliError::Message(format!(
            "{}: нет текста для индексации",
            path.display()
        )));
    }

    let total = chunks.len();
    for (number, chunk) in chunks.into_iter().enumerate() {
        let entry = ArchivalEntry {
            id: Uuid::new_v4(),
            content: chunk,
            embedding: Vec::new(),
            tags: vec!["document".into()],
            source: Some(path.display().to_string()),
            created_at: 0,
            node_id: node_id.clone(),
            metadata: json!({ "chunk": number }),
        };
        store
            .insert(entry)
            .await
            .map_err(|e| CliError::Message(format!("запись чанка {number}: {e}")))?;
    }

    if ctx.output.json_mode() {
        return ctx.output.json(&json!({
            "indexed": total,
            "source": path.display().to_string(),
            "model": section.embedding_model,
        }));
    }
    ctx.output.success(format!(
        "проиндексировано {total} чанков из {} (модель {})",
        path.display(),
        section.embedding_model
    ));
    Ok(())
}

/// `memory search <query>` — семантический поиск по архиву.
async fn search(ctx: &Ctx, query: &str, limit: usize) -> Result<()> {
    let (store, _section) = store(ctx).await?;
    let results = store
        .search(query, limit)
        .await
        .map_err(|e| CliError::Message(format!("поиск: {e}")))?;

    if ctx.output.json_mode() {
        let rows: Vec<_> = results
            .iter()
            .map(|r| {
                json!({
                    "id": r.id.to_string(),
                    "score": r.score,
                    "content": r.content,
                    "timestamp": r.timestamp,
                })
            })
            .collect();
        return ctx.output.json(&json!({ "query": query, "results": rows }));
    }

    if results.is_empty() {
        ctx.output.message("Ничего не найдено.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = results
        .iter()
        .map(|r| {
            let preview: String = r.content.chars().take(90).collect();
            vec![
                format!("{:.3}", r.score),
                preview.replace('\n', " "),
                r.id.to_string(),
            ]
        })
        .collect();
    ctx.output.table(&["близость", "запись", "id"], &rows);
    Ok(())
}

/// `memory list` — размер архива и последние записи.
async fn list(ctx: &Ctx) -> Result<()> {
    let (store, section) = store(ctx).await?;
    let count = store
        .count_entries()
        .await
        .map_err(|e| CliError::Message(format!("архив: {e}")))?;
    let recent = store
        .recent_entries(20)
        .await
        .map_err(|e| CliError::Message(format!("архив: {e}")))?;

    if ctx.output.json_mode() {
        let rows: Vec<_> = recent
            .iter()
            .map(|e| {
                json!({
                    "id": e.id.to_string(),
                    "source": e.source,
                    "created_at": e.created_at,
                    "content": e.content,
                })
            })
            .collect();
        return ctx.output.json(&json!({
            "entries": count,
            "model": section.embedding_model,
            "recent": rows,
        }));
    }

    ctx.output.message(format!(
        "Записей в памяти: {count} (модель {})",
        section.embedding_model
    ));
    if !recent.is_empty() {
        let rows: Vec<Vec<String>> = recent
            .iter()
            .map(|e| {
                let preview: String = e.content.chars().take(70).collect();
                vec![
                    e.id.to_string(),
                    e.source.clone().unwrap_or_default(),
                    preview.replace('\n', " "),
                ]
            })
            .collect();
        ctx.output.table(&["id", "источник", "запись"], &rows);
    }
    Ok(())
}

/// `memory delete <id>` — удалить запись.
async fn delete(ctx: &Ctx, id: &str) -> Result<()> {
    let uuid = Uuid::parse_str(id).map_err(|e| CliError::Message(format!("id не UUID: {e}")))?;
    let (store, _section) = store(ctx).await?;
    store
        .delete(uuid)
        .await
        .map_err(|e| CliError::Message(format!("удаление: {e}")))?;
    ctx.output.success(format!("удалено: {uuid}"));
    Ok(())
}
