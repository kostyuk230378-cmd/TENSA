# Аудит готовых крейтов — результаты и решения

**Дата:** 2026-10-16
**Как проводился:** независимая проверка (субагент-ревьюер), каждая находка
верифицирована по коду и ТЗ; статусы и решения приняты по правилу Архитектора:
критичное — сразу, остальное — после этапов 1→7.

## Сводка

| Крейт | Вердикт | Ключевое | Действие |
|---|---|---|---|
| `tensa-approval` | соответствует | — | — |
| `tensa-core` | частично | нет `TensaCore`/`FirstRunWizard`/`InitParams.secretary_name` (доп. ТЗ `dop_tz_core.md`) | **сразу перед `tensa-ffi`** (нужно для Контракта 9 и PWA-мастера) |
| `tensa-memory` | частично | нет реальных `lance_store.rs`/`fastembed.rs` (архив — SQLite-фallback за фичей `lance`) | после 1→7 |
| `tensa-nodes` | частично (структура) | нет файла `connections.rs`; профиль — заглушка по дизайну | принято, см. ниже |
| `tensa-sync` | частично (структура) | нет файла `vector_clock.rs`; нет каталога `proto/` | принято, см. ниже |
| `tensa-llm` | частично | нет `litert/` (NPU); нативный слой llama.cpp не подключён | TASK-ENV-001 (этап env) |

## Исправлено сразу (коммит 520c16c)

`expect()` в прод-коде — 17 мест, убраны без изменения поведения:

- `tensa-llm/llama/src/engine.rs` — 4 шт.: блокировки движка → `LlmError::LockPoisoned`;
  `model_info()` → честный `None`;
- `tensa-memory/src/cache.rs` — 6 шт.: отравленная блокировка кэша восстанавливается
  (`lock_cache()`), `NonZeroUsize` — без `expect`;
- `tensa-memory/src/core_memory.rs` — 3 шт.: блокировка конфигурации → `MemoryError::Config`;
- `tensa-sync/src/client.rs` — 4 шт.: отравленная блокировка журнала восстанавливается
  (`ledger()`);
- попутно исправлен доккомментарий в `tensa-sync/src/error.rs`.

Правило после правок: **ни одного `unwrap()`/`expect()`/`panic!` в прод-коде**
(в тестах и build-скриптах — допустимо).

## Отклонения структуры ТЗ (функциональность есть — приняты как `[derived]`)

- **tensa-nodes, `connections.rs`:** логика соединений живёт в `registry.rs` +
  `sqlite.rs` (acquire/release, TTL 15 мин, конфликты, освобождение при офлайн) —
  покрыта тестами. Отдельный файл не создавался, чтобы не дробить состояние.
- **tensa-nodes, профилирование:** `StubProfiler` — по дизайну (тесты/CI);
  реальный сбор телеметрии выполняет `tensa-perf::DefaultPerfProfiler`
  (ТЗ tensa-perf), кадр узлу поставляет оркестратор. Ложное обещание
  «фичи `sysinfo`» из доккомментариев убрано.
- **tensa-sync, `vector_clock.rs`:** переиспользуется `tensa_approval::VectorClock`
  (единый тип, без дублирования); конверсии — в `grpc.rs`.
- **tensa-sync, `proto/`:** `.proto` — единственный источник истины в
  `teh_doc/contracts/proto/`; код генерируется build-скриптом (tonic-build,
  vendored protoc). Копия в крейте не нужна.
- **tensa-llm, `build.rs`:** генерация/сборка нативного слоя — в `llama-sys`
  (там `build.rs` есть); на уровне `tensa-llm` собственного build-скрипта не требуется.

## Открытые пункты (в работе по плану)

1. **tensa-core** — `TensaCore`, `FirstRunWizard` (8 шагов), `InitParams`
   с `secretary_name` + тесты `tests/first_run.rs`. Блокирует `tensa-ffi`.
2. **tensa-memory** — реальные реализации `LanceMemoryStore` и
   fastembed-эмбеддера за фичами `lance`/`fastembed` (API-поверхность уже
   зафиксирована `tests/api_surface.rs`, CI-шаг добавлен).
3. **tensa-llm / TASK-ENV-001** — cmake + LLVM/libclang, пиновка llama.cpp,
   GGUF-модели, затем `litert/` (NPU) и NVML (GPU в tensa-perf).

## Контрольная проверка на 2026-10-16

- `cargo check --workspace` — чисто;
- тесты: **197/197** (166 существующих + 31 новых в `tensa-perf`);
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 предупреждений;
- `cargo fmt --all --check` — чисто;
- `cargo test -p tensa-memory --features lance,fastembed` — 12/12 (CI-шаг добавлен).
