# tensa-memory

Память Tensa: иерархическая память (core/recall/archival, ADR-021),
векторный поиск, эмбеддинги, чанкинг, кэш (Контракты 2/13, dop_tz).

## Назначение

- **Core memory** (`core_memory.rs`, SQLite): блоки
  persona/human/tasks/preferences — всегда в контексте LLM, редактируются
  агентом (replace/append + история, Контракт 13);
- **Recall memory** (`recall_memory.rs`, SQLite + FTS5): история разговоров,
  поиск с префиксными запросами, TTL-cleanup;
- **Archival memory** (`archival_memory.rs`): долговременные факты,
  семантический поиск (косинус), теги (SQLite-реализация; Lance — фича);
- **MemoryManager** (`manager.rs`): координатор трёх уровней, `get_context`
  (core + recent), `search_all`;
- **Tool calls** (`tools.rs`): `core_memory_replace/append`,
  `recall_memory_search`, `archival_memory_search/insert` (dop_tz);
- **Чанкинг** (`chunker.rs`): 1000/200, SHA-256, char-безопасно;
- **Кэш** (`cache.rs`): LRU + TTL 1 час;
- **Embedder** (`embedder.rs`): трейт с префиксами `query:`/`passage:`,
  `HashEmbedder` для тестов.

## Фичи

| Фича | Что включает |
|---|---|
| `lance` | векторное хранилище Lance (для будущего `LanceMemoryStore`) |
| `fastembed` | локальные эмбеддинги multilingual-e5-small (ONNX) |

## Тесты

```bash
cargo test -p tensa-memory
```

## Сборка с тяжёлыми фичами (`lance`, `fastembed`)

Требуется protoc (для `lance-encoding`):

- `PROTOC` — путь к `protoc.exe` (или protoc в PATH);
- `PROTOC_INCLUDE` — каталог `include` с well-known типами
  (`google/protobuf/*.proto`). Читается prost-build.

Проверено: protoc 36.2 (vendored-копия в `C:\tensa-tools\protoc`).

Версии `lance 13` / `fastembed 7.1` — апрув Архитектора 2026-10-15 (версии
руководства 0.9/4.0 не компилируются rustc 1.98: arrow-arith 50, конфликт
`quarter()` с chrono). Совместимость API зафиксирована компиляционной
проверкой `tests/api_surface.rs` (в CI — `--features lance,fastembed`).
