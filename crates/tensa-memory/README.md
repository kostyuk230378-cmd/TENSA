# tensa-memory

Иерархическая память Tensa (ADR-021, Контракты 2/13): core memory (SQLite),
recall (SQLite + FTS5), archival (SQLite-векторный fallback / Lance за фичей),
координатор `MemoryManager` и tool calls агента.

## Назначение

- **core memory** — блоки persona/human/tasks/preferences, всегда в контексте LLM
  (Контракт 13); лимиты `max_block_tokens` / `max_core_tokens`;
- **recall memory** — история разговоров, полнотекстовый поиск (FTS5);
- **archival memory** — долговременное хранилище фактов с семантическим поиском
  (Lance за фичей `lance`; SQLite-векторный fallback через `HashEmbedder`);
- **MemoryManager** — координатор трёх уровней + `get_context` (core + recall,
  зажатый в `max_context_tokens`);
- **tool calls** — `core_memory_replace/append`, `recall_memory_search`,
  `archival_memory_search/insert` через `dispatch_tool`;
- чанкинг документов (1000/200, SHA-256), кэш поиска (TTL 1 ч).

## Принципы

- Не извлекает текст, не выполняет OCR, не синхронизирует, не логирует секреты.
- Core memory редактируется агентом через tool calls (ADR-021) с записью истории.
- `get_context` всегда укладывается в `max_context_tokens` — старейшие
  recall-сообщения вытесняются первыми, core не трогается.

## Использование

```rust
use std::sync::Arc;
use tensa_memory::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let core = SqliteCoreMemoryStore::new_in_memory().await?;
    let recall = SqliteRecallMemoryStore::new_in_memory().await?;
    let archival = SqliteArchivalMemoryStore::new_in_memory(Arc::new(HashEmbedder::new(384))).await?;
    let manager = DefaultMemoryManager::new(
        Arc::new(core),
        Arc::new(recall),
        Arc::new(archival),
        MemoryManagerConfig::default(),
        "node-1".into(),
    ).await?;
    let fact = json_fact(&manager, "Иван предпочитает сметы в PDF").await?;
    println!("сохранено: {fact}");
    Ok(())
}

// tool call агента: сохранить факт в архив.
async fn json_fact(manager: &dyn MemoryManager, content: &str) -> tensa_memory::Result<serde_json::Value> {
    tensa_memory::dispatch_tool(manager, "archival_memory_insert", &serde_json::json!({ "content": content }))
}
```

## Структура

- `src/core_memory.rs` — core memory (SQLite, Контракт 13);
- `src/recall_memory.rs` — recall (SQLite + FTS5);
- `src/archival_memory.rs` — archival (SQLite-векторный fallback);
- `src/manager.rs` — `DefaultMemoryManager` + `MemoryManager`;
- `src/tools.rs` — tool calls агента (dop_tz);
- `src/chunker.rs`, `src/cache.rs`, `src/embedder.rs` — чанкинг/кэш/эмбеддер;
- `benches/search.rs` — criterion `search_10k`.

## Тесты

```bash
cargo test -p tensa-memory
# с векторными фичами (CI-джоба memory-vector):
cargo test -p tensa-memory --features lance,fastembed
```

## Связанные ADR и контракты

- ADR-004, ADR-005, ADR-021; Контракты 2, 13; доп. ТЗ tensa-memory
  (`FROM_GLM/dop_tz.md`).
